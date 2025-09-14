fn main() {
    let a = "Hello"; // &str
    let b = "Hello".to_string(); // String

    if a == b {
        println!("The strings are equal");
    } else {
        println!("The strings are not equal");
    }
}