use actix_web::{web, HttpResponse};
use crate::utils::sanitize::sanitize_for_display;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::middleware::auth::AuthenticatedUser;
use crate::services::ConversationService;

#[derive(Debug, Serialize)]
pub struct ConversationResponse {
    pub id: String,
    pub preview: String,
    pub conversation_type: String,
    pub icon: String,
    pub subject: String,
    pub participants: Vec<String>,
    pub last_message_date: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
    pub unread_count: usize,
    pub preview_messages: Vec<MessagePreview>,
    pub has_attachments: bool,
    pub is_starred: bool,
    pub folder: String,
}

#[derive(Debug, Serialize)]
pub struct MessagePreview {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub preview: String,
    pub date: chrono::DateTime<chrono::Utc>,
    pub is_read: bool,
}

/// Get Goose Chat conversations (separate tab)
pub async fn get_goose_chat_conversations(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Fetch Goose emails (from goose@squareup.com)
    let goose_emails = sqlx::query_as::<_, crate::models::Email>(
        "SELECT * FROM emails WHERE user_id = ? 
         AND (from_address LIKE '%goose@squareup.com%' OR to_addresses LIKE '%goose@squareup.com%')
         ORDER BY date DESC LIMIT 50"
    )
    .bind(user.user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch Goose emails")
    })?;
    
    // Group into conversations
    let conversation_service = ConversationService::new(pool.get_ref().clone());
    let conversations = conversation_service
        .group_emails_into_conversations(goose_emails)
        .await
        .map_err(|e| {
            log::error!("Conversation grouping error: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to group conversations")
        })?;
    
    // Convert to response format
    let response: Vec<ConversationResponse> = conversations
        .into_iter()
        .map(|conv| {
            let preview_messages: Vec<MessagePreview> = conv.preview_messages
                .into_iter()
                .map(|msg| MessagePreview {
                    id: msg.message_id,
                    from: msg.from_address,
                    subject: msg.subject,
                    preview: {
                        let text = if let Some(ref body_text) = msg.body_text {
                            body_text.clone()
                        } else if let Some(ref body_html) = msg.body_html {
                            sanitize_for_display(body_html)
                        } else {
                            String::new()
                        };
                        
                        text.chars()
                        .take(200)
                        .collect()
                    },
                    date: msg.date,
                    is_read: msg.is_read,
                })
                .collect();
            
            ConversationResponse {
                id: conv.id,
                preview: preview_messages.first()
                    .map(|m| m.preview.clone())
                    .unwrap_or_else(|| "No preview available".to_string()),
                conversation_type: "goose_chat".to_string(),
                icon: "🪿".to_string(),
                subject: conv.subject,
                participants: conv.participants,
                last_message_date: conv.last_message_date,
                message_count: conv.message_count,
                unread_count: conv.unread_count,
                preview_messages,
                has_attachments: conv.has_attachments,
                is_starred: conv.is_starred,
                folder: "GOOSE_CHAT".to_string(),
            }
        })
        .collect();
    
    Ok(HttpResponse::Ok().json(response))
}
