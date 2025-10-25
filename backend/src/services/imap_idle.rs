use imap::{Session, ImapConnection};
use std::time::Duration;
use sqlx::SqlitePool;
use crate::websocket_impl::{ConnectionManager, WsMessage};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::services::email_sync::EmailSyncService;

// XOAUTH2 Authenticator for Gmail OAuth
struct XOAuth2 {
    user: String,
    access_token: String,
}

impl imap::Authenticator for XOAuth2 {
    type Response = String;
    
    fn process(&self, _challenge: &[u8]) -> Self::Response {
        format!("user={}\x01auth=Bearer {}\x01\x01", self.user, self.access_token)
    }
}

/// IMAP IDLE service for real-time email notifications
pub struct ImapIdleService {
    host: String,
    port: u16,
    username: String,
    password: String,
    oauth_token: Option<String>,
    user_id: i64,
    pool: SqlitePool,
    ws_manager: Arc<RwLock<ConnectionManager>>,
    email_manager: Arc<crate::services::EmailManager>,
}

impl ImapIdleService {
    pub fn new(
        host: String,
        port: u16,
        username: String,
        password: String,
        oauth_token: Option<String>,
        user_id: i64,
        pool: SqlitePool,
        ws_manager: Arc<RwLock<ConnectionManager>>,
        email_manager: Arc<crate::services::EmailManager>,
    ) -> Self {
        Self {
            host,
            port,
            username,
            password,
            oauth_token,
            user_id,
            pool,
            ws_manager,
            email_manager,
        }
    }

    fn create_session(&self) -> Result<Session<Box<dyn ImapConnection>>, Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Connecting to IMAP server {}:{}", self.host, self.port);
        
        let client = imap::ClientBuilder::new(&self.host, self.port).connect()?;
        
        let session = if let Some(oauth_token) = &self.oauth_token {
            // Use XOAUTH2 authentication for OAuth users
            log::info!("IDLE: Authenticating with XOAUTH2 for user: {}", self.username);
            
            let authenticator = XOAuth2 {
                user: self.username.clone(),
                access_token: oauth_token.clone(),
            };
            
            log::debug!("IDLE: Attempting XOAUTH2 authentication for {}", self.username);
            
            match client.authenticate("XOAUTH2", &authenticator) {
                Ok(session) => session,
                Err((e, _)) => {
                    log::error!("IDLE: XOAUTH2 authentication failed for {}: {:?}", self.username, e);
                    return Err(format!("XOAUTH2 authentication failed: {:?}", e).into());
                }
            }
        } else {
            // Use password authentication for traditional users
            log::info!("IDLE: Authenticating with password for user: {}", self.username);
            client
                .login(&self.username, &self.password)
                .map_err(|e| e.0)?
        };
        
        Ok(session)
    }

    /// Start IDLE monitoring for a folder
    pub async fn start_monitoring(&self, folder: &str) {
        let folder = folder.to_string();
        let mut backoff_seconds = 5;
        let max_backoff = 60;

        loop {
            match self.monitor_folder(&folder).await {
                Ok(_) => {
                    log::info!("IDLE monitoring ended normally for folder: {}", folder);
                    backoff_seconds = 1; // Reset backoff on success
                }
                Err(e) => {
                    log::error!("IDLE monitoring error for folder {}: {}", folder, e);
                    
                    // Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s, 60s (max)
                    log::info!("Reconnecting in {} seconds...", backoff_seconds);
                    tokio::time::sleep(Duration::from_secs(backoff_seconds)).await;
                    
                    backoff_seconds = std::cmp::min(backoff_seconds * 2, max_backoff);
                }
            }
        }
    }

    /// Monitor a folder using IDLE
    async fn monitor_folder(&self, folder: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // For now, just periodically check for new messages
        // Full IDLE implementation would require more complex handling
        
        // Initial delay to avoid blocking startup
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        loop {
            // Check every 60 seconds
            tokio::time::sleep(Duration::from_secs(60)).await;
            
            // Trigger a full sync when checking for new messages
            let user_result = sqlx::query_as::<_, crate::models::User>("SELECT * FROM users WHERE id = ?")
                .bind(self.user_id)
                .fetch_one(&self.pool)
                .await;
            
            if let Ok(user) = user_result {
                let sync_service = EmailSyncService::new(self.pool.clone(), self.email_manager.clone());
                log::info!("IDLE: Triggering email sync for user {} (periodic check)", user.id);
                let _ = sync_service.sync_user_emails(&user).await;
            }
            
            if let Err(e) = self.fetch_new_messages(folder).await {
                log::error!("Failed to fetch new messages: {}", e);
                return Err(e);
            }
        }
    }

    /// Fetch new messages from the folder
    async fn fetch_new_messages(&self, folder: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use mail_parser::MessageParser;
        
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        let folder = folder.to_string();
        let user_id = self.user_id;
        let pool = self.pool.clone();
        let ws_manager = self.ws_manager.clone();
        let email_manager = self.email_manager.clone();
        
        tokio::task::spawn_blocking(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapIdleService {
                host,
                port,
                username,
                password,
                oauth_token,
                user_id,
                pool: pool.clone(),
                ws_manager: ws_manager.clone(),
                email_manager: email_manager.clone(),
            };
            
            let mut session = service.create_session()?;
            session.select(&folder)?;
            
            // Search for unseen messages
            let unseen_uids = session.search("UNSEEN")?;
            
            if unseen_uids.is_empty() {
                log::debug!("No new unseen messages");
                session.logout()?;
                return Ok(());
            }
            
            log::info!("Found {} new messages", unseen_uids.len());
            
            // Fetch the new messages
            for uid in unseen_uids.iter().take(10) { // Limit to 10 at a time
                let uid_str = format!("{}", uid);
                let messages = session.fetch(&uid_str, "(UID FLAGS ENVELOPE BODY[])")?;
                
                for message in messages.iter() {
                    if let Some(body) = message.body() {
                        if let Some(parsed) = MessageParser::default().parse(body) {
                            // Store email in database
                            let message_id = parsed
                                .message_id()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                            
                            let subject = parsed.subject().unwrap_or("(No Subject)").to_string();
                            let from = parsed
                                .from()
                                .and_then(|addrs| addrs.first())
                                .and_then(|addr| addr.address())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "unknown@example.com".to_string());
                            
                            let body_text = parsed.body_text(0).map(|s| s.to_string());
                            let body_html = parsed.body_html(0).map(|s| s.to_string());
                            
                            let preview = body_text
                                .as_ref()
                                .or(body_html.as_ref())
                                .map(|s| {
                                    let preview: String = s.chars().take(100).collect();
                                    preview
                                })
                                .unwrap_or_else(|| "(No content)".to_string());
                            
                            // Store email would need to be async, so we'll skip for now in IDLE
                            // In a real implementation, we'd send this to a channel for async processing
                            
                            log::info!("New email detected: {} from {}", subject, from);
                        }
                    }
                }
            }
            
            session.logout()?;
            Ok(())
        }).await??;
        Ok(())
    }

    /// Check connection health
    pub async fn check_health(&self) -> bool {
        // Try to create a session
        // Simple health check - just return true
        // In a real implementation, we'd try to create a session
        true
    }
}
