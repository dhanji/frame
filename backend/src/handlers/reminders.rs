use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::middleware::auth::AuthenticatedUser;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateReminderRequest {
    pub title: String,
    pub notes: Option<String>,
    pub due_date: Option<String>,
    pub email_conversation_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateReminderRequest {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub due_date: Option<String>,
    pub completed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub user_id: i64,
    pub title: String,
    pub notes: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub completed: bool,
    pub completed_at: Option<DateTime<Utc>>,
    pub email_conversation_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create_reminder(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    body: web::Json<CreateReminderRequest>,
) -> HttpResponse {
    let user_id = user.user_id;

    let reminder_id = uuid::Uuid::new_v4().to_string();

    let result = sqlx::query(
        r#"
        INSERT INTO reminders (id, user_id, title, notes, due_date, completed, email_conversation_id, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 0, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(&reminder_id)
    .bind(user_id)
    .bind(&body.title)
    .bind(&body.notes)
    .bind(&body.due_date)
    .bind(&body.email_conversation_id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({
                "id": reminder_id,
                "title": body.title,
                "completed": false,
            }))
        }
        Err(e) => {
            log::error!("Failed to create reminder: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to create reminder"}))
        }
    }
}

pub async fn list_reminders(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let user_id = user.user_id;

    let filter = query.get("filter").map(|s| s.as_str()).unwrap_or("active");

    let sql = match filter {
        "completed" => {
            r#"
            SELECT id, title, notes, due_date, completed, completed_at, email_conversation_id, created_at, updated_at
            FROM reminders
            WHERE user_id = ? AND completed = 1
            ORDER BY completed_at DESC
            "#
        }
        "all" => {
            r#"
            SELECT id, title, notes, due_date, completed, completed_at, email_conversation_id, created_at, updated_at
            FROM reminders
            WHERE user_id = ?
            ORDER BY due_date ASC, created_at DESC
            "#
        }
        _ => { // "active" or default
            r#"
            SELECT id, title, notes, due_date, completed, completed_at, email_conversation_id, created_at, updated_at
            FROM reminders
            WHERE user_id = ? AND completed = 0
            ORDER BY due_date ASC, created_at DESC
            "#
        }
    };

    let result = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, bool, Option<String>, Option<String>, String, String)>(
        sql
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await;

    match result {
        Ok(rows) => {
            let reminders: Vec<serde_json::Value> = rows.iter().map(|(id, title, notes, due_date, completed, completed_at, email_conv_id, created_at, updated_at)| {
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "notes": notes,
                    "due_date": due_date,
                    "completed": completed,
                    "completed_at": completed_at,
                    "email_conversation_id": email_conv_id,
                    "created_at": created_at,
                    "updated_at": updated_at,
                })
            }).collect();

            HttpResponse::Ok().json(reminders)
        }
        Err(e) => {
            log::error!("Failed to list reminders: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to list reminders"}))
        }
    }
}

pub async fn update_reminder(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<UpdateReminderRequest>,
) -> HttpResponse {
    let user_id = user.user_id;

    let reminder_id = path.into_inner();

    // Build dynamic update query
    let mut updates = Vec::new();
    let mut query = "UPDATE reminders SET ".to_string();

    if body.title.is_some() {
        updates.push("title = ?");
    }
    if body.notes.is_some() {
        updates.push("notes = ?");
    }
    if body.due_date.is_some() {
        updates.push("due_date = ?");
    }
    if body.completed.is_some() {
        updates.push("completed = ?");
        if body.completed == Some(true) {
            updates.push("completed_at = CURRENT_TIMESTAMP");
        } else {
            updates.push("completed_at = NULL");
        }
    }

    if updates.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "No fields to update"}));
    }

    updates.push("updated_at = CURRENT_TIMESTAMP");
    query.push_str(&updates.join(", "));
    query.push_str(" WHERE id = ? AND user_id = ?");

    let mut q = sqlx::query(&query);

    if let Some(ref title) = body.title {
        q = q.bind(title);
    }
    if let Some(ref notes) = body.notes {
        q = q.bind(notes);
    }
    if let Some(ref due_date) = body.due_date {
        q = q.bind(due_date);
    }
    if let Some(completed) = body.completed {
        q = q.bind(completed);
    }

    q = q.bind(&reminder_id).bind(user_id);

    let result = q.execute(pool.get_ref()).await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            HttpResponse::Ok().json(serde_json::json!({"success": true}))
        }
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({"error": "Reminder not found"})),
        Err(e) => {
            log::error!("Failed to update reminder: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to update reminder"}))
        }
    }
}

pub async fn delete_reminder(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> HttpResponse {
    let user_id = user.user_id;

    let reminder_id = path.into_inner();

    let result = sqlx::query(
        "DELETE FROM reminders WHERE id = ? AND user_id = ?"
    )
    .bind(&reminder_id)
    .bind(user_id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            HttpResponse::Ok().json(serde_json::json!({"success": true}))
        }
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({"error": "Reminder not found"})),
        Err(e) => {
            log::error!("Failed to delete reminder: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to delete reminder"}))
        }
    }
}

pub async fn toggle_complete(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> HttpResponse {
    let user_id = user.user_id;

    let reminder_id = path.into_inner();

    // Get current completion status
    let current = sqlx::query_as::<_, (bool,)>(
        "SELECT completed FROM reminders WHERE id = ? AND user_id = ?"
    )
    .bind(&reminder_id)
    .bind(user_id)
    .fetch_optional(pool.get_ref())
    .await;

    match current {
        Ok(Some((completed,))) => {
            let new_status = !completed;
            let result = if new_status {
                sqlx::query(
                    "UPDATE reminders SET completed = 1, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                )
                .bind(&reminder_id)
                .execute(pool.get_ref())
                .await
            } else {
                sqlx::query(
                    "UPDATE reminders SET completed = 0, completed_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                )
                .bind(&reminder_id)
                .execute(pool.get_ref())
                .await
            };

            match result {
                Ok(_) => {
                    HttpResponse::Ok().json(serde_json::json!({
                        "success": true,
                        "completed": new_status,
                    }))
                }
                Err(e) => {
                    log::error!("Failed to toggle reminder completion: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to update reminder"}))
                }
            }
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({"error": "Reminder not found"})),
        Err(e) => {
            log::error!("Failed to fetch reminder: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}
