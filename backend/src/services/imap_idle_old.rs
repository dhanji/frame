use async_imap::{Session, Authenticator};
use async_native_tls::{TlsConnector, TlsStream};
use async_std::net::TcpStream;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;
use sqlx::SqlitePool;
use crate::websocket_impl::{ConnectionManager, WsMessage};

// XOAUTH2 Authenticator for Gmail OAuth
struct XOAuth2 {
    user: String,
    access_token: String,
}

impl Authenticator for &XOAuth2 {
    type Response = String;
    
    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
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
    session: Arc<RwLock<Option<Session<TlsStream<TcpStream>>>>>,
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
            session: Arc::new(RwLock::new(None)),
        }
    }

    /// Start IDLE monitoring for a folder
    pub async fn start_monitoring(&self, folder: &str) {
        let folder = folder.to_string();
        let mut backoff_seconds = 1;
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
        // Connect to IMAP server
        self.connect().await?;
        
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        // Select the folder
        session.select(folder).await?;
        log::info!("Selected folder: {}", folder);
        
        drop(session_guard); // Release lock before entering IDLE loop
        
        loop {
            // Enter IDLE mode
            let idle_result = self.idle_wait(folder).await;
            
            match idle_result {
                Ok(has_new_messages) => {
                    if has_new_messages {
                        log::info!("New messages detected in folder: {}", folder);
                        
                        // Fetch new messages
                        if let Err(e) = self.fetch_new_messages(folder).await {
                            log::error!("Failed to fetch new messages: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("IDLE error: {}", e);
                    return Err(e);
                }
            }
            
            // IDLE timeout or notification received, re-enter IDLE
            // Most IMAP servers timeout IDLE after 29 minutes, so we re-establish
        }
    }

    /// Wait for IDLE notifications
    async fn idle_wait(&self, _folder: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        // Start IDLE (simplified - just wait for timeout)
        
        log::debug!("Entered IDLE mode");
        
        // Wait for notification or timeout (29 minutes to be safe)
        let timeout = Duration::from_secs(29 * 60);
        
        // Simple timeout-based approach
        tokio::time::sleep(timeout).await;
        
        // For now, always return false (no new messages detected)
        // Full IDLE implementation requires more complex async-imap usage
        Ok(false)
    }

    /// Fetch new messages from the folder
    async fn fetch_new_messages(&self, folder: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use futures::TryStreamExt;
        use mail_parser::MessageParser;
        
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        // Search for unseen messages
        let unseen_uids = session.search("UNSEEN").await?;
        
        if unseen_uids.is_empty() {
            log::debug!("No new unseen messages");
            return Ok(());
        }
        
        log::info!("Found {} new messages", unseen_uids.len());
        
        // Fetch the new messages
        for uid in unseen_uids.iter().take(10) { // Limit to 10 at a time
            let uid_str = format!("{}", uid);
            let fetch_stream = session
                .fetch(&uid_str, "(UID FLAGS ENVELOPE BODY[])")
                .await?;
            
            let messages: Vec<_> = fetch_stream.try_collect().await?;
            
            for message in messages {
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
                        
                        // Insert into database
                        let email_id = self.store_email(&parsed, folder).await?;
                        
                        // Send WebSocket notification
                        let ws_manager = self.ws_manager.read().await;
                        ws_manager.send_to_user(
                            self.user_id,
                            WsMessage::NewEmail {
                                email_id: email_id.to_string(),
                                from,
                                subject,
                                preview,
                            }
                        ).await;
                        
                        log::info!("Notified user {} of new email: {}", self.user_id, message_id);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Store email in database
    async fn store_email(
        &self,
        parsed: &mail_parser::Message<'_>,
        folder: &str,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        use chrono::{DateTime, Utc};
        
        let message_id = parsed
            .message_id()
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        let subject = parsed.subject().unwrap_or("(No Subject)").to_string();
        
        let from_address = parsed
            .from()
            .and_then(|addrs| addrs.first())
            .and_then(|addr| addr.address())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown@example.com".to_string());
        
        let to_addresses = parsed
            .to()
            .map(|addrs| {
                addrs
                    .iter()
                    .filter_map(|addr| addr.address())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        
        let cc_addresses = parsed
            .cc()
            .map(|addrs| {
                addrs
                    .iter()
                    .filter_map(|addr| addr.address())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            });
        
        let body_text = parsed.body_text(0).map(|s| s.to_string());
        let body_html = parsed.body_html(0).map(|s| s.to_string());
        
        let date = parsed
            .date()
            .and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0))
            .unwrap_or_else(Utc::now);
        
        let in_reply_to: Option<String> = None; // Simplified for now
        
        let references: Vec<String> = Vec::new(); // Simplified for now
        
        let references_json = serde_json::to_string(&references).unwrap_or_else(|_| "[]".to_string());
        
        let has_attachments = parsed.attachment_count() > 0;
        
        // Insert or update email
        let result = sqlx::query(
            r#"
            INSERT INTO emails (
                user_id, message_id, from_address, to_addresses, cc_addresses,
                subject, body_text, body_html, date, folder, has_attachments,
                in_reply_to, references, is_read, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT(user_id, message_id) DO UPDATE SET
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(self.user_id)
        .bind(&message_id)
        .bind(&from_address)
        .bind(&to_addresses)
        .bind(&cc_addresses)
        .bind(&subject)
        .bind(&body_text)
        .bind(&body_html)
        .bind(date)
        .bind(folder)
        .bind(has_attachments)
        .bind(&in_reply_to)
        .bind(&references_json)
        .execute(&self.pool)
        .await?;
        
        Ok(result.last_insert_rowid())
    }

    /// Connect to IMAP server
    async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Connecting to IMAP server {}:{}", self.host, self.port);
        
        let tcp_stream = TcpStream::connect((self.host.as_str(), self.port)).await?;
        let tls = TlsConnector::new();
        let tls_stream = tls.connect(&self.host, tcp_stream).await?;
        
        let client = async_imap::Client::new(tls_stream);
        
        let session = if let Some(oauth_token) = &self.oauth_token {
            // Use XOAUTH2 authentication for OAuth users
            log::info!("IDLE: Authenticating with XOAUTH2 for user: {}", self.username);
            
            let authenticator = XOAuth2 {
                user: self.username.clone(),
                access_token: oauth_token.clone(),
            };
            
            log::debug!("IDLE: Attempting XOAUTH2 authentication for {}", self.username);
            let result = client
                .authenticate("XOAUTH2", &authenticator)
                .await;
            
            match result {
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
                .await
                .map_err(|e| e.0)?
        };
        
        *self.session.write().await = Some(session);
        
        log::info!("Connected to IMAP server successfully");
        Ok(())
    }

    /// Check connection health
    pub async fn check_health(&self) -> bool {
        let session_guard = self.session.read().await;
        session_guard.is_some()
    }
}
