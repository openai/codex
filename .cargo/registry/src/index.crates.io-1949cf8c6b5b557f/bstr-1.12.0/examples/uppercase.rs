use std::error::Error;
use std::io::{self, Write};

use bstr::{io::BufReadExt, ByteSlice};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());

    let mut upper = vec![];
    stdin.lock().for_byte_line_with_terminator(|line| {
        upper.clear();
        line.to_uppercase_into(&mut upper);
        stdout.write_all(&upper)?;
        Ok(true)
    })?;
    Ok(())
}
