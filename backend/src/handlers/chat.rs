use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::middleware::auth::AuthenticatedUser;
use chrono::{DateTime, Utc};
use crate::services::agent::AgentEngine;
use crate::services::agent::tools::create_tool_registry;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: serde_json::Value,
}

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
    log::info!("Creating new chat conversation for user {}", user.user_id);
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
    log::info!("Chat conversation creation result: {:?}", result.is_ok());

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

            log::info!("Successfully created chat conversation: {}", conversation_id);
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

    let result = sqlx::query_as::<_, (String, String, Option<String>, String, String, i32)>(
        r#"
        SELECT 
            c.id,
            c.title,
            c.automation_id,
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
            let conversations: Vec<serde_json::Value> = rows.iter().map(|(id, title, automation_id, created_at, updated_at, count)| {
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "automation_id": automation_id,
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

                    // Create per-user tool registry and agent engine
                    let tool_registry = Arc::new(create_tool_registry(pool.get_ref().clone(), user_id as i64));
                    
                    // Get user's AI settings
                    let user_settings = sqlx::query_as::<_, (String,)>(
                        "SELECT settings FROM users WHERE id = ?"
                    )
                    .bind(user_id)
                    .fetch_optional(pool.get_ref())
                    .await
                    .ok()
                    .flatten();
                    
                    // Create provider based on user settings or environment
                    let provider = create_user_provider(user_settings).await;
                    
                    // Create agent engine for this user
                    let agent_engine = AgentEngine::new(provider, tool_registry);

                    // Call AI agent
                    log::info!("Calling AI agent for user {} with {} messages in history", user.user_id, messages.len());
                    let (assistant_response, tool_calls) = match agent_engine
                        .process_message(body.content.clone(), messages)
                        .await {
                        Ok(response) => response,
                        Err(e) => {
                            let error_msg = e.to_string();
                            log::error!("Agent error for user {}: {}", user.user_id, error_msg);
                            
                            // Provide helpful error message
                            if error_msg.contains("No AI provider configured") || error_msg.contains("not configured") {
                                ("I'm sorry, but the AI assistant is not configured for your account yet. Please go to Settings and configure your AI provider (Anthropic, OpenAI, etc.) with a valid API key.".to_string(), vec![])
                            } else if error_msg.contains("API") || error_msg.contains("401") || error_msg.contains("authentication") || error_msg.contains("Unauthorized") {
                                ("I'm sorry, but there's an authentication issue with the AI service. Please check your API key in Settings and make sure it's valid. You can get an API key from:\n\n".to_string() + 
                                "- Anthropic: https://console.anthropic.com/\n" +
                                "- OpenAI: https://platform.openai.com/api-keys\n" +
                                "- Databricks: Your Databricks workspace", vec![])
                            } else if error_msg.contains("dummy-key") || error_msg.contains("test-key") {
                                ("I'm sorry, but I'm running in demo mode without a valid API key. To use the AI assistant, please set the ANTHROPIC_API_KEY environment variable and restart the server.".to_string(), vec![])
                            } else if error_msg.contains("rate limit") || error_msg.contains("quota") {
                                ("I'm sorry, but the AI service rate limit has been exceeded. Please try again in a few moments.".to_string(), vec![])
                            } else if error_msg.contains("timeout") {
                                ("I'm sorry, but the request timed out. The AI service might be experiencing high load. Please try again.".to_string(), vec![])
                            } else {
                                (format!("I encountered an error while processing your request: {}. Please check the server logs for more details.", error_msg), vec![])
                            }
                        }
                    };
                    
                    // Save assistant response
                    let assistant_message_id = uuid::Uuid::new_v4().to_string();
                    
                    // Serialize tool calls to JSON for storage
                    let tool_calls_json = serde_json::to_string(&tool_calls).unwrap_or_else(|_| "[]".to_string());
                    
                    let _ = sqlx::query(
                        r#"INSERT INTO chat_messages (id, conversation_id, role, content, tool_calls, created_at)
                           VALUES (?, ?, 'assistant', ?, ?, CURRENT_TIMESTAMP)"#
                    )
                    .bind(&assistant_message_id)
                    .bind(&conversation_id)
                    .bind(&assistant_response)
                    .bind(&tool_calls_json)
                    .execute(pool.get_ref())
                    .await;

                    HttpResponse::Ok().json(serde_json::json!({
                        "user_message_id": message_id,
                        "assistant_message_id": assistant_message_id,
                        "assistant_response": assistant_response,
                        "tool_calls": tool_calls,
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
    agent_engine: web::Data<Arc<AgentEngine>>,
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
                        .unwrap_or_else(|e| (format!("Error: {}", e), vec![]));
                    
                    HttpResponse::Ok()
                        .content_type("text/event-stream")
                        .body(format!("data: {}\n\n", serde_json::json!({"content": assistant_response})))
                }
                Err(_e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to send message"}))
            }
        }
        _ => HttpResponse::Forbidden().json(serde_json::json!({"error": "Access denied"}))
    }
}

async fn create_user_provider(user_settings: Option<(String,)>) -> Box<dyn crate::services::agent::provider::LLMProvider> {
    use crate::services::agent::provider::{create_provider, ProviderConfig};
    
    // Parse user settings if available
    if let Some((settings_json,)) = user_settings {
        if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&settings_json) {
            let ai_provider = settings["ai_provider"].as_str().unwrap_or("anthropic");
            let ai_api_key = settings["ai_api_key"].as_str();
            let ai_model = settings["ai_model"].as_str();
            
            // If user has configured their own API key, use it
            if let Some(api_key) = ai_api_key {
                if !api_key.is_empty() && api_key != "null" {
                    let model = ai_model.unwrap_or("claude-3-5-sonnet-20241022").to_string();
                    
                    return match ai_provider {
                        "anthropic" => create_provider(ProviderConfig::Anthropic {
                            api_key: api_key.to_string(),
                            model,
                        }),
                        "openai" => create_provider(ProviderConfig::OpenAI {
                            api_key: api_key.to_string(),
                            model: ai_model.unwrap_or("gpt-4").to_string(),
                        }),
                        _ => create_provider(ProviderConfig::Anthropic {
                            api_key: api_key.to_string(),
                            model,
                        }),
                    };
                }
            }
        }
    }
    
    // Fall back to environment variable
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| {
        log::warn!("No AI provider configured for user and ANTHROPIC_API_KEY not found in environment");
        "dummy-key".to_string()
    });
    
    create_provider(ProviderConfig::Anthropic {
        api_key,
        model: std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string()),
    })
}
