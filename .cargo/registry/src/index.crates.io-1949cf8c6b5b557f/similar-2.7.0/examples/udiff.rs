use std::fs::read;
use std::io;
use std::process::exit;

use similar::TextDiff;

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        eprintln!("usage: udiff [old] [new]");
        exit(1);
    }

    let old = read(&args[1]).unwrap();
    let new = read(&args[2]).unwrap();
    TextDiff::from_lines(&old, &new)
        .unified_diff()
        .header(
            &args[1].as_os_str().to_string_lossy(),
            &args[2].as_os_str().to_string_lossy(),
        )
        .to_writer(io::stdout())
        .unwrap();
}
