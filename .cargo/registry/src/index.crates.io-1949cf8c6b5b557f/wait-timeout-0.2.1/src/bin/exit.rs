fn main() {
    let code = std::env::args().nth(1).unwrap().parse().unwrap();
    std::process::exit(code);
}
