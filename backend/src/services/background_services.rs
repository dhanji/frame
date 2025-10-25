use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::services::imap_idle::ImapIdleService;
use crate::websocket_impl::ConnectionManager;
use crate::utils::encryption::Encryption;
use crate::services::EmailManager;

/// Background service manager for IMAP IDLE monitoring
pub struct BackgroundServiceManager {
    pool: SqlitePool,
    ws_manager: Arc<RwLock<ConnectionManager>>,
    email_manager: Arc<EmailManager>,
}

impl BackgroundServiceManager {
    pub fn new(pool: SqlitePool, ws_manager: Arc<RwLock<ConnectionManager>>, email_manager: Arc<EmailManager>) -> Self {
        Self { pool, ws_manager, email_manager }
    }

    /// Start IMAP IDLE monitoring for a user
    pub async fn start_imap_idle_for_user(
        &self,
        user_id: i64,
        imap_host: String,
        imap_port: u16,
        username: String,
        password: String,
        oauth_token: Option<String>,
        email_manager: Arc<EmailManager>,
    ) {
        let pool = self.pool.clone();
        let ws_manager = self.ws_manager.clone();

        tokio::spawn(async move {
            log::info!("Starting IMAP IDLE monitoring for user {}", user_id);

            let idle_service = ImapIdleService::new(
                imap_host,
                imap_port,
                username,
                password,
                oauth_token,
                user_id,
                pool,
                ws_manager,
                email_manager,
            );

            // Monitor INBOX folder
            idle_service.start_monitoring("INBOX").await;
        });
    }

    /// Start IMAP IDLE monitoring for all active users
    pub async fn start_all_imap_idle_monitors(&self) {
        log::info!("Starting IMAP IDLE monitors for all active users");
        
        let encryption = Encryption::new();

        // Fetch all active users from database
        let users = match sqlx::query!(
            r#"
            SELECT id, email, email_password, oauth_provider, oauth_access_token, imap_host, imap_port
            FROM users
            WHERE is_active = TRUE
            "#
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(users) => users,
            Err(e) => {
                log::error!("Failed to fetch users for IMAP IDLE: {}", e);
                return;
            }
        };

        for user in users {
            // Determine authentication method
            let (password, oauth_token) = if user.oauth_provider.is_some() {
                // OAuth user - decrypt access token
                log::info!("User {} is OAuth user for IDLE monitoring", user.id);
                let oauth_token = user.oauth_access_token.as_ref()
                    .and_then(|token| {
                        match encryption.decrypt(token) {
                            Ok(t) => {
                                log::info!("Successfully decrypted OAuth token for IDLE user {}", user.id);
                                Some(t)
                            }
                            Err(e) => {
                                log::error!("Failed to decrypt OAuth token for IDLE user {}: {}", user.id, e);
                                None
                            }
                        }
                    });
                (String::new(), oauth_token) // Empty password for OAuth users
            } else {
                // Password user - decrypt email password
                log::info!("User {} is password user for IDLE monitoring", user.id);
                let password = user.email_password.as_ref()
                    .and_then(|p| encryption.decrypt(p).ok())
                    .unwrap_or_default();
                (password, None)
            };
            
            // Only start IDLE if we have credentials
            if !password.is_empty() || oauth_token.is_some() {
                log::info!("Starting IDLE monitoring for user {} with {} auth", 
                    user.id, if oauth_token.is_some() { "OAuth" } else { "password" });

                self.start_imap_idle_for_user(
                    user.id,
                    user.imap_host,
                    user.imap_port as u16,
                    user.email.clone(),
                    password,
                    oauth_token,
                    self.email_manager.clone(),
                )
                .await;
            } else {
                log::warn!("Skipping IDLE monitoring for user {} - no credentials available", user.id);
            }
        }
    }

    /// Start attachment cleanup job (runs daily)
    pub async fn start_attachment_cleanup_job(&self) {
        let pool = self.pool.clone();

        tokio::spawn(async move {
            log::info!("Starting attachment cleanup job");

            loop {
                // Wait 24 hours
                tokio::time::sleep(tokio::time::Duration::from_secs(24 * 60 * 60)).await;

                log::info!("Running attachment cleanup job");

                // Find and delete orphaned attachments
                match sqlx::query!(
                    r#"
                    SELECT id, path, storage_path
                    FROM attachments
                    WHERE email_id IS NULL 
                    AND draft_id IS NULL
                    AND created_at < datetime('now', '-1 day')
                    "#
                )
                .fetch_all(&pool)
                .await
                {
                    Ok(orphaned) => {
                        let mut deleted_count = 0;

                        for attachment in orphaned {
                            // Delete files
                            if let Some(path) = &attachment.path {
                                let _ = std::fs::remove_file(path);
                            } else {
                                let _ = std::fs::remove_file(&attachment.storage_path);
                            }

                            // Delete from database
                            let attachment_id = attachment.id.unwrap_or(0);
                            if let Err(e) = sqlx::query!("DELETE FROM attachments WHERE id = ?", attachment_id)
                                .execute(&pool)
                                .await
                            {
                                log::error!("Failed to delete attachment {:?}: {}", attachment.id, e);
                            } else {
                                deleted_count += 1;
                            }
                        }

                        if deleted_count > 0 {
                            log::info!("Cleaned up {} orphaned attachments", deleted_count);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to fetch orphaned attachments: {}", e);
                    }
                }
            }
        });
    }
}
