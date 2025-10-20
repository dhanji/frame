use actix_web::{web, HttpResponse, HttpRequest};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::middleware::auth::AuthenticatedUser;
use chrono::{DateTime, Utc};
use crate::services::agent::AgentEngine;
use futures::stream::StreamExt;
use actix_web::rt::time::interval;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateConversationRequest {
    pub title: Option<String>,
    pub initial_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatConversation {
    pub id: String,
    pub user_id: i64,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: i32,
    pub last_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub tool_calls: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn create_conversation(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    body: web::Json<CreateConversationRequest>,
) -> HttpResponse {
    let user_id = user.user_id;

    let conversation_id = uuid::Uuid::new_v4().to_string();
    let title = body.title.clone().unwrap_or_else(|| "New Chat".to_string());

    let result = sqlx::query(
        r#"
        INSERT INTO chat_conversations (id, user_id, title, created_at, updated_at)
        VALUES (?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(&conversation_id)
    .bind(user_id)
    .bind(&title)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            // If there's an initial message, add it
            if let Some(initial_msg) = &body.initial_message {
                let message_id = uuid::Uuid::new_v4().to_string();
                let _ = sqlx::query(
                    r#"
                    INSERT INTO chat_messages (id, conversation_id, role, content, created_at)
                    VALUES (?, ?, 'user', ?, CURRENT_TIMESTAMP)
                    "#
                )
                .bind(&message_id)
                .bind(&conversation_id)
                .bind(initial_msg)
                .execute(pool.get_ref())
                .await;
            }

            HttpResponse::Ok().json(serde_json::json!({
                "id": conversation_id,
                "title": title,
                "created_at": Utc::now(),
            }))
        }
        Err(e) => {
            log::error!("Failed to create chat conversation: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to create conversation"}))
        }
    }
}

pub async fn list_conversations(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> HttpResponse {
    let user_id = user.user_id;

    let result = sqlx::query_as::<_, (String, String, String, String, i32)>(
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
        "#
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await;

    match result {
        Ok(rows) => {
            let conversations: Vec<serde_json::Value> = rows.iter().map(|(id, title, created_at, updated_at, count)| {
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "created_at": created_at,
                    "updated_at": updated_at,
                    "message_count": count,
                })
            }).collect();

            HttpResponse::Ok().json(conversations)
        }
        Err(e) => {
            log::error!("Failed to list chat conversations: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to list conversations"}))
        }
    }
}

pub async fn get_conversation(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> HttpResponse {
    let user_id = user.user_id;

    let conversation_id = path.into_inner();

    // Verify ownership
    let owner_check = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM chat_conversations WHERE id = ?"
    )
    .bind(&conversation_id)
    .fetch_optional(pool.get_ref())
    .await;

    match owner_check {
        Ok(Some(owner_id)) if owner_id == user_id => {
            // Fetch messages
            let messages = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
                r#"
                SELECT id, conversation_id, role, content, created_at
                FROM chat_messages
                WHERE conversation_id = ?
                ORDER BY created_at ASC
                "#
            )
            .bind(&conversation_id)
            .fetch_all(pool.get_ref())
            .await;

            match messages {
                Ok(rows) => {
                    let messages: Vec<serde_json::Value> = rows.iter().map(|(id, conv_id, role, content, created_at)| {
                        serde_json::json!({
                            "id": id,
                            "conversation_id": conv_id,
                            "role": role,
                            "content": content,
                            "created_at": created_at,
                        })
                    }).collect();

                    HttpResponse::Ok().json(messages)
                }
                Err(e) => {
                    log::error!("Failed to fetch messages: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to fetch messages"}))
                }
            }
        }
        Ok(Some(_)) => HttpResponse::Forbidden().json(serde_json::json!({"error": "Access denied"})),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({"error": "Conversation not found"})),
        Err(e) => {
            log::error!("Failed to check conversation ownership: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn send_message(
    pool: web::Data<SqlitePool>,
    agent_engine: web::Data<AgentEngine>,
    user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<SendMessageRequest>,
) -> HttpResponse {
    let user_id = user.user_id;

    let conversation_id = path.into_inner();

    // Verify ownership
    let owner_check = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM chat_conversations WHERE id = ?"
    )
    .bind(&conversation_id)
    .fetch_optional(pool.get_ref())
    .await;

    match owner_check {
        Ok(Some(owner_id)) if owner_id == user_id => {
            let message_id = uuid::Uuid::new_v4().to_string();
            
            // Insert user message
            let result = sqlx::query(
                r#"
                INSERT INTO chat_messages (id, conversation_id, role, content, created_at)
                VALUES (?, ?, 'user', ?, CURRENT_TIMESTAMP)
                "#
            )
            .bind(&message_id)
            .bind(&conversation_id)
            .bind(&body.content)
            .execute(pool.get_ref())
            .await;

            match result {
                Ok(_) => {
                    // Update conversation timestamp
                    let _ = sqlx::query(
                        "UPDATE chat_conversations SET updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                    )
                    .bind(&conversation_id)
                    .execute(pool.get_ref())
                    .await;

                    // Get conversation history
                    let history = sqlx::query_as::<_, (String, String)>(
                        r#"
                        SELECT role, content
                        FROM chat_messages
                        WHERE conversation_id = ?
                        ORDER BY created_at ASC
                        "#
                    )
                    .bind(&conversation_id)
                    .fetch_all(pool.get_ref())
                    .await
                    .unwrap_or_default();

                    let messages: Vec<crate::services::agent::provider::Message> = history
                        .iter()
                        .map(|(role, content)| crate::services::agent::provider::Message {
                            role: role.clone(),
                            content: content.clone(),
                        })
                        .collect();

                    // Call AI agent
                    let assistant_response = agent_engine
                        .process_message(body.content.clone(), messages)
                        .await
                        .unwrap_or_else(|e| format!("Error: {}", e));
                    
                    // Save assistant response
                    let assistant_message_id = uuid::Uuid::new_v4().to_string();
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO chat_messages (id, conversation_id, role, content, created_at)
                        VALUES (?, ?, 'assistant', ?, CURRENT_TIMESTAMP)
                        "#
                    )
                    .bind(&assistant_message_id)
                    .bind(&conversation_id)
                    .bind(&assistant_response)
                    .execute(pool.get_ref())
                    .await;

                    HttpResponse::Ok().json(serde_json::json!({
                        "user_message_id": message_id,
                        "assistant_message_id": assistant_message_id,
                        "assistant_response": assistant_response,
                    }))
                }
                Err(e) => {
                    log::error!("Failed to send message: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to send message"}))
                }
            }
        }
        Ok(Some(_)) => HttpResponse::Forbidden().json(serde_json::json!({"error": "Access denied"})),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({"error": "Conversation not found"})),
        Err(e) => {
            log::error!("Failed to check conversation ownership: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn delete_conversation(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> HttpResponse {
    let user_id = user.user_id;

    let conversation_id = path.into_inner();

    // Verify ownership and delete
    let result = sqlx::query(
        "DELETE FROM chat_conversations WHERE id = ? AND user_id = ?"
    )
    .bind(&conversation_id)
    .bind(user_id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            HttpResponse::Ok().json(serde_json::json!({"success": true}))
        }
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({"error": "Conversation not found"})),
        Err(e) => {
            log::error!("Failed to delete conversation: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to delete conversation"}))
        }
    }
}

