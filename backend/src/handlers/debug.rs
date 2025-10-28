use actix_web::{web, HttpResponse};
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::EmailManager;
use crate::models::User;
use chrono::Datelike;
use crate::services::caldav::CalDavClient;

#[derive(Debug, Serialize)]
pub struct SyncDebugInfo {
    pub user_id: i64,
    pub email: String,
    pub is_initialized: bool,
    pub has_imap_service: bool,
    pub has_smtp_service: bool,
    pub oauth_provider: Option<String>,
    pub has_oauth_tokens: bool,
    pub imap_host: String,
    pub imap_port: i32,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub total_emails: i64,
    pub emails_by_folder: Vec<FolderEmailCount>,
    pub recent_sync_errors: Vec<String>,
    pub last_sync_attempt: Option<String>,
    pub calendar_sync_status: CalendarSyncStatus,
    pub agent_tools: Vec<AgentToolInfo>,
}

#[derive(Debug, Serialize)]
pub struct FolderEmailCount {
    pub folder: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct CalendarSyncStatus {
    pub total_events: i64,
    pub events_this_month: i64,
    pub events_next_month: i64,
    pub caldav_configured: bool,
    pub caldav_url: Option<String>,
    pub last_sync: Option<String>,
    pub sync_enabled: bool,
    pub pending_sync: i64,
    pub sync_errors: Vec<String>,
    pub calendars: Vec<CalendarInfo>,
}

#[derive(Debug, Serialize)]
pub struct AgentToolInfo {
    pub name: String,
    pub description: String,
    pub category: String,
}

#[derive(Debug, Serialize)]
pub struct CalendarInfo {
    pub calendar_id: String,
    pub event_count: i64,
}

#[derive(Debug, Serialize)]
pub struct ImapTestResult {
    pub success: bool,
    pub message: String,
    pub folders: Option<Vec<String>>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CalDavTestResult {
    pub success: bool,
    pub message: String,
    pub event_count: Option<usize>,
    pub error: Option<String>,
}

/// Get detailed sync debug information
pub async fn get_sync_debug(
    pool: web::Data<SqlitePool>,
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    // Get user info
    let db_user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ?"
    )
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    // Check if services are initialized
    let is_initialized = email_manager.is_user_initialized(user.user_id).await;
    let has_imap = email_manager.get_imap_service(user.user_id).await.is_some();
    let has_smtp = email_manager.get_smtp_service(user.user_id).await.is_some();

    // Get email counts by folder
    let folder_counts = sqlx::query_as::<_, (String, i64)>(
        "SELECT COALESCE(folder, 'Unknown') as folder, COUNT(*) as count 
         FROM emails 
         WHERE user_id = ? 
         GROUP BY folder"
    )
    .bind(user.user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?
    .into_iter()
    .map(|(folder, count)| FolderEmailCount { folder, count })
    .collect();

    // Get total email count
    let total_emails = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM emails WHERE user_id = ?"
    )
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .unwrap_or(0);

    let has_oauth_tokens = db_user.oauth_access_token.is_some() && !db_user.oauth_access_token.as_ref().unwrap().is_empty();

    // Get calendar sync status
    let calendar_sync_status = get_calendar_sync_status(&pool, user.user_id).await;

    let agent_tools = get_agent_tools_info();

    Ok(HttpResponse::Ok().json(SyncDebugInfo {
        user_id: user.user_id,
        email: db_user.email.clone(),
        is_initialized,
        has_imap_service: has_imap,
        has_smtp_service: has_smtp,
        oauth_provider: db_user.oauth_provider.clone(),
        has_oauth_tokens,
        imap_host: db_user.imap_host.clone(),
        imap_port: db_user.imap_port,
        smtp_host: db_user.smtp_host.clone(),
        smtp_port: db_user.smtp_port,
        total_emails,
        emails_by_folder: folder_counts,
        recent_sync_errors: vec![], // TODO: implement error tracking
        last_sync_attempt: None, // TODO: implement sync tracking
        calendar_sync_status,
        agent_tools,
    }))
}

/// Get calendar sync status
async fn get_calendar_sync_status(pool: &SqlitePool, user_id: i64) -> CalendarSyncStatus {
    // Get total events
    let total_events = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM calendar_events WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    // Get events this month
    let now = chrono::Utc::now();
    let start_of_month = now.date_naive().with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let start_of_next_month = if now.month() == 12 {
        chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc()
    } else {
        chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc()
    };

    let events_this_month = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM calendar_events WHERE user_id = ? AND start_time >= ? AND start_time < ?"
    )
    .bind(user_id)
    .bind(start_of_month)
    .bind(start_of_next_month)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    // Get events next month
    let start_of_month_after = if now.month() == 11 {
        chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc()
    } else if now.month() == 12 {
        chrono::NaiveDate::from_ymd_opt(now.year() + 1, 2, 1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc()
    } else {
        chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 2, 1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc()
    };

    let events_next_month = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM calendar_events WHERE user_id = ? AND start_time >= ? AND start_time < ?"
    )
    .bind(user_id)
    .bind(start_of_next_month)
    .bind(start_of_month_after)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    // Get calendar breakdown
    let calendars = sqlx::query_as::<_, (String, i64)>(
        "SELECT calendar_id, COUNT(*) as count FROM calendar_events WHERE user_id = ? GROUP BY calendar_id"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(calendar_id, event_count)| CalendarInfo { calendar_id, event_count })
    .collect();

