fn main() {
    let amt = std::env::args().nth(1).unwrap().parse().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(amt));
}
