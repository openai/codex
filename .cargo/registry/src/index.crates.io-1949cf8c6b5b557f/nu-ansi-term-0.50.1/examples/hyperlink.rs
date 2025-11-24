use nu_ansi_term::Color;
mod may_sleep;
use may_sleep::{parse_cmd_args, sleep};

fn main() {
    #[cfg(windows)]
    nu_ansi_term::enable_ansi_support().unwrap();

    let sleep_ms = parse_cmd_args();
    let link = Color::Blue
        .underline()
        .paint("Link to example.com")
        .hyperlink("https://example.com");

    println!("{}", link);
    sleep(sleep_ms);
}
