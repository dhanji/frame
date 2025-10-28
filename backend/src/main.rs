use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer, HttpResponse};
use actix_files as fs;
use actix_web_httpauth::middleware::HttpAuthentication;
use email_client_backend::utils::encryption::Encryption;
use std::sync::Arc;
use tokio::sync::RwLock;
use email_client_backend::{handlers, services, websocket, db};

use services::{EmailManager, EmailSyncService, BackgroundServiceManager, AgentEngine, ProviderConfig, AutomationScheduler};
use websocket::ConnectionManager;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    log::info!("Starting Frame Email Client Backend...");
    log::info!("Server starting on http://localhost:8080");

    // Initialize database pool
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://email_client.db".to_string());
    
    let pool = db::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");
    
    // Run migrations (if enabled)
    if let Err(e) = db::run_migrations(&pool).await {
        log::warn!("Failed to run migrations: {}. Creating tables manually...", e);
        // Create tables manually if migrations fail
        create_tables(&pool).await;
    }
    
    // Create shared EmailManager
    let email_manager = Arc::new(EmailManager::new());
    
    // Create WebSocket connection manager
    let ws_manager = Arc::new(RwLock::new(ConnectionManager::new()));
    
    // Start background email sync service (delayed to not block server startup)
    let pool_sync = pool.clone();
    let email_manager_sync = email_manager.clone();
    tokio::spawn(async move {
        // Wait 5 seconds before starting sync to allow server to become responsive
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        log::info!("Starting background email sync service...");
        let email_sync_service = EmailSyncService::new(pool_sync, email_manager_sync);
        email_sync_service.start().await;
    });
    
    // Start background services (IMAP IDLE, attachment cleanup)
    let bg_service_manager = BackgroundServiceManager::new(pool.clone(), ws_manager.clone(), email_manager.clone());
    
    // Start attachment cleanup job
    bg_service_manager.start_attachment_cleanup_job().await;
    
    // Start IMAP IDLE monitors for all active users
    let pool_idle = pool.clone();
    let ws_manager_idle = ws_manager.clone();
    let email_manager_idle = email_manager.clone();
    tokio::spawn(async move {
        let bg_service_manager_idle = BackgroundServiceManager::new(pool_idle, ws_manager_idle, email_manager_idle);
        bg_service_manager_idle.start_all_imap_idle_monitors().await;
    });
    
    // Create AI Agent Engine
    let provider_config = ProviderConfig::Anthropic {
        api_key: std::env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| {
            log::error!("ANTHROPIC_API_KEY not found in environment!");
            "dummy-key".to_string()
        }),
        model: std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string()),
    };
    let provider = services::agent::provider::create_provider(provider_config);
    // Note: Tool registry will be created per-user in handlers
    let tool_registry = Arc::new(services::agent::tools::create_tool_registry(pool.clone(), 1));
    let agent_engine = Arc::new(AgentEngine::new(provider, tool_registry));
    
    // Start automation scheduler
    let pool_automation = pool.clone();
    let agent_engine_automation = agent_engine.clone();
    tokio::spawn(async move {
        match AutomationScheduler::new(pool_automation, agent_engine_automation).await {
            Ok(mut scheduler) => {
                if let Err(e) = scheduler.start().await {
                    log::error!("Failed to start automation scheduler: {:?}", e);
                }
            }
            Err(e) => {
                log::error!("Failed to create automation scheduler: {:?}", e);
            }
        }
    });
    
    // Start OAuth token refresh service
    let pool_oauth = pool.clone();
    tokio::spawn(async move {
        log::info!("Starting OAuth token refresh service");
        services::token_refresh::start_token_refresh_service(Arc::new(pool_oauth)).await;
    });
    
    // Start CalDAV sync service (CRITICAL FEATURE)
    let pool_caldav = pool.clone();
    tokio::spawn(async move {
        log::info!("Starting CalDAV sync service");
        let caldav_sync = services::caldav_sync::CalDavSyncService::new(pool_caldav);
        caldav_sync.start().await;
    });

    // Start HTTP server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(Logger::default())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(email_manager.clone()))
            .app_data(web::Data::new(ws_manager.clone()))
            .app_data(web::Data::new(agent_engine.clone()))
            .app_data(web::Data::new(Encryption::new()))
            .route("/health", web::get().to(health_check))
            .route("/ws", web::get().to(websocket::ws_handler))
            // Public auth endpoints - NO AUTHENTICATION REQUIRED
            .service(
                web::scope("/api/auth")
                    .route("/auto-login", web::get().to(handlers::auth::auto_login))
                    .route("/google", web::get().to(handlers::auth::google_auth_url))
                    .route("/google/callback", web::get().to(handlers::auth::google_callback))
            )
            .route("/api/register", web::post().to(handlers::auth::register))
            .route("/api/login", web::post().to(handlers::auth::login))
            // Protected API routes
            .service(
                web::scope("/api")
                    .wrap(HttpAuthentication::bearer(email_client_backend::middleware::auth::validator))
                    // All routes in this scope require authentication
                            // Debug endpoints
                            .route("/debug/sync", web::get().to(handlers::debug::get_sync_debug))
                            .route("/debug/test-imap", web::get().to(handlers::debug::test_imap_connection))
                            .route("/debug/manual-sync", web::post().to(handlers::debug::trigger_manual_sync))
                            .route("/debug/test-caldav", web::get().to(handlers::debug::test_caldav_connection))
                            // Auth endpoints
                            .route("/logout", web::post().to(handlers::auth::logout))
                            .route("/refresh-token", web::post().to(handlers::auth::refresh_token_endpoint))
                            .route("/conversations", web::get().to(handlers::conversations::get_conversations))
                            .route("/conversations/{id}", web::get().to(handlers::conversations::get_conversation))
                            .route("/emails/send", web::post().to(handlers::emails::send_email))
                            .route("/emails/{id}/reply", web::post().to(handlers::emails::reply_to_email))
                            .route("/emails/{id}/read", web::put().to(handlers::emails::mark_as_read))
                            .route("/emails/{id}", web::delete().to(handlers::emails::delete_email))
                            .route("/emails/{id}/move", web::post().to(handlers::emails::move_email))
                            .route("/emails/sync", web::post().to(handlers::emails::trigger_sync))
                            .route("/folders", web::get().to(handlers::folders::get_folders))
                            .route("/folders", web::post().to(handlers::folders::create_folder))
                            // Draft endpoints
                            .route("/drafts", web::get().to(handlers::drafts::get_drafts))
                            .route("/drafts/auto-save", web::post().to(handlers::drafts::auto_save_draft))
                            .route("/drafts/{id}/send", web::post().to(handlers::drafts::send_draft))
                            .route("/drafts/{id}", web::delete().to(handlers::drafts::delete_draft))
                            // Filter endpoints
                            .route("/filters", web::post().to(handlers::filters::create_filter))
                            .route("/filters", web::get().to(handlers::filters::get_filters))
                            .route("/filters/{id}", web::put().to(handlers::filters::update_filter))
                            .route("/filters/{id}", web::delete().to(handlers::filters::delete_filter))
                            .route("/filters/apply", web::post().to(handlers::filters::apply_filters))
                            // Search endpoints
                            .route("/search", web::get().to(handlers::search::search_emails))
                            .route("/search/save", web::post().to(handlers::search::save_search))
                            .route("/search/saved", web::get().to(handlers::search::get_saved_searches))
                            .route("/search/saved/{id}", web::delete().to(handlers::search::delete_saved_search))
                            .route("/search/suggestions", web::get().to(handlers::search::get_search_suggestions))
                            // Attachment endpoints
                            .route("/attachments/upload", web::post().to(handlers::attachments::upload_attachment))
                            .route("/attachments", web::get().to(handlers::attachments::get_attachments))
                            .route("/attachments/{id}", web::get().to(handlers::attachments::download_attachment))
                            .route("/attachments/{id}", web::delete().to(handlers::attachments::delete_attachment))
                            .route("/attachments/{id}/thumbnail", web::get().to(handlers::attachments::download_thumbnail))
                            .route("/attachments/gallery", web::get().to(handlers::attachments::get_gallery))
                            .route("/attachments/gallery/recents", web::get().to(handlers::attachments::get_gallery_recents))
                            .route("/attachments/gallery/by-sender", web::get().to(handlers::attachments::get_gallery_by_sender))                            .route("/maintenance/cleanup-attachments", web::post().to(handlers::attachments::cleanup_orphaned_attachments))
                            // Settings endpoints
                            .route("/settings", web::get().to(handlers::settings::get_settings))
                            .route("/settings", web::put().to(handlers::settings::update_settings))
                            .route("/settings/env", web::get().to(handlers::settings::get_env_settings))
                            .route("/settings/env", web::post().to(handlers::settings::save_env_settings))
                            // Agent endpoints
                            .route("/agent/tools", web::get().to(handlers::agent::list_tools))
                            // Chat/Agent endpoints
                            .route("/chat/conversations", web::post().to(handlers::chat::create_conversation))
                            .route("/chat/conversations", web::get().to(handlers::chat::list_conversations))
                            .route("/chat/conversations/{id}", web::get().to(handlers::chat::get_conversation))
                            .route("/chat/conversations/{id}/messages", web::post().to(handlers::chat::send_message))
                            .route("/chat/conversations/{id}", web::delete().to(handlers::chat::delete_conversation))
                            .route("/chat/conversations/{id}/stream", web::post().to(handlers::chat::send_message_stream))
                            // Automation endpoints
                            .route("/automations", web::post().to(handlers::automations::create_automation))
                            .route("/automations", web::get().to(handlers::automations::list_automations))
                            .route("/automations/{id}", web::get().to(handlers::automations::get_automation))
                            .route("/automations/{id}", web::put().to(handlers::automations::update_automation))
                            .route("/automations/{id}", web::delete().to(handlers::automations::delete_automation))
                            .route("/automations/{id}/trigger", web::post().to(handlers::automations::trigger_automation))
                            .route("/automations/{id}/runs", web::get().to(handlers::automations::get_runs))
                            .route("/automations/{automation_id}/runs/{run_id}", web::get().to(handlers::automations::get_run_details))
                            // Reminder endpoints
                            .route("/reminders", web::post().to(handlers::reminders::create_reminder))
                            .route("/reminders", web::get().to(handlers::reminders::list_reminders))
                            .route("/reminders/{id}", web::put().to(handlers::reminders::update_reminder))
                            .route("/reminders/{id}", web::delete().to(handlers::reminders::delete_reminder))
                            .route("/reminders/{id}/complete", web::put().to(handlers::reminders::toggle_complete))
                            // Calendar endpoints
                            .route("/calendar/events", web::get().to(handlers::calendar::list_events))
                            .route("/calendar/events", web::post().to(handlers::calendar::create_event))
                            .route("/calendar/events/{id}", web::put().to(handlers::calendar::update_event))
                            .route("/calendar/events/{id}", web::delete().to(handlers::calendar::delete_event))
                            .route("/calendar/sync", web::post().to(handlers::calendar::sync_calendar))
                            // Money endpoints
                            .route("/money/accounts", web::get().to(handlers::money::list_accounts))
                            .route("/money/accounts", web::post().to(handlers::money::create_account))
                            .route("/money/transactions", web::get().to(handlers::money::list_transactions))
                            .route("/money/transactions", web::post().to(handlers::money::add_transaction))
                            .route("/money/sync", web::post().to(handlers::money::sync_accounts))
            )
            // Serve static files and SPA fallback
            .service(fs::Files::new("/", "../frontend").index_file("index.html"))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

