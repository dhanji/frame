use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::middleware::auth::AuthenticatedUser;
use chrono::{DateTime, Utc};
use crate::services::agent::AgentEngine;
use std::sync::Arc;
use crate::services::agent::tools::create_tool_registry;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAutomationRequest {
    pub name: String,
    pub description: Option<String>,
    pub schedule: String, // Cron expression
    pub prompt: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAutomationRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub schedule: Option<String>,
    pub prompt: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Automation {
    pub id: String,
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub schedule: String,
    pub prompt: String,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AutomationRun {
    pub id: String,
    pub automation_id: String,
    pub status: String, // "success", "failed", "running"
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub async fn create_automation(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    body: web::Json<CreateAutomationRequest>,
) -> HttpResponse {
    let user_id = user.user_id;

    let automation_id = uuid::Uuid::new_v4().to_string();
    let enabled = body.enabled.unwrap_or(true);

    let result = sqlx::query(
        r#"
        INSERT INTO automations (id, user_id, name, description, schedule, prompt, enabled, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(&automation_id)
    .bind(user_id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(&body.schedule)
    .bind(&body.prompt)
    .bind(enabled)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({
                "id": automation_id,
                "name": body.name,
                "enabled": enabled,
            }))
        }
        Err(e) => {
            log::error!("Failed to create automation: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to create automation"}))
        }
    }
}

pub async fn list_automations(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> HttpResponse {
    let user_id = user.user_id;

    let result = sqlx::query_as::<_, (String, String, Option<String>, String, String, bool, Option<String>, Option<String>, String, String)>(
        r#"
        SELECT id, name, description, schedule, prompt, enabled, last_run, next_run, created_at, updated_at
        FROM automations
        WHERE user_id = ?
        ORDER BY created_at DESC
        "#
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await;

    match result {
        Ok(rows) => {
            let automations: Vec<serde_json::Value> = rows.iter().map(|(id, name, desc, schedule, prompt, enabled, last_run, next_run, created_at, updated_at)| {
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "description": desc,
                    "schedule": schedule,
                    "prompt": prompt,
                    "enabled": enabled,
                    "last_run": last_run,
                    "next_run": next_run,
                    "created_at": created_at,
                    "updated_at": updated_at,
                })
            }).collect();

            HttpResponse::Ok().json(automations)
        }
        Err(e) => {
            log::error!("Failed to list automations: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to list automations"}))
        }
    }
}

pub async fn get_automation(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> HttpResponse {
    let user_id = user.user_id;

    let automation_id = path.into_inner();

    let result = sqlx::query_as::<_, (String, String, Option<String>, String, String, bool, Option<String>, Option<String>, String, String)>(
        r#"
        SELECT id, name, description, schedule, prompt, enabled, last_run, next_run, created_at, updated_at
        FROM automations
        WHERE id = ? AND user_id = ?
        "#
    )
    .bind(&automation_id)
    .bind(user_id)
    .fetch_optional(pool.get_ref())
    .await;

    match result {
        Ok(Some((id, name, desc, schedule, prompt, enabled, last_run, next_run, created_at, updated_at))) => {
            HttpResponse::Ok().json(serde_json::json!({
                "id": id,
                "name": name,
                "description": desc,
                "schedule": schedule,
                "prompt": prompt,
                "enabled": enabled,
                "last_run": last_run,
                "next_run": next_run,
                "created_at": created_at,
                "updated_at": updated_at,
            }))
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({"error": "Automation not found"})),
        Err(e) => {
            log::error!("Failed to get automation: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to get automation"}))
        }
    }
}

pub async fn update_automation(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<UpdateAutomationRequest>,
) -> HttpResponse {
    let user_id = user.user_id;

    let automation_id = path.into_inner();

    // Build dynamic update query
    let mut updates = Vec::new();
    let mut query = "UPDATE automations SET ".to_string();

    if body.name.is_some() {
        updates.push("name = ?");
    }
    if body.description.is_some() {
        updates.push("description = ?");
    }
    if body.schedule.is_some() {
        updates.push("schedule = ?");
    }
    if body.prompt.is_some() {
        updates.push("prompt = ?");
    }
    if body.enabled.is_some() {
        updates.push("enabled = ?");
    }

    if updates.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "No fields to update"}));
    }

    updates.push("updated_at = CURRENT_TIMESTAMP");
    query.push_str(&updates.join(", "));
    query.push_str(" WHERE id = ? AND user_id = ?");

    let mut q = sqlx::query(&query);

    if let Some(ref name) = body.name {
        q = q.bind(name);
    }
    if let Some(ref description) = body.description {
        q = q.bind(description);
    }
    if let Some(ref schedule) = body.schedule {
        q = q.bind(schedule);
    }
    if let Some(ref prompt) = body.prompt {
        q = q.bind(prompt);
    }
    if let Some(enabled) = body.enabled {
        q = q.bind(enabled);
    }

    q = q.bind(&automation_id).bind(user_id);

    let result = q.execute(pool.get_ref()).await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            HttpResponse::Ok().json(serde_json::json!({"success": true}))
        }
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({"error": "Automation not found"})),
        Err(e) => {
            log::error!("Failed to update automation: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to update automation"}))
        }
    }
}

