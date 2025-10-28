use sqlx::SqlitePool;

/// Service to enrich attachment metadata from parent emails
pub struct AttachmentEnrichmentService {
    pool: SqlitePool,
}

impl AttachmentEnrichmentService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Enrich attachment metadata when an email is saved
    pub async fn enrich_email_attachments(
        &self,
        email_id: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get email details
        let email = sqlx::query!(
            r#"
            SELECT from_address, date
            FROM emails
            WHERE id = ?
            "#,
            email_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(email) = email {
            // Extract sender name and email
            let (sender_name, sender_email) = parse_email_address(&email.from_address);

            // Update all attachments for this email
            sqlx::query!(
                r#"
                UPDATE attachments
                SET 
                    sender_email = ?,
                    sender_name = ?,
                    received_at = ?,
                    source_account = 'default'
                WHERE email_id = ?
                "#,
                sender_email,
                sender_name,
                email.date,
                email_id
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Enrich all existing attachments that are missing metadata
    pub async fn enrich_all_attachments(
        &self,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        // Get all attachments with email_id but missing sender info
        let attachments = sqlx::query!(
            r#"
            SELECT a.id, a.email_id, e.from_address, e.date
            FROM attachments a
            INNER JOIN emails e ON a.email_id = e.id
            WHERE a.sender_email IS NULL
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut count = 0;

        for attachment in attachments {
            if let Some(_email_id) = attachment.email_id {
                let (sender_name, sender_email) = parse_email_address(&attachment.from_address);

                sqlx::query!(
                    r#"
                    UPDATE attachments
                    SET 
                        sender_email = ?,
                        sender_name = ?,
                        received_at = ?,
                        source_account = 'default'
                    WHERE id = ?
                    "#,
                    sender_email,
                    sender_name,
                    attachment.date,
                    attachment.id
                )
                .execute(&self.pool)
                .await?;

                count += 1;
            }
        }

        Ok(count)
    }
}

/// Parse email address to extract name and email
fn parse_email_address(address: &str) -> (String, String) {
    // Handle formats like "John Doe <john@example.com>" or "john@example.com"
    if let Some(start) = address.find('<') {
        if let Some(end) = address.find('>') {
            let name = address[..start].trim().trim_matches('"').to_string();
            let email = address[start + 1..end].trim().to_string();
            return (name, email);
        }
    }
    
    // Just an email address
    (address.to_string(), address.to_string())
}
