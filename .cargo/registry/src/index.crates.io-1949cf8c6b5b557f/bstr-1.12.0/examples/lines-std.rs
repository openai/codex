use std::error::Error;
use std::io::{self, BufRead, Write};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut stdout = io::BufWriter::new(io::stdout());

    let mut line = String::new();
    while stdin.read_line(&mut line)? > 0 {
        if line.contains("Dimension") {
            stdout.write_all(line.as_bytes())?;
        }
        line.clear();
    }
    Ok(())
}
