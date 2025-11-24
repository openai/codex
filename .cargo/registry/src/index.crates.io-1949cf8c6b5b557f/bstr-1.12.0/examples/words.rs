use std::error::Error;
use std::io;

use bstr::{io::BufReadExt, ByteSlice};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut words = 0;
    stdin.lock().for_byte_line_with_terminator(|line| {
        words += line.words().count();
        Ok(true)
    })?;
    println!("{}", words);
    Ok(())
}
