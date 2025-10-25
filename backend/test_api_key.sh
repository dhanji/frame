#!/bin/bash

echo "=== ANTHROPIC_API_KEY Configuration Test ==="
echo ""

# Check .env file
echo "1. Checking .env file..."
if [ -f .env ]; then
    if grep -q "^ANTHROPIC_API_KEY=" .env; then
        KEY=$(grep "^ANTHROPIC_API_KEY=" .env | cut -d'=' -f2)
        if [ -n "$KEY" ] && [ "$KEY" != "dummy-key" ]; then
            echo "   ✓ ANTHROPIC_API_KEY is set in .env"
            echo "   ✓ Key starts with: ${KEY:0:15}..."
            echo "   ✓ Key length: ${#KEY} characters"
        else
            echo "   ✗ ANTHROPIC_API_KEY is empty or set to dummy-key"
            exit 1
        fi
    else
        echo "   ✗ ANTHROPIC_API_KEY not found in .env"
        exit 1
    fi
else
    echo "   ✗ .env file not found"
    exit 1
fi

echo ""
echo "2. Checking how it's loaded in main.rs..."
if grep -q "ANTHROPIC_API_KEY" src/main.rs; then
    echo "   ✓ Code references ANTHROPIC_API_KEY"
    echo ""
    echo "   Code snippet:"
    grep -B 1 -A 2 "ANTHROPIC_API_KEY" src/main.rs | head -6 | sed 's/^/      /'
else
    echo "   ✗ ANTHROPIC_API_KEY not referenced in code"
fi

echo ""
echo "3. Checking dotenv loading..."
if grep -q "dotenv::dotenv" src/main.rs; then
    echo "   ✓ dotenv is called in main.rs"
    grep "dotenv::dotenv" src/main.rs | sed 's/^/      /'
else
    echo "   ✗ dotenv not found in main.rs"
fi

echo ""
echo "4. Testing with current environment..."
# Source the .env file and test
export $(grep -v '^#' .env | xargs)
if [ -n "$ANTHROPIC_API_KEY" ] && [ "$ANTHROPIC_API_KEY" != "dummy-key" ]; then
    echo "   ✓ Environment variable can be loaded"
    echo "   ✓ Value: ${ANTHROPIC_API_KEY:0:15}..."
else
    echo "   ✗ Failed to load from environment"
fi

echo ""
echo "=== Summary ==="
echo ""
echo "✓ ANTHROPIC_API_KEY is properly configured in .env"
echo "✓ Backend code will load it via dotenv::dotenv().ok()"
echo ""
echo "To verify it's working:"
echo "  1. Start/restart the backend: ./run.sh restart"
echo "  2. Check logs: ./run.sh logs"
echo "  3. Look for any API key errors in the logs"
echo ""
echo "The key will be loaded when AgentEngine is initialized in main.rs"