    // Check CalDAV configuration from user settings first, then environment
    let user_settings = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
        "SELECT caldav_url, caldav_username, caldav_password FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let caldav_url = user_settings.as_ref()
        .and_then(|(url, _, _)| url.clone())
        .or_else(|| std::env::var("CALDAV_URL").ok());
    
    let caldav_configured = caldav_url.is_some() && (
        (user_settings.as_ref().map(|(_, u, p)| u.is_some() && p.is_some()).unwrap_or(false)) ||
        (std::env::var("CALDAV_USERNAME").is_ok() && std::env::var("CALDAV_PASSWORD").is_ok()));

    CalendarSyncStatus {
        total_events,
        events_this_month,
        events_next_month,
        caldav_configured,
        caldav_url,
        last_sync: None, // TODO: Track last sync time
        sync_enabled: caldav_configured,
        pending_sync: 0, // TODO: Track pending syncs
        sync_errors: vec![], // TODO: Track sync errors
        calendars,
    }
}

/// Get list of available agent tools
fn get_agent_tools_info() -> Vec<AgentToolInfo> {
    vec![
        AgentToolInfo {
            name: "search_emails".to_string(),
            description: "Search through emails using keywords, sender, date range, etc.".to_string(),
            category: "Email".to_string(),
        },
        AgentToolInfo {
            name: "get_email".to_string(),
            description: "Retrieve a specific email by ID with full content".to_string(),
            category: "Email".to_string(),
        },
        AgentToolInfo {
            name: "send_email".to_string(),
            description: "Send a new email to recipients".to_string(),
            category: "Email".to_string(),
        },
        AgentToolInfo {
            name: "reply_to_email".to_string(),
            description: "Reply to an existing email".to_string(),
            category: "Email".to_string(),
        },
        AgentToolInfo {
            name: "create_draft".to_string(),
            description: "Create a draft email for later editing".to_string(),
            category: "Email".to_string(),
        },
        AgentToolInfo {
            name: "get_calendar_events".to_string(),
            description: "Retrieve calendar events for a date range".to_string(),
            category: "Calendar".to_string(),
        },
        AgentToolInfo {
            name: "create_calendar_event".to_string(),
            description: "Create a new calendar event".to_string(),
            category: "Calendar".to_string(),
        },
        // Add more tools as they are implemented
    ]
}

/// Test IMAP connection and list folders
pub async fn test_imap_connection(
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("Testing IMAP connection for user {}", user.user_id);

    // Check if IMAP service exists
    let imap_service = match email_manager.get_imap_service(user.user_id).await {
        Some(service) => service,
        None => {
            return Ok(HttpResponse::Ok().json(ImapTestResult {
                success: false,
                message: "IMAP service not initialized".to_string(),
                folders: None,
                error: Some("Service not found. Try logging out and back in.".to_string()),
            }));
        }
    };

    // Try to list folders
    match imap_service.list_folders().await {
        Ok(folders) => {
            log::info!("Successfully listed {} folders for user {}", folders.len(), user.user_id);
            Ok(HttpResponse::Ok().json(ImapTestResult {
                success: true,
                message: format!("Successfully connected. Found {} folders.", folders.len()),
                folders: Some(folders),
                error: None,
            }))
        }
        Err(e) => {
            log::error!("IMAP test failed for user {}: {}", user.user_id, e);
            Ok(HttpResponse::Ok().json(ImapTestResult {
                success: false,
                message: "Failed to connect to IMAP server".to_string(),
                folders: None,
                error: Some(format!("{}", e)),
            }))
        }
    }
}

