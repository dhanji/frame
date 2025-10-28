use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn get_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
            })
            .collect()
    }

    pub fn to_anthropic_format(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "input_schema": tool.parameters()
                })
            })
            .collect()
    }
}

// Email Search Tool
pub struct EmailSearchTool {
    pool: SqlitePool,
    user_id: i64,
}

impl EmailSearchTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for EmailSearchTool {
    fn name(&self) -> &str {
        "search_emails"
    }

    fn description(&self) -> &str {
        "Search emails by sender, subject, content, or date range. Returns a list of matching emails with relevance ranking."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query for email content, subject, or sender"
                },
                "sender": {
                    "type": "string",
                    "description": "Filter by sender email address"
                },
                "date_from": {
                    "type": "string",
                    "description": "Start date in ISO format (YYYY-MM-DD)"
                },
                "date_to": {
                    "type": "string",
                    "description": "End date in ISO format (YYYY-MM-DD)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default 10)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let query = arguments["query"].as_str().unwrap_or("");
        let limit = arguments["limit"].as_i64().unwrap_or(10);

        let results = sqlx::query_as::<_, (i64, String, String, String, String)>(
            r#"
            SELECT e.id, e.subject, e.from_address, e.body_text, e.date
            FROM emails e
            WHERE e.user_id = ? AND e.deleted_at IS NULL
            AND (e.subject LIKE ? OR e.body_text LIKE ? OR e.from_address LIKE ?)
            ORDER BY e.date DESC
            LIMIT ?
            "#,
        )
        .bind(self.user_id)
        .bind(format!("%{}%", query))
        .bind(format!("%{}%", query))
        .bind(format!("%{}%", query))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let emails: Vec<Value> = results
            .iter()
            .map(|(id, subject, from, body, date)| {
                let preview = body.chars().take(200).collect::<String>();
                serde_json::json!({
                    "id": id,
                    "subject": subject,
                    "from": from,
                    "preview": preview,
                    "date": date
                })
            })
            .collect();

        Ok(serde_json::json!({
            "results": emails,
            "count": emails.len()
        }))
    }
}

// Email Read Tool
pub struct EmailReadTool {
    pool: SqlitePool,
    user_id: i64,
}

impl EmailReadTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for EmailReadTool {
    fn name(&self) -> &str {
        "read_email"
    }

    fn description(&self) -> &str {
        "Read the full content of an email by its ID. Returns headers, body, and attachment metadata."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "email_id": {
                    "type": "integer",
                    "description": "The ID of the email to read"
                }
            },
            "required": ["email_id"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let email_id = arguments["email_id"].as_i64().ok_or("Missing email_id")?;

        let email = sqlx::query_as::<_, (String, String, String, String, Option<String>, Option<String>, String, bool)>(
            r#"
            SELECT subject, from_address, to_addresses, cc_addresses, body_text, body_html, date, has_attachments
            FROM emails
            WHERE id = ? AND user_id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(email_id)
        .bind(self.user_id)
        .fetch_optional(&self.pool)
        .await?;

        match email {
            Some((subject, from, to, cc, body_text, body_html, date, has_attachments)) => {
                Ok(serde_json::json!({
                    "id": email_id,
                    "subject": subject,
                    "from": from,
                    "to": to,
                    "cc": cc,
                    "body_text": body_text,
                    "body_html": body_html,
                    "date": date,
                    "has_attachments": has_attachments
                }))
            }
            None => Err("Email not found".into()),
        }
    }
}

// Email Compose Tool
pub struct EmailComposeTool {
    pool: SqlitePool,
    user_id: i64,
}

impl EmailComposeTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for EmailComposeTool {
    fn name(&self) -> &str {
        "compose_email"
    }

    fn description(&self) -> &str {
        "Create a new email draft with recipients, subject, and body. Returns the draft ID."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of recipient email addresses"
                },
                "subject": {
                    "type": "string",
                    "description": "Email subject line"
                },
                "body": {
                    "type": "string",
                    "description": "Email body content"
                },
                "cc": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "CC recipients (optional)"
                }
            },
            "required": ["to", "subject", "body"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let to = arguments["to"]
            .as_array()
            .ok_or("Missing 'to' field")?;
        let subject = arguments["subject"].as_str().ok_or("Missing subject")?;
        let body = arguments["body"].as_str().ok_or("Missing body")?;

        let to_str = to
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(",");

        let draft_id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO drafts (id, user_id, to_addresses, subject, body_text, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&draft_id)
        .bind(self.user_id)
        .bind(&to_str)
        .bind(subject)
        .bind(body)
        .execute(&self.pool)
        .await?;

        Ok(serde_json::json!({
            "draft_id": draft_id,
            "message": "Draft created successfully"
        }))
    }
}

