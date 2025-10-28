use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub text: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    pub has_attachments: Option<bool>,
    pub is_unread: Option<bool>,
    pub is_starred: Option<bool>,
    pub folder: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SaveSearchRequest {
    pub name: String,
    pub query: crate::services::search::SearchQuery,
}

// Remove the actix_web::get attribute since we're using manual routing
pub async fn search_emails(
    pool: web::Data<SqlitePool>,
    query: web::Query<SearchRequest>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let search_service = crate::services::search::SearchService::new(pool.get_ref().clone());
    
    let search_query = crate::services::search::SearchQuery {
        text: query.text.clone(),
        from: query.from.clone(),
        to: query.to.clone(),
        subject: query.subject.clone(),
        has_attachments: query.has_attachments,
        is_read: query.is_unread.map(|u| !u),  // Convert is_unread to is_read
        is_starred: query.is_starred,
        folder: query.folder.clone(),
        date_from: query.date_from.as_ref().and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok()).map(|d| d.with_timezone(&chrono::Utc)),
        date_to: query.date_to.as_ref().and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok()).map(|d| d.with_timezone(&chrono::Utc)),
        size_min: None,
        size_max: None,
    };
    
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    
    match search_service.search_conversations(&search_query, user.user_id, limit, offset).await {
        Ok(results) => Ok(HttpResponse::Ok().json(results)),
        Err(e) => {
            log::error!("Search error: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to search emails"
            })))
        }
    }
}

pub async fn save_search(
    pool: web::Data<SqlitePool>,
    body: web::Json<SaveSearchRequest>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let search_service = crate::services::search::SearchService::new(pool.get_ref().clone());
    
    match search_service.save_search(user.user_id, body.name.clone(), body.query.clone()).await {
        Ok(id) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "id": id,
            "message": "Search saved successfully"
        }))),
        Err(e) => {
            log::error!("Save search error: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to save search"
            })))
        }
    }
}

pub async fn get_saved_searches(
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let search_service = crate::services::search::SearchService::new(pool.get_ref().clone());
    
    match search_service.get_saved_searches(user.user_id).await {
        Ok(searches) => Ok(HttpResponse::Ok().json(searches)),
        Err(e) => {
            log::error!("Get saved searches error: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get saved searches"
            })))
        }
    }
}

pub async fn delete_saved_search(
    pool: web::Data<SqlitePool>,
    path: web::Path<String>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let search_service = crate::services::search::SearchService::new(pool.get_ref().clone());
    let search_id = path.into_inner();
    
    match search_service.delete_saved_search(user.user_id, &search_id).await {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": "Search deleted successfully"
        }))),
        Err(e) => {
            log::error!("Delete saved search error: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to delete saved search"
            })))
        }
    }
}

pub async fn get_search_suggestions(
    pool: web::Data<SqlitePool>,
    query: web::Query<std::collections::HashMap<String, String>>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let search_service = crate::services::search::SearchService::new(pool.get_ref().clone());
    let prefix = query.get("q").cloned().unwrap_or_default();
    let limit = query.get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    
    match search_service.get_suggestions(&prefix, user.user_id, limit).await {
        Ok(suggestions) => Ok(HttpResponse::Ok().json(suggestions)),
        Err(e) => {
            log::error!("Get search suggestions error: {}", e);
            Ok(HttpResponse::Ok().json(Vec::<String>::new()))
        }
    }
}