// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


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
