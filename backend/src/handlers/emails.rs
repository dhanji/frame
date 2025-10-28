use actix_web::{web, HttpResponse};
use crate::utils::sanitize::sanitize_for_storage;
use crate::services::email_sync::EmailSyncService;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::{
    middleware::auth::AuthenticatedUser,
    models::Email,
    services::smtp::{EmailComposition, AttachmentData},
    services::EmailManager,
};


#[derive(Debug, Deserialize, Clone)]
pub struct SendEmailRequest {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<AttachmentData>,
}

#[derive(Debug, Deserialize)]
pub struct ReplyEmailRequest {
    pub conversation_id: String,
    pub reply_type: String,  // "reply", "reply-all", "forward"
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<AttachmentData>,
}

#[derive(Debug, Deserialize)]
pub struct MarkReadRequest {
    pub is_read: bool,
}

#[derive(Debug, Deserialize)]
pub struct MoveEmailRequest {
    pub folder: String,
}

// Internal function for sending emails
pub async fn send_email_internal(
    pool: &SqlitePool,
    body: &SendEmailRequest,
    email_manager: &Arc<EmailManager>,
    user: &AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Get user details for SMTP
    let user_data = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE id = ?"
    )
    .bind(user.user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch user")
    })?;
    
    let user_data = user_data.ok_or_else(|| {
        actix_web::error::ErrorNotFound("User not found")
    })?;
    
    // Get SMTP service from EmailManager
    let smtp_service = email_manager.get_smtp_service(user.user_id).await
        .ok_or_else(|| {
            log::error!("SMTP service not initialized for user {}", user.user_id);
            actix_web::error::ErrorInternalServerError("Email service not initialized. Please log in again.")
        })?;
    
    // Initialize SMTP service if needed
    // (This is handled in the login process, but we check here for safety)
    
    // Sanitize HTML content
    let sanitized_html = body.body_html.as_ref().map(|html| sanitize_for_storage(html));
    
    // Prepare email composition
    let composition = EmailComposition {
        from: user_data.email.clone(),
        to: body.to.clone(),
        cc: body.cc.clone(),
        bcc: body.bcc.clone(),
        subject: body.subject.clone(),
        body_text: body.body_text.clone(),
        body_html: sanitized_html.clone(),
        attachments: body.attachments.clone(),
        in_reply_to: None,
        references: vec![],
    };
    
    // Send email
    let message_id = smtp_service.send_email(composition).await
        .map_err(|e| {
            log::error!("Failed to send email: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to send email")
        })?;
    
    // Store in database
    sqlx::query(
        r#"
        INSERT INTO emails (
            user_id, message_id, from_address, to_addresses, cc_addresses, bcc_addresses,
            subject, body_text, body_html, date, folder, is_read, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, 'Sent', true, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(user.user_id)
    .bind(&message_id)
    .bind(&user_data.email)
    .bind(serde_json::to_string(&body.to).unwrap())
    .bind(serde_json::to_string(&body.cc).unwrap())
    .bind(serde_json::to_string(&body.bcc).unwrap())
    .bind(&body.subject)
    .bind(&body.body_text)
    .bind(&sanitized_html)
    .execute(pool)
    .await
    .map_err(|e| {
        log::error!("Failed to store sent email: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to store sent email")
    })?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Email sent successfully",
        "message_id": message_id
    })))
}

pub async fn send_email(
    pool: web::Data<SqlitePool>,
    body: web::Json<SendEmailRequest>,
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    send_email_internal(pool.get_ref(), &body, &email_manager, &user).await
}

