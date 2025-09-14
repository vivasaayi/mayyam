fn c(growableStr:&String) {
    println!("c: {}", growableStr);
}

fn b(growableStr:&String) {
    println!("b: {}", growableStr);
    c(growableStr);
}

fn a(growableStr:&String) {
    println!("a: {}", growableStr);
    b(growableStr);
}

fn stringTest() {
    let strSlice: &str = "hello";
    let growableStr: String = "world".to_string();

    let growableStrSlice = &growableStr[2..4];
    let sliceOfStrSlice = &strSlice[1..4];

    println!("growableStrSlice: {}", growableStrSlice);
    println!("sliceOfStrSlice: {}", sliceOfStrSlice);
    println!("Completed");

    let borrowsGrowableString = &growableStr;
    let anotherBorrow = &growableStr;
    let anAdditionalBorrow = &growableStr;

    println!("borrowsGrowableString: {}", borrowsGrowableString);
    println!("anotherBorrow: {}", anotherBorrow);
    println!("anAdditionalBorrow: {}", anAdditionalBorrow);

    a(anAdditionalBorrow);
}
fn main() {
    stringTest();
}
