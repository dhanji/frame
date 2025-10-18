use actix_web::{web, HttpResponse};
use sqlx::SqlitePool;
use sqlx::FromRow;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::middleware::auth::AuthenticatedUser;

#[derive(Debug, Deserialize, Serialize)]
pub struct FilterRule {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub conditions: FilterConditions,
    pub actions: FilterActions,
    pub is_active: bool,
    pub priority: i32,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FilterConditions {
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    pub body_contains: Option<String>,
    pub has_attachments: Option<bool>,
    pub size_greater_than: Option<i64>,
    pub size_less_than: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FilterActions {
    pub move_to_folder: Option<String>,
    pub mark_as_read: Option<bool>,
    pub mark_as_starred: Option<bool>,
    pub add_label: Option<String>,
    pub forward_to: Option<String>,
    pub delete: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFilterRequest {
    pub name: String,
    pub conditions: FilterConditions,
    pub actions: FilterActions,
    pub is_active: bool,
    pub priority: i32,
}

pub async fn create_filter(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    req: web::Json<CreateFilterRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let filter_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    
    let conditions_json = serde_json::to_string(&req.conditions)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    let actions_json = serde_json::to_string(&req.actions)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    sqlx::query(
        r#"
        INSERT INTO filters (
            id, user_id, name, conditions, actions,
            is_active, priority, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(&filter_id)
    .bind(&user.user_id)
    .bind(&req.name)
    .bind(&conditions_json)
    .bind(&actions_json)
    .bind(req.is_active)
    .bind(req.priority)
    .bind(now)
    .bind(now)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to create filter: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to create filter")
    })?;
    
    Ok(HttpResponse::Ok().json(json!({
        "message": "Filter created successfully",
        "filter_id": filter_id
    })))
}

pub async fn get_filters(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    #[derive(FromRow)]
    struct FilterRow {
        id: i64,
        name: String,
        conditions: String,
        actions: String,
        is_active: bool,
        priority: i32,
        created_at: chrono::DateTime<Utc>,
        updated_at: chrono::DateTime<Utc>,
    }
    
    let filters = sqlx::query_as::<_, FilterRow>(
        r#"
        SELECT id, name, conditions, actions, is_active, priority, created_at, updated_at
        FROM filters
        WHERE user_id = ?
        ORDER BY priority ASC, created_at DESC
        "#
    )
    .bind(user.user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to fetch filters: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch filters")
    })?;
    
    let filter_list: Vec<_> = filters.iter().map(|f| {
        json!({
            "id": f.id,
            "name": f.name,
            "conditions": serde_json::from_str::<FilterConditions>(&f.conditions).unwrap_or_default(),
            "actions": serde_json::from_str::<FilterActions>(&f.actions).unwrap_or_default(),
            "is_active": f.is_active,
            "priority": f.priority,
            "created_at": f.created_at,
            "updated_at": f.updated_at
        })
    }).collect();
    
    Ok(HttpResponse::Ok().json(filter_list))
}

pub async fn update_filter(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    filter_id: web::Path<String>,
    req: web::Json<CreateFilterRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let now = Utc::now();
    let filter_id = filter_id.into_inner();
    
    let conditions_json = serde_json::to_string(&req.conditions)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    let actions_json = serde_json::to_string(&req.actions)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    let result = sqlx::query(
        r#"
        UPDATE filters SET
            name = ?,
            conditions = ?,
            actions = ?,
            is_active = ?,
            priority = ?,
            updated_at = ?
        WHERE id = ? AND user_id = ?
        "#
    )
    .bind(&req.name)
    .bind(&conditions_json)
    .bind(&actions_json)
    .bind(req.is_active)
    .bind(req.priority)
    .bind(now)
    .bind(&filter_id)
    .bind(&user.user_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to update filter: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to update filter")
    })?;
    
    if result.rows_affected() > 0 {
        Ok(HttpResponse::Ok().json(json!({
            "message": "Filter updated successfully"
        })))
    } else {
        Ok(HttpResponse::NotFound().json(json!({
            "error": "Filter not found"
        })))
    }
}

pub async fn delete_filter(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    filter_id: web::Path<String>,
) -> Result<HttpResponse, actix_web::Error> {
    let filter_id = filter_id.into_inner();
    
    let result = sqlx::query(
        "DELETE FROM filters WHERE id = ? AND user_id = ?"
    )
    .bind(&filter_id)
    .bind(&user.user_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to delete filter: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to delete filter")
    })?;
    
