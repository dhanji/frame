use actix_web::{web, HttpResponse};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use oauth2::{
    AuthorizationCode, AuthUrl, ClientId, ClientSecret, CsrfToken,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use crate::middleware::auth::Claims;
use crate::models::User;
use crate::services::EmailManager;
use crate::services::email_sync::EmailSyncService;
use oauth2::RefreshToken;

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
pub struct GoogleAuthUrlResponse {
    pub auth_url: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct GoogleCallbackRequest {
    pub code: String,
    pub state: String,
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

// Google OAuth2 Configuration
fn get_google_oauth_client() -> Result<BasicClient, String> {
    let google_client_id = std::env::var("GOOGLE_CLIENT_ID")
        .map_err(|_| "GOOGLE_CLIENT_ID not set".to_string())?;
    let google_client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
        .map_err(|_| "GOOGLE_CLIENT_SECRET not set".to_string())?;
    let redirect_url = std::env::var("GOOGLE_REDIRECT_URL")
        .unwrap_or_else(|_| "http://localhost:8080/api/auth/google/callback".to_string());

    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .map_err(|e| format!("Invalid authorization endpoint URL: {}", e))?;
    let token_url = TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
        .map_err(|e| format!("Invalid token endpoint URL: {}", e))?;

    Ok(BasicClient::new(
        ClientId::new(google_client_id),
        Some(ClientSecret::new(google_client_secret)),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(
        RedirectUrl::new(redirect_url)
            .map_err(|e| format!("Invalid redirect URL: {}", e))?,
    ))
}

// Initiate Google OAuth2 flow
pub async fn google_auth_url() -> Result<HttpResponse, actix_web::Error> {
    let client = get_google_oauth_client()
        .map_err(|e| {
            log::error!("OAuth client error: {}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("https://www.googleapis.com/auth/gmail.readonly".to_string()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/gmail.send".to_string()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/gmail.modify".to_string()))
        .add_scope(Scope::new("https://mail.google.com/".to_string()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.email".to_string()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.profile".to_string()))
        .url();

    Ok(HttpResponse::Ok().json(GoogleAuthUrlResponse {
        auth_url: auth_url.to_string(),
        state: csrf_token.secret().to_string(),
    }))
}

/// Auto-login endpoint for single-user desktop app mode
pub async fn auto_login(
    pool: web::Data<SqlitePool>,
) -> Result<HttpResponse, actix_web::Error> {
    // Check if exactly one user exists
    let user_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Database error checking user count: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;
    
    if user_count != 1 {
        // Only auto-login if exactly one user exists
        return Ok(HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Auto-login not available"
        })));
    }
    
    // Get the single user
    let user = sqlx::query_as::<_, User>("SELECT * FROM users LIMIT 1")
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Database error fetching user: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;
    
    // Generate JWT token
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .expect("valid timestamp")
        .timestamp() as usize;
    
    let claims = Claims {
        sub: user.username.clone(),
        user_id: user.id,
        email: user.email.clone(),
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
    
    log::info!("Auto-login successful for user: {}", user.email);
    
    Ok(HttpResponse::Ok().json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id,
            email: user.email,
            username: user.username,
        },
    }))
}

// Handle Google OAuth2 callback
pub async fn google_callback(
    pool: web::Data<SqlitePool>,
    email_manager: web::Data<Arc<EmailManager>>,
    encryption: web::Data<crate::utils::encryption::Encryption>,
    query: web::Query<GoogleCallbackRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let client = get_google_oauth_client()
        .map_err(|e| {
            log::error!("OAuth client error: {}", e);
            actix_web::error::ErrorInternalServerError(e)
        })?;

    // Exchange the code for an access token
    let token_result = client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            log::error!("Token exchange error: {:?}", e);
            actix_web::error::ErrorInternalServerError("Failed to exchange authorization code")
        })?;

    let access_token = token_result.access_token().secret();
    let refresh_token = token_result.refresh_token()
        .map(|t| t.secret().to_string())
        .unwrap_or_default();

    // Get user info from Google
    let user_info_response = reqwest::Client::new()
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            log::error!("Failed to get user info: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to get user info")
        })?;

    let user_info: serde_json::Value = user_info_response.json().await
        .map_err(|e| {
            log::error!("Failed to parse user info: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to parse user info")
        })?;

    let email = user_info["email"].as_str()
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Email not found in user info"))?;
    let name = user_info["name"].as_str().unwrap_or(email);

    // Check if user exists
    let existing_user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = ?"
    )
    .bind(email)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let user = if let Some(mut user) = existing_user {
        // Update existing user with new OAuth tokens
        let encrypted_access_token = encryption.encrypt(access_token)
            .map_err(|e| {
                log::error!("Encryption error: {}", e);
                actix_web::error::ErrorInternalServerError("Encryption failed")
            })?;
        let encrypted_refresh_token = encryption.encrypt(&refresh_token)
            .map_err(|e| {
                log::error!("Encryption error: {}", e);
                actix_web::error::ErrorInternalServerError("Encryption failed")
            })?;

        sqlx::query(
            "UPDATE users SET oauth_access_token = ?, oauth_refresh_token = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(&encrypted_access_token)
        .bind(&encrypted_refresh_token)
        .bind(user.id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to update user")
        })?;

        user
    } else {
        // Create new user with OAuth
        let encrypted_access_token = encryption.encrypt(access_token)
            .map_err(|e| {
                log::error!("Encryption error: {}", e);
                actix_web::error::ErrorInternalServerError("Encryption failed")
            })?;
        let encrypted_refresh_token = encryption.encrypt(&refresh_token)
            .map_err(|e| {
                log::error!("Encryption error: {}", e);
                actix_web::error::ErrorInternalServerError("Encryption failed")
            })?;

        // For Gmail OAuth, we use Gmail's IMAP/SMTP with OAuth2
        let result = sqlx::query(
            r#"INSERT INTO users (email, username, password_hash, oauth_provider, oauth_access_token, oauth_refresh_token,
               imap_host, imap_port, smtp_host, smtp_port, smtp_use_tls, is_active, created_at, updated_at)
               VALUES (?, ?, '', 'google', ?, ?, 'imap.gmail.com', 993, 'smtp.gmail.com', 587, true, true, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#
        )
        .bind(email)
        .bind(name)
        .bind(&encrypted_access_token)
        .bind(&encrypted_refresh_token)
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
                "INSERT INTO folders (user_id, name, sort_order, is_system, created_at, updated_at) VALUES (?, ?, ?, true, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
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

        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(pool.get_ref())
            .await
            .map_err(|e| {
                log::error!("Database error: {}", e);
                actix_web::error::ErrorInternalServerError("Database error")
            })?
    };

    // Generate JWT token
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user.username.clone(),
        user_id: user.id,
        email: user.email.clone(),
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

    // Initialize email services (non-blocking)
    if let Err(e) = email_manager.initialize_user(&user).await {
        log::error!("Failed to initialize email services for user {}: {}", user.id, e);
    }

    // Redirect to frontend with token
    Ok(HttpResponse::Found()
        .append_header(("Location", format!("/?token={}&email={}&oauth_success=true", token, email)))
        .finish())
}

/// Manual OAuth token refresh endpoint
pub async fn refresh_token_endpoint(
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let refresh_service = crate::services::OAuthRefreshService::new();
    
    match refresh_service.refresh_token(pool.get_ref(), user.user_id).await {
        Ok(_) => {
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": "Token refreshed successfully"
            })))
        }
        Err(e) => {
            log::error!("Failed to refresh token for user {}: {}", user.user_id, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to refresh token: {}", e)
            })))
        }
    }
}