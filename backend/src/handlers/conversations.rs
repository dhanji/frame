use actix_web::{web, HttpResponse};
use crate::utils::sanitize::sanitize_for_display;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::middleware::auth::AuthenticatedUser;
use crate::services::ConversationService;

#[derive(Debug, Deserialize)]
pub struct GetConversationsQuery {
    pub folder: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub unread_only: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ConversationResponse {
    pub id: String,
    pub preview: String,  // Add preview field for frontend
    pub conversation_type: String,  // "email", "chat", or "automation"
    pub icon: String,  // Icon identifier for frontend
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

pub async fn get_conversations(
    pool: web::Data<SqlitePool>,
    query: web::Query<GetConversationsQuery>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let folder = query.folder.as_deref().unwrap_or("INBOX");
    
    // Special handling for Goose Chat tab
    if folder == "GOOSE_CHAT" {
        return crate::handlers::conversations_goose::get_goose_chat_conversations(pool, user).await;
    }
    
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);
    
    let mut all_conversations = Vec::new();
    
    // Fetch emails from database
    let mut sql_query = String::from(
        // Exclude emails from goose@squareup.com (Goose Chat)
        "SELECT * FROM emails WHERE user_id = ? AND folder = ? 
         AND from_address NOT LIKE '%goose@squareup.com%'
         AND from_address NOT LIKE '%Goose%<%goose@squareup.com%>%'"
    );
    
    if query.unread_only.unwrap_or(false) {
        sql_query.push_str(" AND is_read = false");
    }
    
    sql_query.push_str(" ORDER BY date DESC LIMIT ? OFFSET ?");
    
    let emails = sqlx::query_as::<_, crate::models::Email>(&sql_query)
        .bind(user.user_id)
        .bind(folder)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to fetch emails")
        })?;
    
    // Group emails into conversations
    let conversation_service = ConversationService::new(pool.get_ref().clone());
    let conversations = conversation_service
        .group_emails_into_conversations(emails)
        .await
        .map_err(|e| {
            log::error!("Conversation grouping error: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to group conversations")
        })?;
    
    // Convert to response format with sanitized HTML
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
                conversation_type: "email".to_string(),
                icon: "📧".to_string(),
                subject: conv.subject,
                participants: conv.participants,
                last_message_date: conv.last_message_date,
                message_count: conv.message_count,
                unread_count: conv.unread_count,
                preview_messages,
                has_attachments: conv.has_attachments,
                is_starred: conv.is_starred,
                folder: conv.folder,
            }
        })
        .collect();
    
    all_conversations.extend(response);
    
    // Fetch chat conversations but don't include them in INBOX
    if folder == "INBOX" {
        // Fetch chat conversations
        let _chat_convs = sqlx::query_as::<_, (String, String, String, String, i32)>(
            r#"
            SELECT 
                c.id,
                c.title,
                c.created_at,
                c.updated_at,
                COUNT(m.id) as message_count
            FROM chat_conversations c
            LEFT JOIN chat_messages m ON c.id = m.conversation_id
            WHERE c.user_id = ?
            GROUP BY c.id
            ORDER BY c.updated_at DESC
            LIMIT 50
            "#
        )
        .bind(user.user_id)
        .fetch_all(pool.get_ref())
        .await
        .unwrap_or_default();
        
    }
    
    Ok(HttpResponse::Ok().json(all_conversations))
}

pub async fn get_conversation(
    pool: web::Data<SqlitePool>,
    path: web::Path<String>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let conversation_id = path.into_inner();
    
    let conversation_service = ConversationService::new(pool.get_ref().clone());
    let conversation = conversation_service
        .get_conversation_by_id(&conversation_id, user.user_id)
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to fetch conversation")
        })?;
    
    match conversation {
        Some(conv) => {
            // Sanitize HTML content in messages
            let mut sanitized_conv = conv;
            for msg in &mut sanitized_conv.messages {
                if let Some(html) = &msg.body_html {
                    msg.body_html = Some(sanitize_for_display(html));
                }
            }
            
            // Mark conversation as read after 2 seconds
            let _pool_clone = pool.get_ref().clone();
            let conv_id = conversation_id.clone();
            let user_id = user.user_id;
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                let _ = conversation_service
                    .mark_conversation_as_read(&conv_id, user_id)
                    .await;
            });
            
            Ok(HttpResponse::Ok().json(sanitized_conv))
        }
        None => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Conversation not found"
        }))),
    }
}

#[derive(Debug, Deserialize)]
pub struct BulkActionRequest {
    pub conversation_ids: Vec<String>,
    pub action: String,  // "mark_read", "mark_unread", "delete", "archive", "star", "unstar"
    pub folder: Option<String>,  // For move action
}

pub async fn bulk_conversation_action(
    pool: web::Data<SqlitePool>,
    body: web::Json<BulkActionRequest>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let conversation_service = ConversationService::new(pool.get_ref().clone());
    
    for conv_id in &body.conversation_ids {
        match body.action.as_str() {
            "mark_read" => {
                conversation_service
                    .mark_conversation_as_read(conv_id, user.user_id)
                    .await
                    .map_err(|e| {
                        log::error!("Failed to mark as read: {}", e);
                        actix_web::error::ErrorInternalServerError("Operation failed")
                    })?;
            }
            "mark_unread" => {
                sqlx::query(
                    "UPDATE emails SET is_read = false WHERE user_id = ? AND thread_id = ?"
                )
                .bind(user.user_id)
                .bind(conv_id)
                .execute(pool.get_ref())
                .await
                .map_err(|e| {
                    log::error!("Failed to mark as unread: {}", e);
                    actix_web::error::ErrorInternalServerError("Operation failed")
                })?;
            }
            "delete" => {
                conversation_service
                    .delete_conversation(conv_id, user.user_id)
                    .await
                    .map_err(|e| {
                        log::error!("Failed to delete: {}", e);
                        actix_web::error::ErrorInternalServerError("Operation failed")
                    })?;
            }
            "archive" => {
                conversation_service
                    .move_conversation_to_folder(conv_id, "Archive", user.user_id)
                    .await
                    .map_err(|e| {
                        log::error!("Failed to archive: {}", e);
                        actix_web::error::ErrorInternalServerError("Operation failed")
                    })?;
            }
            "star" => {
                conversation_service
                    .star_conversation(conv_id, true, user.user_id)
                    .await
                    .map_err(|e| {
                        log::error!("Failed to star: {}", e);
                        actix_web::error::ErrorInternalServerError("Operation failed")
                    })?;
            }
            "unstar" => {
                conversation_service
                    .star_conversation(conv_id, false, user.user_id)
                    .await
                    .map_err(|e| {
                        log::error!("Failed to unstar: {}", e);
                        actix_web::error::ErrorInternalServerError("Operation failed")
                    })?;
            }
            "move" => {
                if let Some(folder) = &body.folder {
                    conversation_service
                        .move_conversation_to_folder(conv_id, folder, user.user_id)
                        .await
                        .map_err(|e| {
                            log::error!("Failed to move: {}", e);
                            actix_web::error::ErrorInternalServerError("Operation failed")
                        })?;
                }
            }
            _ => {
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid action"
                })));
            }
        }
    }
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Bulk action completed successfully",
        "affected": body.conversation_ids.len()
    })))
}