pub async fn delete_automation(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> HttpResponse {
    let user_id = user.user_id;

    let automation_id = path.into_inner();

    let result = sqlx::query(
        "DELETE FROM automations WHERE id = ? AND user_id = ?"
    )
    .bind(&automation_id)
    .bind(user_id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            HttpResponse::Ok().json(serde_json::json!({"success": true}))
        }
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({"error": "Automation not found"})),
        Err(e) => {
            log::error!("Failed to delete automation: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to delete automation"}))
        }
    }
}

pub async fn trigger_automation(
    pool: web::Data<SqlitePool>,
    agent_engine: web::Data<Arc<AgentEngine>>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> HttpResponse {
    let user_id = user.user_id;

    let automation_id = path.into_inner();

    // Verify ownership
    let automation = sqlx::query_as::<_, (String, String)>(
        "SELECT id, prompt FROM automations WHERE id = ? AND user_id = ?"
    )
    .bind(&automation_id)
    .bind(user_id)
    .fetch_optional(pool.get_ref())
    .await;

    match automation {
        Ok(Some((id, prompt))) => {
            let run_id = uuid::Uuid::new_v4().to_string();
            
            // Create automation run record
            let _ = sqlx::query(
                r#"
                INSERT INTO automation_runs (id, automation_id, status, started_at)
                VALUES (?, ?, 'running', CURRENT_TIMESTAMP)
                "#
            )
            .bind(&run_id)
            .bind(&id)
            .execute(pool.get_ref())
            .await;

            // Execute automation with AI agent
            let result_text = agent_engine
                .process_message(prompt.clone(), vec![])
                .await
                .unwrap_or_else(|e| format!("Error: {}", e));

            // Update automation run with result
            let _ = sqlx::query(
                r#"
                UPDATE automation_runs 
                SET status = 'success', result = ?, completed_at = CURRENT_TIMESTAMP
                WHERE id = ?
                "#
            )
            .bind(&result_text)
            .bind(&run_id)
            .execute(pool.get_ref())
            .await;

            // Create a chat conversation entry for the automation result
            let conv_id = uuid::Uuid::new_v4().to_string();
            let automation_name = sqlx::query_scalar::<_, String>(
                "SELECT name FROM automations WHERE id = ?"
            )
            .bind(&id)
            .fetch_one(pool.get_ref())
            .await
            .unwrap_or_else(|_| "Automation".to_string());

            let title = format!("🤖 {} - Result", automation_name);
            
            // Create conversation
            let _ = sqlx::query(
                r#"INSERT INTO chat_conversations (id, user_id, title, created_at, updated_at)
                   VALUES (?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#
            )
            .bind(&conv_id)
            .bind(user_id)
            .bind(&title)
            .execute(pool.get_ref())
            .await;

            // Add result as assistant message
            let msg_id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                r#"INSERT INTO chat_messages (id, conversation_id, role, content, created_at)
                   VALUES (?, ?, 'assistant', ?, CURRENT_TIMESTAMP)"#
            )
            .bind(&msg_id)
            .bind(&conv_id)
            .bind(&result_text)
            .execute(pool.get_ref())
            .await;

            // Update last_run timestamp
            let _ = sqlx::query(
                "UPDATE automations SET last_run = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&id)
            .execute(pool.get_ref())
            .await;

            HttpResponse::Ok().json(serde_json::json!({
                "run_id": run_id,
                "status": "success",
            }))
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({"error": "Automation not found"})),
        Err(e) => {
            log::error!("Failed to trigger automation: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to trigger automation"}))
        }
    }
}

pub async fn get_runs(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> HttpResponse {
    let user_id = user.user_id;

    let automation_id = path.into_inner();

    // Verify ownership
    let owner_check = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM automations WHERE id = ?"
    )
    .bind(&automation_id)
    .fetch_optional(pool.get_ref())
    .await;

    match owner_check {
        Ok(Some(owner_id)) if owner_id == user_id => {
            let runs_result = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, Option<String>)>(
                r#"
                SELECT id, status, result, error, started_at, completed_at
                FROM automation_runs
                WHERE automation_id = ?
                ORDER BY started_at DESC
                LIMIT 50
                "#
            )
            .bind(&automation_id)
            .fetch_all(pool.get_ref())
            .await;

            match runs_result {
                Ok(rows) => {
                    let runs: Vec<serde_json::Value> = rows.iter().map(|(id, status, result, error, started_at, completed_at)| {
                        serde_json::json!({
                            "id": id,
                            "status": status,
                            "result": result,
                            "error": error,
                            "started_at": started_at,
                            "completed_at": completed_at,
                        })
                    }).collect();

                    HttpResponse::Ok().json(runs)
                }
                Err(e) => {
                    log::error!("Failed to fetch automation runs: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to fetch runs"}))
                }
            }
        }
        Ok(Some(_)) => HttpResponse::Forbidden().json(serde_json::json!({"error": "Access denied"})),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({"error": "Automation not found"})),
        Err(e) => {
            log::error!("Failed to check automation ownership: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}