// Email Organization Tool
pub struct EmailOrganizeTool {
    pool: SqlitePool,
    user_id: i64,
}

impl EmailOrganizeTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for EmailOrganizeTool {
    fn name(&self) -> &str {
        "organize_email"
    }

    fn description(&self) -> &str {
        "Organize emails by moving to folders, marking as read/unread, or starring. Can operate on single or multiple emails."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "email_ids": {
                    "type": "array",
                    "items": {"type": "integer"},
                    "description": "List of email IDs to organize"
                },
                "action": {
                    "type": "string",
                    "enum": ["move", "mark_read", "mark_unread", "star", "unstar"],
                    "description": "Action to perform"
                },
                "folder": {
                    "type": "string",
                    "description": "Target folder name (required for 'move' action)"
                }
            },
            "required": ["email_ids", "action"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let email_ids = arguments["email_ids"]
            .as_array()
            .ok_or("Missing email_ids")?;
        let action = arguments["action"].as_str().ok_or("Missing action")?;

        let mut affected = 0;

        for id in email_ids {
            let email_id = id.as_i64().ok_or("Invalid email ID")?;

            let result = match action {
                "move" => {
                    let folder = arguments["folder"].as_str().ok_or("Missing folder")?;
                    sqlx::query("UPDATE emails SET folder = ? WHERE id = ? AND user_id = ?")
                        .bind(folder)
                        .bind(email_id)
                        .bind(self.user_id)
                        .execute(&self.pool)
                        .await?
                }
                "mark_read" => {
                    sqlx::query("UPDATE emails SET is_read = 1 WHERE id = ? AND user_id = ?")
                        .bind(email_id)
                        .bind(self.user_id)
                        .execute(&self.pool)
                        .await?
                }
                "mark_unread" => {
                    sqlx::query("UPDATE emails SET is_read = 0 WHERE id = ? AND user_id = ?")
                        .bind(email_id)
                        .bind(self.user_id)
                        .execute(&self.pool)
                        .await?
                }
                "star" => {
                    sqlx::query("UPDATE emails SET is_starred = 1 WHERE id = ? AND user_id = ?")
                        .bind(email_id)
                        .bind(self.user_id)
                        .execute(&self.pool)
                        .await?
                }
                "unstar" => {
                    sqlx::query("UPDATE emails SET is_starred = 0 WHERE id = ? AND user_id = ?")
                        .bind(email_id)
                        .bind(self.user_id)
                        .execute(&self.pool)
                        .await?
                }
                _ => return Err("Invalid action".into()),
            };

            affected += result.rows_affected();
        }

        Ok(serde_json::json!({
            "affected": affected,
            "message": format!("Successfully {} {} email(s)", action, affected)
        }))
    }
}

// Contact Lookup Tool
pub struct ContactLookupTool {
    pool: SqlitePool,
    user_id: i64,
}

impl ContactLookupTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for ContactLookupTool {
    fn name(&self) -> &str {
        "lookup_contact"
    }

    fn description(&self) -> &str {
        "Search for contacts by name or email address. Returns contact information and recent communication history."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Name or email address to search for"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let query = arguments["query"].as_str().ok_or("Missing query")?;

        // Search in emails for contacts
        let results = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT from_address, COUNT(*) as email_count
            FROM emails
            WHERE user_id = ? AND deleted_at IS NULL
            AND from_address LIKE ?
            GROUP BY from_address
            ORDER BY email_count DESC
            LIMIT 10
            "#,
        )
        .bind(self.user_id)
        .bind(format!("%{}%", query))
        .fetch_all(&self.pool)
        .await?;

        let contacts: Vec<Value> = results
            .iter()
            .map(|(email, count)| {
                serde_json::json!({
                    "email": email,
                    "email_count": count
                })
            })
            .collect();

