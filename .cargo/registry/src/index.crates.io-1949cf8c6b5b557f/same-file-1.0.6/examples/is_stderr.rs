use std::io;
use std::process;

use same_file::Handle;

fn main() {
    if let Err(err) = run() {
        println!("{}", err);
        process::exit(1);
    }
}

fn run() -> io::Result<()> {
    // Run with `cargo run --example is_stderr 2> examples/stderr` to see
    // interesting output.
    let candidates = &[
        "examples/is_same_file.rs",
        "examples/is_stderr.rs",
        "examples/stderr",
    ];
    let stderr_handle = Handle::stderr()?;
    for candidate in candidates {
        let handle = Handle::from_path(candidate)?;
        if stderr_handle == handle {
            println!("{:?} is stderr!", candidate);
        } else {
            println!("{:?} is NOT stderr!", candidate);
        }
    }
    Ok(())
}
