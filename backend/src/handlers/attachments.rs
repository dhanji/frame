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
    pub sender_email: Option<String>,
    pub sender_name: Option<String>,
    pub received_at: Option<String>,
    pub source_account: Option<String>,
    pub keywords: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Attachment {
    pub id: i64,
    pub email_id: Option<i64>,
    pub draft_id: Option<i64>,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub path: Option<String>,
    pub storage_path: Option<String>,
    pub thumbnail_path: Option<String>,
    pub sender_email: Option<String>,
    pub sender_name: Option<String>,
    pub received_at: Option<String>,
    pub source_account: Option<String>,
    pub keywords: Option<String>,
    pub preview_generated: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct GalleryQuery {
    pub view: Option<String>,
    pub search: Option<String>,
    pub sender: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub content_type: Option<String>,
    pub size_min: Option<i64>,
    pub size_max: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct GalleryResponse {
    pub attachments: Vec<AttachmentInfo>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct SenderGroup {
    pub sender_email: String,
    pub sender_name: String,
    pub attachment_count: i64,
    pub total_size: i64,
    pub attachments: Vec<AttachmentInfo>,
}

#[derive(Debug, Serialize)]
pub struct BySenderResponse {
    pub senders: Vec<SenderGroup>,
    pub total: i64,
}

pub async fn upload_attachment(
    mut payload: Multipart,
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let mut attachments = Vec::new();
    
    let upload_dir = format!("./uploads/{}", user.user_id);
    let thumbnail_dir = format!("./uploads/{}/thumbnails", user.user_id);
    std::fs::create_dir_all(&upload_dir)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    std::fs::create_dir_all(&thumbnail_dir)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
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
        
        let mut file = std::fs::File::create(&storage_path)
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        
        let mut size = 0;
        let mut file_data = Vec::new();
        
        while let Some(chunk) = field.try_next().await? {
            size += chunk.len();
            
            if size > 25 * 1024 * 1024 {
                let _ = std::fs::remove_file(&storage_path);
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "File size exceeds 25MB limit"
                })));
            }
            
            file.write_all(&chunk)
                .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
            file_data.extend_from_slice(&chunk);
        }
        
        let thumbnail_path = if is_image(&content_type) {
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
        
        let preview_generated = thumbnail_path.is_some();
        let storage_path_str = storage_path.clone();
        let size_i64 = size as i64;
        
        let result = sqlx::query!(
            r#"
            INSERT INTO attachments (
                email_id, draft_id, filename, content_type, size, 
                storage_path, thumbnail_path, preview_generated, 
                received_at, source_account, created_at
            )
            VALUES (NULL, NULL, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, 'default', CURRENT_TIMESTAMP)
            "#,
            filename,
            content_type,
            size_i64,
            storage_path_str,
            thumbnail_path,
            preview_generated
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
            thumbnail_url: thumbnail_path.map(|_| format!("/api/attachments/{}/thumbnail", db_id)),
            sender_email: None,
            sender_name: None,
            received_at: None,
            source_account: Some("default".to_string()),
            keywords: None,
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
        .map(|a| attachment_to_info(&a))
        .collect();
    
    Ok(HttpResponse::Ok().json(attachment_infos))
}

pub async fn get_gallery(
    pool: web::Data<SqlitePool>,
    query: web::Query<GalleryQuery>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);
    
    let mut where_clauses = vec!["(e.user_id = ? OR d.user_id = ?)".to_string()];
    
    if let Some(search) = &query.search {
        if !search.is_empty() {
            where_clauses.push("(a.filename LIKE ? OR a.sender_email LIKE ? OR a.sender_name LIKE ?)".to_string());
        }
    }
    
    if let Some(sender) = &query.sender {
        if !sender.is_empty() {
            where_clauses.push("a.sender_email = ?".to_string());
        }
    }
    
    if let Some(date_from) = &query.date_from {
        where_clauses.push("a.received_at >= ?".to_string());
    }
    if let Some(date_to) = &query.date_to {
        where_clauses.push("a.received_at <= ?".to_string());
    }
    
    if let Some(content_type) = &query.content_type {
        if !content_type.is_empty() {
            where_clauses.push("a.content_type LIKE ?".to_string());
        }
    }
    
    if let Some(size_min) = query.size_min {
        where_clauses.push("a.size >= ?".to_string());
    }
    if let Some(size_max) = query.size_max {
        where_clauses.push("a.size <= ?".to_string());
    }
    
    let where_clause = where_clauses.join(" AND ");
    
    let count_query = format!(
        r#"
        SELECT COUNT(*) as count
        FROM attachments a
        LEFT JOIN emails e ON a.email_id = e.id
        LEFT JOIN drafts d ON a.draft_id = d.id
        WHERE {}
        "#,
        where_clause
    );
    
    let total: i64 = sqlx::query_scalar(&count_query)
        .bind(user.user_id)
        .bind(user.user_id)
        .fetch_one(pool.get_ref())
        .await
        .unwrap_or(0);
    
    let attachments_query = format!(
        r#"
        SELECT a.*
        FROM attachments a
        LEFT JOIN emails e ON a.email_id = e.id
        LEFT JOIN drafts d ON a.draft_id = d.id
        WHERE {}
        ORDER BY a.received_at DESC, a.created_at DESC
        LIMIT ? OFFSET ?
        "#,
        where_clause
    );
    
    let attachments = sqlx::query_as::<_, Attachment>(&attachments_query)
        .bind(user.user_id)
        .bind(user.user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool.get_ref())
        .await
        .unwrap_or_default();
    
    let attachment_infos: Vec<AttachmentInfo> = attachments
        .into_iter()
        .map(|a| attachment_to_info(&a))
        .collect();
    
    Ok(HttpResponse::Ok().json(GalleryResponse {
        attachments: attachment_infos,
        total,
        has_more: offset + limit < total,
    }))
}