pub async fn reply_to_email(
    pool: web::Data<SqlitePool>,
    path: web::Path<String>,
    body: web::Json<ReplyEmailRequest>,
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let email_id = path.into_inner();
    
    // Get original email
    let original = sqlx::query_as::<_, Email>(
        "SELECT * FROM emails WHERE user_id = ? AND message_id = ?"
    )
    .bind(user.user_id)
    .bind(&email_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch email")
    })?;
    
    let mut original = original.ok_or_else(|| {
        actix_web::error::ErrorNotFound("Email not found")
    })?;
    
    // Parse JSON fields
    original.parse_json_fields();
    
    // Get user details
    let user_data = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE id = ?"
    )
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch user")
    })?;
    
    // Get SMTP service from EmailManager
    let smtp_service = email_manager.get_smtp_service(user.user_id).await
        .ok_or_else(|| {
            actix_web::error::ErrorInternalServerError("Email service not initialized")
        })?;
    
    // Sanitize HTML content
    let sanitized_html = body.body_html.as_ref().map(|html| sanitize_for_storage(html));
    
    // Prepare reply composition
    let composition = EmailComposition {
        from: user_data.email.clone(),
        to: body.to.clone(),
        cc: body.cc.clone(),
        bcc: body.bcc.clone(),
        subject: body.subject.clone(),
        body_text: body.body_text.clone(),
        body_html: sanitized_html.clone(),
        attachments: body.attachments.clone(),
        in_reply_to: Some(original.message_id.clone()),
        references: {
            let mut refs = original.references_list.clone();
            refs.push(original.message_id.clone());
            refs
        },
    };
    
    // Send reply
    let message_id = match body.reply_type.as_str() {
        "forward" => smtp_service.send_forward(
            &original.body_text.unwrap_or_default(),
            composition
        ).await,
        _ => smtp_service.send_email(composition).await,
    }
    .map_err(|e| {
        log::error!("Failed to send reply: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to send reply")
    })?;
    
    // Store reply in database
    sqlx::query(
        r#"
        INSERT INTO emails (
            user_id, message_id, thread_id, from_address, to_addresses, cc_addresses, bcc_addresses,
            subject, body_text, body_html, date, folder, is_read, in_reply_to, references,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, 'Sent', true, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(user.user_id)
    .bind(&message_id)
    .bind(&original.thread_id)
    .bind(&user_data.email)
    .bind(serde_json::to_string(&body.to).unwrap())
    .bind(serde_json::to_string(&body.cc).unwrap())
    .bind(serde_json::to_string(&body.bcc).unwrap())
    .bind(&body.subject)
    .bind(&body.body_text)
    .bind(&sanitized_html)
    .bind(&original.message_id)
    .bind(serde_json::to_string(&original.references).unwrap())
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to store reply: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to store reply")
    })?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Reply sent successfully",
        "message_id": message_id
    })))
}

pub async fn mark_as_read(
    pool: web::Data<SqlitePool>,
    path: web::Path<String>,
    body: web::Json<MarkReadRequest>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let email_id = path.into_inner();
    
    sqlx::query(
        "UPDATE emails SET is_read = ?, updated_at = CURRENT_TIMESTAMP WHERE user_id = ? AND message_id = ?"
    )
    .bind(body.is_read)
    .bind(user.user_id)
    .bind(&email_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to update email")
    })?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Email marked as read"
    })))
}

pub async fn delete_email(
    pool: web::Data<SqlitePool>,
    path: web::Path<String>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let email_id = path.into_inner();
    
    sqlx::query(
        "UPDATE emails SET folder = 'Trash', deleted_at = CURRENT_TIMESTAMP WHERE user_id = ? AND message_id = ?"
    )
    .bind(user.user_id)
    .bind(&email_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to delete email")
    })?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Email moved to trash"
    })))
}

pub async fn move_email(
    pool: web::Data<SqlitePool>,
    path: web::Path<String>,
    body: web::Json<MoveEmailRequest>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let email_id = path.into_inner();
    
    sqlx::query(
        "UPDATE emails SET folder = ?, updated_at = CURRENT_TIMESTAMP WHERE user_id = ? AND message_id = ?"
    )
    .bind(&body.folder)
    .bind(user.user_id)
    .bind(&email_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to move email")
    })?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Email moved to {}", body.folder)
    })))
}

pub async fn forward_email(
    pool: web::Data<SqlitePool>,
    path: web::Path<String>,
    body: web::Json<SendEmailRequest>,
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let email_id = path.into_inner();
    
    // Get original email
    let original = sqlx::query_as::<_, Email>(
        "SELECT * FROM emails WHERE user_id = ? AND message_id = ?"
    )
    .bind(user.user_id)
    .bind(&email_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch email")
    })?;
    
    let original = original.ok_or_else(|| {
        actix_web::error::ErrorNotFound("Email not found")
    })?;
    
    // Create forward request
    let forward_request = ReplyEmailRequest {
        conversation_id: email_id.clone(),
        reply_type: "forward".to_string(),
        to: body.to.clone(),
        cc: body.cc.clone(),
        bcc: body.bcc.clone(),
        subject: format!("Fwd: {}", original.subject),
        body_text: body.body_text.clone(),
        body_html: body.body_html.clone(),
        attachments: body.attachments.clone(),
    };
    
    // Get user details
    let user_data = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE id = ?"
    )
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch user")
    })?;
    
    // Get SMTP service from EmailManager
    let smtp_service = email_manager.get_smtp_service(user.user_id).await
        .ok_or_else(|| {
            actix_web::error::ErrorInternalServerError("Email service not initialized")
        })?;
    
    // Sanitize HTML content
    let sanitized_html = forward_request.body_html.as_ref().map(|html| sanitize_for_storage(html));
    
    // Prepare forward composition
    let composition = EmailComposition {
        from: user_data.email.clone(),
        to: forward_request.to.clone(),
        cc: forward_request.cc.clone(),
        bcc: forward_request.bcc.clone(),
        subject: forward_request.subject.clone(),
        body_text: forward_request.body_text.clone(),
        body_html: sanitized_html.clone(),
        attachments: forward_request.attachments.clone(),
        in_reply_to: None,
        references: vec![],
    };
    
    // Send forward
    let message_id = smtp_service.send_forward(
        &original.body_text.unwrap_or_default(),
        composition
    ).await
    .map_err(|e| {
        log::error!("Failed to send forward: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to send forward")
    })?;
    
    // Store forwarded email in database
    sqlx::query(
        r#"
        INSERT INTO emails (
            user_id, message_id, from_address, to_addresses, cc_addresses, bcc_addresses,
            subject, body_text, body_html, date, folder, is_read, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, 'Sent', true, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(user.user_id)
    .bind(&message_id)
    .bind(&user_data.email)
    .bind(serde_json::to_string(&forward_request.to).unwrap())
    .bind(serde_json::to_string(&forward_request.cc).unwrap())
    .bind(serde_json::to_string(&forward_request.bcc).unwrap())
    .bind(&forward_request.subject)
    .bind(&forward_request.body_text)
    .bind(&sanitized_html)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to store forwarded email: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to store forwarded email")
    })?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Email forwarded successfully",
        "message_id": message_id
    })))
}

