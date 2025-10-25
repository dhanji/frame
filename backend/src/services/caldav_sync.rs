use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use crate::services::caldav::CalDavClient;

pub struct CalDavSyncService {
    pool: SqlitePool,
    sync_interval: Duration,
}

impl CalDavSyncService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            sync_interval: Duration::from_secs(5 * 60), // Sync every 5 minutes
        }
    }

    /// Start the CalDAV sync service
    pub async fn start(self) {
        let mut interval = interval(self.sync_interval);
        
        log::info!("CalDAV sync service starting, will sync every {} seconds", self.sync_interval.as_secs());
        
        loop {
            interval.tick().await;
            
            log::info!("Starting CalDAV sync cycle");
            
            // Get all users with CalDAV configured
            match self.get_users_with_caldav().await {
                Ok(users) => {
                    for user in users {
                        if let Err(e) = self.sync_user_calendar(&user).await {
                            log::error!("Failed to sync calendar for user {}: {}", user.id, e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to fetch users for CalDAV sync: {}", e);
                }
            }
        }
    }

    /// Get users who have CalDAV configured
    async fn get_users_with_caldav(&self) -> Result<Vec<crate::models::User>, sqlx::Error> {
        // Check if users have caldav_url in their settings
        let users = sqlx::query_as::<_, crate::models::User>(
            "SELECT * FROM users WHERE is_active = TRUE AND settings LIKE '%caldav_url%'"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(users)
    }

    /// Sync calendar for a specific user
    async fn sync_user_calendar(&self, user: &crate::models::User) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Parse user settings to get CalDAV configuration
        let settings: serde_json::Value = serde_json::from_str(&user.settings)
            .unwrap_or_else(|_| serde_json::json!({}));
        
        let caldav_url = settings["caldav_url"].as_str();
        let caldav_username = settings["caldav_username"].as_str();
        let caldav_password = settings["caldav_password"].as_str();
        let caldav_calendar_path = settings["caldav_calendar_path"].as_str();
        
        if caldav_url.is_none() || caldav_username.is_none() || caldav_password.is_none() {
            log::debug!("User {} does not have complete CalDAV configuration, skipping", user.id);
            return Ok(());
        }
        
        let caldav_url = caldav_url.unwrap().to_string();
        let caldav_username = caldav_username.unwrap().to_string();
        let caldav_password = caldav_password.unwrap().to_string();
        let caldav_calendar_path = caldav_calendar_path.map(|s| s.to_string());
        
        log::info!("Syncing calendar for user {} with CalDAV server {}", user.id, caldav_url);
        
        // Create CalDAV client
        let client = CalDavClient::new(
            caldav_url,
            caldav_username,
            caldav_password,
            caldav_calendar_path,
        );
        
        // Test connection first
        if let Err(e) = client.test_connection().await {
            log::error!("CalDAV connection test failed for user {}: {}", user.id, e);
            return Err(e);
        }
        
        // Fetch events from CalDAV server
        let remote_events = match client.fetch_events().await {
            Ok(events) => {
                log::info!("Fetched {} events from CalDAV server for user {}", events.len(), user.id);
                events
            }
            Err(e) => {
                log::error!("Failed to fetch events from CalDAV for user {}: {}", user.id, e);
                return Err(e);
            }
        };
        
        // Sync remote events to local database
        for event in remote_events {
            // Check if event exists in database
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM calendar_events WHERE user_id = ? AND id = ?"
            )
            .bind(user.id)
            .bind(&event.uid)
            .fetch_one(&self.pool)
            .await?;
            
            if exists > 0 {
                // Update existing event
                sqlx::query(
                    r#"
                    UPDATE calendar_events
                    SET title = ?, description = ?, location = ?, start_time = ?, end_time = ?, 
                        all_day = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE user_id = ? AND id = ?
                    "#
                )
                .bind(&event.title)
                .bind(&event.description)
                .bind(&event.location)
                .bind(event.start_time.to_rfc3339())
                .bind(event.end_time.to_rfc3339())
                .bind(event.all_day)
                .bind(user.id)
                .bind(&event.uid)
                .execute(&self.pool)
                .await?;
                
                log::debug!("Updated event {} for user {}", event.uid, user.id);
            } else {
                // Insert new event
                sqlx::query(
                    r#"
                    INSERT INTO calendar_events 
                    (id, user_id, calendar_id, title, description, location, start_time, end_time, 
                     all_day, recurrence_rule, attendees, status, created_at, updated_at)
                    VALUES (?, ?, 'default', ?, ?, ?, ?, ?, ?, ?, ?, 'confirmed', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                    "#
                )
                .bind(&event.uid)
                .bind(user.id)
                .bind(&event.title)
                .bind(&event.description)
                .bind(&event.location)
                .bind(event.start_time.to_rfc3339())
                .bind(event.end_time.to_rfc3339())
                .bind(event.all_day)
                .bind(&event.recurrence)
                .bind(serde_json::to_string(&event.attendees).ok())
                .execute(&self.pool)
                .await?;
                
                log::debug!("Created event {} for user {}", event.uid, user.id);
            }
        }
        
        log::info!("Successfully synced calendar for user {}", user.id);
        Ok(())
    }
}