pub async fn get_gallery_recents(
    pool: web::Data<SqlitePool>,
    query: web::Query<GalleryQuery>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);
    
    let attachments = sqlx::query_as::<_, Attachment>(
        r#"
        SELECT a.*
        FROM attachments a
        LEFT JOIN emails e ON a.email_id = e.id
        LEFT JOIN drafts d ON a.draft_id = d.id
        WHERE (e.user_id = ? OR d.user_id = ?)
        ORDER BY a.received_at DESC, a.created_at DESC
        LIMIT ? OFFSET ?
        "#
    )
    .bind(user.user_id)
    .bind(user.user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM attachments a
        LEFT JOIN emails e ON a.email_id = e.id
        LEFT JOIN drafts d ON a.draft_id = d.id
        WHERE (e.user_id = ? OR d.user_id = ?)
        "#
    )
    .bind(user.user_id)
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    let attachment_infos: Vec<AttachmentInfo> = attachments
        .into_iter()
        .map(|a| attachment_to_info(&a))
        .collect();
    
    Ok(HttpResponse::Ok().json(GalleryResponse {
        attachments: attachment_infos,
        total,
        has_more: offset + limit < total,
    }))
}

pub async fn get_gallery_by_sender(
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let senders = sqlx::query!(
        r#"
        SELECT 
            a.sender_email,
            a.sender_name,
            COUNT(*) as attachment_count,
            SUM(a.size) as total_size
        FROM attachments a
        LEFT JOIN emails e ON a.email_id = e.id
        LEFT JOIN drafts d ON a.draft_id = d.id
        WHERE (e.user_id = ? OR d.user_id = ?)
        AND a.sender_email IS NOT NULL
        GROUP BY a.sender_email, a.sender_name
        ORDER BY attachment_count DESC
        "#,
        user.user_id,
        user.user_id
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    let mut sender_groups = Vec::new();
    
    for sender in senders {
        let sender_email = sender.sender_email.unwrap_or_default();
        let sender_name = sender.sender_name.unwrap_or_else(|| sender_email.clone());
        
        let attachments = sqlx::query_as::<_, Attachment>(
            r#"
            SELECT a.*
            FROM attachments a
            LEFT JOIN emails e ON a.email_id = e.id
            LEFT JOIN drafts d ON a.draft_id = d.id
            WHERE (e.user_id = ? OR d.user_id = ?)
            AND a.sender_email = ?
            ORDER BY a.received_at DESC, a.created_at DESC
            LIMIT 10
            "#
        )
        .bind(user.user_id)
        .bind(user.user_id)
        .bind(&sender_email)
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        
        let attachment_infos: Vec<AttachmentInfo> = attachments
            .into_iter()
            .map(|a| attachment_to_info(&a))
            .collect();
        
        sender_groups.push(SenderGroup {
            sender_email,
            sender_name,
            attachment_count: sender.attachment_count,
            total_size: sender.total_size,
            attachments: attachment_infos,
        });
    }
    
    let total = sender_groups.len() as i64;
    
    Ok(HttpResponse::Ok().json(BySenderResponse {
        senders: sender_groups,
        total,
    }))
}

pub async fn download_attachment(
    path: web::Path<String>,
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let attachment_id: i64 = path.into_inner().parse()
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid attachment ID"))?;
    
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
    
    let file_path = attachment.storage_path
        .or(attachment.path)
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("No file path found"))?;
    
    let file_content = std::fs::read(&file_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok()
        .content_type(attachment.content_type)
        .append_header(("Content-Disposition", format!("attachment; filename=\"{}\"", attachment.filename)))
        .body(file_content))
}

