use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::middleware::auth::AuthenticatedUser;

#[derive(Debug, Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct FolderResponse {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub sort_order: i32,
    pub is_system: bool,
    pub unread_count: i64,
    pub total_count: i64,
}

pub async fn get_folders(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Get all folders for the user
    let folders = sqlx::query_as::<_, (i64, String, Option<i64>, i32, bool)>(
        "SELECT id, name, parent_id, sort_order, is_system FROM folders WHERE user_id = ? ORDER BY sort_order, name"
    )
    .bind(user.user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch folders")
    })?;
    
    // Get email counts for each folder
    let mut folder_responses = Vec::new();
    
    for (id, name, parent_id, sort_order, is_system) in folders {
        // Get unread count
        let unread_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM emails WHERE user_id = ? AND folder = ? AND is_read = false"
        )
        .bind(user.user_id)
        .bind(&name)
        .fetch_one(pool.get_ref())
        .await
        .unwrap_or(0);
        
        // Get total count
        let total_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM emails WHERE user_id = ? AND folder = ?"
        )
        .bind(user.user_id)
        .bind(&name)
        .fetch_one(pool.get_ref())
        .await
        .unwrap_or(0);
        
        folder_responses.push(FolderResponse {
            id,
            name,
            parent_id,
            sort_order,
            is_system,
            unread_count,
            total_count,
        });
    }
    
    Ok(HttpResponse::Ok().json(folder_responses))
}

pub async fn create_folder(
    pool: web::Data<SqlitePool>,
    body: web::Json<CreateFolderRequest>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Check if folder already exists
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM folders WHERE user_id = ? AND name = ?"
    )
    .bind(user.user_id)
    .bind(&body.name)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;
    
    if exists > 0 {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Folder already exists"
        })));
    }
    
    // Get next sort order
    let sort_order = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT MAX(sort_order) FROM folders WHERE user_id = ?"
    )
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .unwrap_or(Some(0))
    .unwrap_or(0) + 1;
    
    // Create folder
    let result = sqlx::query(
        r#"
        INSERT INTO folders (user_id, name, parent_id, sort_order, is_system, created_at, updated_at)
        VALUES (?, ?, ?, ?, false, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(user.user_id)
    .bind(&body.name)
    .bind(body.parent_id)
    .bind(sort_order)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Failed to create folder: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to create folder")
    })?;
    
    let folder_id = result.last_insert_rowid();
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Folder created successfully",
        "folder_id": folder_id
    })))
}