        Ok(serde_json::json!({
            "contacts": contacts,
            "count": contacts.len()
        }))
    }
}

// Email Send Tool
pub struct EmailSendTool {
    pool: SqlitePool,
    user_id: i64,
}

impl EmailSendTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for EmailSendTool {
    fn name(&self) -> &str {
        "send_email"
    }

    fn description(&self) -> &str {
        "Send an email immediately or send a draft. Requires either draft_id or all email fields (to, subject, body)."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "draft_id": {
                    "type": "string",
                    "description": "ID of an existing draft to send"
                },
                "to": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of recipient email addresses (required if no draft_id)"
                },
                "subject": {
                    "type": "string",
                    "description": "Email subject line (required if no draft_id)"
                },
                "body": {
                    "type": "string",
                    "description": "Email body content (required if no draft_id)"
                },
                "cc": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "CC recipients (optional)"
                }
            }
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        // Check if draft_id is provided
        if let Some(draft_id) = arguments["draft_id"].as_str() {
            // Queue the draft for sending
            sqlx::query(
                r#"UPDATE drafts SET updated_at = CURRENT_TIMESTAMP WHERE id = ? AND user_id = ?"#,
            )
            .bind(draft_id)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

            return Ok(serde_json::json!({
                "status": "queued",
                "message": "Email queued for sending",
                "draft_id": draft_id
            }));
        }

        // Otherwise, create and send a new email
        let to = arguments["to"]
            .as_array()
            .ok_or("Missing 'to' field")?;
        let subject = arguments["subject"].as_str().ok_or("Missing subject")?;
        let body = arguments["body"].as_str().ok_or("Missing body")?;

        let to_str = to
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(",");

        let draft_id = uuid::Uuid::new_v4().to_string();

        // Create draft and queue for sending
        sqlx::query(
            r#"
            INSERT INTO drafts (id, user_id, to_addresses, subject, body_text, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&draft_id)
        .bind(self.user_id)
        .bind(&to_str)
        .bind(subject)
        .bind(body)
        .execute(&self.pool)
        .await?;

        Ok(serde_json::json!({
            "status": "queued",
            "message": "Email created and queued for sending",
            "draft_id": draft_id
        }))
    }
}

// Calendar Search Tool
pub struct CalendarSearchTool {
    pool: SqlitePool,
    user_id: i64,
}

impl CalendarSearchTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for CalendarSearchTool {
    fn name(&self) -> &str {
        "search_calendar_events"
    }

    fn description(&self) -> &str {
        "Search calendar events by date range, title, or attendee. Returns matching events."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in ISO format (YYYY-MM-DD)"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in ISO format (YYYY-MM-DD)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query for event title or description"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default 20)"
                }
            }
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let limit = arguments["limit"].as_i64().unwrap_or(20);
        let query = arguments["query"].as_str().unwrap_or("");

        let mut sql = String::from(
            "SELECT id, title, description, location, start_time, end_time, all_day FROM calendar_events WHERE user_id = ?"
        );
        let mut conditions = vec![];

        if let Some(start_date) = arguments["start_date"].as_str() {
            conditions.push(format!("start_time >= '{}'", start_date));
        }
        if let Some(end_date) = arguments["end_date"].as_str() {
            conditions.push(format!("end_time <= '{}'", end_date));
        }
        if !query.is_empty() {
            conditions.push(format!("(title LIKE '%{}%' OR description LIKE '%{}%')", query, query));
        }

        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY start_time ASC LIMIT ?");

        let results = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, String, bool)>(&sql)
            .bind(self.user_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let events: Vec<Value> = results
            .iter()
            .map(|(id, title, description, location, start_time, end_time, all_day)| {
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "description": description,
                    "location": location,
                    "start_time": start_time,
                    "end_time": end_time,
                    "all_day": all_day
                })
            })
            .collect();

        Ok(serde_json::json!({
            "events": events,
            "count": events.len()
        }))
    }
}

// Calendar Create Tool
pub struct CalendarCreateTool {
    pool: SqlitePool,
    user_id: i64,
}

impl CalendarCreateTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for CalendarCreateTool {
    fn name(&self) -> &str {
        "create_calendar_event"
    }

