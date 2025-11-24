//! Naive re-implementation of the Linux `hostname` program.

use std::io;

fn main() -> io::Result<()> {
    let name = hostname::get()?;

    println!("{}", name.to_string_lossy());

    Ok(())
}
