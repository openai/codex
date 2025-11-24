use std::error::Error;
use std::io::{self, Write};

use bstr::{io::BufReadExt, ByteSlice};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());

    stdin.lock().for_byte_line_with_terminator(|line| {
        if line.contains_str("Dimension") {
            stdout.write_all(line)?;
        }
        Ok(true)
    })?;
    Ok(())
}
