use crate::models::{Email, User};
use crate::services::{ImapService, SmtpService};
use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct EmailService {
    pool: SqlitePool,
    imap_services: Arc<RwLock<std::collections::HashMap<i64, ImapService>>>,
    smtp_services: Arc<RwLock<std::collections::HashMap<i64, SmtpService>>>,
}

impl EmailService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            imap_services: Arc::new(RwLock::new(std::collections::HashMap::new())),
            smtp_services: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn initialize_user_services(&self, user: &User) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create IMAP service for user
        let imap_service = ImapService::new(
            user.imap_host.clone(),
            user.imap_port as u16,
            user.email.clone(),
            user.email_password.clone().unwrap_or_default(),
        );
        
        // Connect to IMAP
        imap_service.connect().await?;
        
        // Store IMAP service
        self.imap_services.write().await.insert(user.id, imap_service);
        
        // Create SMTP service for user
        let smtp_service = SmtpService::new(
            user.smtp_host.clone(),
            user.smtp_port as u16,
            user.email.clone(),
            user.email_password.clone().unwrap_or_default(),
            user.smtp_use_tls,
        );
        
        // Test SMTP connection
        smtp_service.test_connection().await?;
        
        // Store SMTP service
        self.smtp_services.write().await.insert(user.id, smtp_service);
        
        Ok(())
    }

    pub async fn sync_emails(&self, user_id: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let imap_services = self.imap_services.read().await;
        let imap_service = imap_services
            .get(&user_id)
            .ok_or("IMAP service not initialized for user")?;
        
        // Fetch emails from IMAP
        let messages = imap_service.fetch_messages("INBOX", 50).await?;
        
        // Store emails in database
        for message in messages {
            // Create a properly initialized Email struct with all required fields
            let email_id = sqlx::query(
                r#"
                INSERT OR REPLACE INTO emails (
                    user_id, message_id, thread_id, from_address, to_addresses, 
                    cc_addresses, bcc_addresses, subject, body_text, body_html, 
                    date, is_read, is_starred, has_attachments, attachments, 
                    folder, size, in_reply_to, references, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(user_id)
            .bind(&message.message_id)
            .bind(&message.thread_id)
            .bind(message.from.iter()
                .map(|a| format!("{} <{}>", a.name.as_ref().unwrap_or(&String::new()), a.email))
                .collect::<Vec<_>>()
                .join(", "))
            .bind(serde_json::to_string(&message.to).unwrap())
            .bind(serde_json::to_string(&message.cc).unwrap())
            .bind(serde_json::to_string(&message.bcc).unwrap())
            .bind(&message.subject)
            .bind(&message.body_text)
            .bind(&message.body_html)
            .bind(message.date)
            .bind(message.flags.contains(&"\\Seen".to_string()))
            .bind(message.flags.contains(&"\\Flagged".to_string()))
            .bind(!message.attachments.is_empty())
            .bind(serde_json::to_string(&message.attachments).ok())
            .bind("INBOX")
            .bind(0i64) // size
            .bind(&message.in_reply_to)
            .bind(serde_json::to_string(&message.references).unwrap())
            .bind(Utc::now())
            .bind(Utc::now())
            .execute(&self.pool)
            .await?;
        }
        
        Ok(())
    }

    pub async fn send_email(
        &self,
        user_id: i64,
        composition: crate::services::smtp::EmailComposition,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let smtp_services = self.smtp_services.read().await;
        let smtp_service = smtp_services
            .get(&user_id)
            .ok_or("SMTP service not initialized for user")?;
        
        // Send email via SMTP
        let message_id = smtp_service.send_email(composition.clone()).await?;
        
        // Store sent email in database
        sqlx::query(
            r#"
            INSERT INTO emails (
                user_id, message_id, thread_id, from_address, to_addresses, 
                cc_addresses, bcc_addresses, subject, body_text, body_html, 
                date, is_read, is_starred, has_attachments, attachments, 
                folder, size, in_reply_to, references, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(user_id)
        .bind(&message_id)
        .bind(&composition.in_reply_to)
        .bind(&composition.from)
        .bind(serde_json::to_string(&composition.to).unwrap())
        .bind(serde_json::to_string(&composition.cc).unwrap())
        .bind(serde_json::to_string(&composition.bcc).unwrap())
        .bind(&composition.subject)
        .bind(&composition.body_text)
        .bind(&composition.body_html)
        .bind(Utc::now())
        .bind(true) // is_read
        .bind(false) // is_starred
        .bind(!composition.attachments.is_empty())
        .bind(serde_json::to_string(&composition.attachments).ok())
        .bind("Sent")
        .bind(0i64) // size
        .bind(&composition.in_reply_to)
        .bind(serde_json::to_string(&composition.references).unwrap())
        .bind(Utc::now())
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;
        
        Ok(message_id)
    }

    pub async fn mark_as_read(
        &self,
        user_id: i64,
        message_id: &str,
        is_read: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update in database
        sqlx::query(
            "UPDATE emails SET is_read = ?, updated_at = ? WHERE user_id = ? AND message_id = ?",
        )
        .bind(is_read)
        .bind(Utc::now())
        .bind(user_id)
        .bind(message_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn delete_email(
        &self,
        user_id: i64,
        message_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Move to trash in database
        sqlx::query(
            "UPDATE emails SET folder = 'Trash', deleted_at = ?, updated_at = ? WHERE user_id = ? AND message_id = ?",
        )
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(user_id)
        .bind(message_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn move_to_folder(
        &self,
        user_id: i64,
        message_id: &str,
        folder: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update in database
        sqlx::query(
            "UPDATE emails SET folder = ?, updated_at = ? WHERE user_id = ? AND message_id = ?",
        )
        .bind(folder)
        .bind(Utc::now())
        .bind(user_id)
        .bind(message_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}