/// Test CalDAV sync service initialization
pub async fn test_caldav_sync_init(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("🗓️  Manual CalDAV sync initialization test triggered by user {}", user.user_id);
    
    // Try to create the service
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::services::caldav_sync::CalDavSyncService::new(pool.get_ref().clone())
    }));
    
    match result {
        Ok(service) => {
            log::info!("🗓️  CalDAV sync service created successfully");
            
            // Try to get users with CalDAV
            // Note: We can't call private methods, so we'll just return success
            
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "CalDAV sync service initialized successfully",
            })))
        }
        Err(e) => {
            log::error!("🗓️  CalDAV sync service panicked: {:?}", e);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": false,
                "error": "CalDAV sync service panicked during initialization"
            })))
        }
    }
}

/// Trigger manual sync for the current user
pub async fn trigger_manual_sync(
    pool: web::Data<SqlitePool>,
    email_manager: web::Data<Arc<EmailManager>>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("Manual sync triggered for user {}", user.user_id);

    // Get user info
    let db_user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ?"
    )
    .bind(user.user_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    // Check if user's services are initialized
    if !email_manager.is_user_initialized(user.user_id).await {
        // Try to initialize
        if let Err(e) = email_manager.initialize_user(&db_user).await {
            log::error!("Failed to initialize email services for user {}: {}", user.user_id, e);
            return Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to initialize email services: {}", e)
            })));
        }
    }

    // Get IMAP service
    let imap_service = match email_manager.get_imap_service(user.user_id).await {
        Some(service) => service,
        None => {
            return Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": false,
                "error": "IMAP service not available"
            })));
        }
    };

    // Sync INBOX
    let folders = vec!["INBOX", "[Gmail]/Sent Mail", "[Gmail]/Drafts"];
    let mut synced_counts = std::collections::HashMap::new();
    let mut errors = Vec::new();

    for folder in folders {
        log::info!("Syncing folder {} for user {}", folder, user.user_id);
        match sync_folder(&imap_service, &pool, user.user_id, folder).await {
            Ok(count) => {
                log::info!("Synced {} emails from {} for user {}", count, folder, user.user_id);
                synced_counts.insert(folder.to_string(), count);
            }
            Err(e) => {
                log::error!("Failed to sync {} for user {}: {}", folder, user.user_id, e);
                errors.push(format!("{}: {}", folder, e));
            }
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": errors.is_empty(),
        "synced_counts": synced_counts,
        "errors": errors,
    })))
}

