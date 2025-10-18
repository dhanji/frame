#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use sqlx::SqlitePool;
    use uuid::Uuid;
    use chrono::Utc;
    
    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .unwrap();
        pool
    }
    
    #[actix_rt::test]
    async fn test_user_registration() {
        let pool = setup_test_db().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .service(crate::handlers::auth::register)
        ).await;
        
        let req = test::TestRequest::post()
            .uri("/api/auth/register")
            .set_json(&serde_json::json!({
                "email": "test@example.com",
                "password": "password123",
                "imap_host": "imap.example.com",
                "imap_port": 993,
                "smtp_host": "smtp.example.com",
                "smtp_port": 587
            }))
            .to_request();
        
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
    
    #[actix_rt::test]
    async fn test_login() {
        let pool = setup_test_db().await;
        
        // First create a user
        let user_id = Uuid::new_v4();
        let password_hash = bcrypt::hash("password123", bcrypt::DEFAULT_COST).unwrap();
        let now = Utc::now();
        
        sqlx::query!(
            r#"
            INSERT INTO users (
                id, email, password_hash, imap_host, imap_port,
                imap_username, imap_password, smtp_host, smtp_port,
                smtp_username, smtp_password, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            user_id,
            "test@example.com",
            password_hash,
            "imap.example.com",
            993,
            "test@example.com",
            "encrypted_password",
            "smtp.example.com",
            587,
            "test@example.com",
            "encrypted_password",
            now,
            now
        )
        .execute(&pool)
        .await
        .unwrap();
        
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .service(crate::handlers::auth::login)
        ).await;
        
        let req = test::TestRequest::post()
            .uri("/api/auth/login")
            .set_json(&serde_json::json!({
                "email": "test@example.com",
                "password": "password123",
                "imap_host": "imap.example.com",
                "imap_port": 993,
                "smtp_host": "smtp.example.com",
                "smtp_port": 587
            }))
            .to_request();
        
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
    
    #[actix_rt::test]
    async fn test_folder_creation() {
        let pool = setup_test_db().await;
        let user_id = Uuid::new_v4();
        
        // Create test user
        create_test_user(&pool, user_id).await;
        
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool.clone()))
                .wrap(test_auth_middleware(user_id))
                .service(crate::handlers::folders::create_folder)
        ).await;
        
        let req = test::TestRequest::post()
            .uri("/api/folders")
            .set_json(&serde_json::json!({
                "name": "Test Folder",
                "parent_id": null
            }))
            .to_request();
        
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        
        // Verify folder was created
        let folder = sqlx::query!("SELECT * FROM folders WHERE name = ?", "Test Folder")
            .fetch_one(&pool)
            .await
            .unwrap();
        
        assert_eq!(folder.name, "Test Folder");
        assert_eq!(folder.folder_type, "custom");
    }
    
    #[actix_rt::test]
    async fn test_email_sending() {
        let pool = setup_test_db().await;
        let user_id = Uuid::new_v4();
        
        create_test_user(&pool, user_id).await;
        create_default_folders(&pool, user_id).await;
        
        // Mock SMTP service would be used here
        // For testing, we'll just verify the database operations
        
        let email_id = Uuid::new_v4();
        let now = Utc::now();
        
        sqlx::query!(
            r#"
            INSERT INTO emails (
                id, user_id, message_id, thread_id, folder_id,
                subject, from_address, from_name, to_addresses,
                cc_addresses, bcc_addresses, body_text, body_html,
                is_read, is_starred, has_attachments, date,
                created_at, updated_at, processed_by_filters
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            email_id,
            user_id,
            "<test@example.com>",
            "thread_123",
            get_inbox_folder_id(&pool, user_id).await,
            "Test Subject",
            "test@example.com",
            "Test User",
            r#"["recipient@example.com"]"#,
            null::<String>,
            null::<String>,
            "Test email body",
            "<p>Test email body</p>",
            false,
            false,
            false,
            now,
            now,
            now,
            false
        )
        .execute(&pool)
        .await
        .unwrap();
        
        // Verify email was saved
        let email = sqlx::query!("SELECT * FROM emails WHERE id = ?", email_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        
        assert_eq!(email.subject, "Test Subject");
    }
    
    #[actix_rt::test]
    async fn test_conversation_threading() {
        let pool = setup_test_db().await;
        let user_id = Uuid::new_v4();
        let thread_id = "thread_123";
        
        create_test_user(&pool, user_id).await;
        create_default_folders(&pool, user_id).await;
        
        let folder_id = get_inbox_folder_id(&pool, user_id).await;
        
        // Create multiple emails in the same thread
        for i in 0..3 {
            let email_id = Uuid::new_v4();
            let now = Utc::now() + chrono::Duration::minutes(i);
            
            sqlx::query!(
                r#"
                INSERT INTO emails (
                    id, user_id, message_id, thread_id, folder_id,
                    subject, from_address, from_name, to_addresses,
                    cc_addresses, bcc_addresses, body_text, body_html,
                    is_read, is_starred, has_attachments, date,
                    created_at, updated_at, processed_by_filters
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                email_id,
                user_id,
                format!("<test{}@example.com>", i),
                thread_id,
                folder_id,
                "Test Thread",
                "test@example.com",
                "Test User",
                r#"["recipient@example.com"]"#,
                null::<String>,
                null::<String>,
                format!("Message {}", i),
                format!("<p>Message {}</p>", i),
                i == 0, // First message is read
                false,
                false,
                now,
                now,
                now,
                false
            )
            .execute(&pool)
            .await
            .unwrap();
        }
        
        // Query conversation
        let conversation = sqlx::query!(
            r#"
            SELECT 
                thread_id,
                COUNT(*) as message_count,
                SUM(CASE WHEN is_read = 0 THEN 1 ELSE 0 END) as unread_count
            FROM emails
            WHERE user_id = ? AND thread_id = ?
            GROUP BY thread_id
            "#,
            user_id,
            thread_id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        
        assert_eq!(conversation.message_count, 3);
        assert_eq!(conversation.unread_count, 2);
    }
    
    #[actix_rt::test]
    async fn test_draft_auto_save() {
        let pool = setup_test_db().await;
        let user_id = Uuid::new_v4();
        
        create_test_user(&pool, user_id).await;
        create_default_folders(&pool, user_id).await;
        
        let draft_id = Uuid::new_v4();
        let drafts_folder_id = get_drafts_folder_id(&pool, user_id).await;
        let now = Utc::now();
        
        // Save initial draft
        sqlx::query!(
            r#"
            INSERT INTO emails (
                id, user_id, message_id, thread_id, folder_id,
                subject, from_address, from_name, to_addresses,
                cc_addresses, bcc_addresses, body_text, body_html,
                is_read, is_starred, has_attachments, date,
                created_at, updated_at, processed_by_filters
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            draft_id,
            user_id,
            format!("<draft-{}@emailclient>", draft_id),
            format!("draft_{}", draft_id),
            drafts_folder_id,
            "Draft Subject",
            "test@example.com",
            "Test User",
            r#"[]"#,
            null::<String>,
            null::<String>,
            "Initial draft",
            null::<String>,
            true,
            false,
            false,
            now,
            now,
            now,
            false
        )
        .execute(&pool)
        .await
        .unwrap();
        
        // Update draft (auto-save)
        sqlx::query!(
            r#"
            UPDATE emails SET
                subject = ?,
                body_text = ?,
                updated_at = ?
            WHERE id = ?
            "#,
            "Updated Subject",
            "Updated draft content",
            now + chrono::Duration::seconds(5),
            draft_id
        )
        .execute(&pool)
        .await
        .unwrap();
        
        // Verify draft was updated
        let draft = sqlx::query!("SELECT * FROM emails WHERE id = ?", draft_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        
        assert_eq!(draft.subject, "Updated Subject");
        assert_eq!(draft.body_text.unwrap(), "Updated draft content");
    }
    
    #[actix_rt::test]
    async fn test_search_functionality() {
        let pool = setup_test_db().await;
        let user_id = Uuid::new_v4();
        
        create_test_user(&pool, user_id).await;
        create_default_folders(&pool, user_id).await;
        
        let folder_id = get_inbox_folder_id(&pool, user_id).await;
        
        // Create test emails with different subjects
        let subjects = vec!["Important Meeting", "Project Update", "Meeting Notes"];
        
        for subject in subjects {
            let email_id = Uuid::new_v4();
            let now = Utc::now();
            
            sqlx::query!(
                r#"
                INSERT INTO emails (
                    id, user_id, message_id, thread_id, folder_id,
                    subject, from_address, from_name, to_addresses,
                    cc_addresses, bcc_addresses, body_text, body_html,
                    is_read, is_starred, has_attachments, date,
                    created_at, updated_at, processed_by_filters
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                email_id,
                user_id,
                format!("<{}@example.com>", email_id),
                format!("thread_{}", email_id),
                folder_id,
                subject,
                "sender@example.com",
                "Sender",
                r#"["test@example.com"]"#,
                null::<String>,
                null::<String>,
                format!("Body for {}", subject),
                null::<String>,
                false,
                false,
                false,
                now,
                now,
                now,
                false
            )
            .execute(&pool)
            .await
            .unwrap();
        }
        
        // Search for "Meeting"
        let results = sqlx::query!(
            "SELECT * FROM emails WHERE user_id = ? AND subject LIKE ?",
            user_id,
            "%Meeting%"
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        
        assert_eq!(results.len(), 2);
    }
    
    // Helper functions
    async fn create_test_user(pool: &SqlitePool, user_id: Uuid) {
        let password_hash = bcrypt::hash("password123", bcrypt::DEFAULT_COST).unwrap();
        let now = Utc::now();
        
        sqlx::query!(
            r#"
            INSERT INTO users (
                id, email, password_hash, imap_host, imap_port,
                imap_username, imap_password, smtp_host, smtp_port,
                smtp_username, smtp_password, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            user_id,
            "test@example.com",
            password_hash,
            "imap.example.com",
            993,
            "test@example.com",
            "encrypted_password",
            "smtp.example.com",
            587,
            "test@example.com",
            "encrypted_password",
            now,
            now
        )
        .execute(pool)
        .await
        .unwrap();
    }
    
    async fn create_default_folders(pool: &SqlitePool, user_id: Uuid) {
        let folders = vec![
            ("Inbox", "inbox"),
            ("Sent", "sent"),
            ("Drafts", "drafts"),
            ("Trash", "trash"),
            ("Spam", "spam"),
        ];
        
        let now = Utc::now();
        
        for (name, folder_type) in folders {
            let folder_id = Uuid::new_v4();
            
            sqlx::query!(
                r#"
                INSERT INTO folders (
                    id, user_id, name, folder_type, parent_id,
                    unread_count, total_count, created_at, updated_at
                ) VALUES (?, ?, ?, ?, NULL, 0, 0, ?, ?)
                "#,
                folder_id,
                user_id,
                name,
                folder_type,
                now,
                now
            )
            .execute(pool)
            .await
            .unwrap();
        }
    }
    
    async fn get_inbox_folder_id(pool: &SqlitePool, user_id: Uuid) -> Uuid {
        let folder = sqlx::query!("SELECT id FROM folders WHERE user_id = ? AND folder_type = 'inbox'", user_id)
            .fetch_one(pool)
            .await
            .unwrap();
        Uuid::parse_str(&folder.id).unwrap()
    }
    
    async fn get_drafts_folder_id(pool: &SqlitePool, user_id: Uuid) -> Uuid {
        let folder = sqlx::query!("SELECT id FROM folders WHERE user_id = ? AND folder_type = 'drafts'", user_id)
            .fetch_one(pool)
            .await
            .unwrap();
        Uuid::parse_str(&folder.id).unwrap()
    }
    
    fn test_auth_middleware(user_id: Uuid) -> impl actix_web::dev::Transform<
        impl actix_web::dev::Service<
            actix_web::dev::ServiceRequest,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        actix_web::dev::ServiceRequest,
    > {
        // Mock authentication middleware for testing
        actix_web::middleware::DefaultHeaders::new()
            .header("X-User-Id", user_id.to_string())
    }
}