use actix_multipart::Multipart;
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::io::Write;
use uuid::Uuid;
use image::ImageFormat;

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size: usize,
    pub url: String,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Attachment {
    pub id: i64,
    pub email_id: Option<i64>,
    pub draft_id: Option<i64>,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub path: String,
    // pub thumbnail_path: Option<String>,  // Will be added after migration
}

pub async fn upload_attachment(
    mut payload: Multipart,
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let mut attachments = Vec::new();
    
    // Create upload directories
    let upload_dir = format!("./uploads/{}", user.user_id);
    let thumbnail_dir = format!("./uploads/{}/thumbnails", user.user_id);
    std::fs::create_dir_all(&upload_dir)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    std::fs::create_dir_all(&thumbnail_dir)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    // Process each file in the multipart upload
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition();
        let filename = if let Some(filename) = content_disposition.get_filename() {
            filename.to_string()
        } else {
            "attachment".to_string()
        };
        
        let content_type = field.content_type()
            .map(|ct| ct.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        
        let attachment_id = Uuid::new_v4().to_string();
        let storage_path = format!("{}/{}", upload_dir, attachment_id);
        
        // Create file and write chunks
        let mut file = std::fs::File::create(&storage_path)
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        
        let mut size = 0;
        let mut file_data = Vec::new();
        
        while let Some(chunk) = field.try_next().await? {
            size += chunk.len();
            
            // Check size limit (25MB)
            if size > 25 * 1024 * 1024 {
                // Clean up partial file
                let _ = std::fs::remove_file(&storage_path);
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "File size exceeds 25MB limit"
                })));
            }
            
            file.write_all(&chunk)
                .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
            file_data.extend_from_slice(&chunk);
        }
        
        // Generate thumbnail for images
        let _thumbnail_path = if is_image(&content_type) {
            match generate_thumbnail(&file_data, &thumbnail_dir, &attachment_id) {
                Ok(path) => Some(path),
                Err(e) => {
                    log::warn!("Failed to generate thumbnail: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        // Store in database
        let storage_path_str = storage_path.clone();
        let size_i64 = size as i64;
        let result = sqlx::query!(
            r#"
            INSERT INTO attachments (
                email_id, draft_id, filename, content_type, size, 
                storage_path, created_at
            )
            VALUES (NULL, NULL, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
            filename,
            content_type,
            size_i64,
            storage_path_str
        )
        .execute(pool.get_ref())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        
        let db_id = result.last_insert_rowid();
        
        attachments.push(AttachmentInfo {
            id: db_id.to_string(),
            filename,
            content_type,
            size,
            url: format!("/api/attachments/{}", db_id),
            thumbnail_url: None, // Will be enabled after migration adds thumbnail_path column
        });
    }
    
    Ok(HttpResponse::Ok().json(attachments))
}

pub async fn get_attachments(
    pool: web::Data<SqlitePool>,
    query: web::Query<std::collections::HashMap<String, String>>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let email_id: Option<i64> = query.get("email_id")
        .and_then(|s| s.parse().ok());
    
    let draft_id: Option<i64> = query.get("draft_id")
        .and_then(|s| s.parse().ok());
    
    let attachments = if let Some(eid) = email_id {
        sqlx::query_as::<_, Attachment>(
            r#"
            SELECT a.* FROM attachments a
            INNER JOIN emails e ON a.email_id = e.id
            WHERE e.user_id = ? AND a.email_id = ?
            "#
        )
        .bind(user.user_id)
        .bind(eid)
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
    } else if let Some(did) = draft_id {
        sqlx::query_as::<_, Attachment>(
            r#"
            SELECT a.* FROM attachments a
            INNER JOIN drafts d ON a.draft_id = d.id
            WHERE d.user_id = ? AND a.draft_id = ?
            "#
        )
        .bind(user.user_id)
        .bind(did)
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
    } else {
        vec![]
    };
    
    let attachment_infos: Vec<AttachmentInfo> = attachments
        .into_iter()
        .map(|a| AttachmentInfo {
            id: a.id.to_string(),
            filename: a.filename,
            content_type: a.content_type.clone(),
            size: a.size as usize,
            url: format!("/api/attachments/{}", a.id),
            thumbnail_url: None, // Will be enabled after migration
        })
        .collect();
    
    Ok(HttpResponse::Ok().json(attachment_infos))
}

pub async fn download_attachment(
    path: web::Path<String>,
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let attachment_id: i64 = path.into_inner().parse()
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid attachment ID"))?;
    
    // Get attachment from database with user verification
    let attachment = sqlx::query_as::<_, Attachment>(
        r#"
        SELECT a.* FROM attachments a
        LEFT JOIN emails e ON a.email_id = e.id
        LEFT JOIN drafts d ON a.draft_id = d.id
        WHERE a.id = ? AND (e.user_id = ? OR d.user_id = ?)
        "#
    )
    .bind(attachment_id)
    .bind(user.user_id)
    .bind(user.user_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
    .ok_or_else(|| actix_web::error::ErrorNotFound("Attachment not found"))?;
    
    // Read file
    let file_content = std::fs::read(&attachment.path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok()
        .content_type(attachment.content_type)
        .append_header(("Content-Disposition", format!("attachment; filename=\"{}\"", attachment.filename)))
        .body(file_content))
}

pub async fn download_thumbnail(
    _path: web::Path<String>,
    _pool: web::Data<SqlitePool>,
    _user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Thumbnail feature will be enabled after migration adds thumbnail_path column
    Ok(HttpResponse::NotImplemented().json(serde_json::json!({
        "error": "Thumbnail feature not yet available"
    })))
}

pub async fn delete_attachment(
    path: web::Path<String>,
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let attachment_id: i64 = path.into_inner().parse()
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid attachment ID"))?;
    
    // Get attachment from database with user verification
    let attachment = sqlx::query_as::<_, Attachment>(
        r#"
        SELECT a.* FROM attachments a
        LEFT JOIN emails e ON a.email_id = e.id
        LEFT JOIN drafts d ON a.draft_id = d.id
        WHERE a.id = ? AND (e.user_id = ? OR d.user_id = ?)
        "#
    )
    .bind(attachment_id)
    .bind(user.user_id)
    .bind(user.user_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
    .ok_or_else(|| actix_web::error::ErrorNotFound("Attachment not found"))?;
    
    // Delete files
    let _ = std::fs::remove_file(&attachment.path);
    
    // Delete from database
    sqlx::query!("DELETE FROM attachments WHERE id = ?", attachment_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Attachment deleted successfully"
    })))
}

/// Check if content type is an image
fn is_image(content_type: &str) -> bool {
    content_type.starts_with("image/")
}

/// Generate thumbnail for image
fn generate_thumbnail(
    image_data: &[u8],
    thumbnail_dir: &str,
    attachment_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Load image
    let img = image::load_from_memory(image_data)?;
    
    // Create thumbnail (max 200x200)
    let thumbnail = img.thumbnail(200, 200);
    
    // Save thumbnail
    let thumbnail_path = format!("{}/{}_thumb.jpg", thumbnail_dir, attachment_id);
    thumbnail.save_with_format(&thumbnail_path, ImageFormat::Jpeg)?;
    
    Ok(thumbnail_path)
}

/// Cleanup orphaned attachments (attachments with no email or draft)
pub async fn cleanup_orphaned_attachments(
    pool: web::Data<SqlitePool>,
) -> Result<HttpResponse, actix_web::Error> {
    // Find orphaned attachments (older than 24 hours)
    let orphaned = sqlx::query_as::<_, Attachment>(
        r#"
        SELECT * FROM attachments
        WHERE email_id IS NULL 
        AND draft_id IS NULL
        AND created_at < datetime('now', '-1 day')
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    let mut deleted_count = 0;
    
    for attachment in orphaned {
        // Delete files
        let _ = std::fs::remove_file(&attachment.path);
        
        // Delete from database
        sqlx::query!("DELETE FROM attachments WHERE id = ?", attachment.id)
            .execute(pool.get_ref())
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        
        deleted_count += 1;
    }
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Cleaned up {} orphaned attachments", deleted_count),
        "count": deleted_count
    })))
}
