use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer, HttpResponse};
use actix_files as fs;
use actix_web_httpauth::middleware::HttpAuthentication;
use email_client_backend::utils::encryption::Encryption;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::SqlitePool;
use email_client_backend::{handlers, services, websocket, db};

use services::{EmailManager, EmailSyncService, BackgroundServiceManager};
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
    
    // Start background email sync service
    let sync_service = EmailSyncService::new(pool.clone(), email_manager.clone());
    tokio::spawn(async move {
        log::info!("Starting background email sync service");
        sync_service.start().await;
    });
    
    // Start background services (IMAP IDLE, attachment cleanup)
    let bg_service_manager = BackgroundServiceManager::new(pool.clone(), ws_manager.clone());
    
    // Start IMAP IDLE monitors for all active users
    bg_service_manager.start_all_imap_idle_monitors().await;
    
    // Start attachment cleanup job
    bg_service_manager.start_attachment_cleanup_job().await;
    
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
            .app_data(web::Data::new(Encryption::new()))
            .route("/health", web::get().to(health_check))
            // Serve static files from frontend directory
            .service(fs::Files::new("/", "../frontend").index_file("index.html"))
            .service(
                web::scope("/api")
                    // Public endpoints (no authentication required)
                    .route("/register", web::post().to(handlers::auth::register))
                    .route("/login", web::post().to(handlers::auth::login))
                    // Protected endpoints (authentication required)
                    .service(
                        web::scope("")
                            .wrap(HttpAuthentication::bearer(email_client_backend::middleware::auth::validator))
                            .route("/conversations", web::get().to(handlers::conversations::get_conversations))
                            .route("/conversations/{id}", web::get().to(handlers::conversations::get_conversation))
                            .route("/emails/send", web::post().to(handlers::emails::send_email))
                            .route("/emails/{id}/reply", web::post().to(handlers::emails::reply_to_email))
                            .route("/emails/{id}/read", web::put().to(handlers::emails::mark_as_read))
                            .route("/emails/{id}", web::delete().to(handlers::emails::delete_email))
                            .route("/emails/{id}/move", web::post().to(handlers::emails::move_email))
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
                            .route("/maintenance/cleanup-attachments", web::post().to(handlers::attachments::cleanup_orphaned_attachments))
                            // Settings endpoints
                            .route("/settings", web::get().to(handlers::settings::get_settings))
                            .route("/settings", web::put().to(handlers::settings::update_settings))
                    )
            )
            .route("/ws", web::get().to(websocket::ws_handler))
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
    ];
    
    for query in queries {
        if let Err(e) = sqlx::query(query).execute(pool).await {
            log::error!("Failed to execute query: {}", e);
        }
    }
    
    log::info!("Database tables created successfully");
}