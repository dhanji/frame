#!/bin/bash

# Test script to verify ANTHROPIC_API_KEY is being picked up

echo "=== Testing Environment Variable Loading ==="
echo ""

# Check if .env file exists
if [ -f .env ]; then
    echo "✓ .env file exists"
    echo ""
    
    # Check if ANTHROPIC_API_KEY is in .env
    if grep -q "ANTHROPIC_API_KEY" .env; then
        echo "✓ ANTHROPIC_API_KEY found in .env file"
        
        # Show the key (masked for security)
        KEY=$(grep "ANTHROPIC_API_KEY" .env | cut -d'=' -f2)
        KEY_PREFIX=$(echo "$KEY" | cut -c1-20)
        echo "  Value: ${KEY_PREFIX}..."
        echo ""
    else
        echo "✗ ANTHROPIC_API_KEY not found in .env file"
        exit 1
    fi
else
    echo "✗ .env file not found"
    exit 1
fi

# Test if Rust can load it
echo "=== Testing Rust Environment Loading ==="
echo ""

# Create a simple test program
cat > /tmp/test_env.rs << 'EOF'
fn main() {
    dotenv::dotenv().ok();
    
    match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) => {
            if key.is_empty() {
                println!("✗ ANTHROPIC_API_KEY is empty");
                std::process::exit(1);
            } else if key == "dummy-key" {
                println!("✗ ANTHROPIC_API_KEY is set to 'dummy-key' (default fallback)");
                std::process::exit(1);
            } else {
                let prefix = if key.len() > 20 { &key[..20] } else { &key };
                println!("✓ ANTHROPIC_API_KEY loaded successfully");
                println!("  Prefix: {}...", prefix);
                println!("  Length: {} characters", key.len());
                
                // Validate format (should start with sk-ant-)
                if key.starts_with("sk-ant-") {
                    println!("✓ Key format looks valid (starts with sk-ant-)");
                } else {
                    println!("⚠ Warning: Key doesn't start with expected prefix 'sk-ant-'");
                }
            }
        }
        Err(_) => {
            println!("✗ ANTHROPIC_API_KEY not found in environment");
            std::process::exit(1);
        }
    }
}
EOF

# Try to compile and run the test
if command -v rustc &> /dev/null; then
    cd /tmp
    if cargo new --bin test_env_check &> /dev/null; then
        cd test_env_check
        echo 'dotenv = "0.15"' >> Cargo.toml
        cp /tmp/test_env.rs src/main.rs
        cp "$(dirname "$0")/.env" .
        
        if cargo run --quiet 2>/dev/null; then
            echo ""
            echo "=== Summary ==="
            echo "✓ Environment variable is properly configured and accessible"
        else
            echo ""
            echo "=== Summary ==="
            echo "✗ Failed to load environment variable in Rust"
        fi
        
        cd - > /dev/null
        rm -rf /tmp/test_env_check
    fi
else
    echo "⚠ Rust compiler not found, skipping Rust test"
fi

echo ""
echo "=== Checking Backend Code ==="
echo ""

# Check how it's used in main.rs
if grep -q "ANTHROPIC_API_KEY" src/main.rs; then
    echo "✓ ANTHROPIC_API_KEY is referenced in main.rs"
    echo ""
    echo "Usage in code:"
    grep -A 1 -B 1 "ANTHROPIC_API_KEY" src/main.rs | head -5
else
    echo "✗ ANTHROPIC_API_KEY not found in main.rs"
fi

echo ""
echo "=== Recommendations ==="
echo ""
echo "1. Make sure to restart the backend server after changing .env"
echo "2. The backend loads .env on startup via dotenv::dotenv().ok()"
echo "3. Check server logs for any API key related errors"
echo "4. To verify at runtime, check the logs when the server starts"
