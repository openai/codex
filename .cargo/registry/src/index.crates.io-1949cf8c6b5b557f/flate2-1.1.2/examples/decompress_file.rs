use flate2::bufread;
use std::env::args;
use std::fs::File;
use std::io::copy;
use std::io::BufReader;
use std::time::Instant;

fn main() {
    // E.g. `cargo run --example decompress_file examples/hello_world.txt.gz hello_world.txt`
    if args().len() != 3 {
        eprintln!("Usage: ./decompress_file `source` `target`");
        return;
    }
    let input = BufReader::new(File::open(args().nth(1).unwrap()).unwrap());
    let mut output = File::create(args().nth(2).unwrap()).unwrap();
    let source_len = input.get_ref().metadata().unwrap().len();
    let start = Instant::now();
    let mut decoder = bufread::GzDecoder::new(input);
    copy(&mut decoder, &mut output).unwrap();
    println!("Source len: {:?}", source_len);
    println!("Target len: {:?}", output.metadata().unwrap().len());
    println!("Elapsed: {:?}", start.elapsed());
}
