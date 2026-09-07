fn main() {
    let dir = std::env::args().nth(1).unwrap_or_default();
    println!("{dir}");
}
