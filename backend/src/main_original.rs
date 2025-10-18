use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
tokio::sync::RwLock;

#[derive(Debug, Serialize, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegisterRequest {
    email: String,
    password: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Conversation {
    id: String,
    subject: String,
    participants: Vec<String>,
    last_message: String,
    timestamp: String,
    unread: bool,
    messages: Vec<EmailMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EmailMessage {
    id: String,
    from: String,
    to: Vec<String>,
    subject: String,
    body: String,
    timestamp: String,
    is_read: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Folder {
    id: String,
    name: String,
    folder_type: String,
    unread_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct SendEmailRequest {
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    subject: String,
    body: String,
}

// Mock data for demo
fn get_demo_conversations() -> Vec<Conversation> {
    vec![
        Conversation {
            id: "1".to_string(),
            subject: "Project Update".to_string(),
            participants: vec!["john@example.com".to_string()],
            last_message: "We're making good progress on the project...".to_string(),
            timestamp: "2 hours ago".to_string(),
            unread: true,
            messages: vec![
                EmailMessage {
                    id: "m1".to_string(),
                    from: "john@example.com".to_string(),
                    to: vec!["you@example.com".to_string()],
                    subject: "Project Update".to_string(),
                    body: "Hey team, just wanted to give you a quick update on the project status. We're making good progress!".to_string(),
                    timestamp: "2 hours ago".to_string(),
                    is_read: false,
                },
                EmailMessage {
                    id: "m2".to_string(),
                    from: "you@example.com".to_string(),
                    to: vec!["john@example.com".to_string()],
                    subject: "Re: Project Update".to_string(),
                    body: "Thanks for the update John! When do you think we'll have the first milestone completed?".to_string(),
                    timestamp: "1 hour ago".to_string(),
                    is_read: true,
                },
            ],
        },
        Conversation {
            id: "2".to_string(),
            subject: "Meeting Tomorrow".to_string(),
            participants: vec!["sarah@example.com".to_string()],
            last_message: "Don't forget about our meeting tomorrow at 10 AM...".to_string(),
            timestamp: "5 hours ago".to_string(),
            unread: false,
            messages: vec![
                EmailMessage {
                    id: "m3".to_string(),
                    from: "sarah@example.com".to_string(),
                    to: vec!["you@example.com".to_string()],
                    subject: "Meeting Tomorrow".to_string(),
                    body: "Don't forget about our meeting tomorrow at 10 AM. I've sent the agenda.".to_string(),
                    timestamp: "5 hours ago".to_string(),
                    is_read: true,
                },
            ],
        },
    ]
}

fn get_demo_folders() -> Vec<Folder> {
    vec![
        Folder {
            id: "inbox".to_string(),
            name: "Inbox".to_string(),
            folder_type: "inbox".to_string(),
            unread_count: 3,
        },
        Folder {
            id: "sent".to_string(),
            name: "Sent".to_string(),
            folder_type: "sent".to_string(),
            unread_count: 0,
        },
        Folder {
            id: "drafts".to_string(),
            name: "Drafts".to_string(),
            folder_type: "drafts".to_string(),
            unread_count: 1,
        },
        Folder {
            id: "trash".to_string(),
            name: "Trash".to_string(),
            folder_type: "trash".to_string(),
            unread_count: 0,
        },
    ]
}

// API Handlers
async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now()
    }))
}

async fn register(body: web::Json<RegisterRequest>) -> HttpResponse {
    log::info!("Register request for: {}", body.email);
    
    // Mock registration
    let token = uuid::Uuid::new_v4().to_string();
    
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Registration successful",
        "token": token,
        "user": {
            "id": uuid::Uuid::new_v4().to_string(),
            "email": body.email,
            "name": body.name
        }
    }))
}

async fn login(body: web::Json<LoginRequest>) -> HttpResponse {
    log::info!("Login request for: {}", body.email);
    
    // Mock authentication
    if body.email == "test@example.com" && body.password == "Test123!" {
        let token = uuid::Uuid::new_v4().to_string();
        HttpResponse::Ok().json(serde_json::json!({
            "message": "Login successful",
            "token": token,
            "user": {
                "id": "user123",
                "email": body.email,
                "name": "Test User"
            }
        }))
    } else {
        HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Invalid credentials"
        }))
    }
}

async fn get_conversations() -> HttpResponse {
    HttpResponse::Ok().json(get_demo_conversations())
}

async fn get_conversation(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    let conversations = get_demo_conversations();
    
    if let Some(conversation) = conversations.into_iter().find(|c| c.id == id) {
        HttpResponse::Ok().json(conversation)
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "error": "Conversation not found"
        }))
    }
}

async fn send_email(body: web::Json<SendEmailRequest>) -> HttpResponse {
    log::info!("Sending email to: {:?}", body.to);
    
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Email sent successfully",
        "message_id": uuid::Uuid::new_v4().to_string()
    }))
}

async fn reply_to_email(path: web::Path<String>, body: web::Json<SendEmailRequest>) -> HttpResponse {
    let email_id = path.into_inner();
    log::info!("Replying to email: {}", email_id);
    
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Reply sent successfully",
        "message_id": uuid::Uuid::new_v4().to_string()
    }))
}

async fn mark_as_read(path: web::Path<String>) -> HttpResponse {
    let email_id = path.into_inner();
    log::info!("Marking email as read: {}", email_id);
    
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Email marked as read"
    }))
}

async fn delete_email(path: web::Path<String>) -> HttpResponse {
    let email_id = path.into_inner();
    log::info!("Deleting email: {}", email_id);
    
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Email moved to trash"
    }))
}

async fn move_email(path: web::Path<String>, body: web::Json<serde_json::Value>) -> HttpResponse {
    let email_id = path.into_inner();
    log::info!("Moving email: {}", email_id);
    
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Email moved successfully"
    }))
}

async fn get_folders() -> HttpResponse {
    HttpResponse::Ok().json(get_demo_folders())
}

async fn create_folder(body: web::Json<serde_json::Value>) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Folder created successfully",
        "folder_id": uuid::Uuid::new_v4().to_string()
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    log::info!("Starting Frame Email Client Backend...");

    // Initialize database (create if not exists)
    let database_url = "sqlite:email_client.db";
    let _ = SqlitePool::connect(database_url).await;

    log::info!("Server starting on http://localhost:8080");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(Logger::default())
            .route("/health", web::get().to(health_check))
            .service(
                web::scope("/api")
                    .route("/register", web::post().to(register))
                    .route("/login", web::post().to(login))
                    .route("/conversations", web::get().to(get_conversations))
                    .route("/conversations/{id}", web::get().to(get_conversation))
                    .route("/emails/send", web::post().to(send_email))
                    .route("/emails/{id}/reply", web::post().to(reply_to_email))
                    .route("/emails/{id}/read", web::put().to(mark_as_read))
                    .route("/emails/{id}", web::delete().to(delete_email))
                    .route("/emails/{id}/move", web::post().to(move_email))
                    .route("/folders", web::get().to(get_folders))
                    .route("/folders", web::post().to(create_folder))
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
