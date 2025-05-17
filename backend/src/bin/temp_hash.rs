// This is a temporary file to generate a bcrypt hash
use bcrypt::hash;

fn main() {
    let password = "password1";
    match hash(password, 10) {
        Ok(hashed) => println!("{}", hashed),
        Err(e) => println!("Error: {}", e),
    }
}