pub async fn star_email(
    pool: web::Data<SqlitePool>,
    path: web::Path<String>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let email_id = path.into_inner();
    
    sqlx::query(
        "UPDATE emails SET is_starred = NOT is_starred, updated_at = CURRENT_TIMESTAMP WHERE user_id = ? AND message_id = ?"
    )
    .bind(user.user_id)
    .bind(&email_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to star email")
    })?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Email star status toggled"
    })))
}

#[derive(Debug, Deserialize)]
pub struct BulkActionRequest {
    pub email_ids: Vec<String>,
    pub action: String, // "read", "unread", "delete", "star", "unstar", "move"
    pub folder: Option<String>, // Required for "move" action
}

pub async fn bulk_action(
    pool: web::Data<SqlitePool>,
    body: web::Json<BulkActionRequest>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let email_ids_str = body.email_ids.iter()
        .map(|id| format!("'{}'", id))
        .collect::<Vec<_>>()
        .join(",");
    
    let query = match body.action.as_str() {
        "read" => format!(
            "UPDATE emails SET is_read = true, updated_at = CURRENT_TIMESTAMP WHERE user_id = {} AND message_id IN ({})",
            user.user_id, email_ids_str
        ),
        "unread" => format!(
            "UPDATE emails SET is_read = false, updated_at = CURRENT_TIMESTAMP WHERE user_id = {} AND message_id IN ({})",
            user.user_id, email_ids_str
        ),
        "delete" => format!(
            "UPDATE emails SET folder = 'Trash', deleted_at = CURRENT_TIMESTAMP WHERE user_id = {} AND message_id IN ({})",
            user.user_id, email_ids_str
        ),
        "star" => format!(
            "UPDATE emails SET is_starred = true, updated_at = CURRENT_TIMESTAMP WHERE user_id = {} AND message_id IN ({})",
            user.user_id, email_ids_str
        ),
        "unstar" => format!(
            "UPDATE emails SET is_starred = false, updated_at = CURRENT_TIMESTAMP WHERE user_id = {} AND message_id IN ({})",
            user.user_id, email_ids_str
        ),
        "move" => {
            let folder = body.folder.as_ref().ok_or_else(|| {
                actix_web::error::ErrorBadRequest("Folder is required for move action")
            })?;
            format!(
                "UPDATE emails SET folder = '{}', updated_at = CURRENT_TIMESTAMP WHERE user_id = {} AND message_id IN ({})",
                folder, user.user_id, email_ids_str
            )
        },
        _ => return Err(actix_web::error::ErrorBadRequest("Invalid action")),
    };
    
    sqlx::query(&query)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to perform bulk action")
        })?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Bulk {} action completed for {} emails", body.action, body.email_ids.len())
    })))
}

/// Trigger manual email sync for the authenticated user
pub async fn trigger_sync(
    pool: web::Data<SqlitePool>,
    email_manager: web::Data<Arc<crate::services::EmailManager>>,
    user: web::ReqData<crate::middleware::auth::Claims>,
) -> Result<HttpResponse, actix_web::Error> {
    // Get user from database
    let user_record = sqlx::query_as::<_, crate::models::User>("SELECT * FROM users WHERE id = ?")
        .bind(user.user_id)
        .fetch_one(pool.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    
    let sync_service = EmailSyncService::new(pool.get_ref().clone(), email_manager.get_ref().clone());
    
    tokio::spawn(async move {
        let _ = sync_service.sync_user_emails(&user_record).await;
    });
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Email sync triggered successfully"
    })))
}