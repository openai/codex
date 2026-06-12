use std::io::Write;

fn main() {
    println!("WINE_TEST_READY");
    std::io::stdout().flush().expect("flush readiness marker");

    if std::env::args().any(|arg| arg == "--wait") {
        loop {
            std::thread::park();
        }
    }
}
