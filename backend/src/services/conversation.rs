use crate::models::{Conversation, Email};
use crate::services::threading::{JwzThreading, ThreadableEmail, ThreadNode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationThread {
    pub id: String,
    pub subject: String,
    pub participants: Vec<String>,
    pub last_message_date: DateTime<Utc>,
    pub message_count: usize,
    pub unread_count: usize,
    pub messages: Vec<Email>,
    pub preview_messages: Vec<Email>,
    pub has_attachments: bool,
    pub is_starred: bool,
    pub folder: String,
}

pub struct ConversationService {
    pool: SqlitePool,
}

impl ConversationService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn group_emails_into_conversations(
        &self,
        emails: Vec<Email>,
    ) -> Result<Vec<ConversationThread>, sqlx::Error> {
        // Use JWZ threading algorithm for better conversation grouping
        self.group_emails_with_jwz(emails).await
    }

    /// Group emails using JWZ threading algorithm
    async fn group_emails_with_jwz(
        &self,
        emails: Vec<Email>,
    ) -> Result<Vec<ConversationThread>, sqlx::Error> {
        // Convert emails to threadable format
        let threadable_emails: Vec<ThreadableEmail> = emails
            .iter()
            .map(|e| ThreadableEmail {
                id: e.id,
                message_id: e.message_id.clone(),
                subject: e.subject.clone(),
                in_reply_to: e.in_reply_to.clone(),
                references: serde_json::from_str(&e.references).unwrap_or_default(),
                date: e.date,
            })
            .collect();

        // Thread emails using JWZ algorithm
        let mut threading = JwzThreading::new();
        let thread_trees = threading.thread_emails(threadable_emails);

        // Convert thread trees to conversation threads
        self.convert_thread_trees_to_conversations(thread_trees, &emails).await
    }

    /// Fallback: Simple threading (old method)
    async fn group_emails_simple(
        &self,
        emails: Vec<Email>,
    ) -> Result<Vec<ConversationThread>, sqlx::Error> {
        let mut conversations: HashMap<String, Vec<Email>> = HashMap::new();
        let mut thread_subjects: HashMap<String, String> = HashMap::new();
        
        // Group emails by thread
        for email in emails {
            let thread_id = self.determine_thread_id(&email);
            
            // Store the subject for this thread (use the first non-Re: subject)
            if !thread_subjects.contains_key(&thread_id) || !email.subject.starts_with("Re:") {
                thread_subjects.insert(thread_id.clone(), email.subject.clone());
            }
            
            conversations
                .entry(thread_id)
                .or_insert_with(Vec::new)
                .push(email);
        }
        
        // Convert to ConversationThread objects
        let mut threads: Vec<ConversationThread> = Vec::new();
        
        for (thread_id, mut messages) in conversations {
            // Sort messages by date (newest first)
            messages.sort_by(|a, b| b.date.cmp(&a.date));
            
            // Get unique participants
            let mut participants = HashSet::new();
            for msg in &messages {
                // Parse email addresses from the from_address field
                participants.insert(msg.from_address.clone());
                
                // Parse to_addresses from JSON
                if let Ok(to_list) = serde_json::from_str::<Vec<String>>(&msg.to_addresses) {
                    for recipient in to_list {
                        participants.insert(recipient);
                    }
                }
            }
            
            // Count unread messages
            let unread_count = messages.iter().filter(|m| !m.is_read).count();
            
            // Check for attachments and starred messages
            let has_attachments = messages.iter().any(|m| m.has_attachments);
            let is_starred = messages.iter().any(|m| m.is_starred);
            
            // Get preview messages (last 2-3 messages)
            let preview_messages = messages
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>();
            
            let thread = ConversationThread {
                id: thread_id.clone(),
                subject: thread_subjects.get(&thread_id).unwrap_or(&String::new()).clone(),
                participants: participants.into_iter().collect(),
                last_message_date: messages[0].date,
                message_count: messages.len(),
                unread_count,
                messages: messages.clone(),
                preview_messages,
                has_attachments,
                is_starred,
                folder: messages[0].folder.clone(),
            };
            
            threads.push(thread);
        }
        
        // Sort threads by last message date (newest first)
        threads.sort_by(|a, b| b.last_message_date.cmp(&a.last_message_date));
        
        Ok(threads)
    }

    /// Convert JWZ thread trees to conversation threads
    async fn convert_thread_trees_to_conversations(
        &self,
        thread_trees: Vec<ThreadNode>,
        emails: &[Email],
    ) -> Result<Vec<ConversationThread>, sqlx::Error> {
        let mut conversations = Vec::new();
        
        // Create a map of email_id -> Email for quick lookup
        let email_map: HashMap<i64, &Email> = emails
            .iter()
            .map(|e| (e.id, e))
            .collect();
        
        for tree in thread_trees {
            // Flatten the tree to get all emails in this conversation
            let mut thread_emails = Vec::new();
            self.flatten_thread_tree(&tree, &email_map, &mut thread_emails);
            
            if thread_emails.is_empty() {
                continue;
            }
            
            // Sort by date (newest first)
            thread_emails.sort_by(|a, b| b.date.cmp(&a.date));
            
            // Get unique participants
            let mut participants = HashSet::new();
            for email in &thread_emails {
                participants.insert(email.from_address.clone());
                if let Ok(to_list) = serde_json::from_str::<Vec<String>>(&email.to_addresses) {
                    for recipient in to_list {
                        participants.insert(recipient);
                    }
                }
            }
            
            // Count unread messages
            let unread_count = thread_emails.iter().filter(|m| !m.is_read).count();
            
            // Check for attachments and starred messages
            let has_attachments = thread_emails.iter().any(|m| m.has_attachments);
            let is_starred = thread_emails.iter().any(|m| m.is_starred);
            
            // Get preview messages (last 3)
            let preview_messages = thread_emails.iter().take(3).cloned().collect();
            
            let conversation = ConversationThread {
                id: tree.message_id.clone(),
                subject: tree.subject.unwrap_or_else(|| "(No Subject)".to_string()),
                participants: participants.into_iter().collect(),
                last_message_date: thread_emails[0].date,
                message_count: thread_emails.len(),
                unread_count,
                messages: thread_emails,
                preview_messages,
                has_attachments,
                is_starred,
                folder: email_map.get(&tree.email_id.unwrap_or(0))
                    .map(|e| e.folder.clone())
                    .unwrap_or_else(|| "INBOX".to_string()),
            };
            
            conversations.push(conversation);
        }
        
        // Sort by last message date (newest first)
        conversations.sort_by(|a, b| b.last_message_date.cmp(&a.last_message_date));
        
        Ok(conversations)
    }

    /// Flatten thread tree to get all emails
    fn flatten_thread_tree(&self, node: &ThreadNode, email_map: &HashMap<i64, &Email>, result: &mut Vec<Email>) {
        if let Some(email_id) = node.email_id {
            if let Some(email) = email_map.get(&email_id) {
                result.push((*email).clone());
            }
        }
        
        for child in &node.children {
            self.flatten_thread_tree(child, email_map, result);
        }
    }

    fn determine_thread_id(&self, email: &Email) -> String {
        // Use In-Reply-To or References to determine thread
        if let Some(in_reply_to) = &email.in_reply_to {
            return self.clean_message_id(in_reply_to);
        }
        
        // Parse references from JSON string
        let references: Vec<String> = serde_json::from_str(&email.references)
            .unwrap_or_default();
        
        if !references.is_empty() {
            // Use the first reference as the thread root
            return self.clean_message_id(&references[0]);
        }
        
        // If no threading information, use the message ID itself
        // This creates a new thread
        self.clean_message_id(&email.message_id)
    }

    fn clean_message_id(&self, message_id: &str) -> String {
        // Remove angle brackets and whitespace
        message_id
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>')
            .to_string()
    }

    pub async fn get_conversation_by_id(
        &self,
        conversation_id: &str,
        user_id: i64,
    ) -> Result<Option<ConversationThread>, sqlx::Error> {
        // Fetch all emails in this conversation
        let emails = sqlx::query_as::<_, Email>(
            r#"
            SELECT * FROM emails 
            WHERE user_id = ? 
            AND (message_id = ? OR in_reply_to = ? OR references LIKE ?)
            ORDER BY date DESC
            "#,
        )
        .bind(user_id)
        .bind(conversation_id)
        .bind(conversation_id)
        .bind(format!("%{}%", conversation_id))
        .fetch_all(&self.pool)
        .await?;
        
        if emails.is_empty() {
            return Ok(None);
        }
        
        let conversations = self.group_emails_into_conversations(emails).await?;
        Ok(conversations.into_iter().next())
    }

    pub async fn mark_conversation_as_read(
        &self,
        conversation_id: &str,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE emails 
            SET is_read = TRUE 
            WHERE user_id = ? 
            AND (message_id = ? OR in_reply_to = ? OR references LIKE ?)
            "#,
        )
        .bind(user_id)
        .bind(conversation_id)
        .bind(conversation_id)
        .bind(format!("%{}%", conversation_id))
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn delete_conversation(
        &self,
        conversation_id: &str,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        // Move to trash folder
        sqlx::query(
            r#"
            UPDATE emails 
            SET folder = 'Trash', deleted_at = CURRENT_TIMESTAMP
            WHERE user_id = ? 
            AND (message_id = ? OR in_reply_to = ? OR references LIKE ?)
            "#,
        )
        .bind(user_id)
        .bind(conversation_id)
        .bind(conversation_id)
        .bind(format!("%{}%", conversation_id))
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn move_conversation_to_folder(
        &self,
        conversation_id: &str,
        folder: &str,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE emails 
            SET folder = ?
            WHERE user_id = ? 
            AND (message_id = ? OR in_reply_to = ? OR references LIKE ?)
            "#,
        )
        .bind(folder)
        .bind(user_id)
        .bind(conversation_id)
        .bind(conversation_id)
        .bind(format!("%{}%", conversation_id))
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn star_conversation(
        &self,
        conversation_id: &str,
        starred: bool,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE emails 
            SET is_starred = ?
            WHERE user_id = ? 
            AND (message_id = ? OR in_reply_to = ? OR references LIKE ?)
            "#,
        )
        .bind(starred)
        .bind(user_id)
        .bind(conversation_id)
        .bind(conversation_id)
        .bind(format!("%{}%", conversation_id))
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn get_conversation_count(
        &self,
        folder: &str,
        user_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(DISTINCT COALESCE(in_reply_to, message_id)) 
            FROM emails 
            WHERE user_id = ? AND folder = ?
            "#,
        )
        .bind(user_id)
        .bind(folder)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(result)
    }
}