#!/bin/bash

echo "=========================================="
echo "ANTHROPIC_API_KEY Configuration Check"
echo "=========================================="
echo ""

# 1. Check .env file
echo "1. Checking .env file..."
if [ -f .env ]; then
    KEY=$(grep "^ANTHROPIC_API_KEY=" .env | cut -d'=' -f2)
    if [ -n "$KEY" ] && [ "$KEY" != "dummy-key" ]; then
        echo "   ✅ ANTHROPIC_API_KEY found in .env"
        echo "   📝 Key prefix: ${KEY:0:20}..."
        echo "   📏 Key length: ${#KEY} characters"
        
        # Validate key format
        if [[ $KEY == sk-ant-* ]]; then
            echo "   ✅ Key format is valid (starts with sk-ant-)"
        else
            echo "   ⚠️  Warning: Key doesn't start with 'sk-ant-'"
        fi
    else
        echo "   ❌ ANTHROPIC_API_KEY is missing or set to dummy-key"
        exit 1
    fi
else
    echo "   ❌ .env file not found"
    exit 1
fi

echo ""

# 2. Check code implementation
echo "2. Checking code implementation..."
if grep -q "std::env::var(\"ANTHROPIC_API_KEY\")" src/main.rs; then
    echo "   ✅ Code loads ANTHROPIC_API_KEY from environment"
    echo ""
    echo "   Code snippet:"
    grep -B 2 -A 2 "ANTHROPIC_API_KEY" src/main.rs | sed 's/^/      /'
else
    echo "   ❌ ANTHROPIC_API_KEY not found in code"
    exit 1
fi

echo ""

# 3. Check dotenv loading
echo "3. Checking dotenv initialization..."
if grep -q "dotenv::dotenv()" src/main.rs; then
    echo "   ✅ dotenv::dotenv() is called at startup"
    LINE=$(grep -n "dotenv::dotenv()" src/main.rs | head -1)
    echo "   📍 Location: $LINE"
else
    echo "   ❌ dotenv not initialized"
    exit 1
fi

echo ""

# 4. Check server status
echo "4. Checking server status..."
if lsof -i :8080 > /dev/null 2>&1; then
    PID=$(lsof -i :8080 | grep LISTEN | awk '{print $2}')
    echo "   ✅ Server is running (PID: $PID)"
    
    # Check when server was started
    START_TIME=$(ps -p $PID -o lstart= 2>/dev/null)
    echo "   🕐 Started: $START_TIME"
    
    # Check when .env was modified
    ENV_MOD=$(stat -f '%Sm' -t '%Y-%m-%d %H:%M:%S' .env 2>/dev/null || stat -c '%y' .env 2>/dev/null)
    echo "   📝 .env modified: $ENV_MOD"
    
    echo ""
    echo "   ⚠️  To ensure the latest .env is loaded, restart the server:"
    echo "      ./run.sh restart"
else
    echo "   ⚠️  Server is not running"
    echo "   💡 Start it with: ./run.sh start"
fi

echo ""

# 5. Summary
echo "=========================================="
echo "Summary"
echo "=========================================="
echo ""
echo "✅ ANTHROPIC_API_KEY is properly configured in .env"
echo "✅ Backend code will load it via dotenv at startup"
echo "✅ The key will be used by AgentEngine for AI features"
echo ""
echo "How it works:"
echo "  1. Server starts and calls dotenv::dotenv().ok()"
echo "  2. This loads all variables from .env into environment"
echo "  3. AgentEngine reads ANTHROPIC_API_KEY via std::env::var()"
echo "  4. If not found, it falls back to 'dummy-key'"
echo ""
echo "To test if it's working:"
echo "  1. Restart server: ./run.sh restart"
echo "  2. Login to http://localhost:8080"
echo "  3. Use the AI Chat feature"
echo "  4. If you get AI responses, the key is working!"
echo "  5. If you see 'dummy-key' errors, the key wasn't loaded"
echo ""
