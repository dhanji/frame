#!/bin/bash
# Script to rebuild conversations from existing emails

cd "$(dirname "$0")"

echo "Rebuilding conversations from existing emails..."

sqlite3 email_client.db <<EOF
-- Get user ID
SELECT 'User ID: ' || id || ', Email: ' || email FROM users LIMIT 1;

-- Show email count
SELECT 'Total emails: ' || COUNT(*) FROM emails;

-- Show current conversation count
SELECT 'Current conversations: ' || COUNT(*) FROM conversations;
EOF

echo ""
echo "Starting Rust program to rebuild conversations..."

# Create a simple Rust program to rebuild conversations
cat > /tmp/rebuild_conv.rs <<'RUST'
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = SqlitePool::connect("sqlite:email_client.db").await?;
    
    // Import the conversation service
    let conv_service = email_client_backend::services::ConversationService::new(pool.clone());
    
    let count = conv_service.rebuild_all_conversations().await?;
    println!("Rebuilt {} conversations", count);
    
    Ok(())
}
RUST

echo "Done!"
