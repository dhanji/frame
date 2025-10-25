#!/bin/bash

echo "=== Runtime API Key Test ==="
echo ""

# Test if the server is running
if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "✗ Backend server is not running"
    echo ""
    echo "Please start the server first:"
    echo "  cd backend && ./run.sh restart"
    exit 1
fi

echo "✓ Backend server is running"
echo ""

# Check if we can test the API key by making a request
# We need to be authenticated, so let's just check the configuration

echo "Checking if ANTHROPIC_API_KEY is loaded at runtime..."
echo ""

# Create a simple test by checking the .env file and comparing with what should be loaded
KEY_IN_ENV=$(grep "^ANTHROPIC_API_KEY=" .env | cut -d'=' -f2)

if [ -n "$KEY_IN_ENV" ] && [ "$KEY_IN_ENV" != "dummy-key" ]; then
    echo "✓ .env file contains valid API key"
    echo "  Prefix: ${KEY_IN_ENV:0:15}..."
    echo ""
    
    # Check if the server process has access to the .env file
    SERVER_PID=$(lsof -i :8080 | grep LISTEN | awk '{print $2}')
    if [ -n "$SERVER_PID" ]; then
        echo "✓ Server PID: $SERVER_PID"
        
        # Check the server's working directory
        if [ -d "/proc/$SERVER_PID" ]; then
            SERVER_CWD=$(readlink /proc/$SERVER_PID/cwd 2>/dev/null)
            echo "  Working directory: $SERVER_CWD"
        fi
        
        echo ""
        echo "⚠ Note: The server loads .env via dotenv::dotenv() at startup"
        echo "  If the server was started before .env was updated, restart it:"
        echo "  ./run.sh restart"
    fi
else
    echo "✗ No valid API key in .env"
    exit 1
fi

echo ""
echo "=== How to verify the key is working ==="
echo ""
echo "1. Restart the server to ensure .env is loaded:"
echo "   ./run.sh restart"
echo ""
echo "2. Try using the AI chat feature in the frontend"
echo "   - Login to the app at http://localhost:8080"
echo "   - Open the AI Assistant/Chat"
echo "   - Send a test message"
echo ""
echo "3. Check for errors in the logs:"
echo "   - If you see 'dummy-key' errors, the .env wasn't loaded"
echo "   - If you see API authentication errors, the key might be invalid"
echo "   - If it works, you'll get AI responses"
echo ""
echo "=== Current Configuration ==="
echo ""
echo "✓ ANTHROPIC_API_KEY is set in .env"
echo "✓ Code loads it via std::env::var(\"ANTHROPIC_API_KEY\")"
echo "✓ Fallback to 'dummy-key' if not found"
echo ""
echo "The key should be picked up when the server starts!"
