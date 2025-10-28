use actix_web::HttpResponse;
use serde::{Deserialize, Serialize};
use crate::middleware::auth::AuthenticatedUser;

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub param_type: String,
    pub description: String,
    pub required: bool,
}

pub async fn list_tools(
    _user: AuthenticatedUser,
) -> HttpResponse {
    let tools = vec![
        ToolInfo {
            name: "email_search".to_string(),
            description: "Search through emails using natural language queries. Can filter by sender, subject, date range, and content.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "query".to_string(),
                    param_type: "string".to_string(),
                    description: "Search query (e.g., 'emails from john about project')".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "limit".to_string(),
                    param_type: "number".to_string(),
                    description: "Maximum number of results (default: 10)".to_string(),
                    required: false,
                },
            ],
        },
        ToolInfo {
            name: "email_read".to_string(),
            description: "Read the full content of a specific email by its ID.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "email_id".to_string(),
                    param_type: "string".to_string(),
                    description: "The unique identifier of the email".to_string(),
                    required: true,
                },
            ],
        },
        ToolInfo {
            name: "email_compose".to_string(),
            description: "Draft a new email. Creates a draft that can be reviewed before sending.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "to".to_string(),
                    param_type: "string".to_string(),
                    description: "Recipient email address".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "subject".to_string(),
                    param_type: "string".to_string(),
                    description: "Email subject line".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "body".to_string(),
                    param_type: "string".to_string(),
                    description: "Email body content".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "cc".to_string(),
                    param_type: "string".to_string(),
                    description: "CC recipients (comma-separated)".to_string(),
                    required: false,
                },
            ],
        },
        ToolInfo {
            name: "email_send".to_string(),
            description: "Send an email immediately. Use this after composing or for quick messages.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "to".to_string(),
                    param_type: "string".to_string(),
                    description: "Recipient email address".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "subject".to_string(),
                    param_type: "string".to_string(),
                    description: "Email subject line".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "body".to_string(),
                    param_type: "string".to_string(),
                    description: "Email body content".to_string(),
                    required: true,
                },
            ],
        },
        ToolInfo {
            name: "email_organize".to_string(),
            description: "Move or organize emails into folders. Can archive, trash, or move to custom folders.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "email_id".to_string(),
                    param_type: "string".to_string(),
                    description: "The email to organize".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "action".to_string(),
                    param_type: "string".to_string(),
                    description: "Action: 'archive', 'trash', 'move', 'star', 'unstar'".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "folder".to_string(),
                    param_type: "string".to_string(),
                    description: "Target folder (required for 'move' action)".to_string(),
                    required: false,
                },
            ],
        },
        ToolInfo {
            name: "contact_lookup".to_string(),
            description: "Look up contact information from your email history. Finds email addresses and names.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "name".to_string(),
                    param_type: "string".to_string(),
                    description: "Name or partial name to search for".to_string(),
                    required: true,
                },
            ],
        },
        ToolInfo {
            name: "calendar_search".to_string(),
            description: "Search calendar events by date range, title, or description.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "query".to_string(),
                    param_type: "string".to_string(),
                    description: "Search query (e.g., 'meetings next week')".to_string(),
                    required: false,
                },
                ToolParameter {
                    name: "date_from".to_string(),
                    param_type: "string".to_string(),
                    description: "Start date (ISO 8601 format)".to_string(),
                    required: false,
                },
                ToolParameter {
                    name: "date_to".to_string(),
                    param_type: "string".to_string(),
                    description: "End date (ISO 8601 format)".to_string(),
                    required: false,
                },
            ],
        },
        ToolInfo {
            name: "calendar_create".to_string(),
            description: "Create a new calendar event with title, time, location, and description.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "title".to_string(),
                    param_type: "string".to_string(),
                    description: "Event title".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "start_time".to_string(),
                    param_type: "string".to_string(),
                    description: "Start time (ISO 8601 format)".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "end_time".to_string(),
                    param_type: "string".to_string(),
                    description: "End time (ISO 8601 format)".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "description".to_string(),
                    param_type: "string".to_string(),
                    description: "Event description".to_string(),
                    required: false,
                },
                ToolParameter {
                    name: "location".to_string(),
                    param_type: "string".to_string(),
                    description: "Event location".to_string(),
                    required: false,
                },
            ],
        },
        ToolInfo {
            name: "reminder_create".to_string(),
            description: "Create a reminder for a specific date and time.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "title".to_string(),
                    param_type: "string".to_string(),
                    description: "Reminder title".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "due_date".to_string(),
                    param_type: "string".to_string(),
                    description: "Due date/time (ISO 8601 format)".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "description".to_string(),
                    param_type: "string".to_string(),
                    description: "Reminder description".to_string(),
                    required: false,
                },
            ],
        },
        ToolInfo {
            name: "reminder_search".to_string(),
            description: "Search and list reminders. Can filter by status (pending/completed).".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "status".to_string(),
                    param_type: "string".to_string(),
                    description: "Filter by status: 'pending', 'completed', or 'all'".to_string(),
                    required: false,
                },
            ],
        },
        ToolInfo {
            name: "money_account_list".to_string(),
            description: "List all connected financial accounts with balances.".to_string(),
            parameters: vec![],
        },
        ToolInfo {
            name: "money_transaction_search".to_string(),
            description: "Search financial transactions by date range, amount, or description.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "query".to_string(),
                    param_type: "string".to_string(),
                    description: "Search query (e.g., 'coffee purchases last month')".to_string(),
                    required: false,
                },
                ToolParameter {
                    name: "date_from".to_string(),
                    param_type: "string".to_string(),
                    description: "Start date (ISO 8601 format)".to_string(),
                    required: false,
                },
                ToolParameter {
                    name: "date_to".to_string(),
                    param_type: "string".to_string(),
                    description: "End date (ISO 8601 format)".to_string(),
                    required: false,
                },
            ],
        },
    ];

    HttpResponse::Ok().json(serde_json::json!({
        "tools": tools,
        "total": tools.len(),
    }))
}
