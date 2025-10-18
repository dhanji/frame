use thiserror::Error;
use actix_web::{error::ResponseError, HttpResponse};

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    #[error("Migration error: {0}")]
    MigrationError(#[from] sqlx::migrate::MigrateError),
    
    #[error("Authentication error: {0}")]
    AuthError(String),
    
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
    
    #[error("Bcrypt error: {0}")]
    BcryptError(#[from] bcrypt::BcryptError),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("Internal server error: {0}")]
    InternalError(String),
    
    #[error("Forbidden: {0}")]
    Forbidden(String),
    
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::DatabaseError(_) | AppError::MigrationError(_) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Database error occurred"
                }))
            }
            AppError::AuthError(msg) => {
                HttpResponse::Unauthorized().json(serde_json::json!({
                    "error": msg
                }))
            }
            AppError::JwtError(_) | AppError::BcryptError(_) => {
                HttpResponse::Unauthorized().json(serde_json::json!({
                    "error": "Authentication failed"
                }))
            }
            AppError::ValidationError(msg) | AppError::BadRequest(msg) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": msg
                }))
            }
            AppError::NotFound(msg) => {
                HttpResponse::NotFound().json(serde_json::json!({
                    "error": msg
                }))
            }
            AppError::Forbidden(msg) => {
                HttpResponse::Forbidden().json(serde_json::json!({
                    "error": msg
                }))
            }
            AppError::ServiceUnavailable(msg) => {
                HttpResponse::ServiceUnavailable().json(serde_json::json!({
                    "error": msg
                }))
            }
            AppError::InternalError(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": msg
                }))
            }
            AppError::IoError(_) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "IO error occurred"
                }))
            }
        }
    }
}

// Type alias for convenience
pub type Result<T> = std::result::Result<T, AppError>;