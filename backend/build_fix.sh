#!/bin/bash

# Fix compilation errors in the backend

echo "Fixing compilation errors..."

# Create a minimal working version
cat > src/lib.rs << 'EOF'
pub mod config;
pub mod models;
pub mod handlers;
pub mod services;
pub mod middleware;
pub mod db;
pub mod websocket;
pub mod error;
pub mod utils;

pub use error::AppError;
EOF

# Fix the models to ensure they compile
cat > src/models/mod.rs << 'EOF'
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub name: String,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub smtp_use_tls: bool,
    pub imap_host: String,
    pub imap_port: i32,
    pub email_password: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Email {
    pub id: String,
    pub user_id: String,
    pub message_id: String,
    pub thread_id: Option<String>,
    pub folder: String,
    pub subject: String,
    pub from_address: String,
    pub to_addresses: String,  // JSON
    pub cc_addresses: Option<String>,  // JSON
    pub bcc_addresses: Option<String>,  // JSON
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub has_attachments: bool,
    pub in_reply_to: Option<String>,
    pub references: String,  // JSON
    pub date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    
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
    pub user_id: String,
    pub subject: String,
    pub participants: String,  // JSON
    pub last_message_date: DateTime<Utc>,
    pub message_count: i32,
    pub unread_count: i32,
    pub is_starred: bool,
    pub folder_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Folder {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub folder_type: String,
    pub parent_id: Option<String>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Draft {
    pub id: i64,
    pub user_id: String,
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
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub conditions: String,  // JSON
    pub actions: String,  // JSON
    pub is_active: bool,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Attachment {
    pub id: String,
    pub email_id: Option<String>,
    pub draft_id: Option<i64>,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SavedSearch {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub query: String,  // JSON
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Settings {
    pub id: String,
    pub user_id: String,
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
EOF

echo "Build fixes applied. Now building..."
cargo build --release 2>&1 | tail -20
