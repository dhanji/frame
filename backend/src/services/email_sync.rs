use crate::models::{Email, User};
use crate::services::EmailManager;
use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::time;

/// Background service that periodically syncs emails from IMAP servers
pub struct EmailSyncService {
    pool: SqlitePool,
    email_manager: Arc<EmailManager>,
    sync_interval: Duration,
}

impl EmailSyncService {
    pub fn new(pool: SqlitePool, email_manager: Arc<EmailManager>) -> Self {
        Self {
            pool,
            email_manager,
            sync_interval: Duration::minutes(5), // Sync every 5 minutes
        }
    }

    /// Start the background sync service
    pub async fn start(self) {
        let mut interval = time::interval(time::Duration::from_secs(
            self.sync_interval.num_seconds() as u64,
        ));

        loop {
            interval.tick().await;
            
            log::info!("Starting email sync cycle");
            
            // Get all active users
            match self.get_active_users().await {
                Ok(users) => {
                    for user in users {
                        if let Err(e) = self.sync_user_emails(&user).await {
                            log::error!("Failed to sync emails for user {}: {}", user.id, e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to fetch active users: {}", e);
                }
            }
        }
    }

    /// Get all active users from the database
    async fn get_active_users(&self) -> Result<Vec<User>, sqlx::Error> {
        sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE is_active = true"
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Sync emails for a specific user
    async fn sync_user_emails(&self, user: &User) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Syncing emails for user {}", user.id);
        
        // Check if user's services are initialized
        if !self.email_manager.is_user_initialized(user.id).await {
            // Try to initialize
            if let Err(e) = self.email_manager.initialize_user(user).await {
                log::warn!("Could not initialize email services for user {}: {}", user.id, e);
                return Ok(()); // Skip this user
            }
        }
        
        // Get IMAP service
        let imap_service = self.email_manager.get_imap_service(user.id).await
            .ok_or("IMAP service not available")?;
        
        // Sync different folders
        let folders = vec!["INBOX", "Sent", "Drafts"];
        
        for folder in folders {
            match self.sync_folder(&imap_service, user.id, folder).await {
                Ok(count) => {
                    log::info!("Synced {} emails from {} for user {}", count, folder, user.id);
                }
                Err(e) => {
                    log::error!("Failed to sync {} for user {}: {}", folder, user.id, e);
                }
            }
        }
        
        Ok(())
    }

    /// Sync a specific folder for a user
    async fn sync_folder(
        &self,
        imap_service: &Arc<crate::services::ImapService>,
        user_id: i64,
        folder: &str,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        // Fetch messages from IMAP
        let messages = imap_service.fetch_messages(folder, 100).await?;
        let message_count = messages.len();
        
        // Store each message in the database
        for message in messages {
            // Check if message already exists
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM emails WHERE user_id = ? AND message_id = ?"
            )
            .bind(user_id)
            .bind(&message.message_id)
            .fetch_one(&self.pool)
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
                .execute(&self.pool)
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
                .bind(serde_json::to_string(&message.attachments).ok())
                .bind(folder)
                .bind(0i64) // size placeholder
                .bind(&message.in_reply_to)
                .bind(serde_json::to_string(&message.references).unwrap_or_default())
                .bind(Utc::now())
                .bind(Utc::now())
                .execute(&self.pool)
                .await?;
            }
        }
        
        Ok(message_count)
    }

    /// Clean up old deleted emails (older than 30 days)
    pub async fn cleanup_deleted_emails(&self) -> Result<(), sqlx::Error> {
        let thirty_days_ago = Utc::now() - Duration::days(30);
        
        sqlx::query(
            "DELETE FROM emails WHERE folder = 'Trash' AND deleted_at < ?"
        )
        .bind(thirty_days_ago)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
