#![forbid(unsafe_code)]
use std::path::PathBuf;
use std::{env, ffi, fs, io, process};

extern crate weezl;
use weezl::{decode as delzw, encode as enlzw, BitOrder};

fn main() {
    let args = env::args_os().skip(1);
    let flags = Flags::from_args(args).unwrap_or_else(|ParamError| explain());

    let out = io::stdout();
    let out = out.lock();

    let mut files = flags.files;
    let input = files.pop().unwrap_or_else(explain);
    if !files.is_empty() {
        return explain();
    }
    let operation = flags.operation.unwrap_or_else(explain);
    let min_code = if flags.min_code < 2 || flags.min_code > 12 {
        return explain();
    } else {
        flags.min_code
    };
    let bit_order = flags.bit_order;

    let result = match (input, operation) {
        (Input::File(file), Operation::Encode) => (|| {
            let data = fs::File::open(file)?;
            let file = io::BufReader::with_capacity(1 << 26, data);

            let mut encoder = enlzw::Encoder::new(bit_order, min_code);
            encoder.into_stream(out).encode_all(file).status
        })(),
        (Input::Stdin, Operation::Encode) => {
            let input = io::BufReader::with_capacity(1 << 26, io::stdin());
            let mut encoder = enlzw::Encoder::new(bit_order, min_code);
            encoder.into_stream(out).encode_all(input).status
        }
        (Input::File(file), Operation::Decode) => (|| {
            let data = fs::File::open(file)?;
            let file = io::BufReader::with_capacity(1 << 26, data);

            let mut decoder = delzw::Decoder::new(bit_order, min_code);
            decoder.into_stream(out).decode_all(file).status
        })(),
        (Input::Stdin, Operation::Decode) => {
            let input = io::BufReader::with_capacity(1 << 26, io::stdin());
            let mut decoder = delzw::Decoder::new(bit_order, min_code);
            decoder.into_stream(out).decode_all(input).status
        }
    };

    result.expect("Operation Failed: ");
}

struct Flags {
    files: Vec<Input>,
    operation: Option<Operation>,
    min_code: u8,
    bit_order: BitOrder,
}

struct ParamError;

enum Input {
    File(PathBuf),
    Stdin,
}

enum Operation {
    Encode,
    Decode,
}

fn explain<T>() -> T {
    println!(
        "Usage: lzw [-e|-d] <file>\n\
        Arguments:\n\
        -e\t operation encode (default)\n\
        -d\t operation decode\n\
        <file>\tfilepath or '-' for stdin"
    );
    process::exit(1);
}

impl Default for Flags {
    fn default() -> Flags {
        Flags {
            files: vec![],
            operation: None,
            min_code: 8,
            bit_order: BitOrder::Msb,
        }
    }
}

impl Flags {
    fn from_args(mut args: impl Iterator<Item = ffi::OsString>) -> Result<Self, ParamError> {
        let mut flags = Flags::default();
        let mut operation = None;
        loop {
            match args.next().as_ref().and_then(|s| s.to_str()) {
                Some("-d") | Some("--decode") => {
                    if operation.is_some() {
                        return Err(ParamError);
                    }
                    operation = Some(Operation::Decode);
                }
                Some("-e") | Some("--encode") => {
                    if operation.is_some() {
                        return Err(ParamError);
                    }
                    operation = Some(Operation::Encode);
                }
                Some("-w") | Some("--word-bits") => match args.next() {
                    None => return Err(ParamError),
                    Some(bits) => {
                        let st = bits.to_str().ok_or(ParamError)?;
                        flags.min_code = st.parse().ok().ok_or(ParamError)?;
                    }
                },
                Some("-le") | Some("--little-endian") => {
                    flags.bit_order = BitOrder::Lsb;
                }
                Some("-be") | Some("--big-endian") | Some("-ne") | Some("--network-endian") => {
                    flags.bit_order = BitOrder::Msb;
                }
                Some("-") => {
                    flags.files.push(Input::Stdin);
                }
                Some(other) if other.starts_with('-') => {
                    // Reserved for future use.
                    // -a: self-describing archive format, similar to actual compress
                    // -b: maximum bits
                    // -v: verbosity
                    // some compress compatibility mode? Probably through arg(0) though.
                    return Err(ParamError);
                }
                Some(file) => {
                    flags.files.push(Input::File(file.into()));
                }
                None => break,
            };
        }

        flags.files.extend(args.map(|file| {
            if let Some("-") = file.to_str() {
                Input::Stdin
            } else {
                Input::File(file.into())
            }
        }));

        flags.operation = operation;
        Ok(flags)
    }
}
