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


fn main() {
    // ---- Block A: owned String inside Option ----
    let a = "Hello".to_string(); // (A1)
    let optA = Some(a); // (A2)  MOVE a into optA
    let unwrappedA = optA.unwrap(); // (A3)  MOVE String out of Option
    println!("{}", a); // (A4)  ❌ compile error: a was moved at (A2)
    println!("{:?}", optA); // (A5)  ❌ compile error: optA was moved at (A3)

    // ---- Block B: borrowed String reference inside Option ----
    let b = "Shark".to_string(); // (B1)
    let optB = Some(&b); // (B2)  store &String (borrow) inside Option
    let unwrappedB = optB.unwrap(); // (B3)  MOVE Option, but only returns &String
    println!("{}", b); // (B4)  ✅ ok, b not moved
    println!("{:?}", optB); // (B5)  ❌ compile error: optB was moved at (B3)
}
