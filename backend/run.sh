#!/bin/bash

# Frame Email Client Backend Runner
# This script starts the backend server

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo "🚀 Starting Frame Email Client Backend..."

# Check if database exists
if [ ! -f "email_client.db" ]; then
    echo "📦 Database not found. Creating..."
    sqlx database create 2>/dev/null || true
    sqlx migrate run 2>/dev/null || true
fi

# Check if binary exists
if [ ! -f "target/release/email-server" ]; then
    echo "🔨 Building backend..."
    cargo build --release
fi

# Start the server
echo "✅ Starting server on http://localhost:8080"
exec ./target/release/email-server
