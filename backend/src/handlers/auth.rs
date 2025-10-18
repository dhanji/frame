use actix_web::{web, HttpResponse};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::middleware::auth::Claims;
use crate::models::User;
use crate::services::EmailManager;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub username: String,
    pub password: String,
    pub imap_host: String,
    pub imap_port: Option<i32>,
    pub smtp_host: String,
    pub smtp_port: Option<i32>,
    pub smtp_use_tls: Option<bool>,
    pub email_password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: i64,
    pub email: String,
    pub username: String,
}

pub async fn login(
    pool: web::Data<SqlitePool>,
    email_manager: web::Data<Arc<EmailManager>>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    // Find user by email or username (for backward compatibility)
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = ? OR username = ?"
    )
    .bind(&body.username)
    .bind(&body.username) // Check both email and username fields
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;
    
    match user {
        Some(user) => {
            // Verify password
            if verify(&body.password, &user.password_hash).unwrap_or(false) {
                // Generate JWT token
                let expiration = Utc::now()
                    .checked_add_signed(Duration::days(7))
                    .expect("valid timestamp")
                    .timestamp() as usize;
                
                let claims = Claims {
                    sub: user.username.clone(),
                    user_id: user.id,
                    email: user.email.clone(),  // Include email in claims
                    exp: expiration,
                };
                
                let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
                let token = encode(
                    &Header::default(),
                    &claims,
                    &EncodingKey::from_secret(secret.as_ref()),
                )
                .map_err(|e| {
                    log::error!("Token generation error: {}", e);
                    actix_web::error::ErrorInternalServerError("Token generation failed")
                })?;
                
                // Initialize email services (IMAP/SMTP) for the user
                if let Err(e) = email_manager.initialize_user(&user).await {
                    log::error!("Failed to initialize email services for user {}: {}", user.id, e);
                    // Don't fail login, but warn the user
                    return Ok(HttpResponse::Ok().json(serde_json::json!({
                        "token": token,
                        "user": UserInfo { id: user.id, email: user.email, username: user.username },
                        "warning": "Email services could not be initialized. Some features may be unavailable."
                    })));
                }
                
                Ok(HttpResponse::Ok().json(AuthResponse {
                    token,
                    user: UserInfo {
                        id: user.id,
                        email: user.email,
                        username: user.username,
                    },
                }))
            } else {
                Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                    "error": "Invalid credentials"
                })))
            }
        }
        None => Ok(HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Invalid credentials"
        }))),
    }
}

pub async fn register(
    pool: web::Data<SqlitePool>,
    body: web::Json<RegisterRequest>,
    encryption: web::Data<crate::utils::encryption::Encryption>,
) -> Result<HttpResponse, actix_web::Error> {
    // Check if user already exists
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE username = ? OR email = ?"
    )
    .bind(&body.username)
    .bind(&body.email)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;
    
    if existing > 0 {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Username or email already exists"
        })));
    }
    
    // Hash password
    let password_hash = hash(&body.password, DEFAULT_COST)
        .map_err(|e| {
            log::error!("Password hashing error: {}", e);
            actix_web::error::ErrorInternalServerError("Password hashing failed")
        })?;
    
    // Encrypt email password
    let encrypted_email_password = encryption.encrypt(&body.email_password)
        .map_err(|e| {
            log::error!("Encryption error: {}", e);
            actix_web::error::ErrorInternalServerError("Encryption failed")
        })?;
    
    // Create user
    let result = sqlx::query(
        r#"
        INSERT INTO users (
            email, username, password_hash, email_password,
            imap_host, imap_port, smtp_host, smtp_port, smtp_use_tls,
            is_active, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, true, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(&body.email)
    .bind(&body.username)
    .bind(&password_hash)
    .bind(&encrypted_email_password)
    .bind(&body.imap_host)
    .bind(body.imap_port.unwrap_or(993))
    .bind(&body.smtp_host)
    .bind(body.smtp_port.unwrap_or(587))
    .bind(body.smtp_use_tls.unwrap_or(true))
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to create user")
    })?;
    
    let user_id = result.last_insert_rowid();
    
    // Create default folders
    let default_folders = vec!["INBOX", "Sent", "Drafts", "Trash", "Spam", "Archive"];
    for (i, folder) in default_folders.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO folders (user_id, name, sort_order, is_system, created_at, updated_at)
            VALUES (?, ?, ?, true, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#
        )
        .bind(user_id)
        .bind(folder)
        .bind(i as i32)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Failed to create folder {}: {}", folder, e);
            actix_web::error::ErrorInternalServerError("Failed to create default folders")
        })?;
    }
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "User registered successfully",
        "user_id": user_id
    })))
}

pub async fn logout(
    pool: web::Data<SqlitePool>,
    email_manager: web::Data<Arc<EmailManager>>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Invalidate session
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user.user_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to logout")
        })?;
    
    // Remove user's email services from memory
    email_manager.remove_user(user.user_id).await;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Logged out successfully"
    })))
}