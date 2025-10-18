use lettre::{
    message::{header::ContentType, Mailbox, Message, MultiPart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailComposition {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<AttachmentData>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentData {
    pub filename: String,
    pub content_type: String,
    pub content: Vec<u8>,
}

pub struct SmtpService {
    host: String,
    port: u16,
    username: String,
    password: String,
    use_tls: bool,
}

impl SmtpService {
    pub fn new(
        host: String,
        port: u16,
        username: String,
        password: String,
        use_tls: bool,
    ) -> Self {
        Self {
            host,
            port,
            username,
            password,
            use_tls,
        }
    }

    pub async fn send_email(
        &self,
        email: EmailComposition,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Parse from address
        let from_mailbox: Mailbox = email.from.parse()?;
        
        // Build the message
        let mut message_builder = Message::builder()
            .from(from_mailbox)
            .subject(&email.subject);
        
        // Add To recipients
        for to_addr in &email.to {
            let mailbox: Mailbox = to_addr.parse()?;
            message_builder = message_builder.to(mailbox);
        }
        
        // Add CC recipients
        for cc_addr in &email.cc {
            let mailbox: Mailbox = cc_addr.parse()?;
            message_builder = message_builder.cc(mailbox);
        }
        
        // Add BCC recipients
        for bcc_addr in &email.bcc {
            let mailbox: Mailbox = bcc_addr.parse()?;
            message_builder = message_builder.bcc(mailbox);
        }
        
        // Add In-Reply-To header if this is a reply
        if let Some(in_reply_to) = &email.in_reply_to {
            message_builder = message_builder.in_reply_to(in_reply_to.clone());
        }
        
        // Add References header
        if !email.references.is_empty() {
            let references = email.references.join(" ");
            message_builder = message_builder.references(references);
        }
        
        // Build the message body
        let multipart;
        
        // Add text/html content
        if let Some(html) = &email.body_html {
            if let Some(text) = &email.body_text {
                // Both HTML and plain text
                let alternative = MultiPart::alternative()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text.clone()),
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html.clone()),
                    );
                // Use mixed multipart with alternative as subpart
                multipart = MultiPart::mixed().multipart(alternative);
            } else {
                // HTML only
                multipart = MultiPart::mixed().singlepart(
                    lettre::message::SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html.clone()),
                );
            }
        } else if let Some(text) = &email.body_text {
            // Plain text only
            multipart = MultiPart::mixed().singlepart(
                lettre::message::SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(text.clone()),
            );
        } else {
            // No body content
            multipart = MultiPart::mixed().build();
        }
        
        // Attachments not implemented yet - would need to chain .singlepart() calls
        
        let message = message_builder.multipart(multipart)?;
        
        // Create SMTP transport
        let mailer = if self.use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.host)?
                .port(self.port)
                .credentials(Credentials::new(
                    self.username.clone(),
                    self.password.clone(),
                ))
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.host)
                .port(self.port)
                .credentials(Credentials::new(
                    self.username.clone(),
                    self.password.clone(),
                ))
                .build()
        };
        
        // Send the email
        let response = mailer.send(message).await?;
        
        Ok(response.message().collect::<String>())
    }

    pub async fn send_reply(
        &self,
        original_message_id: &str,
        original_references: Vec<String>,
        reply: EmailComposition,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut email = reply;
        
        // Set In-Reply-To header
        email.in_reply_to = Some(original_message_id.to_string());
        
        // Build References header
        let mut references = original_references;
        references.push(original_message_id.to_string());
        email.references = references;
        
        self.send_email(email).await
    }

    pub async fn send_forward(
        &self,
        original_message: &str,
        forward: EmailComposition,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut email = forward;
        
        // Prepend forwarded message to body
        let forward_header = "\n\n---------- Forwarded message ----------\n";
        
        if let Some(text) = &mut email.body_text {
            text.push_str(forward_header);
            text.push_str(original_message);
        }
        
        if let Some(html) = &mut email.body_html {
            html.push_str("<br><br><hr><p>---------- Forwarded message ----------</p>");
            html.push_str("<blockquote>");
            html.push_str(original_message);
            html.push_str("</blockquote>");
        }
        
        self.send_email(email).await
    }

    pub async fn test_connection(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mailer: AsyncSmtpTransport<Tokio1Executor> = if self.use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.host)?
                .port(self.port)
                .credentials(Credentials::new(
                    self.username.clone(),
                    self.password.clone(),
                ))
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.host)
                .port(self.port)
                .credentials(Credentials::new(
                    self.username.clone(),
                    self.password.clone(),
                ))
                .build()
        };
        
        mailer.test_connection().await?;
        Ok(())
    }
}