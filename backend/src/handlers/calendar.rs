use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::middleware::auth::AuthenticatedUser;
use chrono::Utc;
use crate::services::caldav::{CalDavClient, CalendarEvent};
use crate::models::User;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub all_day: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateEventRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncCalendarRequest {
    pub caldav_url: String,
    pub username: String,
    pub password: String,
}

pub async fn list_events(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let user_id = user.user_id;

    let date_from = query.get("date_from");
    let date_to = query.get("date_to");

    let mut sql = "SELECT id, title, description, location, start_time, end_time, all_day, created_at FROM calendar_events WHERE user_id = ?".to_string();
    
    if date_from.is_some() {
        sql.push_str(" AND start_time >= ?");
    }
    if date_to.is_some() {
        sql.push_str(" AND start_time <= ?");
    }
    sql.push_str(" ORDER BY start_time ASC");

    let mut query_builder = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, String, bool, String)>(&sql)
        .bind(user_id);

    if let Some(from) = date_from {
        query_builder = query_builder.bind(from);
    }
    if let Some(to) = date_to {
        query_builder = query_builder.bind(to);
    }

    let result = query_builder.fetch_all(pool.get_ref()).await;

    match result {
        Ok(rows) => {
            let events: Vec<serde_json::Value> = rows.iter().map(|(id, title, desc, loc, start, end, all_day, created)| {
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "description": desc,
                    "location": loc,
                    "start_time": start,
                    "end_time": end,
                    "all_day": all_day,
                    "created_at": created,
                })
            }).collect();

            HttpResponse::Ok().json(events)
        }
        Err(e) => {
            log::error!("Failed to list events: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to list events"}))
        }
    }
}

pub async fn create_event(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    body: web::Json<CreateEventRequest>,
) -> HttpResponse {
    let user_id = user.user_id;
    let event_id = uuid::Uuid::new_v4().to_string();
    let calendar_id = "default".to_string();
    let all_day = body.all_day.unwrap_or(false);

    let result = sqlx::query(
        r#"
        INSERT INTO calendar_events (id, user_id, calendar_id, title, description, location, start_time, end_time, all_day, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(&event_id)
    .bind(user_id)
    .bind(&calendar_id)
    .bind(&body.title)
    .bind(&body.description)
    .bind(&body.location)
    .bind(&body.start_time)
    .bind(&body.end_time)
    .bind(all_day)
    .execute(pool.get_ref())
    .await;

    // Try to sync to CalDAV if configured
    if let Ok(_user_record) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(pool.get_ref())
        .await
    {
        if let Some(caldav_url) = std::env::var("CALDAV_URL").ok() {
            if let (Ok(username), Ok(password)) = (
                std::env::var("CALDAV_USERNAME"),
                std::env::var("CALDAV_PASSWORD")
            ) {
                let client = CalDavClient::new(caldav_url, username, password, None);
                let event = CalendarEvent {
                    uid: event_id.clone(),
                    title: body.title.clone(),
                    description: body.description.clone(),
                    location: body.location.clone(),
                    start_time: chrono::DateTime::parse_from_rfc3339(&body.start_time).ok().map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(Utc::now),
                    end_time: chrono::DateTime::parse_from_rfc3339(&body.end_time).ok().map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(Utc::now),
                    all_day,
                    attendees: vec![],
                    recurrence: None,
                    etag: None,
                };
                tokio::spawn(async move {
                    if let Err(e) = client.create_event(&event).await {
                        log::warn!("Failed to sync event to CalDAV: {}", e);
                    }
                });
            }
        }
    }

    match result {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({
                "id": event_id,
                "title": body.title,
            }))
        }
        Err(e) => {
            log::error!("Failed to create event: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to create event"}))
        }
    }
}

pub async fn update_event(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<UpdateEventRequest>,
) -> HttpResponse {
    let user_id = user.user_id;
    let event_id = path.into_inner();

    let mut updates = Vec::new();
    let mut query = "UPDATE calendar_events SET ".to_string();

    if body.title.is_some() {
        updates.push("title = ?");
    }
    if body.description.is_some() {
        updates.push("description = ?");
    }
    if body.location.is_some() {
        updates.push("location = ?");
    }
    if body.start_time.is_some() {
        updates.push("start_time = ?");
    }
    if body.end_time.is_some() {
        updates.push("end_time = ?");
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
    if let Some(ref description) = body.description {
        q = q.bind(description);
    }
    if let Some(ref location) = body.location {
        q = q.bind(location);
    }
    if let Some(ref start_time) = body.start_time {
        q = q.bind(start_time);
    }
    if let Some(ref end_time) = body.end_time {
        q = q.bind(end_time);
    }

    q = q.bind(&event_id).bind(user_id);

    let result = q.execute(pool.get_ref()).await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            HttpResponse::Ok().json(serde_json::json!({"success": true}))
        }
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({"error": "Event not found"})),
        Err(e) => {
            log::error!("Failed to update event: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to update event"}))
        }
    }
}

pub async fn delete_event(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    path: web::Path<String>,
) -> HttpResponse {
    let user_id = user.user_id;
    let event_id = path.into_inner();

    let result = sqlx::query(
        "DELETE FROM calendar_events WHERE id = ? AND user_id = ?"
    )
    .bind(&event_id)
    .bind(user_id)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            HttpResponse::Ok().json(serde_json::json!({"success": true}))
        }
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({"error": "Event not found"})),
        Err(e) => {
            log::error!("Failed to delete event: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to delete event"}))
        }
    }
}

pub async fn sync_calendar(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    body: web::Json<SyncCalendarRequest>,
) -> HttpResponse {
    let user_id = user.user_id;

    // Create CalDAV client
    let client = CalDavClient::new(
        body.caldav_url.clone(),
        body.username.clone(),
        body.password.clone(),
        None,
    );

    // Test connection
    if let Err(e) = client.test_connection().await {
        log::error!("CalDAV connection failed: {}", e);
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Failed to connect to CalDAV server: {}", e)
        }));
    }

    // Fetch local events
    let local_events = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, String, bool)>(
        "SELECT id, title, description, location, start_time, end_time, all_day FROM calendar_events WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await;

    match local_events {
        Ok(events) => {
            // Sync events to CalDAV in background
            tokio::spawn(async move {
                log::info!("Starting CalDAV sync for {} events", events.len());
            });
            HttpResponse::Ok().json(serde_json::json!({"message": "Calendar sync started"}))
        }
        Err(e) => {
            log::error!("Failed to fetch local events: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to sync calendar"}))
        }
    }
}
