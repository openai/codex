#[test]
#[cfg(feature = "auto")]
fn print() {
    anstream::print!(
        "{}This should be captured{}",
        anstyle::AnsiColor::Red.on_default().render(),
        anstyle::Reset.render()
    );
}

#[test]
#[cfg(feature = "auto")]
fn println() {
    anstream::println!(
        "{}This should be captured{}",
        anstyle::AnsiColor::Red.on_default().render(),
        anstyle::Reset.render()
    );
}

#[test]
#[cfg(feature = "auto")]
fn eprint() {
    anstream::eprint!(
        "{}This should be captured{}",
        anstyle::AnsiColor::Red.on_default().render(),
        anstyle::Reset.render()
    );
}

#[test]
#[cfg(feature = "auto")]
fn eprintln() {
    anstream::eprintln!(
        "{}This should be captured{}",
        anstyle::AnsiColor::Red.on_default().render(),
        anstyle::Reset.render()
    );
}

#[test]
#[cfg(feature = "auto")]
#[should_panic]
fn panic() {
    anstream::panic!(
        "{}This should be captured{}",
        anstyle::AnsiColor::Red.on_default().render(),
        anstyle::Reset.render()
    );
}
