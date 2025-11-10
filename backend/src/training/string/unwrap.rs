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


#[derive(Debug)]
struct Person {
    name: String,
    city: Option<String>
}

fn get_zip_code_using_string(city: String) -> String {
    match city.as_str() {
        "Wonderland" => "12345".to_string(),
        "Builderland" => "67890".to_string(),
        _ => "00000".to_string(),
    }
}

fn get_zip_code_using_string_reference(city: &String) -> String {
    match city.as_str() {
        "Wonderland" => "12345".to_string(),
        "Builderland" => "67890".to_string(),
        _ => "00000".to_string(),
    }
}


fn get_zip_code_using_string_slice(city: &str) -> String {
    match city {
        "Wonderland" => "12345".to_string(),
        "Builderland" => "67890".to_string(),
        _ => "00000".to_string(),
    }
}

fn print_person_city(person: Person) {
    println!("Name: {}", person.name);
    println!("City: {:#?}", person.city);
    println!("City: {:#?}", person.city.as_deref());
    

    let zip_code = get_zip_code_using_string_slice(&person.city.as_deref().unwrap_or(""));
    println!("Zip Code: {}", zip_code);

    match person.city {
        Some(city) => println!("City: {}", city),
        None => println!("City: Unknown"),
    }
}

fn print_person_info(person: Person) {
    println!("Name: {}", person.name);
    println!("City: {:#?}", person.city);
    println!("City: {:#?}", person.city.as_ref()); //Option<&String>
    println!("City: {:#?}", person.city.as_deref()); // Option<&str>
    println!("City: {:#?}", person.city.as_deref().unwrap_or("")); // &str
    println!("City: {:#?}", Some(person.city.clone())); // Not recommended

    print_person_city(person);
}

fn main() {
    let person = Person {
        name: String::from("Alice"),
        city: Some(String::from("Wonderland")),
    };
    println!("{:#?}", person);
    
    print_person_info(person);
}