// Health check endpoint
async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "Frame Email Client Backend",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// Create database tables manually if migrations fail
async fn create_tables(pool: &sqlx::SqlitePool) {
    let queries = vec![
        // Users table
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            email_password TEXT,
            imap_host TEXT NOT NULL,
            imap_port INTEGER NOT NULL DEFAULT 993,
            smtp_host TEXT NOT NULL,
            smtp_port INTEGER NOT NULL DEFAULT 587,
            smtp_use_tls BOOLEAN NOT NULL DEFAULT TRUE,
            settings TEXT DEFAULT '{}',
            is_active BOOLEAN NOT NULL DEFAULT TRUE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
        // Emails table
        r#"
        CREATE TABLE IF NOT EXISTS emails (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            message_id TEXT NOT NULL,
            thread_id TEXT,
            from_address TEXT NOT NULL,
            to_addresses TEXT NOT NULL,
            cc_addresses TEXT,
            bcc_addresses TEXT,
            subject TEXT NOT NULL,
            body_text TEXT,
            body_html TEXT,
            date DATETIME NOT NULL,
            is_read BOOLEAN NOT NULL DEFAULT FALSE,
            is_starred BOOLEAN NOT NULL DEFAULT FALSE,
            has_attachments BOOLEAN NOT NULL DEFAULT FALSE,
            attachments TEXT,
            folder TEXT NOT NULL DEFAULT 'INBOX',
            size INTEGER DEFAULT 0,
            in_reply_to TEXT,
            references TEXT,
            deleted_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
            UNIQUE(user_id, message_id)
        )
        "#,
        // Folders table
        r#"
        CREATE TABLE IF NOT EXISTS folders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            parent_id INTEGER,
            sort_order INTEGER DEFAULT 0,
            is_system BOOLEAN NOT NULL DEFAULT FALSE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
            FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE CASCADE,
            UNIQUE(user_id, name)
        )
        "#,
        // Drafts table
        r#"
        CREATE TABLE IF NOT EXISTS drafts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            to_addresses TEXT,
            cc_addresses TEXT,
            bcc_addresses TEXT,
            subject TEXT,
            body_text TEXT,
            body_html TEXT,
            attachments TEXT,
            in_reply_to TEXT,
            references TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        // Filters table
        r#"
        CREATE TABLE IF NOT EXISTS filters (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            conditions TEXT NOT NULL,
            actions TEXT NOT NULL,
            is_active BOOLEAN NOT NULL DEFAULT TRUE,
            priority INTEGER NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        // Sessions table
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token TEXT NOT NULL UNIQUE,
            expires_at DATETIME NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        // Saved searches table
        r#"
        CREATE TABLE IF NOT EXISTS saved_searches (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            query TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        // Create indexes
        "CREATE INDEX IF NOT EXISTS idx_emails_user_id ON emails(user_id)",
        "CREATE INDEX IF NOT EXISTS idx_emails_thread_id ON emails(thread_id)",
        "CREATE INDEX IF NOT EXISTS idx_emails_folder ON emails(folder)",
        "CREATE INDEX IF NOT EXISTS idx_emails_date ON emails(date)",
        "CREATE INDEX IF NOT EXISTS idx_emails_is_read ON emails(is_read)",
        "CREATE INDEX IF NOT EXISTS idx_folders_user_id ON folders(user_id)",
        "CREATE INDEX IF NOT EXISTS idx_filters_user_id ON filters(user_id)",
        "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)",
        "CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token)",
        // Chat conversations
        r#"
        CREATE TABLE IF NOT EXISTS chat_conversations (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_chat_conversations_user_id ON chat_conversations(user_id)",
        // Chat messages
        r#"
        CREATE TABLE IF NOT EXISTS chat_messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
            content TEXT NOT NULL,
            tool_calls TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (conversation_id) REFERENCES chat_conversations(id) ON DELETE CASCADE
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_chat_messages_conversation_id ON chat_messages(conversation_id)",
        // Automations
        r#"
        CREATE TABLE IF NOT EXISTS automations (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            schedule TEXT NOT NULL,
            prompt TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT TRUE,
            last_run DATETIME,
            next_run DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_automations_user_id ON automations(user_id)",
        "CREATE INDEX IF NOT EXISTS idx_automations_enabled ON automations(enabled)",
        // Automation runs
        r#"
        CREATE TABLE IF NOT EXISTS automation_runs (
            id TEXT PRIMARY KEY,
            automation_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('running', 'success', 'failed')),
            result TEXT,
            error TEXT,
            started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            completed_at DATETIME,
            FOREIGN KEY (automation_id) REFERENCES automations(id) ON DELETE CASCADE
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_automation_runs_automation_id ON automation_runs(automation_id)",
        // Reminders
        r#"
        CREATE TABLE IF NOT EXISTS reminders (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            notes TEXT,
            due_date DATETIME,
            completed BOOLEAN NOT NULL DEFAULT FALSE,
            completed_at DATETIME,
            email_conversation_id TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_reminders_user_id ON reminders(user_id)",
        "CREATE INDEX IF NOT EXISTS idx_reminders_completed ON reminders(completed)",
        "CREATE INDEX IF NOT EXISTS idx_reminders_due_date ON reminders(due_date)",
        // Calendar events
        r#"
        CREATE TABLE IF NOT EXISTS calendar_events (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            calendar_id TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT,
            location TEXT,
            start_time DATETIME NOT NULL,
            end_time DATETIME NOT NULL,
            all_day BOOLEAN NOT NULL DEFAULT FALSE,
            recurrence_rule TEXT,
            attendees TEXT,
            status TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_calendar_events_user_id ON calendar_events(user_id)",
        "CREATE INDEX IF NOT EXISTS idx_calendar_events_start_time ON calendar_events(start_time)",
        "CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_id ON calendar_events(calendar_id)",
        // Money accounts
        r#"
        CREATE TABLE IF NOT EXISTS money_accounts (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            account_type TEXT NOT NULL CHECK(account_type IN ('bank', 'cash_app', 'other')),
            account_name TEXT NOT NULL,
            balance REAL NOT NULL DEFAULT 0.0,
            currency TEXT NOT NULL DEFAULT 'USD',
            credentials TEXT,
            last_sync DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_money_accounts_user_id ON money_accounts(user_id)",
        // Money transactions
        r#"
        CREATE TABLE IF NOT EXISTS money_transactions (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            transaction_date DATETIME NOT NULL,
            description TEXT NOT NULL,
            amount REAL NOT NULL,
            category TEXT,
            transaction_type TEXT NOT NULL CHECK(transaction_type IN ('income', 'expense', 'transfer')),
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (account_id) REFERENCES money_accounts(id) ON DELETE CASCADE
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_money_transactions_account_id ON money_transactions(account_id)",
        "CREATE INDEX IF NOT EXISTS idx_money_transactions_date ON money_transactions(transaction_date)",
    ];
    
    for query in queries {
        if let Err(e) = sqlx::query(query).execute(pool).await {
            log::error!("Failed to execute query: {}", e);
        }
    }
    
    log::info!("Database tables created successfully");
}