    fn description(&self) -> &str {
        "Create a new calendar event with title, description, location, and time details."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Event title"
                },
                "description": {
                    "type": "string",
                    "description": "Event description (optional)"
                },
                "location": {
                    "type": "string",
                    "description": "Event location (optional)"
                },
                "start_time": {
                    "type": "string",
                    "description": "Start time in ISO format"
                },
                "end_time": {
                    "type": "string",
                    "description": "End time in ISO format"
                },
                "all_day": {
                    "type": "boolean",
                    "description": "Whether this is an all-day event (default false)"
                }
            },
            "required": ["title", "start_time", "end_time"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let title = arguments["title"].as_str().ok_or("Missing title")?;
        let start_time = arguments["start_time"].as_str().ok_or("Missing start_time")?;
        let end_time = arguments["end_time"].as_str().ok_or("Missing end_time")?;
        let description = arguments["description"].as_str();
        let location = arguments["location"].as_str();
        let all_day = arguments["all_day"].as_bool().unwrap_or(false);

        let event_id = uuid::Uuid::new_v4().to_string();
        let calendar_id = "default".to_string();

        sqlx::query(
            r#"
            INSERT INTO calendar_events (id, user_id, calendar_id, title, description, location, start_time, end_time, all_day, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&event_id)
        .bind(self.user_id)
        .bind(&calendar_id)
        .bind(title)
        .bind(description)
        .bind(location)
        .bind(start_time)
        .bind(end_time)
        .bind(all_day)
        .execute(&self.pool)
        .await?;

        Ok(serde_json::json!({
            "event_id": event_id,
            "message": "Calendar event created successfully"
        }))
    }
}

// Reminder Create Tool
pub struct ReminderCreateTool {
    pool: SqlitePool,
    user_id: i64,
}

impl ReminderCreateTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for ReminderCreateTool {
    fn name(&self) -> &str {
        "create_reminder"
    }

    fn description(&self) -> &str {
        "Create a new reminder/TODO item with optional due date and notes."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short task description"
                },
                "notes": {
                    "type": "string",
                    "description": "Detailed notes (optional)"
                },
                "due_date": {
                    "type": "string",
                    "description": "Due date in ISO format (optional)"
                }
            },
            "required": ["title"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let title = arguments["title"].as_str().ok_or("Missing title")?;
        let notes = arguments["notes"].as_str();
        let due_date = arguments["due_date"].as_str();

        let reminder_id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO reminders (id, user_id, title, notes, due_date, completed, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&reminder_id)
        .bind(self.user_id)
        .bind(title)
        .bind(notes)
        .bind(due_date)
        .execute(&self.pool)
        .await?;

        Ok(serde_json::json!({
            "reminder_id": reminder_id,
            "message": "Reminder created successfully"
        }))
    }
}

// Reminder Search Tool
pub struct ReminderSearchTool {
    pool: SqlitePool,
    user_id: i64,
}

impl ReminderSearchTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for ReminderSearchTool {
    fn name(&self) -> &str {
        "search_reminders"
    }

    fn description(&self) -> &str {
        "Search and list reminders/TODO items. Can filter by completion status."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "filter": {
                    "type": "string",
                    "enum": ["all", "active", "completed"],
                    "description": "Filter by completion status (default: active)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default 20)"
                }
            }
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let filter = arguments["filter"].as_str().unwrap_or("active");
        let limit = arguments["limit"].as_i64().unwrap_or(20);

        let mut sql = String::from(
            "SELECT id, title, notes, due_date, completed, created_at FROM reminders WHERE user_id = ?"
        );

        if filter == "active" {
            sql.push_str(" AND completed = 0");
        } else if filter == "completed" {
            sql.push_str(" AND completed = 1");
        }

        sql.push_str(" ORDER BY due_date ASC, created_at DESC LIMIT ?");

        let results = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, bool, String)>(&sql)
            .bind(self.user_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let reminders: Vec<Value> = results
            .iter()
            .map(|(id, title, notes, due_date, completed, created_at)| {
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "notes": notes,
                    "due_date": due_date,
                    "completed": completed,
                    "created_at": created_at
                })
            })
            .collect();

        Ok(serde_json::json!({
            "reminders": reminders,
            "count": reminders.len()
        }))
    }
}

// Reminder Update Tool
pub struct ReminderUpdateTool {
    pool: SqlitePool,
    user_id: i64,
}

