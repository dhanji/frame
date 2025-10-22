use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer, HttpResponse};
use actix_files as fs;
use actix_web_httpauth::middleware::HttpAuthentication;
use std::sync::Arc;
use sqlx::SqlitePool;
use email_client_backend::{handlers, services, websocket, db, utils::encryption::Encryption};

use services::{EmailManager, EmailSyncService};
// WebSocket support will be added when actix-web-actors is properly integrated

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
    match db::run_migrations(&pool).await {
        Ok(_) => log::info!("Database migrations completed successfully"),
        Err(e) => {
        log::warn!("Failed to run migrations: {}. Creating tables manually...", e);
        // Create tables manually if migrations fail
        create_tables(&pool).await;
        }
    }
    
    // Create shared EmailManager
    let email_manager = Arc::new(EmailManager::new());
    
    
    // Start background email sync service
    let sync_service = EmailSyncService::new(pool.clone(), email_manager.clone());
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await; // Wait for server to start
        log::info!("Starting background email sync service");
        sync_service.start().await;
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
            .app_data(web::Data::new(Encryption::new()))
            .route("/health", web::get().to(health_check))
            .service(
                web::scope("/api")
                    .route("/register", web::post().to(handlers::auth::register))
                    .route("/login", web::post().to(handlers::auth::login))
                    // Protected endpoints (require authentication)
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
                            .route("/search", web::get().to(handlers::search::search_emails))
                            .route("/emails/bulk", web::post().to(handlers::emails::bulk_action))
                            .route("/logout", web::post().to(handlers::auth::logout))
                    )
            )
            .route("/ws", web::get().to(websocket::websocket_handler))
            // Serve static files from frontend directory (MUST be last to not catch API routes)
            .service(fs::Files::new("/", "../frontend").index_file("index.html"))
    })
    .bind(("127.0.0.1", 8080))?
    .workers(1) // Use single worker to avoid port conflicts
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
            id INTEGER PRIMARY KEY AUTOINCREMENT,
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