/// Helper function to sync a specific folder
async fn sync_folder(
    imap_service: &Arc<crate::services::ImapService>,
    pool: &SqlitePool,
    user_id: i64,
    folder: &str,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    use chrono::Utc;

    // Fetch messages from IMAP
    log::info!("Fetching messages from folder {} for user {}", folder, user_id);
    let messages = imap_service.fetch_messages(folder, 100).await?;
    let message_count = messages.len();
    log::info!("Fetched {} messages from folder {} for user {}", message_count, folder, user_id);

    // Store each message in the database
    for message in messages {
        // Check if message already exists
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM emails WHERE user_id = ? AND message_id = ?"
        )
        .bind(user_id)
        .bind(&message.message_id)
        .fetch_one(pool)
        .await?;

        if exists > 0 {
            // Update existing message (flags, etc.)
            sqlx::query(
                r#"
                UPDATE emails 
                SET is_read = ?, is_starred = ?, updated_at = ?
                WHERE user_id = ? AND message_id = ?
                "#
            )
            .bind(message.flags.contains(&"\\Seen".to_string()))
            .bind(message.flags.contains(&"\\Flagged".to_string()))
            .bind(Utc::now())
            .bind(user_id)
            .bind(&message.message_id)
            .execute(pool)
            .await?;
        } else {
            // Insert new message
            sqlx::query(
                r#"
                INSERT INTO emails (
                    user_id, message_id, thread_id, from_address, to_addresses,
                    cc_addresses, bcc_addresses, subject, body_text, body_html,
                    date, is_read, is_starred, has_attachments, attachments,
                    folder, size, in_reply_to, references, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind(user_id)
            .bind(&message.message_id)
            .bind(&message.thread_id)
            .bind(
                message.from.iter()
                    .map(|a| format!(
                        "{}",
                        if let Some(name) = &a.name {
                            format!("{} <{}>", name, a.email)
                        } else {
                            a.email.clone()
                        }
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .bind(serde_json::to_string(&message.to).unwrap_or_default())
            .bind(serde_json::to_string(&message.cc).unwrap_or_default())
            .bind(serde_json::to_string(&message.bcc).unwrap_or_default())
            .bind(&message.subject)
            .bind(&message.body_text)
            .bind(&message.body_html)
            .bind(message.date)
            .bind(message.flags.contains(&"\\Seen".to_string()))
            .bind(message.flags.contains(&"\\Flagged".to_string()))
            .bind(!message.attachments.is_empty())
            .bind(serde_json::to_string(&message.attachments).unwrap_or_else(|_| "[]".to_string()))
            .bind(folder)
            .bind(0i64) // size placeholder
            .bind(&message.in_reply_to)
            .bind(serde_json::to_string(&message.references).unwrap_or_default())
            .bind(Utc::now())
            .bind(Utc::now())
            .execute(pool)
            .await?;
            
            // If there are attachments, save them to the attachments table
            if !message.attachments.is_empty() {
                let email_id = sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM emails WHERE user_id = ? AND message_id = ?"
                )
                .bind(user_id)
                .bind(&message.message_id)
                .fetch_one(pool)
                .await?;
                
                // Extract sender info
                let sender_email = message.from.first().map(|a| a.email.clone()).unwrap_or_default();
                let sender_name = message.from.first().and_then(|a| a.name.clone()).unwrap_or_else(|| sender_email.clone());
                
                for attachment in &message.attachments {
                    // Save attachment to storage
                    let storage_path = format!("attachments/{}/{}", email_id, attachment.filename);
                    
                    // Insert into attachments table
                    sqlx::query(
                        r#"
                        INSERT INTO attachments (
                            email_id, filename, content_type, size, storage_path,
                            sender_email, sender_name, received_at, source_account
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                        "#
                    )
                    .bind(email_id)
                    .bind(&attachment.filename)
                    .bind(&attachment.content_type)
                    .bind(attachment.size as i64)
                    .bind(&storage_path)
                    .bind(&sender_email)
                    .bind(&sender_name)
                    .bind(message.date)
                    .bind("default")
                    .execute(pool)
                    .await?;
                    
                    log::info!("Saved attachment {} for email {}", attachment.filename, email_id);
                }
            }
        }
    }

    Ok(message_count)
}

/// Test CalDAV connection and fetch events
pub async fn test_caldav_connection(
    user: AuthenticatedUser,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("Testing CalDAV connection for user {}", user.user_id);
    log::info!("CalDAV URL from env: {:?}", std::env::var("CALDAV_URL").ok());

    // Check if CalDAV is configured
    let caldav_url = match std::env::var("CALDAV_URL") {
        Ok(url) => url,
        Err(_) => {
            return Ok(HttpResponse::Ok().json(CalDavTestResult {
                success: false,
                message: "CalDAV not configured".to_string(),
                event_count: None,
                error: Some("CALDAV_URL environment variable not set".to_string()),
            }));
        }
    };

    let username = std::env::var("CALDAV_USERNAME").unwrap_or_default();
    let password = std::env::var("CALDAV_PASSWORD").unwrap_or_default();

    if username.is_empty() || password.is_empty() {
        return Ok(HttpResponse::Ok().json(CalDavTestResult {
            success: false,
            message: "CalDAV credentials not configured".to_string(),
            event_count: None,
            error: Some("CALDAV_USERNAME or CALDAV_PASSWORD not set".to_string()),
        }));
    }

    let client = CalDavClient::new(caldav_url.clone(), username, password, None);

    log::info!("Attempting to fetch events from CalDAV server: {}", caldav_url);
    // Test connection and fetch events
    match client.fetch_events().await {
        Ok(events) => {
            log::info!("Successfully fetched {} events from CalDAV for user {}", events.len(), user.user_id);
            Ok(HttpResponse::Ok().json(CalDavTestResult {
                success: true,
                message: format!("Successfully connected to CalDAV server. Found {} events.", events.len()),
                event_count: Some(events.len()),
                error: None,
            }))
        }
        Err(e) => {
            log::error!("CalDAV test failed for user {}: {}", user.user_id, e);
            Ok(HttpResponse::Ok().json(CalDavTestResult {
                success: false,
                message: "Failed to connect to CalDAV server".to_string(),
                event_count: None,
                error: Some(format!("{}", e)),
            }))
        }
    }
}
