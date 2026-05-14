fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "world".to_string());
    println!("hello {}", name.to_ascii_lowercase());
}
