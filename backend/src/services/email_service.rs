use async_imap::{Client as ImapClient, Session};
use async_native_tls::{TlsConnector, TlsStream};
use async_smtp::{
    ClientSecurity, Envelope, SendableEmail, SmtpClient, SmtpTransport,
    Transport, authentication::{Credentials, Mechanism}
};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use lettre::{
    message::{header, Mailbox, Message, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials as LettreCredentials,
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};
use mail_parser::{HeaderValue, MimeHeaders, PartType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_std::net::TcpStream;

use crate::error::AppError;
use crate::models::{Email, EmailAccount, Conversation};

#[derive(Debug, Clone)]
pub struct EmailService {
    imap_sessions: Arc<RwLock<HashMap<String, ImapSession>>>,
    smtp_transports: Arc<RwLock<HashMap<String, SmtpTransport>>>,
}

struct ImapSession {
    session: Session<TlsStream<TcpStream>>,
    last_used: DateTime<Utc>,
}

struct SmtpTransport {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    last_used: DateTime<Utc>,
}

impl EmailService {
    pub fn new() -> Self {
        Self {
            imap_sessions: Arc::new(RwLock::new(HashMap::new())),
            smtp_transports: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Connect to IMAP server and authenticate
    pub async fn connect_imap(&self, account: &EmailAccount) -> Result<(), AppError> {
        let tls = TlsConnector::new();
        let client = async_imap::connect(
            (account.imap_host.as_str(), account.imap_port),
            account.imap_host.clone(),
            tls,
        ).await.map_err(|e| AppError::ImapError(e.to_string()))?;

        let mut session = client
            .login(&account.email, &account.password)
            .await
            .map_err(|e| AppError::ImapError(format!("Login failed: {}", e.0)))?;

        // Store session for reuse
        let mut sessions = self.imap_sessions.write().await;
        sessions.insert(
            account.id.clone(),
            ImapSession {
                session,
                last_used: Utc::now(),
            },
        );

        Ok(())
    }

    /// Fetch emails from a specific folder
    pub async fn fetch_emails(
        &self,
        account: &EmailAccount,
        folder: &str,
        limit: u32,
    ) -> Result<Vec<Email>, AppError> {
        let mut sessions = self.imap_sessions.write().await;
        let imap_session = sessions
            .get_mut(&account.id)
            .ok_or_else(|| AppError::ImapError("Not connected".to_string()))?;

        // Select folder
        imap_session.session
            .select(folder)
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;

        // Fetch recent messages
        let sequence_set = format!("*:*-{}", limit - 1);
        let messages = imap_session.session
            .fetch(sequence_set, "(UID FLAGS ENVELOPE BODY[])")
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;

        let mut emails = Vec::new();
        
        let mut stream = messages;
        while let Some(fetch) = stream.try_next().await.map_err(|e| AppError::ImapError(e.to_string()))? {
            let email = self.parse_fetch_to_email(fetch, folder, &account.id)?;
            emails.push(email);
        }

        // Update last used time
        imap_session.last_used = Utc::now();

        Ok(emails)
    }

    /// Parse IMAP fetch response to Email model
    fn parse_fetch_to_email(
        &self,
        fetch: async_imap::types::Fetch,
        folder: &str,
        account_id: &str,
    ) -> Result<Email, AppError> {
        let uid = fetch.uid
            .ok_or_else(|| AppError::ImapError("Missing UID".to_string()))?;
        
        let envelope = fetch.envelope()
            .ok_or_else(|| AppError::ImapError("Missing envelope".to_string()))?;
        
        let flags = fetch.flags();
        let is_read = flags.iter().any(|f| f == &async_imap::types::Flag::Seen);
        let is_starred = flags.iter().any(|f| f == &async_imap::types::Flag::Flagged);
        
        // Parse email body
        let body = fetch.body()
            .ok_or_else(|| AppError::ImapError("Missing body".to_string()))?;
        
        let parsed = mail_parser::MessageParser::default()
            .parse(body)
            .ok_or_else(|| AppError::ImapError("Failed to parse email".to_string()))?;
        
        let (body_text, body_html) = self.extract_body_content(&parsed);
        
        // Extract addresses
        let from = envelope.from.as_ref()
            .and_then(|addrs| addrs.first())
            .map(|addr| self.format_address(addr))
            .unwrap_or_default();
        
        let to = envelope.to.as_ref()
            .map(|addrs| addrs.iter().map(|a| self.format_address(a)).collect())
            .unwrap_or_default();
        
        let cc = envelope.cc.as_ref()
            .map(|addrs| addrs.iter().map(|a| self.format_address(a)).collect());
        
        let subject = envelope.subject
            .as_ref()
            .map(|s| String::from_utf8_lossy(s).to_string())
            .unwrap_or_else(|| "(No Subject)".to_string());
        
        let date = envelope.date
            .as_ref()
            .and_then(|d| chrono::DateTime::parse_from_rfc2822(&String::from_utf8_lossy(d)).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        
        // Extract message ID for threading
        let message_id = parsed.message_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("{}@local", uid));
        
        let in_reply_to = parsed.in_reply_to()
            .and_then(|id| id.first())
            .map(|id| id.to_string());
        
        let references = parsed.references()
            .map(|refs| refs.iter().map(|r| r.to_string()).collect());
        
        Ok(Email {
            id: format!("{}-{}", account_id, uid),
            account_id: account_id.to_string(),
            folder_id: folder.to_string(),
            uid: uid as i64,
            message_id,
            thread_id: None, // Will be computed later
            in_reply_to,
            references,
            from_address: from.clone(),
            from_name: self.extract_name(&from),
            to,
            cc,
            bcc: None,
            subject,
            body_text,
            body_html,
            is_read,
            is_starred,
            is_draft: false,
            has_attachments: parsed.attachments().count() > 0,
            attachments: self.extract_attachments(&parsed),
            date,
            size: body.len() as i64,
            labels: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Extract text and HTML body from parsed email
    fn extract_body_content(&self, parsed: &mail_parser::Message) -> (String, Option<String>) {
        let mut text_body = String::new();
        let mut html_body = None;
        
        for part in parsed.parts() {
            match &part.body {
                PartType::Text(text) => {
                    if part.is_text() {
                        text_body = text.as_string().unwrap_or_default();
                    }
                }
                PartType::Html(html) => {
                    if part.is_html() {
                        html_body = Some(html.as_string().unwrap_or_default());
                    }
                }
                _ => {}
            }
        }
        
        (text_body, html_body)
    }

    /// Extract attachments from parsed email
    fn extract_attachments(&self, parsed: &mail_parser::Message) -> Vec<crate::models::Attachment> {
        parsed.attachments()
            .map(|att| crate::models::Attachment {
                id: uuid::Uuid::new_v4().to_string(),
                email_id: String::new(), // Will be set when saving
                filename: att.attachment_name().unwrap_or("attachment").to_string(),
                content_type: att.content_type()
                    .map(|ct| ct.to_string())
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                size: att.contents().len() as i64,
                content: None, // Don't store content in memory
                created_at: Utc::now(),
            })
            .collect()
    }

    /// Format email address
    fn format_address(&self, addr: &async_imap::types::Address) -> String {
        let mailbox = addr.mailbox.as_ref().map(|m| String::from_utf8_lossy(m)).unwrap_or_default();
        let host = addr.host.as_ref().map(|h| String::from_utf8_lossy(h)).unwrap_or_default();
        
        if !mailbox.is_empty() && !host.is_empty() {
            format!("{}@{}", mailbox, host)
        } else {
            String::new()
        }
    }

    /// Extract name from email address
    fn extract_name(&self, email: &str) -> Option<String> {
        if let Some(idx) = email.find('<') {
            let name = email[..idx].trim().trim_matches('"');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        None
    }

    /// Send email via SMTP
    pub async fn send_email(
        &self,
        account: &EmailAccount,
        email: &Email,
    ) -> Result<(), AppError> {
        // Build the email message
        let mut message = Message::builder()
            .from(account.email.parse().map_err(|e| AppError::SmtpError(e.to_string()))?)
            .subject(&email.subject);
        
        // Add recipients
        for to in &email.to {
            message = message.to(to.parse().map_err(|e| AppError::SmtpError(e.to_string()))?);
        }
        
        if let Some(cc) = &email.cc {
            for addr in cc {
                message = message.cc(addr.parse().map_err(|e| AppError::SmtpError(e.to_string()))?);
            }
        }
        
        if let Some(bcc) = &email.bcc {
            for addr in bcc {
                message = message.bcc(addr.parse().map_err(|e| AppError::SmtpError(e.to_string()))?);
            }
        }
        
        // Build message body
        let message = if let Some(html) = &email.body_html {
            message.multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(email.body_text.clone()))
                    .singlepart(SinglePart::html(html.clone()))
            )
        } else {
            message.singlepart(SinglePart::plain(email.body_text.clone()))
        };
        
        let message = message.map_err(|e| AppError::SmtpError(e.to_string()))?;
        
        // Get or create SMTP transport
        let transport = self.get_or_create_smtp_transport(account).await?;
        
        // Send the email
        transport.send(message)
            .await
            .map_err(|e| AppError::SmtpError(e.to_string()))?;
        
        Ok(())
    }

    /// Get or create SMTP transport
    async fn get_or_create_smtp_transport(
        &self,
        account: &EmailAccount,
    ) -> Result<AsyncSmtpTransport<Tokio1Executor>, AppError> {
        let creds = LettreCredentials::new(
            account.email.clone(),
            account.password.clone(),
        );
        
        let transport = if account.smtp_use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&account.smtp_host)
        }
        .map_err(|e| AppError::SmtpError(e.to_string()))?
        .credentials(creds)
        .port(account.smtp_port)
        .build();
        
        Ok(transport)
    }

    /// Mark emails as read/unread
    pub async fn mark_as_read(
        &self,
        account: &EmailAccount,
        uids: &[u64],
        read: bool,
    ) -> Result<(), AppError> {
        let mut sessions = self.imap_sessions.write().await;
        let imap_session = sessions
            .get_mut(&account.id)
            .ok_or_else(|| AppError::ImapError("Not connected".to_string()))?;
        
        let uid_set = uids.iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");
        
        if read {
            imap_session.session
                .store(&uid_set, "+FLAGS (\\Seen)")
                .await
                .map_err(|e| AppError::ImapError(e.to_string()))?;
        } else {
            imap_session.session
                .store(&uid_set, "-FLAGS (\\Seen)")
                .await
                .map_err(|e| AppError::ImapError(e.to_string()))?;
        }
        
        Ok(())
    }

    /// Move emails to another folder
    pub async fn move_emails(
        &self,
        account: &EmailAccount,
        uids: &[u64],
        from_folder: &str,
        to_folder: &str,
    ) -> Result<(), AppError> {
        let mut sessions = self.imap_sessions.write().await;
        let imap_session = sessions
            .get_mut(&account.id)
            .ok_or_else(|| AppError::ImapError("Not connected".to_string()))?;
        
        // Select source folder
        imap_session.session
            .select(from_folder)
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;
        
        let uid_set = uids.iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");
        
        // Copy to destination folder
        imap_session.session
            .uid_copy(&uid_set, to_folder)
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;
        
        // Mark as deleted in source folder
        imap_session.session
            .store(&uid_set, "+FLAGS (\\Deleted)")
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;
        
        // Expunge to remove deleted messages
        imap_session.session
            .expunge()
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;
        
        Ok(())
    }

    /// Delete emails (move to trash)
    pub async fn delete_emails(
        &self,
        account: &EmailAccount,
        uids: &[u64],
        folder: &str,
    ) -> Result<(), AppError> {
        self.move_emails(account, uids, folder, "Trash").await
    }

    /// Star/unstar emails
    pub async fn star_emails(
        &self,
        account: &EmailAccount,
        uids: &[u64],
        starred: bool,
    ) -> Result<(), AppError> {
        let mut sessions = self.imap_sessions.write().await;
        let imap_session = sessions
            .get_mut(&account.id)
            .ok_or_else(|| AppError::ImapError("Not connected".to_string()))?;
        
        let uid_set = uids.iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");
        
        if starred {
            imap_session.session
                .store(&uid_set, "+FLAGS (\\Flagged)")
                .await
                .map_err(|e| AppError::ImapError(e.to_string()))?;
        } else {
            imap_session.session
                .store(&uid_set, "-FLAGS (\\Flagged)")
                .await
                .map_err(|e| AppError::ImapError(e.to_string()))?;
        }
        
        Ok(())
    }

    /// Create a new folder
    pub async fn create_folder(
        &self,
        account: &EmailAccount,
        folder_name: &str,
    ) -> Result<(), AppError> {
        let mut sessions = self.imap_sessions.write().await;
        let imap_session = sessions
            .get_mut(&account.id)
            .ok_or_else(|| AppError::ImapError("Not connected".to_string()))?;
        
        imap_session.session
            .create(folder_name)
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;
        
        Ok(())
    }

    /// List all folders
    pub async fn list_folders(
        &self,
        account: &EmailAccount,
    ) -> Result<Vec<String>, AppError> {
        let mut sessions = self.imap_sessions.write().await;
        let imap_session = sessions
            .get_mut(&account.id)
            .ok_or_else(|| AppError::ImapError("Not connected".to_string()))?;
        
        let folders = imap_session.session
            .list(None, Some("*"))
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;
        
        let mut folder_names = Vec::new();
        let mut stream = folders;
        while let Some(folder) = stream.try_next().await.map_err(|e| AppError::ImapError(e.to_string()))? {
            folder_names.push(folder.name().to_string());
        }
        
        Ok(folder_names)
    }

    /// Search emails
    pub async fn search_emails(
        &self,
        account: &EmailAccount,
        folder: &str,
        query: &str,
    ) -> Result<Vec<u64>, AppError> {
        let mut sessions = self.imap_sessions.write().await;
        let imap_session = sessions
            .get_mut(&account.id)
            .ok_or_else(|| AppError::ImapError("Not connected".to_string()))?;
        
        // Select folder
        imap_session.session
            .select(folder)
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;
        
        // Build search query
        let search_query = format!("OR SUBJECT \"{}\" FROM \"{}\"", query, query);
        
        let uids = imap_session.session
            .uid_search(&search_query)
            .await
            .map_err(|e| AppError::ImapError(e.to_string()))?;
        
        Ok(uids.into_iter().map(|u| u as u64).collect())
    }

    /// Cleanup old connections
    pub async fn cleanup_connections(&self) {
        let cutoff = Utc::now() - chrono::Duration::minutes(30);
        
        // Cleanup IMAP sessions
        let mut sessions = self.imap_sessions.write().await;
        sessions.retain(|_, session| session.last_used > cutoff);
        
        // Cleanup SMTP transports
        let mut transports = self.smtp_transports.write().await;
        transports.retain(|_, transport| transport.last_used > cutoff);
    }
}

/// Thread emails into conversations
pub fn thread_emails(emails: Vec<Email>) -> Vec<Conversation> {
    let mut conversations: HashMap<String, Vec<Email>> = HashMap::new();
    let mut email_by_message_id: HashMap<String, Email> = HashMap::new();
    
    // First pass: index by message ID
    for email in emails.clone() {
        email_by_message_id.insert(email.message_id.clone(), email.clone());
    }
    
    // Second pass: group into conversations
    for mut email in emails {
        let mut thread_id = None;
        
        // Check if this email is a reply to another
        if let Some(in_reply_to) = &email.in_reply_to {
            if let Some(parent) = email_by_message_id.get(in_reply_to) {
                thread_id = parent.thread_id.clone().or(Some(parent.message_id.clone()));
            }
        }
        
        // Check references
        if thread_id.is_none() {
            if let Some(references) = &email.references {
                for reference in references {
                    if let Some(referenced) = email_by_message_id.get(reference) {
                        thread_id = referenced.thread_id.clone().or(Some(referenced.message_id.clone()));
                        break;
                    }
                }
            }
        }
        
        // If no thread found, use message ID as thread ID
        let thread_id = thread_id.unwrap_or_else(|| email.message_id.clone());
        email.thread_id = Some(thread_id.clone());
        
        conversations.entry(thread_id).or_insert_with(Vec::new).push(email);
    }
    
    // Convert to Conversation objects
    conversations.into_iter()
        .map(|(thread_id, mut emails)| {
            // Sort emails by date
            emails.sort_by_key(|e| e.date);
            
            let last_email = emails.last().unwrap();
            let first_email = emails.first().unwrap();
            
            Conversation {
                id: thread_id,
                subject: first_email.subject.clone(),
                participants: extract_participants(&emails),
                last_message_preview: truncate_preview(&last_email.body_text, 150),
                last_message_date: last_email.date,
                message_count: emails.len() as i32,
                unread_count: emails.iter().filter(|e| !e.is_read).count() as i32,
                has_attachments: emails.iter().any(|e| e.has_attachments),
                is_starred: emails.iter().any(|e| e.is_starred),
                labels: extract_labels(&emails),
                folder_id: last_email.folder_id.clone(),
                emails,
            }
        })
        .collect()
}

fn extract_participants(emails: &[Email]) -> Vec<String> {
    let mut participants = std::collections::HashSet::new();
    
    for email in emails {
        participants.insert(email.from_address.clone());
        for to in &email.to {
            participants.insert(to.clone());
        }
        if let Some(cc) = &email.cc {
            for addr in cc {
                participants.insert(addr.clone());
            }
        }
    }
    
    participants.into_iter().collect()
}

fn extract_labels(emails: &[Email]) -> Vec<String> {
    let mut labels = std::collections::HashSet::new();
    
    for email in emails {
        for label in &email.labels {
            labels.insert(label.clone());
        }
    }
    
    labels.into_iter().collect()
}

fn truncate_preview(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}