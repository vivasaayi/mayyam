use bcrypt::hash;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <password>", args[0]);
        return;
    }

    let password = &args[1];
    match hash(password, 10) {
        Ok(hashed) => println!("{}", hashed),
        Err(e) => println!("Error: {}", e),
    }
}
