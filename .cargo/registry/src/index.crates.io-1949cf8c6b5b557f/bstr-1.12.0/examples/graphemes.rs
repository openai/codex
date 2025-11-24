use std::error::Error;
use std::io::{self, Write};

use bstr::{io::BufReadExt, ByteSlice};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());

    stdin.lock().for_byte_line_with_terminator(|line| {
        let end = line
            .grapheme_indices()
            .map(|(_, end, _)| end)
            .take(10)
            .last()
            .unwrap_or(line.len());
        stdout.write_all(line[..end].trim_end())?;
        stdout.write_all(b"\n")?;
        Ok(true)
    })?;
    Ok(())
}
