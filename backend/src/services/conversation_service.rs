use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

use crate::error::AppError;
use crate::models::{Conversation, Email};
use crate::services::email_service;

pub struct ConversationService {
    pool: SqlitePool,
}

impl ConversationService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get conversations for a folder with pagination
    pub async fn get_conversations(
        &self,
        account_id: &str,
        folder_id: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Conversation>, AppError> {
        // Fetch emails from database
        let emails = sqlx::query_as!(
            Email,
            r#"
            SELECT 
                id, account_id, folder_id, uid, message_id, thread_id,
                in_reply_to, references as "references: String", 
                from_address, from_name, 
                to as "to: String", cc as "cc: String", bcc as "bcc: String",
                subject, body_text, body_html,
                is_read, is_starred, is_draft, has_attachments,
                attachments as "attachments: String", 
                date as "date: DateTime<Utc>",
                size, labels as "labels: String",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM emails
            WHERE account_id = ? AND folder_id = ?
            ORDER BY date DESC
            LIMIT ? OFFSET ?
            "#,
            account_id,
            folder_id,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Convert JSON strings back to vectors
        let emails: Vec<Email> = emails.into_iter().map(|mut e| {
            e.to = serde_json::from_str(&e.to.join(",")).unwrap_or_default();
            e.cc = if e.cc.is_some() {
                Some(serde_json::from_str(&e.cc.unwrap().join(",")).unwrap_or_default())
            } else {
                None
            };
            e.bcc = if e.bcc.is_some() {
                Some(serde_json::from_str(&e.bcc.unwrap().join(",")).unwrap_or_default())
            } else {
                None
            };
            e.references = if e.references.is_some() {
                Some(serde_json::from_str(&e.references.unwrap()).unwrap_or_default())
            } else {
                None
            };
            e.labels = serde_json::from_str(&e.labels.join(",")).unwrap_or_default();
            e
        }).collect();

        // Thread emails into conversations
        let conversations = email_service::thread_emails(emails);

        Ok(conversations)
    }

    /// Get a single conversation by thread ID
    pub async fn get_conversation(
        &self,
        account_id: &str,
        thread_id: &str,
    ) -> Result<Conversation, AppError> {
        // Fetch all emails in the thread
        let emails = sqlx::query_as!(
            Email,
            r#"
            SELECT 
                id, account_id, folder_id, uid, message_id, thread_id,
                in_reply_to, references as "references: String",
                from_address, from_name,
                to as "to: String", cc as "cc: String", bcc as "bcc: String",
                subject, body_text, body_html,
                is_read, is_starred, is_draft, has_attachments,
                attachments as "attachments: String",
                date as "date: DateTime<Utc>",
                size, labels as "labels: String",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM emails
            WHERE account_id = ? AND thread_id = ?
            ORDER BY date ASC
            "#,
            account_id,
            thread_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if emails.is_empty() {
            return Err(AppError::NotFound("Conversation not found".to_string()));
        }

        // Convert to conversation
        let mut emails: Vec<Email> = emails.into_iter().map(|mut e| {
            e.to = serde_json::from_str(&e.to.join(",")).unwrap_or_default();
            e.cc = if e.cc.is_some() {
                Some(serde_json::from_str(&e.cc.unwrap().join(",")).unwrap_or_default())
            } else {
                None
            };
            e.bcc = if e.bcc.is_some() {
                Some(serde_json::from_str(&e.bcc.unwrap().join(",")).unwrap_or_default())
            } else {
                None
            };
            e.references = if e.references.is_some() {
                Some(serde_json::from_str(&e.references.unwrap()).unwrap_or_default())
            } else {
                None
            };
            e.labels = serde_json::from_str(&e.labels.join(",")).unwrap_or_default();
            e
        }).collect();

        let last_email = emails.last().unwrap();
        let first_email = emails.first().unwrap();

        Ok(Conversation {
            id: thread_id.to_string(),
            subject: first_email.subject.clone(),
            participants: self.extract_participants(&emails),
            last_message_preview: self.truncate_preview(&last_email.body_text, 150),
            last_message_date: last_email.date,
            message_count: emails.len() as i32,
            unread_count: emails.iter().filter(|e| !e.is_read).count() as i32,
            has_attachments: emails.iter().any(|e| e.has_attachments),
            is_starred: emails.iter().any(|e| e.is_starred),
            labels: self.extract_labels(&emails),
            folder_id: last_email.folder_id.clone(),
            emails,
        })
    }

    /// Update thread IDs for emails based on references
    pub async fn update_thread_ids(&self, account_id: &str) -> Result<(), AppError> {
        // Fetch all emails for the account
        let emails = sqlx::query!(
            r#"
            SELECT id, message_id, in_reply_to, references
            FROM emails
            WHERE account_id = ?
            ORDER BY date ASC
            "#,
            account_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Build thread relationships
        let mut thread_map: HashMap<String, String> = HashMap::new();
        
        for email in emails {
            let mut thread_id = None;
            
            // Check in_reply_to
            if let Some(in_reply_to) = &email.in_reply_to {
                if let Some(parent_thread) = thread_map.get(in_reply_to) {
                    thread_id = Some(parent_thread.clone());
                }
            }
            
            // Check references
            if thread_id.is_none() {
                if let Some(references) = &email.references {
                    let refs: Vec<String> = serde_json::from_str(references).unwrap_or_default();
                    for reference in refs {
                        if let Some(ref_thread) = thread_map.get(&reference) {
                            thread_id = Some(ref_thread.clone());
                            break;
                        }
                    }
                }
            }
            
            // If no thread found, use message_id as thread_id
            let thread_id = thread_id.unwrap_or_else(|| email.message_id.clone());
            thread_map.insert(email.message_id.clone(), thread_id.clone());
            
            // Update database
            sqlx::query!(
                "UPDATE emails SET thread_id = ? WHERE id = ?",
                thread_id,
                email.id
            )
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }

    /// Mark all emails in a conversation as read
    pub async fn mark_conversation_read(
        &self,
        account_id: &str,
        thread_id: &str,
        is_read: bool,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE emails 
            SET is_read = ?, updated_at = ?
            WHERE account_id = ? AND thread_id = ?
            "#,
            is_read,
            Utc::now(),
            account_id,
            thread_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Delete all emails in a conversation
    pub async fn delete_conversation(
        &self,
        account_id: &str,
        thread_id: &str,
    ) -> Result<(), AppError> {
        // Move to trash folder
        sqlx::query!(
            r#"
            UPDATE emails 
            SET folder_id = 'Trash', updated_at = ?
            WHERE account_id = ? AND thread_id = ?
            "#,
            Utc::now(),
            account_id,
            thread_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Archive all emails in a conversation
    pub async fn archive_conversation(
        &self,
        account_id: &str,
        thread_id: &str,
    ) -> Result<(), AppError> {
        // Move to archive folder
        sqlx::query!(
            r#"
            UPDATE emails 
            SET folder_id = 'Archive', updated_at = ?
            WHERE account_id = ? AND thread_id = ?
            "#,
            Utc::now(),
            account_id,
            thread_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Star/unstar all emails in a conversation
    pub async fn star_conversation(
        &self,
        account_id: &str,
        thread_id: &str,
        is_starred: bool,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE emails 
            SET is_starred = ?, updated_at = ?
            WHERE account_id = ? AND thread_id = ?
            "#,
            is_starred,
            Utc::now(),
            account_id,
            thread_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Search conversations
    pub async fn search_conversations(
        &self,
        account_id: &str,
        query: &str,
        folder_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Conversation>, AppError> {
        let search_pattern = format!("%{}%", query);
        
        let mut sql = r#"
            SELECT DISTINCT thread_id
            FROM emails
            WHERE account_id = ?
            AND (
                subject LIKE ? OR
                body_text LIKE ? OR
                from_address LIKE ? OR
                from_name LIKE ? OR
                to LIKE ?
            )
        "#.to_string();
        
        if folder_id.is_some() {
            sql.push_str(" AND folder_id = ?");
        }
        
        sql.push_str(" ORDER BY date DESC LIMIT ?");
        
        let thread_ids = if let Some(folder) = folder_id {
            sqlx::query_scalar::<_, String>(&sql)
                .bind(account_id)
                .bind(&search_pattern)
                .bind(&search_pattern)
                .bind(&search_pattern)
                .bind(&search_pattern)
                .bind(&search_pattern)
                .bind(folder)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query_scalar::<_, String>(&sql)
                .bind(account_id)
                .bind(&search_pattern)
                .bind(&search_pattern)
                .bind(&search_pattern)
                .bind(&search_pattern)
                .bind(&search_pattern)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Fetch conversations for found thread IDs
        let mut conversations = Vec::new();
        for thread_id in thread_ids {
            if let Ok(conversation) = self.get_conversation(account_id, &thread_id).await {
                conversations.push(conversation);
            }
        }

        Ok(conversations)
    }

    // Helper methods
    fn extract_participants(&self, emails: &[Email]) -> Vec<String> {
        let mut participants = HashSet::new();
        
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

    fn extract_labels(&self, emails: &[Email]) -> Vec<String> {
        let mut labels = HashSet::new();
        
        for email in emails {
            for label in &email.labels {
                labels.insert(label.clone());
            }
        }
        
        labels.into_iter().collect()
    }

    fn truncate_preview(&self, text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            text.to_string()
        } else {
            format!("{}...", &text[..max_len])
        }
    }
}