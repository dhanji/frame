use imap::{Session, ImapConnection};
use chrono::{DateTime, Utc};
use mail_parser::MessageParser;
use mail_parser::MimeHeaders;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::services::OAuthRefreshService;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub uid: u32,
    pub message_id: String,
    pub thread_id: Option<String>,
    pub from: Vec<EmailAddress>,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub subject: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub date: DateTime<Utc>,
    pub flags: Vec<String>,
    pub attachments: Vec<Attachment>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAddress {
    pub name: Option<String>,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size: usize,
    pub content: Option<Vec<u8>>,
}

// XOAUTH2 Authenticator for Gmail OAuth
struct XOAuth2 {
    user: String,
    access_token: String,
}

impl imap::Authenticator for XOAuth2 {
    type Response = String;
    
    fn process(&self, _challenge: &[u8]) -> Self::Response {
        // XOAUTH2 format: user=<email>\x01auth=Bearer <token>\x01\x01
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

pub struct ImapService {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    oauth_token: Option<String>,
    user_id: Option<i64>,
}

impl ImapService {
    pub fn new(host: String, port: u16, username: String, password: Option<String>, oauth_token: Option<String>) -> Self {
        Self {
            host,
            port,
            username,
            password,
            oauth_token,
            user_id: None,
        }
    }

    pub fn new_with_user_id(host: String, port: u16, username: String, password: Option<String>, oauth_token: Option<String>, user_id: i64) -> Self {
        Self {
            host,
            port,
            username,
            password,
            oauth_token,
            user_id: Some(user_id),
        }
    }

    fn create_session(&self) -> Result<Session<Box<dyn ImapConnection>>, Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Connecting to IMAP server {}:{}", self.host, self.port);
        
        let client = imap::ClientBuilder::new(&self.host, self.port).connect()?;
        
        log::info!("TLS connection established to {}:{}", self.host, self.port);
        
        let session = if let Some(oauth_token) = &self.oauth_token {
            // Use XOAUTH2 authentication for OAuth users
            log::info!("Authenticating with XOAUTH2 for user: {}", self.username);
            log::info!("OAuth token length: {}", oauth_token.len());
            log::info!("OAuth token starts with: {}...", &oauth_token.chars().take(10).collect::<String>());
            
            let authenticator = XOAuth2 {
                user: self.username.clone(),
                access_token: oauth_token.clone(),
            };
            
            log::info!("Attempting XOAUTH2 authentication for {}", self.username);
            
            match client.authenticate("XOAUTH2", &authenticator) {
                Ok(session) => {
                    log::info!("✅ XOAUTH2 authentication successful for {}", self.username);
                    session
                }
                Err((e, _)) => {
                    log::error!("❌ XOAUTH2 authentication failed for {}: {:?}", self.username, e);
                    return Err(format!("XOAUTH2 authentication failed: {:?}", e).into());
                }
            }
        } else if let Some(password) = &self.password {
            // Use password authentication for traditional users
            log::info!("Authenticating with password for user: {}", self.username);
            client
                .login(&self.username, password)
                .map_err(|e| e.0)?
        } else {
            return Err("No authentication method available (neither OAuth token nor password)".into());
        };
        
        log::info!("IMAP authentication successful for {}", self.username);
        Ok(session)
    }

    /// Create session with automatic OAuth token refresh on auth failure
    async fn create_session_with_refresh(
        &self,
        pool: Option<&SqlitePool>,
    ) -> Result<Session<Box<dyn ImapConnection>>, Box<dyn std::error::Error + Send + Sync>> {
        // Try to create session with current token
        match self.create_session() {
            Ok(session) => Ok(session),
            Err(e) => {
                // Check if it's an authentication error and we have OAuth
                let error_msg = format!("{:?}", e);
                if error_msg.contains("AUTHENTICATIONFAILED") && self.oauth_token.is_some() && self.user_id.is_some() {
                    log::warn!("Authentication failed for user {:?}, attempting token refresh", self.user_id);
                    
                    if let Some(pool) = pool {
                        if let Some(user_id) = self.user_id {
                            // Try to refresh the token
                            let refresh_service = OAuthRefreshService::new();
                            match refresh_service.refresh_token(pool, user_id).await {
                                Ok(new_token) => {
                                    log::info!("Token refreshed, retrying IMAP connection");
                                    // Create a new service with the refreshed token
                                    let refreshed_service = ImapService::new_with_user_id(
                                        self.host.clone(),
                                        self.port,
                                        self.username.clone(),
                                        self.password.clone(),
                                        Some(new_token),
                                        user_id,
                                    );
                                    return refreshed_service.create_session();
                                }
                                Err(refresh_err) => {
                                    log::error!("Failed to refresh token: {}", refresh_err);
                                }
                            }
                        }
                    }
                }
                Err(e)
            }
        }
    }

    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        let user_id = self.user_id;
        
        // Run blocking IMAP operations in a blocking task
        tokio::task::spawn_blocking(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapService::new_with_user_id(host, port, username, password, oauth_token, user_id.unwrap_or(0));
            let mut session = service.create_session()?;
            // Test connection by selecting INBOX
            session.select("INBOX")?;
            session.logout()?;
            Ok(())
        }).await??;
        Ok(())
    }

    pub async fn fetch_messages(
        &self,
        folder: &str,
        limit: u32,
    ) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error + Send + Sync>> {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        let folder = folder.to_string();
        
        tokio::task::spawn_blocking(move || -> Result<Vec<EmailMessage>, Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapService::new(host, port, username, password, oauth_token);
            let mut session = service.create_session()?;
            
            session.select(&folder)?;
            
            // Get the total number of messages in the folder
            let mailbox = session.examine(&folder)?;
            let total_messages = mailbox.exists;
            
            log::info!("Folder {} has {} total messages", folder, total_messages);
            
            // Fetch newest messages first (highest UIDs)
            let sequence_set = if total_messages > limit {
                // Fetch the last 'limit' messages
                let start = total_messages - limit + 1;
                format!("{}:*", start)
            } else {
                // Fetch all messages
                "1:*".to_string()
            };
            
            log::info!("Fetching messages with sequence: {}", sequence_set);
            
            let messages = session.fetch(sequence_set, "(UID FLAGS ENVELOPE BODY[] BODYSTRUCTURE)")?;
            
            let mut email_messages = Vec::new();
            
            for message in messages.iter() {
                if let Some(body) = message.body() {
                    let parsed = MessageParser::default().parse(body);
                    
                    if let Some(parsed_msg) = parsed {
                        let email = EmailMessage {
                            id: uuid::Uuid::new_v4().to_string(),
                            uid: message.uid.unwrap_or(0),
                            message_id: parsed_msg
                                .message_id()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                            thread_id: None,
                            from: parsed_msg
                                .from()
                                .map(|addrs| {
                                    addrs.iter()
                                        .filter_map(|addr| {
                                            addr.address().map(|email| EmailAddress {
                                                name: addr.name().map(|n| n.to_string()),
                                                email: email.to_string(),
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            to: parsed_msg
                                .to()
                                .map(|addrs| {
                                    addrs.iter()
                                        .filter_map(|addr| {
                                            addr.address().map(|email| EmailAddress {
                                                name: addr.name().map(|n| n.to_string()),
                                                email: email.to_string(),
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            cc: parsed_msg
                                .cc()
                                .map(|addrs| {
                                    addrs.iter()
                                        .filter_map(|addr| {
                                            addr.address().map(|email| EmailAddress {
                                                name: addr.name().map(|n| n.to_string()),
                                                email: email.to_string(),
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            bcc: vec![], // BCC is typically not available in received emails
                            subject: parsed_msg.subject().unwrap_or("").to_string(),
                            body_text: parsed_msg.body_text(0).map(|s| s.to_string()),
                            body_html: parsed_msg.body_html(0).map(|s| s.to_string()),
                            date: parsed_msg
                                .date()
                                .and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0))
                                .unwrap_or_else(Utc::now),
                            flags: message.flags()
                                .iter()
                                .map(|f| format!("{:?}", f))
                                .collect(),
                            attachments: {
                                let mut attachments = Vec::new();
                                
                                // Parse attachments from the message
                                for part in parsed_msg.attachments() {
                                    if let Some(filename) = part.attachment_name() {
                                        let content_type = part
                                            .content_type()
                                            .map(|ct| ct.c_type.to_string())
                                            .unwrap_or_else(|| "application/octet-stream".to_string());
                                        
                                        let content = part.contents().to_vec();
                                        let size = content.len();
                                        
                                        // Generate a unique ID for the attachment
                                        let attachment_id = format!(
                                            "{}-{}",
                                            parsed_msg
                                                .message_id()
                                                .map(|id| id.to_string())
                                                .unwrap_or_else(|| "unknown".to_string()),
                                            filename
                                        );
                                        
                                        attachments.push(Attachment {
                                            id: attachment_id,
                                            filename: filename.to_string(),
                                            content_type: content_type.clone(),
                                            size,
                                            content: Some(content),
                                        });
                                        
                                        log::info!(
                                            "Found attachment: {} ({} bytes, {})",
                                            filename,
                                            size,
                                            content_type
                                        );
                                    }
                                }
                                attachments
                            },
                            in_reply_to: None,
                            references: vec![],
                        };
                        email_messages.push(email);
                    }
                }
            }
            
            // Reverse to get newest first
            email_messages.reverse();
            
            log::info!("Fetched {} messages from folder {}", email_messages.len(), folder);
            session.logout()?;
            Ok(email_messages)
        }).await?
    }

    pub async fn mark_as_read(
        &self,
        uid: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        
        tokio::task::spawn_blocking(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapService::new(host, port, username, password, oauth_token);
            let mut session = service.create_session()?;
            session.store(format!("{}", uid), "+FLAGS (\\Seen)")?;
            session.logout()?;
            Ok(())
        }).await?
    }

    pub async fn mark_as_unread(
        &self,
        uid: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        
        tokio::task::spawn_blocking(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapService::new(host, port, username, password, oauth_token);
            let mut session = service.create_session()?;
            session.store(format!("{}", uid), "-FLAGS (\\Seen)")?;
            session.logout()?;
            Ok(())
        }).await?
    }

    pub async fn delete_message(
        &self,
        uid: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        
        tokio::task::spawn_blocking(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapService::new(host, port, username, password, oauth_token);
            let mut session = service.create_session()?;
            session.store(format!("{}", uid), "+FLAGS (\\Deleted)")?;
            session.expunge()?;
            session.logout()?;
            Ok(())
        }).await?
    }

    pub async fn move_message(
        &self,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        let target_folder = target_folder.to_string();
        
        tokio::task::spawn_blocking(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapService::new(host, port, username, password, oauth_token);
            let mut session = service.create_session()?;
            session.mv(format!("{}", uid), &target_folder)?;
            session.logout()?;
            Ok(())
        }).await?
    }

    pub async fn create_folder(
        &self,
        folder_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        let folder_name = folder_name.to_string();
        
        tokio::task::spawn_blocking(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapService::new(host, port, username, password, oauth_token);
            let mut session = service.create_session()?;
            session.create(&folder_name)?;
            session.logout()?;
            Ok(())
        }).await?
    }

    pub async fn list_folders(
        &self,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        
        tokio::task::spawn_blocking(move || -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapService::new(host, port, username, password, oauth_token);
            let mut session = service.create_session()?;
            
            let folders = session.list(None, Some("*"))?;
            let folder_names: Vec<String> = folders
                .iter()
                .map(|f| f.name().to_string())
                .collect();
            
            session.logout()?;
            Ok(folder_names)
        }).await?
    }

    pub async fn search(
        &self,
        query: &str,
    ) -> Result<Vec<u32>, Box<dyn std::error::Error + Send + Sync>> {
        let host = self.host.clone();
        let port = self.port;
        let username = self.username.clone();
        let password = self.password.clone();
        let oauth_token = self.oauth_token.clone();
        let query = query.to_string();
        
        tokio::task::spawn_blocking(move || -> Result<Vec<u32>, Box<dyn std::error::Error + Send + Sync>> {
            let service = ImapService::new(host, port, username, password, oauth_token);
            let mut session = service.create_session()?;
            
            let uids = session.search(&query)?;
            let uid_vec: Vec<u32> = uids.into_iter().collect();
            
            session.logout()?;
            Ok(uid_vec)
        }).await?
    }
}
