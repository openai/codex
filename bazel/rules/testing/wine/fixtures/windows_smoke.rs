use std::io::Write;

fn main() {
    println!("WINE_TEST_READY");
    std::io::stdout().flush().expect("flush readiness marker");

    let args = std::env::args().collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--write-prefix-marker") {
        std::fs::write(r"C:\shared-prefix-marker", b"shared prefix")
            .expect("write shared-prefix marker");
    }
    if args.iter().any(|arg| arg == "--fail") {
        std::process::exit(9);
    }
    if args.iter().any(|arg| arg == "--wait") {
        loop {
            std::thread::park();
        }
    }
}
