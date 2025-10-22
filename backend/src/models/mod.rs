use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub oauth_provider: Option<String>,
    pub oauth_access_token: Option<String>,
    pub oauth_refresh_token: Option<String>,
    pub email: String,
    pub username: String,
    pub password_hash: String,
    pub email_password: Option<String>,
    pub imap_host: String,
    pub imap_port: i32,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub smtp_use_tls: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Email {
    pub id: i64,
    pub user_id: i64,
    pub message_id: String,
    pub thread_id: Option<String>,
    pub from_address: String,
    pub to_addresses: String,  // JSON
    pub cc_addresses: Option<String>,  // JSON
    pub bcc_addresses: Option<String>,  // JSON
    pub subject: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub date: DateTime<Utc>,
    pub is_read: bool,
    pub is_starred: bool,
    pub has_attachments: bool,
    pub attachments: Option<String>,  // JSON
    pub folder: String,
    pub size: i64,
    pub in_reply_to: Option<String>,
    pub references: String,  // JSON
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    #[sqlx(skip)]
    pub to_list: Vec<String>,
    #[sqlx(skip)]
    pub cc_list: Vec<String>,
    #[sqlx(skip)]
    pub bcc_list: Vec<String>,
    #[sqlx(skip)]
    pub references_list: Vec<String>,
}

impl Email {
    pub fn parse_json_fields(&mut self) {
        self.to_list = serde_json::from_str(&self.to_addresses).unwrap_or_default();
        self.cc_list = self.cc_addresses.as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        self.bcc_list = self.bcc_addresses.as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        self.references_list = serde_json::from_str(&self.references).unwrap_or_default();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Conversation {
    pub id: String,
    pub user_id: i64,
    pub subject: String,
    pub participants: String,  // JSON
    pub last_message_date: DateTime<Utc>,
    pub message_count: i32,
    pub unread_count: i32,
    pub is_starred: bool,
    pub folder: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Folder {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub sort_order: i32,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Draft {
    pub id: i64,
    pub user_id: i64,
    pub to_addresses: Option<String>,  // JSON
    pub cc_addresses: Option<String>,  // JSON
    pub bcc_addresses: Option<String>,  // JSON
    pub subject: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Option<String>,  // JSON
    pub in_reply_to: Option<String>,
    pub references: Option<String>,  // JSON
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Filter {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub conditions: String,  // JSON
    pub actions: String,  // JSON
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Attachment {
    pub id: i64,
    pub email_id: Option<i64>,
    pub draft_id: Option<i64>,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SavedSearch {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub query: String,  // JSON
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Settings {
    pub id: i64,
    pub user_id: i64,
    pub theme: String,
    pub notifications_enabled: bool,
    pub auto_mark_read: bool,
    pub auto_mark_read_delay: i32,
    pub conversation_view: bool,
    pub preview_lines: i32,
    pub signature: Option<String>,
    pub vacation_responder: Option<String>,  // JSON
    pub keyboard_shortcuts_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
