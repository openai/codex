use std::error::Error;
use std::io::{self, BufRead};

use unicode_segmentation::UnicodeSegmentation;

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    let mut words = 0;
    let mut line = String::new();
    while stdin.read_line(&mut line)? > 0 {
        words += line.unicode_words().count();
        line.clear();
    }
    println!("{}", words);
    Ok(())
}
