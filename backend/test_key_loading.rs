// Test program to verify ANTHROPIC_API_KEY loading
// This simulates exactly what the backend does

fn main() {
    println!("===========================================");
    println!("ANTHROPIC_API_KEY Loading Test");
    println!("===========================================");
    println!();
    
    // Step 1: Load .env file (same as backend)
    println!("1. Loading .env file...");
    match dotenv::dotenv() {
        Ok(path) => println!("   ✅ Loaded .env from: {:?}", path),
        Err(e) => {
            println!("   ❌ Failed to load .env: {}", e);
            std::process::exit(1);
        }
    }
    println!();
    
    // Step 2: Read ANTHROPIC_API_KEY (same as backend)
    println!("2. Reading ANTHROPIC_API_KEY...");
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .unwrap_or_else(|_| "dummy-key".to_string());
    
    println!("   Key value: {}", api_key);
    println!();
    
    // Step 3: Validate the key
    println!("3. Validating key...");
    
    if api_key == "dummy-key" {
        println!("   ❌ FAILED: Key is 'dummy-key' (fallback value)");
        println!("   This means ANTHROPIC_API_KEY was not found in environment");
        println!();
        println!("   Possible causes:");
        println!("   - .env file doesn't contain ANTHROPIC_API_KEY");
        println!("   - .env file is not in the current directory");
        println!("   - Environment variable is not set");
        std::process::exit(1);
    }
    
    if api_key.is_empty() {
        println!("   ❌ FAILED: Key is empty");
        std::process::exit(1);
    }
    
    if !api_key.starts_with("sk-ant-") {
        println!("   ⚠️  WARNING: Key doesn't start with 'sk-ant-'");
        println!("   Expected format: sk-ant-api03-...");
        println!("   Actual prefix: {}", &api_key[..api_key.len().min(10)]);
    } else {
        println!("   ✅ Key format is valid (starts with 'sk-ant-')");
    }
    
    println!("   ✅ Key length: {} characters", api_key.len());
    println!("   ✅ Key prefix: {}...", &api_key[..api_key.len().min(20)]);
    println!();
    
    // Step 4: Check model
    println!("4. Reading ANTHROPIC_MODEL...");
    let model = std::env::var("ANTHROPIC_MODEL")
        .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string());
    println!("   Model: {}", model);
    println!();
    
    // Summary
    println!("===========================================");
    println!("Summary");
    println!("===========================================");
    println!();
    println!("✅ SUCCESS: ANTHROPIC_API_KEY is properly loaded!");
    println!();
    println!("Configuration:");
    println!("  API Key: {}...", &api_key[..api_key.len().min(20)]);
    println!("  Model: {}", model);
    println!();
    println!("This is exactly what the backend server does.");
    println!("The API key should be working in the application!");
}
