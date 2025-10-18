use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::middleware::auth::AuthenticatedUser;
use crate::models::Draft;
use crate::services::EmailManager;

#[derive(Debug, Deserialize)]
pub struct AutoSaveDraftRequest {
    pub id: Option<String>,
    pub to_addresses: Option<Vec<String>>,
    pub cc_addresses: Option<Vec<String>>,
    pub bcc_addresses: Option<Vec<String>>,
    pub subject: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Option<Vec<String>>,
    pub in_reply_to: Option<String>,
    pub references: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct DraftResponse {
    pub id: String,
    pub message: String,
}

pub async fn auto_save_draft(
    pool: web::Data<SqlitePool>,
    body: web::Json<AutoSaveDraftRequest>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let draft_data = body.into_inner();
    
    // Convert arrays to JSON strings for storage
    let to_json = draft_data.to_addresses.map(|v| serde_json::to_string(&v).unwrap_or_default());
    let cc_json = draft_data.cc_addresses.map(|v| serde_json::to_string(&v).unwrap_or_default());
    let bcc_json = draft_data.bcc_addresses.map(|v| serde_json::to_string(&v).unwrap_or_default());
    let attachments_json = draft_data.attachments.map(|v| serde_json::to_string(&v).unwrap_or_default());
    let references_json = draft_data.references.map(|v| serde_json::to_string(&v).unwrap_or_default());
    
    if let Some(draft_id) = draft_data.id {
        // Update existing draft
        sqlx::query(
            r#"
            UPDATE drafts SET 
                to_addresses = COALESCE(?, to_addresses),
                cc_addresses = COALESCE(?, cc_addresses),
                bcc_addresses = COALESCE(?, bcc_addresses),
                subject = COALESCE(?, subject),
                body_text = COALESCE(?, body_text),
                body_html = COALESCE(?, body_html),
                attachments = COALESCE(?, attachments),
                in_reply_to = COALESCE(?, in_reply_to),
                references = COALESCE(?, references),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ? AND user_id = ?
            "#
        )
        .bind(&to_json)
        .bind(&cc_json)
        .bind(&bcc_json)
        .bind(&draft_data.subject)
        .bind(&draft_data.body_text)
        .bind(&draft_data.body_html)
        .bind(&attachments_json)
        .bind(&draft_data.in_reply_to)
        .bind(&references_json)
        .bind(&draft_id)
        .bind(user.user_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Failed to update draft: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to update draft")
        })?;
        
        Ok(HttpResponse::Ok().json(DraftResponse {
            id: draft_id,
            message: "Draft updated".to_string(),
        }))
    } else {
        // Create new draft
        let result = sqlx::query(
            r#"
            INSERT INTO drafts (
                user_id, to_addresses, cc_addresses, bcc_addresses,
                subject, body_text, body_html, attachments,
                in_reply_to, references, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#
        )
        .bind(user.user_id)
        .bind(&to_json)
        .bind(&cc_json)
        .bind(&bcc_json)
        .bind(&draft_data.subject)
        .bind(&draft_data.body_text)
        .bind(&draft_data.body_html)
        .bind(&attachments_json)
        .bind(&draft_data.in_reply_to)
        .bind(&references_json)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Failed to create draft: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to create draft")
        })?;
        
        let draft_id = result.last_insert_rowid().to_string();
        
        Ok(HttpResponse::Ok().json(DraftResponse {
            id: draft_id,
            message: "Draft saved".to_string(),
        }))
    }
}

pub async fn get_drafts(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let drafts = sqlx::query_as::<_, Draft>(
        "SELECT * FROM drafts WHERE user_id = ? ORDER BY updated_at DESC"
    )
    .bind(user.user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to fetch drafts: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch drafts")
    })?;
    
    // Convert JSON strings back to arrays for response
    let drafts_response: Vec<serde_json::Value> = drafts
        .into_iter()
        .map(|draft| {
            serde_json::json!({
                "id": draft.id,
                "to": draft.to_addresses.and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()),
                "cc": draft.cc_addresses.and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()),
                "bcc": draft.bcc_addresses.and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()),
                "subject": draft.subject,
                "body_text": draft.body_text,
                "body_html": draft.body_html,
                "attachments": draft.attachments.and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()),
                "in_reply_to": draft.in_reply_to,
                "references": draft.references.and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()),
                "created_at": draft.created_at,
                "updated_at": draft.updated_at,
            })
        })
        .collect();
    
    Ok(HttpResponse::Ok().json(drafts_response))
}

pub async fn send_draft(
    pool: web::Data<SqlitePool>,
    path: web::Path<i64>,
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let draft_id = path.into_inner();
    
    // Get draft
    let draft = sqlx::query_as::<_, Draft>(
        "SELECT * FROM drafts WHERE id = ? AND user_id = ?"
    )
    .bind(draft_id)
    .bind(user.user_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to fetch draft: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch draft")
    })?;
    
    let draft = draft.ok_or_else(|| {
        actix_web::error::ErrorNotFound("Draft not found")
    })?;
    
    // Parse JSON fields
    let to_addresses: Vec<String> = draft.to_addresses
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    
    let cc_addresses: Vec<String> = draft.cc_addresses
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    
    let bcc_addresses: Vec<String> = draft.bcc_addresses
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    
    // Create email send request
    let send_request = crate::handlers::emails::SendEmailRequest {
        to: to_addresses,
        cc: cc_addresses,
        bcc: bcc_addresses,
        subject: draft.subject.unwrap_or_default(),
        body_text: draft.body_text,
        body_html: draft.body_html,
        attachments: vec![],
    };
    
    // Send email using the email handler
    use crate::handlers::emails;
    
    let _ = crate::handlers::emails::send_email_internal(pool.get_ref(), &send_request, &email_manager, &user).await?;
    
    // Delete draft after sending
    sqlx::query("DELETE FROM drafts WHERE id = ? AND user_id = ?")
        .bind(draft_id)
        .bind(user.user_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Failed to delete draft: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to delete draft")
        })?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({"message": "Draft sent successfully"})))
}

pub async fn delete_draft(
    pool: web::Data<SqlitePool>,
    path: web::Path<i64>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let draft_id = path.into_inner();
    
    let result = sqlx::query(
        "DELETE FROM drafts WHERE id = ? AND user_id = ?"
    )
    .bind(draft_id)
    .bind(user.user_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to delete draft: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to delete draft")
    })?;
    
    if result.rows_affected() == 0 {
        return Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Draft not found"
        })));
    }
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Draft deleted successfully"
    })))
}