impl ReminderUpdateTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for ReminderUpdateTool {
    fn name(&self) -> &str {
        "update_reminder"
    }

    fn description(&self) -> &str {
        "Update an existing reminder's title, notes, or due date."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "reminder_id": {
                    "type": "string",
                    "description": "ID of the reminder to update"
                },
                "title": {
                    "type": "string",
                    "description": "New title (optional)"
                },
                "notes": {
                    "type": "string",
                    "description": "New notes (optional)"
                },
                "due_date": {
                    "type": "string",
                    "description": "New due date in ISO format (optional)"
                }
            },
            "required": ["reminder_id"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let reminder_id = arguments["reminder_id"].as_str().ok_or("Missing reminder_id")?;
        
        let mut updates = vec![];
        let mut params: Vec<String> = vec![];
        
        if let Some(title) = arguments["title"].as_str() {
            updates.push("title = ?");
            params.push(title.to_string());
        }
        if let Some(notes) = arguments["notes"].as_str() {
            updates.push("notes = ?");
            params.push(notes.to_string());
        }
        if let Some(due_date) = arguments["due_date"].as_str() {
            updates.push("due_date = ?");
            params.push(due_date.to_string());
        }
        
        if updates.is_empty() {
            return Err("No fields to update".into());
        }
        
        updates.push("updated_at = CURRENT_TIMESTAMP");
        
        let sql = format!(
            "UPDATE reminders SET {} WHERE id = ? AND user_id = ?",
            updates.join(", ")
        );
        
        let mut query = sqlx::query(&sql);
        for param in params {
            query = query.bind(param);
        }
        query = query.bind(reminder_id).bind(self.user_id);
        
        let result = query.execute(&self.pool).await?;
        
        if result.rows_affected() == 0 {
            return Err("Reminder not found".into());
        }
        
        Ok(serde_json::json!({
            "reminder_id": reminder_id,
            "message": "Reminder updated successfully"
        }))
    }
}

// Reminder Complete Tool
pub struct ReminderCompleteTool {
    pool: SqlitePool,
    user_id: i64,
}

impl ReminderCompleteTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for ReminderCompleteTool {
    fn name(&self) -> &str {
        "complete_reminder"
    }

    fn description(&self) -> &str {
        "Mark a reminder as completed or uncompleted."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "reminder_id": {
                    "type": "string",
                    "description": "ID of the reminder to mark as completed"
                },
                "completed": {
                    "type": "boolean",
                    "description": "Whether to mark as completed (true) or uncompleted (false)"
                }
            },
            "required": ["reminder_id", "completed"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let reminder_id = arguments["reminder_id"].as_str().ok_or("Missing reminder_id")?;
        let completed = arguments["completed"].as_bool().ok_or("Missing completed")?;
        
        let result = sqlx::query(
            "UPDATE reminders SET completed = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND user_id = ?"
        )
        .bind(completed)
        .bind(reminder_id)
        .bind(self.user_id)
        .execute(&self.pool)
        .await?;
        
        if result.rows_affected() == 0 {
            return Err("Reminder not found".into());
        }
        
        Ok(serde_json::json!({
            "reminder_id": reminder_id,
            "completed": completed,
            "message": format!("Reminder marked as {}", if completed { "completed" } else { "uncompleted" })
        }))
    }
}

// Reminder Delete Tool
pub struct ReminderDeleteTool {
    pool: SqlitePool,
    user_id: i64,
}

impl ReminderDeleteTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for ReminderDeleteTool {
    fn name(&self) -> &str {
        "delete_reminder"
    }

    fn description(&self) -> &str {
        "Delete a reminder permanently."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "reminder_id": {
                    "type": "string",
                    "description": "ID of the reminder to delete"
                }
            },
            "required": ["reminder_id"]
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let reminder_id = arguments["reminder_id"].as_str().ok_or("Missing reminder_id")?;
        
        let result = sqlx::query(
            "DELETE FROM reminders WHERE id = ? AND user_id = ?"
        )
        .bind(reminder_id)
        .bind(self.user_id)
        .execute(&self.pool)
        .await?;
        
        if result.rows_affected() == 0 {
            return Err("Reminder not found".into());
        }
        
        Ok(serde_json::json!({
            "reminder_id": reminder_id,
            "message": "Reminder deleted successfully"
        }))
    }
}