    if result.rows_affected() > 0 {
        Ok(HttpResponse::Ok().json(json!({
            "message": "Filter deleted successfully"
        })))
    } else {
        Ok(HttpResponse::NotFound().json(json!({
            "error": "Filter not found"
        })))
    }
}

pub async fn apply_filters(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Get all active filters for user
    #[derive(FromRow)]
    struct FilterData {
        id: i64,
        conditions: String,
        actions: String,
    }
    
    let filters = sqlx::query_as::<_, FilterData>(
        r#"
        SELECT id, conditions, actions
        FROM filters
        WHERE user_id = ? AND is_active = 1
        ORDER BY priority ASC
        "#
    )
    .bind(user.user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to fetch filters: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch filters")
    })?;
    
    // Get unprocessed emails
    #[derive(FromRow)]
    struct EmailData {
        id: i64,
        from_address: String,
        subject: String,
        has_attachments: bool,
    }
    
    let emails = sqlx::query_as::<_, EmailData>(
        r#"
        SELECT id, from_address, subject, has_attachments
        FROM emails
        WHERE user_id = ?
        LIMIT 100
        "#
    )
    .bind(user.user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to fetch emails: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch emails")
    })?;
    
    let mut processed_count = 0;
    
    for email in emails {
        for filter in &filters {
            let conditions: FilterConditions = serde_json::from_str(&filter.conditions)
                .unwrap_or_else(|_| FilterConditions {
                    from: None,
                    to: None,
                    subject: None,
                    body_contains: None,
                    has_attachments: None,
                    size_greater_than: None,
                    size_less_than: None,
                });
            let actions: FilterActions = serde_json::from_str(&filter.actions)
                .unwrap_or_else(|_| FilterActions {
                    move_to_folder: None,
                    mark_as_read: None,
                    mark_as_starred: None,
                    add_label: None,
                    forward_to: None,
                    delete: None,
                });
            
            // Check if email matches filter conditions
            let mut matches = true;
            
            if let Some(from) = &conditions.from {
                if !email.from_address.contains(from) {
                    matches = false;
                }
            }
            
            if let Some(subject) = &conditions.subject {
                if !email.subject.contains(subject) {
                    matches = false;
                }
            }
            
            if let Some(has_attachments) = conditions.has_attachments {
                if email.has_attachments != has_attachments {
                    matches = false;
                }
            }
            
            // Apply actions if conditions match
            if matches {
                if let Some(folder) = &actions.move_to_folder {
                    sqlx::query(
                        "UPDATE emails SET folder = ? WHERE id = ?"
                    )
                    .bind(folder)
                    .bind(&email.id)
                    .execute(pool.get_ref())
                    .await
                    .map_err(|e| {
                        log::error!("Failed to move email: {}", e);
                        actix_web::error::ErrorInternalServerError("Failed to apply filter")
                    })?;
                }
                
                if let Some(mark_read) = actions.mark_as_read {
                    sqlx::query(
                        "UPDATE emails SET is_read = ? WHERE id = ?"
                    )
                    .bind(mark_read)
                    .bind(&email.id)
                    .execute(pool.get_ref())
                    .await
                    .map_err(|e| {
                        log::error!("Failed to mark email: {}", e);
                        actix_web::error::ErrorInternalServerError("Failed to apply filter")
                    })?;
                }
                
                if let Some(mark_starred) = actions.mark_as_starred {
                    sqlx::query(
                        "UPDATE emails SET is_starred = ? WHERE id = ?"
                    )
                    .bind(mark_starred)
                    .bind(&email.id)
                    .execute(pool.get_ref())
                    .await
                    .map_err(|e| {
                        log::error!("Failed to star email: {}", e);
                        actix_web::error::ErrorInternalServerError("Failed to apply filter")
                    })?;
                }
                
                processed_count += 1;
                break; // Only apply first matching filter
            }
        }
    }
    
    Ok(HttpResponse::Ok().json(json!({
        "message": "Filters applied successfully",
        "processed_count": processed_count
    })))
}

// Implement Default for FilterConditions
impl Default for FilterConditions {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            subject: None,
            body_contains: None,
            has_attachments: None,
            size_greater_than: None,
            size_less_than: None,
        }
    }
}

// Implement Default for FilterActions
impl Default for FilterActions {
    fn default() -> Self {
        Self {
            move_to_folder: None,
            mark_as_read: None,
            mark_as_starred: None,
            add_label: None,
            forward_to: None,
            delete: None,
        }
    }
}