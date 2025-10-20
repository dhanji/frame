use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserSettings {
    pub email_sync_interval: i32,
    pub notifications_enabled: bool,
    pub auto_mark_read: bool,
    pub signature: Option<String>,
    pub default_folder: String,
    pub emails_per_page: i32,
    pub theme: String,
    pub keyboard_shortcuts_enabled: bool,
    pub conversation_view: bool,
    pub preview_pane: bool,
    // AI Agent settings
    pub ai_provider: String,
    pub ai_api_key: Option<String>,
    pub ai_model: String,
    pub ai_context_window: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub email_sync_interval: Option<i32>,
    pub notifications_enabled: Option<bool>,
    pub auto_mark_read: Option<bool>,
    pub signature: Option<String>,
    pub default_folder: Option<String>,
    pub emails_per_page: Option<i32>,
    pub theme: Option<String>,
    pub keyboard_shortcuts_enabled: Option<bool>,
    pub conversation_view: Option<bool>,
    pub preview_pane: Option<bool>,
    // AI Agent settings
    pub ai_provider: Option<String>,
    pub ai_api_key: Option<String>,
    pub ai_model: Option<String>,
    pub ai_context_window: Option<i32>,
}

pub async fn get_settings(
    pool: web::Data<SqlitePool>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    let settings = sqlx::query_as::<_, (String,)>(
        "SELECT settings FROM users WHERE id = ?"
    )
    .bind(user.user_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    let settings = if let Some((settings_json,)) = settings {
        serde_json::from_str::<UserSettings>(&settings_json)
            .unwrap_or_else(|_| default_settings())
    } else {
        default_settings()
    };
    
    Ok(HttpResponse::Ok().json(settings))
}

pub async fn update_settings(
    pool: web::Data<SqlitePool>,
    body: web::Json<UpdateSettingsRequest>,
    user: crate::middleware::auth::AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Get current settings
    let current = sqlx::query_as::<_, (String,)>(
        "SELECT settings FROM users WHERE id = ?"
    )
    .bind(user.user_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    let mut settings = if let Some((settings_json,)) = current {
        serde_json::from_str::<UserSettings>(&settings_json)
            .unwrap_or_else(|_| default_settings())
    } else {
        default_settings()
    };
    
    // Update settings with provided values
    if let Some(v) = body.email_sync_interval {
        settings.email_sync_interval = v;
    }
    if let Some(v) = body.notifications_enabled {
        settings.notifications_enabled = v;
    }
    if let Some(v) = body.auto_mark_read {
        settings.auto_mark_read = v;
    }
    if let Some(ref v) = body.signature {
        settings.signature = Some(v.clone());
    }
    if let Some(ref v) = body.default_folder {
        settings.default_folder = v.clone();
    }
    if let Some(v) = body.emails_per_page {
        settings.emails_per_page = v;
    }
    if let Some(ref v) = body.theme {
        settings.theme = v.clone();
    }
    if let Some(v) = body.keyboard_shortcuts_enabled {
        settings.keyboard_shortcuts_enabled = v;
    }
    if let Some(v) = body.conversation_view {
        settings.conversation_view = v;
    }
    if let Some(v) = body.preview_pane {
        settings.preview_pane = v;
    }
    if let Some(ref v) = body.ai_provider {
        settings.ai_provider = v.clone();
    }
    if let Some(ref v) = body.ai_api_key {
        settings.ai_api_key = Some(v.clone());
    }
    if let Some(ref v) = body.ai_model {
        settings.ai_model = v.clone();
    }
    if let Some(v) = body.ai_context_window {
        settings.ai_context_window = v;
    }
    
    // Save updated settings
    let settings_json = serde_json::to_string(&settings)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    sqlx::query(
        "UPDATE users SET settings = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(settings_json)
    .bind(user.user_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Settings updated successfully",
        "settings": settings
    })))
}

fn default_settings() -> UserSettings {
    UserSettings {
        email_sync_interval: 300,
        notifications_enabled: true,
        auto_mark_read: true,
        signature: None,
        default_folder: "INBOX".to_string(),
        emails_per_page: 50,
        theme: "light".to_string(),
        keyboard_shortcuts_enabled: true,
        conversation_view: true,
        preview_pane: true,
        // AI Agent defaults
        ai_provider: "anthropic".to_string(),
        ai_api_key: None,
        ai_model: "claude-3-5-sonnet-20241022".to_string(),
        ai_context_window: 200000,
    }
}