// Money Account List Tool
pub struct MoneyAccountListTool {
    pool: SqlitePool,
    user_id: i64,
}

impl MoneyAccountListTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for MoneyAccountListTool {
    fn name(&self) -> &str {
        "list_money_accounts"
    }

    fn description(&self) -> &str {
        "List all money accounts with their current balances and account types."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(
        &self,
        _arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let results = sqlx::query_as::<_, (String, String, String, f64, String)>(
            "SELECT id, account_name, account_type, balance, currency FROM money_accounts WHERE user_id = ?"
        )
        .bind(self.user_id)
        .fetch_all(&self.pool)
        .await?;

        let accounts: Vec<Value> = results
            .iter()
            .map(|(id, name, account_type, balance, currency)| {
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "type": account_type,
                    "balance": balance,
                    "currency": currency
                })
            })
            .collect();

        let total_balance: f64 = results.iter().map(|(_, _, _, balance, _)| balance).sum();

        Ok(serde_json::json!({
            "accounts": accounts,
            "total_balance": total_balance,
            "count": accounts.len()
        }))
    }
}

// Money Transaction Search Tool
pub struct MoneyTransactionSearchTool {
    pool: SqlitePool,
    user_id: i64,
}

impl MoneyTransactionSearchTool {
    pub fn new(pool: SqlitePool, user_id: i64) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl Tool for MoneyTransactionSearchTool {
    fn name(&self) -> &str {
        "search_transactions"
    }

    fn description(&self) -> &str {
        "Search money transactions by date range, category, or type (income/expense)."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in ISO format (optional)"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in ISO format (optional)"
                },
                "transaction_type": {
                    "type": "string",
                    "enum": ["income", "expense", "transfer"],
                    "description": "Filter by transaction type (optional)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default 50)"
                }
            }
        })
    }

    async fn execute(
        &self,
        arguments: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let limit = arguments["limit"].as_i64().unwrap_or(50);

        let mut sql = String::from(
            r#"SELECT t.id, t.transaction_date, t.description, t.amount, t.category, t.transaction_type, a.account_name
               FROM money_transactions t
               JOIN money_accounts a ON t.account_id = a.id
               WHERE a.user_id = ?"#
        );

        let mut conditions = vec![];

        if let Some(start_date) = arguments["start_date"].as_str() {
            conditions.push(format!("t.transaction_date >= '{}'", start_date));
        }
        if let Some(end_date) = arguments["end_date"].as_str() {
            conditions.push(format!("t.transaction_date <= '{}'", end_date));
        }
        if let Some(tx_type) = arguments["transaction_type"].as_str() {
            conditions.push(format!("t.transaction_type = '{}'", tx_type));
        }

        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY t.transaction_date DESC LIMIT ?");

        let results = sqlx::query_as::<_, (String, String, String, f64, Option<String>, String, String)>(&sql)
            .bind(self.user_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let transactions: Vec<Value> = results
            .iter()
            .map(|(id, date, description, amount, category, tx_type, account_name)| {
                serde_json::json!({
                    "id": id,
                    "date": date,
                    "description": description,
                    "amount": amount,
                    "category": category,
                    "type": tx_type,
                    "account": account_name
                })
            })
            .collect();

        Ok(serde_json::json!({
            "transactions": transactions,
            "count": transactions.len()
        }))
    }
}

pub fn create_tool_registry(pool: SqlitePool, user_id: i64) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // Email tools
    registry.register(Arc::new(EmailSearchTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(EmailReadTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(EmailComposeTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(EmailSendTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(EmailOrganizeTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(ContactLookupTool::new(pool.clone(), user_id)));
    
    // Calendar tools
    registry.register(Arc::new(CalendarSearchTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(CalendarCreateTool::new(pool.clone(), user_id)));
    
    // Reminder tools
    registry.register(Arc::new(ReminderCreateTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(ReminderSearchTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(ReminderUpdateTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(ReminderCompleteTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(ReminderDeleteTool::new(pool.clone(), user_id)));
    
    // Money tools
    registry.register(Arc::new(MoneyAccountListTool::new(pool.clone(), user_id)));
    registry.register(Arc::new(MoneyTransactionSearchTool::new(pool.clone(), user_id)));

    registry
}
