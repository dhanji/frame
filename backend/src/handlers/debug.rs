use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::EmailManager;
use crate::models::User;

#[derive(Debug, Serialize)]
pub struct SyncDebugInfo {
    pub user_id: i64,
    pub email: String,
    pub is_initialized: bool,
    pub has_imap_service: bool,
    pub has_smtp_service: bool,
    pub oauth_provider: Option<String>,
    pub has_oauth_tokens: bool,
    pub imap_host: String,
    pub imap_port: i32,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub total_emails: i64,
    pub emails_by_folder: Vec<FolderEmailCount>,
    pub recent_sync_errors: Vec<String>,
    pub last_sync_attempt: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FolderEmailCount {
    pub folder: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ImapTestResult {
    pub success: bool,
    pub message: String,
    pub folders: Option<Vec<String>>,
    pub error: Option<String>,
}

/// Get detailed sync debug information
pub async fn get_sync_debug(
    pool: web::Data<SqlitePool>,
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Get user info
    let db_user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ?"
    )
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    // Check if services are initialized
    let is_initialized = email_manager.is_user_initialized(user.user_id).await;
    let has_imap = email_manager.get_imap_service(user.user_id).await.is_some();
    let has_smtp = email_manager.get_smtp_service(user.user_id).await.is_some();

    // Get email counts by folder
    let folder_counts = sqlx::query_as::<_, (String, i64)>(
        "SELECT COALESCE(folder, 'Unknown') as folder, COUNT(*) as count 
         FROM emails 
         WHERE user_id = ? 
         GROUP BY folder"
    )
    .bind(user.user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?
    .into_iter()
    .map(|(folder, count)| FolderEmailCount { folder, count })
    .collect();

    // Get total email count
    let total_emails = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM emails WHERE user_id = ?"
    )
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .unwrap_or(0);

    let has_oauth_tokens = db_user.oauth_access_token.is_some() && !db_user.oauth_access_token.as_ref().unwrap().is_empty();

    Ok(HttpResponse::Ok().json(SyncDebugInfo {
        user_id: user.user_id,
        email: db_user.email.clone(),
        is_initialized,
        has_imap_service: has_imap,
        has_smtp_service: has_smtp,
        oauth_provider: db_user.oauth_provider.clone(),
        has_oauth_tokens,
        imap_host: db_user.imap_host.clone(),
        imap_port: db_user.imap_port,
        smtp_host: db_user.smtp_host.clone(),
        smtp_port: db_user.smtp_port,
        total_emails,
        emails_by_folder: folder_counts,
        recent_sync_errors: vec![], // TODO: implement error tracking
        last_sync_attempt: None, // TODO: implement sync tracking
    }))
}

/// Test IMAP connection and list folders
pub async fn test_imap_connection(
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("Testing IMAP connection for user {}", user.user_id);

    // Check if IMAP service exists
    let imap_service = match email_manager.get_imap_service(user.user_id).await {
        Some(service) => service,
        None => {
            return Ok(HttpResponse::Ok().json(ImapTestResult {
                success: false,
                message: "IMAP service not initialized".to_string(),
                folders: None,
                error: Some("Service not found. Try logging out and back in.".to_string()),
            }));
        }
    };

    // Try to list folders
    match imap_service.list_folders().await {
        Ok(folders) => {
            log::info!("Successfully listed {} folders for user {}", folders.len(), user.user_id);
            Ok(HttpResponse::Ok().json(ImapTestResult {
                success: true,
                message: format!("Successfully connected. Found {} folders.", folders.len()),
                folders: Some(folders),
                error: None,
            }))
        }
        Err(e) => {
            log::error!("IMAP test failed for user {}: {}", user.user_id, e);
            Ok(HttpResponse::Ok().json(ImapTestResult {
                success: false,
                message: "Failed to connect to IMAP server".to_string(),
                folders: None,
                error: Some(format!("{}", e)),
            }))
        }
    }
}

/// Trigger manual sync for the current user
pub async fn trigger_manual_sync(
    pool: web::Data<SqlitePool>,
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("Manual sync triggered for user {}", user.user_id);

    // Get user info
    let db_user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ?"
    )
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    // Check if user's services are initialized
    if !email_manager.is_user_initialized(user.user_id).await {
        // Try to initialize
        if let Err(e) = email_manager.initialize_user(&db_user).await {
            log::error!("Failed to initialize email services for user {}: {}", user.user_id, e);
            return Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to initialize email services: {}", e)
            })));
        }
    }

    // Get IMAP service
    let imap_service = match email_manager.get_imap_service(user.user_id).await {
        Some(service) => service,
        None => {
            return Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": false,
                "error": "IMAP service not available"
            })));
        }
    };

    // Sync INBOX
    let folders = vec!["INBOX", "[Gmail]/Sent Mail", "[Gmail]/Drafts"];
    let mut synced_counts = std::collections::HashMap::new();
    let mut errors = Vec::new();

    for folder in folders {
        log::info!("Syncing folder {} for user {}", folder, user.user_id);
        match sync_folder(&imap_service, &pool, user.user_id, folder).await {
            Ok(count) => {
                log::info!("Synced {} emails from {} for user {}", count, folder, user.user_id);
                synced_counts.insert(folder.to_string(), count);
            }
            Err(e) => {
                log::error!("Failed to sync {} for user {}: {}", folder, user.user_id, e);
                errors.push(format!("{}: {}", folder, e));
            }
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": errors.is_empty(),
        "synced_counts": synced_counts,
        "errors": errors,
    })))
}

