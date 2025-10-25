use email_client_backend::services::ConversationService;
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("Connecting to database...");
    let pool = SqlitePool::connect("sqlite:email_client.db").await?;
    
    println!("Creating conversation service...");
    let conv_service = ConversationService::new(pool.clone());
    
    println!("Rebuilding conversations from existing emails...");
    let count = conv_service.rebuild_all_conversations().await?;
    
    println!("✅ Successfully rebuilt {} conversations", count);
    
    // Show results
    let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM conversations")
        .fetch_one(&pool)
        .await?;
    
    println!("Total conversations in database: {}", result.0);
    
    Ok(())
}
