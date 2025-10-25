use async_imap::{Session, Authenticator};
use base64::{Engine as _, engine::general_purpose};
use async_native_tls::{TlsConnector, TlsStream};
use async_std::net::TcpStream;
use chrono::{DateTime, Utc};
use mail_parser::{MessageParser, MimeHeaders};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use futures::TryStreamExt;
use tokio::sync::RwLock;

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

impl Authenticator for &XOAuth2 {
    type Response = String;
    
    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        // XOAUTH2 format: user=<email>\x01auth=Bearer <token>\x01\x01
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

pub struct ImapService {
    session: Arc<RwLock<Option<Session<TlsStream<TcpStream>>>>>,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    oauth_token: Option<String>,
}

impl ImapService {
    pub fn new(host: String, port: u16, username: String, password: Option<String>, oauth_token: Option<String>) -> Self {
        Self {
            session: Arc::new(RwLock::new(None)),
            host,
            port,
            username,
            password,
            oauth_token,
        }
    }

    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tcp_stream = TcpStream::connect((self.host.as_str(), self.port)).await?;
        log::info!("TCP connection established to {}:{}", self.host, self.port);
        let tls = TlsConnector::new();
        let tls_stream = tls.connect(&self.host, tcp_stream).await?;
        
        let client = async_imap::Client::new(tls_stream);
        
        let session = if let Some(oauth_token) = &self.oauth_token {
            // Use XOAUTH2 authentication for OAuth users
            log::info!("Authenticating with XOAUTH2 for user: {}", self.username);
            log::info!("OAuth token length: {}", oauth_token.len());
            log::info!("OAuth token starts with: {}...", &oauth_token.chars().take(10).collect::<String>());
            log::info!("OAuth token ends with: ...{}", &oauth_token.chars().rev().take(10).collect::<String>().chars().rev().collect::<String>());
            
            let authenticator = XOAuth2 {
                user: self.username.clone(),
                access_token: oauth_token.clone(),
            };
            
            log::info!("Attempting XOAUTH2 authentication for {}", self.username);
            log::debug!("XOAUTH2 auth string format: user={}\\x01auth=Bearer <token>\\x01\\x01", self.username);
            
            let auth_future = client.authenticate("XOAUTH2", &authenticator);
            log::info!("XOAUTH2 authenticate() called, waiting for response...");
            
            // Add timeout to see if it's hanging
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                auth_future
            ).await;
            
            match result {
                Ok(Ok(session)) => {
                    log::info!("XOAUTH2 authentication successful for {}", self.username);
                    session
                }
                Ok(Err((e, _))) => {
                    log::error!("XOAUTH2 authentication failed for {}: {:?}", self.username, e);
                    return Err(format!("XOAUTH2 authentication failed: {:?}", e).into());
                }
                Err(_) => {
                    log::error!("XOAUTH2 authentication timed out after 15 seconds for {}", self.username);
                    return Err("XOAUTH2 authentication timed out".into());
                }
            }
        } else if let Some(password) = &self.password {
            // Use password authentication for traditional users
            log::info!("Authenticating with password for user: {}", self.username);
            client
                .login(&self.username, password)
                .await
                .map_err(|e| e.0)?
        } else {
            return Err("No authentication method available (neither OAuth token nor password)".into());
        };
        
        *self.session.write().await = Some(session);
        Ok(())
    }

    pub async fn fetch_messages(
        &self,
        folder: &str,
        limit: u32,
    ) -> Result<Vec<EmailMessage>, Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        
        if session_guard.is_none() {
            drop(session_guard);
            self.connect().await?;
            session_guard = self.session.write().await;
        }
        
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        session.select(folder).await?;
        
        let sequence_set = format!("1:{}", limit);
        let fetch_stream = session
            .fetch(sequence_set, "(UID FLAGS ENVELOPE BODY[] BODYSTRUCTURE)")
            .await?;
        
        // Collect messages from stream
        let messages: Vec<_> = fetch_stream.try_collect::<Vec<_>>().await?;
        
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
                        thread_id: None, // Simplified - threading not fully implemented
                        from: vec![], // Simplified - use empty for now
                        to: vec![], // Simplified - use empty for now
                        cc: vec![], // Simplified - use empty for now
                        bcc: vec![], // Simplified - use empty for now
                        subject: parsed_msg.subject().unwrap_or("").to_string(),
                        body_text: parsed_msg.body_text(0).map(|s| s.to_string()),
                        body_html: parsed_msg.body_html(0).map(|s| s.to_string()),
                        date: parsed_msg
                            .date()
                            .and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0))
                            .unwrap_or_else(Utc::now),
                        flags: message
                            .flags()
                            .map(|f| format!("{:?}", f))
                            .collect(),
                        attachments: vec![], // Simplified for now
                        in_reply_to: None, // Simplified - threading not fully implemented
                        references: vec![], // Simplified for now
                    };
                    email_messages.push(email);
                }
            }
        }
        
        Ok(email_messages)
    }

    pub async fn mark_as_read(
        &self,
        uid: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        session.store(format!("{}", uid), "+FLAGS (\\Seen)").await?;
        Ok(())
    }

    pub async fn mark_as_unread(
        &self,
        uid: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        session.store(format!("{}", uid), "-FLAGS (\\Seen)").await?;
        Ok(())
    }

    pub async fn delete_message(
        &self,
        uid: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        session.store(format!("{}", uid), "+FLAGS (\\Deleted)").await?;
        session.expunge().await?;
        Ok(())
    }

    pub async fn move_message(
        &self,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        session.mv(format!("{}", uid), target_folder).await?;
        Ok(())
    }

    pub async fn create_folder(
        &self,
        folder_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        session.create(folder_name).await?;
        Ok(())
    }

    pub async fn list_folders(
        &self,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        let folders_stream = session.list(None, Some("*")).await?;
        let folders: Vec<_> = futures::TryStreamExt::try_collect(folders_stream).await?;
        
        Ok(folders
            .iter()
            .map(|f| f.name().to_string())
            .collect())
    }

    pub async fn search(
        &self,
        query: &str,
    ) -> Result<Vec<u32>, Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or("No IMAP session")?;
        
        let uids = session.search(query).await?;
        Ok(uids.into_iter().collect())
    }
}

fn parse_header_addresses(header: &mail_parser::HeaderValue) -> Option<Vec<EmailAddress>> {
    let mut addresses = Vec::new();
    
    // Parse addresses from header
    match header {
        mail_parser::HeaderValue::Text(text) => {
            addresses.push(EmailAddress {
                name: None,
                email: text.to_string(),
            });
        }
        mail_parser::HeaderValue::TextList(text_list) => {
            for text in text_list {
                addresses.push(EmailAddress {
                    name: None,
                    email: text.to_string(),
                });
            }
        }
        _ => {}
    }
    
    if addresses.is_empty() { None } else { Some(addresses) }
}

fn parse_attachments(message: &mail_parser::Message) -> Vec<Attachment> {
    let mut attachments = Vec::new();
    
    for i in 0..message.attachment_count() {
        if let Some(part) = message.attachment(i) {
            let content_type = if let Some(_ct) = part.content_type() {
                // Use default content type if not available
                "application/octet-stream".to_string()
                // TODO: Parse actual content type from headers
            } else {
                "application/octet-stream".to_string()
            };
            
            attachments.push(Attachment {
                id: uuid::Uuid::new_v4().to_string(),
                filename: part
                    .attachment_name()
                    .unwrap_or("attachment")
                    .to_string(),
                content_type,
                size: part.contents().len(),
                content: Some(part.contents().to_vec()),
            });
        }
    }
    
    attachments
}