pub async fn download_thumbnail(
    path: web::Path<String>,
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let attachment_id: i64 = path.into_inner().parse()
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid attachment ID"))?;
    
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
    
    let thumbnail_path = attachment.thumbnail_path
        .ok_or_else(|| actix_web::error::ErrorNotFound("Thumbnail not available"))?;
    
    let file_content = std::fs::read(&thumbnail_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok()
        .content_type("image/jpeg")
        .body(file_content))
}

pub async fn delete_attachment(
    path: web::Path<String>,
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let attachment_id: i64 = path.into_inner().parse()
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid attachment ID"))?;
    
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
    
    if let Some(path) = &attachment.path {
        let _ = std::fs::remove_file(path);
    }
    if let Some(path) = &attachment.storage_path {
        let _ = std::fs::remove_file(path);
    }
    if let Some(path) = &attachment.thumbnail_path {
        let _ = std::fs::remove_file(path);
    }
    
    sqlx::query!("DELETE FROM attachments WHERE id = ?", attachment_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Attachment deleted successfully"
    })))
}

fn is_image(content_type: &str) -> bool {
    content_type.starts_with("image/")
}

fn generate_thumbnail(
    image_data: &[u8],
    thumbnail_dir: &str,
    attachment_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let img = image::load_from_memory(image_data)?;
    let thumbnail = img.thumbnail(200, 200);
    let thumbnail_path = format!("{}/{}_thumb.jpg", thumbnail_dir, attachment_id);
    thumbnail.save_with_format(&thumbnail_path, ImageFormat::Jpeg)?;
    Ok(thumbnail_path)
}

fn attachment_to_info(a: &Attachment) -> AttachmentInfo {
    AttachmentInfo {
        id: a.id.to_string(),
        filename: a.filename.clone(),
        content_type: a.content_type.clone(),
        size: a.size as usize,
        url: format!("/api/attachments/{}", a.id),
        thumbnail_url: if a.thumbnail_path.is_some() {
            Some(format!("/api/attachments/{}/thumbnail", a.id))
        } else {
            None
        },
        sender_email: a.sender_email.clone(),
        sender_name: a.sender_name.clone(),
        received_at: a.received_at.clone(),
        source_account: a.source_account.clone(),
        keywords: a.keywords.clone(),
    }
}

pub async fn cleanup_orphaned_attachments(
    pool: web::Data<SqlitePool>,
) -> Result<HttpResponse, actix_web::Error> {
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
        if let Some(path) = &attachment.path {
            let _ = std::fs::remove_file(path);
        }
        if let Some(path) = &attachment.storage_path {
            let _ = std::fs::remove_file(path);
        }
        if let Some(path) = &attachment.thumbnail_path {
            let _ = std::fs::remove_file(path);
        }
        
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
