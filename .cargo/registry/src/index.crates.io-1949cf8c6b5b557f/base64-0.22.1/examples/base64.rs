use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;

use base64::{alphabet, engine, read, write};
use clap::Parser;

#[derive(Clone, Debug, Parser, strum::EnumString, Default)]
#[strum(serialize_all = "kebab-case")]
enum Alphabet {
    #[default]
    Standard,
    UrlSafe,
}

/// Base64 encode or decode FILE (or standard input), to standard output.
#[derive(Debug, Parser)]
struct Opt {
    /// Decode the base64-encoded input (default: encode the input as base64).
    #[structopt(short = 'd', long = "decode")]
    decode: bool,

    /// The encoding alphabet: "standard" (default) or "url-safe".
    #[structopt(long = "alphabet")]
    alphabet: Option<Alphabet>,

    /// Omit padding characters while encoding, and reject them while decoding.
    #[structopt(short = 'p', long = "no-padding")]
    no_padding: bool,

    /// The file to encode or decode.
    #[structopt(name = "FILE", parse(from_os_str))]
    file: Option<PathBuf>,
}

fn main() {
    let opt = Opt::parse();
    let stdin;
    let mut input: Box<dyn Read> = match opt.file {
        None => {
            stdin = io::stdin();
            Box::new(stdin.lock())
        }
        Some(ref f) if f.as_os_str() == "-" => {
            stdin = io::stdin();
            Box::new(stdin.lock())
        }
        Some(f) => Box::new(File::open(f).unwrap()),
    };

    let alphabet = opt.alphabet.unwrap_or_default();
    let engine = engine::GeneralPurpose::new(
        &match alphabet {
            Alphabet::Standard => alphabet::STANDARD,
            Alphabet::UrlSafe => alphabet::URL_SAFE,
        },
        match opt.no_padding {
            true => engine::general_purpose::NO_PAD,
            false => engine::general_purpose::PAD,
        },
    );

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let r = if opt.decode {
        let mut decoder = read::DecoderReader::new(&mut input, &engine);
        io::copy(&mut decoder, &mut stdout)
    } else {
        let mut encoder = write::EncoderWriter::new(&mut stdout, &engine);
        io::copy(&mut input, &mut encoder)
    };
    if let Err(e) = r {
        eprintln!(
            "Base64 {} failed with {}",
            if opt.decode { "decode" } else { "encode" },
            e
        );
        process::exit(1);
    }
}
