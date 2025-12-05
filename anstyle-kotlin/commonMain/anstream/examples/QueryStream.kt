//! Report a terminal's capabilities

fn main() {
    println!("stdout:");
    println!(
        "  choice: {:?}",
        anstream::AutoStream::choice(&std::io::stdout())
    );
    println!(
        "  choice: {:?}",
        anstream::AutoStream::auto(std::io::stdout()).current_choice()
    );
    println!("stderr:");
    println!(
        "  choice: {:?}",
        anstream::AutoStream::choice(&std::io::stderr())
    );
    println!(
        "  choice: {:?}",
        anstream::AutoStream::auto(std::io::stderr()).current_choice()
    );
}