pub async fn send_message_stream(
    pool: web::Data<SqlitePool>,
    agent_engine: web::Data<AgentEngine>,
    user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<SendMessageRequest>,
) -> HttpResponse {
    let user_id = user.user_id;
    let conversation_id = path.into_inner();

    // Verify ownership
    let owner_check = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM chat_conversations WHERE id = ?"
    )
    .bind(&conversation_id)
    .fetch_optional(pool.get_ref())
    .await;

    match owner_check {
        Ok(Some(owner_id)) if owner_id == user_id => {
            let message_id = uuid::Uuid::new_v4().to_string();
            
            // Insert user message
            let result = sqlx::query(
                r#"
                INSERT INTO chat_messages (id, conversation_id, role, content, created_at)
                VALUES (?, ?, 'user', ?, CURRENT_TIMESTAMP)
                "#
            )
            .bind(&message_id)
            .bind(&conversation_id)
            .bind(&body.content)
            .execute(pool.get_ref())
            .await;

            match result {
                Ok(_) => {
                    // Update conversation timestamp
                    let _ = sqlx::query(
                        "UPDATE chat_conversations SET updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                    )
                    .bind(&conversation_id)
                    .execute(pool.get_ref())
                    .await;

                    // Get conversation history
                    let history = sqlx::query_as::<_, (String, String)>(
                        r#"
                        SELECT role, content
                        FROM chat_messages
                        WHERE conversation_id = ?
                        ORDER BY created_at ASC
                        "#
                    )
                    .bind(&conversation_id)
                    .fetch_all(pool.get_ref())
                    .await
                    .unwrap_or_default();

                    let messages: Vec<crate::services::agent::provider::Message> = history
                        .iter()
                        .map(|(role, content)| crate::services::agent::provider::Message {
                            role: role.clone(),
                            content: content.clone(),
                        })
                        .collect();

                    // For now, return a simple streaming response
                    // Full SSE implementation would require more complex streaming
                    let assistant_response = agent_engine
                        .process_message(body.content.clone(), messages)
                        .await
                        .unwrap_or_else(|e| format!("Error: {}", e));
                    
                    HttpResponse::Ok()
                        .content_type("text/event-stream")
                        .body(format!("data: {}\n\n", serde_json::json!({"content": assistant_response})))
                }
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to send message"}))
            }
        }
        _ => HttpResponse::Forbidden().json(serde_json::json!({"error": "Access denied"}))
    }
}
