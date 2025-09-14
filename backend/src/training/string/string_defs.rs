fn stringTest() {
    let strSlice: &str = "hello";
    let growableStr: String = "world".to_string();

    let growableStrSlice = &growableStr[2..4];
    let sliceOfStrSlice = &strSlice[1..4];

    println!("growableStrSlice: {}", growableStrSlice);
    println!("sliceOfStrSlice: {}", sliceOfStrSlice);
    println!("Completed");
}
fn main() {
    stringTest();
}
