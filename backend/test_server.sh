#!/bin/bash
cd "$(dirname "$0")"

# Kill any existing servers
pkill -9 email-server server 2>/dev/null
sleep 1

# Start the server binary
RUST_LOG=info ./target/release/server &
SERVER_PID=$!

echo "Server started with PID: $SERVER_PID"
echo "Waiting for server to be ready..."

# Wait for server to be ready (max 10 seconds)
for i in {1..10}; do
    if curl -s http://localhost:8080/health > /dev/null 2>&1; then
        echo "Server is ready!"
        exit 0
    fi
    sleep 1
done

echo "Server failed to start within 10 seconds"
exit 1
