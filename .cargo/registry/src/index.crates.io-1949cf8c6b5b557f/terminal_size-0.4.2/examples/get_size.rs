fn main() {
    println!(
        "Size from terminal_size():                    {:?}",
        terminal_size::terminal_size()
    );

    println!(
        "Size from terminal_size_of(stdout):           {:?}",
        terminal_size::terminal_size_of(std::io::stdout())
    );
    println!(
        "Size from terminal_size_of(stderr):           {:?}",
        terminal_size::terminal_size_of(std::io::stderr())
    );
    println!(
        "Size from terminal_size_of(stdin):            {:?}",
        terminal_size::terminal_size_of(std::io::stdin())
    );
}