/// Helper function to sync a specific folder
async fn sync_folder(
    imap_service: &Arc<crate::services::ImapService>,
    pool: &SqlitePool,
    user_id: i64,
    folder: &str,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    use chrono::Utc;

    // Fetch messages from IMAP
    log::info!("Fetching messages from folder {} for user {}", folder, user_id);
    let messages = imap_service.fetch_messages(folder, 100).await?;
    let message_count = messages.len();
    log::info!("Fetched {} messages from folder {} for user {}", message_count, folder, user_id);

    // Store each message in the database
    for message in messages {
        // Check if message already exists
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM emails WHERE user_id = ? AND message_id = ?"
        )
        .bind(user_id)
        .bind(&message.message_id)
        .fetch_one(pool)
        .await?;

        if exists > 0 {
            // Update existing message (flags, etc.)
            sqlx::query(
                r#"
                UPDATE emails 
                SET is_read = ?, is_starred = ?, updated_at = ?
                WHERE user_id = ? AND message_id = ?
                "#
            )
            .bind(message.flags.contains(&"\\Seen".to_string()))
            .bind(message.flags.contains(&"\\Flagged".to_string()))
            .bind(Utc::now())
            .bind(user_id)
            .bind(&message.message_id)
            .execute(pool)
            .await?;
        } else {
            // Insert new message
            sqlx::query(
                r#"
                INSERT INTO emails (
                    user_id, message_id, thread_id, from_address, to_addresses,
                    cc_addresses, bcc_addresses, subject, body_text, body_html,
                    date, is_read, is_starred, has_attachments, attachments,
                    folder, size, in_reply_to, references, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind(user_id)
            .bind(&message.message_id)
            .bind(&message.thread_id)
            .bind(
                message.from.iter()
                    .map(|a| format!(
                        "{}",
                        if let Some(name) = &a.name {
                            format!("{} <{}>", name, a.email)
                        } else {
                            a.email.clone()
                        }
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .bind(serde_json::to_string(&message.to).unwrap_or_default())
            .bind(serde_json::to_string(&message.cc).unwrap_or_default())
            .bind(serde_json::to_string(&message.bcc).unwrap_or_default())
            .bind(&message.subject)
            .bind(&message.body_text)
            .bind(&message.body_html)
            .bind(message.date)
            .bind(message.flags.contains(&"\\Seen".to_string()))
            .bind(message.flags.contains(&"\\Flagged".to_string()))
            .bind(!message.attachments.is_empty())
            .bind(serde_json::to_string(&message.attachments).ok())
            .bind(folder)
            .bind(0i64) // size placeholder
            .bind(&message.in_reply_to)
            .bind(serde_json::to_string(&message.references).unwrap_or_default())
            .bind(Utc::now())
            .bind(Utc::now())
            .execute(pool)
            .await?;
        }
    }

    Ok(message_count)
}
