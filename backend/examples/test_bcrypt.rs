use bcrypt::{hash, verify, DEFAULT_COST};

fn main() {
    let password = "test";
    let new_hash = hash(password, DEFAULT_COST).unwrap();
    println!("New hash: {}", new_hash);
    println!("Verify new: {}", verify(password, &new_hash).unwrap());
    
    let old_hash = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyYqgOqKqKqK";
    match verify(password, old_hash) {
        Ok(valid) => println!("Verify old: {}", valid),
        Err(e) => println!("Error: {}", e),
    }
}
