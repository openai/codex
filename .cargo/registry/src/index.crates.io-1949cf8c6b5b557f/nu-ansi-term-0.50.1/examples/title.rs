use nu_ansi_term::AnsiGenericString;
mod may_sleep;
use may_sleep::{parse_cmd_args, sleep};

fn main() {
    #[cfg(windows)]
    nu_ansi_term::enable_ansi_support().unwrap();

    let sleep_ms = parse_cmd_args();
    let title = AnsiGenericString::title("My Title");
    println!(
        "{}Terminal title set for the next {:?} milliseconds",
        title, sleep_ms
    );

    // sleep because often prompts change this before you can see
    // the results
    sleep(sleep_ms);
}
