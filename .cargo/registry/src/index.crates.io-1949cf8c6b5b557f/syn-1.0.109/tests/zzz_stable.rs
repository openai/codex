#![cfg(syn_disable_nightly_tests)]

use std::io::{self, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

const MSG: &str = "\
‖
‖   WARNING:
‖   This is not a nightly compiler so not all tests were able to
‖   run. Syn includes tests that compare Syn's parser against the
‖   compiler's parser, which requires access to unstable librustc
‖   data structures and a nightly compiler.
‖
";

#[test]
fn notice() -> io::Result<()> {
    let header = "WARNING";
    let index_of_header = MSG.find(header).unwrap();
    let before = &MSG[..index_of_header];
    let after = &MSG[index_of_header + header.len()..];

    let mut stderr = StandardStream::stderr(ColorChoice::Auto);
    stderr.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
    write!(&mut stderr, "{}", before)?;
    stderr.set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Yellow)))?;
    write!(&mut stderr, "{}", header)?;
    stderr.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
    write!(&mut stderr, "{}", after)?;
    stderr.reset()?;

    Ok(())
}
