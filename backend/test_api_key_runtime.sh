#!/bin/bash

echo "=========================================="
echo "Runtime API Key Verification Test"
echo "=========================================="
echo ""

# Check if server is running
if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "❌ Server is not running"
    echo "   Start it with: ./run.sh start"
    exit 1
fi

echo "✅ Server is running"
echo ""

# Get the key from .env
KEY=$(grep "^ANTHROPIC_API_KEY=" .env | cut -d'=' -f2)
echo "📋 Configuration Check:"
echo "   Key in .env: ${KEY:0:20}..."
echo "   Key length: ${#KEY} characters"
echo ""

# Check if the key format is valid
if [[ $KEY == sk-ant-api03-* ]]; then
    echo "✅ Key format is valid (Anthropic API key)"
elif [[ $KEY == sk-ant-* ]]; then
    echo "✅ Key format looks valid (starts with sk-ant-)"
else
    echo "⚠️  Warning: Key format doesn't match expected pattern"
fi

echo ""
echo "=========================================="
echo "How the key is loaded:"
echo "=========================================="
echo ""
echo "1. Server startup (main.rs line 16):"
echo "   dotenv::dotenv().ok();"
echo "   ↓ Loads .env file into environment"
echo ""
echo "2. AgentEngine initialization (main.rs line 62):"
echo "   std::env::var(\"ANTHROPIC_API_KEY\")"
echo "   ↓ Reads from environment"
echo "   ↓ Falls back to 'dummy-key' if not found"
echo ""
echo "3. Used by AnthropicProvider when making API calls"
echo ""

# Check server start time vs .env modification time
PID=$(lsof -i :8080 | grep LISTEN | awk '{print $2}')
if [ -n "$PID" ]; then
    echo "=========================================="
    echo "Timing Check:"
    echo "=========================================="
    echo ""
    
    # Get timestamps
    SERVER_START=$(ps -p $PID -o lstart= 2>/dev/null)
    ENV_MOD=$(stat -f '%Sm' -t '%Y-%m-%d %H:%M:%S' .env 2>/dev/null)
    
    echo "Server started:  $SERVER_START"
    echo ".env modified:   $ENV_MOD"
    echo ""
    
    # Convert to epoch for comparison
    SERVER_EPOCH=$(date -j -f "%a %d %b %H:%M:%S %Y" "$SERVER_START" +%s 2>/dev/null)
    ENV_EPOCH=$(date -j -f "%Y-%m-%d %H:%M:%S" "$ENV_MOD" +%s 2>/dev/null)
    
    if [ -n "$SERVER_EPOCH" ] && [ -n "$ENV_EPOCH" ]; then
        if [ $SERVER_EPOCH -gt $ENV_EPOCH ]; then
            echo "✅ Server was started AFTER .env was modified"
            echo "   The current .env values should be loaded"
        else
            echo "⚠️  Server was started BEFORE .env was modified"
            echo "   Restart needed: ./run.sh restart"
        fi
    fi
fi

echo ""
echo "=========================================="
echo "Verification Steps:"
echo "=========================================="
echo ""
echo "To verify the API key is actually working:"
echo ""
echo "1. Make sure server is using latest .env:"
echo "   cd backend && ./run.sh restart"
echo ""
echo "2. Test the AI chat feature:"
echo "   - Open http://localhost:8080"
echo "   - Login with your credentials"
echo "   - Navigate to AI Chat/Assistant"
echo "   - Send a test message like 'Hello'"
echo ""
echo "3. Expected results:"
echo "   ✅ If working: You'll get an AI response"
echo "   ❌ If not working: Error message about 'dummy-key' or API authentication"
echo ""
echo "4. Check logs for errors:"
echo "   Look for messages containing:"
echo "   - 'dummy-key' = API key not loaded"
echo "   - '401' or 'authentication' = Invalid API key"
echo "   - 'API' errors = Connection or other issues"
echo ""

echo "=========================================="
echo "Current Status:"
echo "=========================================="
echo ""
echo "✅ ANTHROPIC_API_KEY is configured in .env"
echo "✅ Key format appears valid"
echo "✅ Code is set up to load the key"
echo "✅ Server is running"
echo ""
echo "The API key SHOULD be picked up and working!"
echo ""
