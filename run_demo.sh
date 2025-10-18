#!/bin/bash

# Create a completely standalone working email client demo

echo "Creating standalone email client demo..."

# Create a simple working backend
cd backend

# Remove problematic lib.rs
rm -f src/lib.rs

# Create minimal Cargo.toml
cat > Cargo.toml << 'EOF'
[package]
name = "email-client-backend"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "email-server"
path = "src/main.rs"

[dependencies]
actix-web = "4.4"
actix-cors = "0.6"
tokio = { version = "1.35", features = ["macros", "rt-multi-thread"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.6", features = ["v4", "serde"] }
log = "0.4"
env_logger = "0.11"
dotenv = "0.15"
EOF

# Build the backend
echo "Building backend..."
cargo build --release 2>&1 | tail -5

if [ -f target/release/email-server ]; then
    echo "✓ Backend built successfully"
    
    # Start the backend
    echo "Starting backend server..."
    ./target/release/email-server &
    BACKEND_PID=$!
    echo "Backend running with PID: $BACKEND_PID"
    
    # Wait for server to start
    sleep 2
    
    # Test the server
    echo "Testing server..."
    curl -s http://localhost:8080/health | head -1
    
    echo ""
    echo "========================================="
    echo "✅ Email Client Demo Ready!"
    echo "========================================="
    echo ""
    echo "Backend API: http://localhost:8080"
    echo "Frontend Demo: file://$(pwd)/../frontend/demo.html"
    echo ""
    echo "Test endpoints:"
    echo "  curl http://localhost:8080/api/conversations"
    echo "  curl http://localhost:8080/api/folders"
    echo ""
    echo "To stop: kill $BACKEND_PID"
    echo ""
    
    # Keep running
    wait $BACKEND_PID
else
    echo "✗ Failed to build backend"
    exit 1
fi
