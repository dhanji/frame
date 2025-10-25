use bcrypt::{hash, DEFAULT_COST};

fn main() {
    let password = "password";
    let hashed = hash(password, DEFAULT_COST).unwrap();
    println!("{}", hashed);
}
