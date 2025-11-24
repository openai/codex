mod parser;

use std::io::Read;

use winnow::error::ContextError;
use winnow::error::ErrMode;
use winnow::error::Needed;
use winnow::prelude::*;
use winnow::stream::Offset;

fn main() -> Result<(), lexopt::Error> {
    let args = Args::parse()?;
    let input = args.input.ok_or_else(|| lexopt::Error::MissingValue {
        option: Some("<PATH>".to_owned()),
    })?;

    let mut file = std::fs::File::open(input).map_err(to_lexopt)?;

    // Intentionally starting with a small buffer to make it easier to show `Incomplete` handling
    let buffer_size = 10;
    let min_buffer_growth = 100;
    let buffer_growth_factor = 2;
    let mut buffer = circular::Buffer::with_capacity(buffer_size);
    loop {
        let read = file.read(buffer.space()).map_err(to_lexopt)?;
        eprintln!("read {read}");
        if read == 0 {
            // Should be EOF since we always make sure there is `available_space`
            assert_ne!(buffer.available_space(), 0);
            assert_eq!(
                buffer.available_data(),
                0,
                "leftover data: {}",
                String::from_utf8_lossy(buffer.data())
            );
            break;
        }
        buffer.fill(read);

        loop {
            let mut input =
                parser::Stream::new(std::str::from_utf8(buffer.data()).map_err(to_lexopt)?);
            let start = input.checkpoint();
            match parser::ndjson::<ContextError>.parse_next(&mut input) {
                Ok(value) => {
                    println!("{value:?}");
                    println!();
                    // Tell the buffer how much we read
                    let consumed = input.offset_from(&start);
                    buffer.consume(consumed);
                }
                Err(ErrMode::Backtrack(e)) | Err(ErrMode::Cut(e)) => {
                    return Err(fmt_lexopt(e.to_string()));
                }
                Err(ErrMode::Incomplete(Needed::Size(size))) => {
                    // Without the format telling us how much space is required, we really should
                    // treat this the same as `Unknown` but are doing this to demonstrate how to
                    // handle `Size`.
                    //
                    // Even when the format has a header to tell us `Size`, we could hit incidental
                    // `Size(1)`s, so make sure we buffer more space than that to avoid reading
                    // one byte at a time
                    let head_room = size.get().max(min_buffer_growth);
                    let new_capacity = buffer.available_data() + head_room;
                    eprintln!("growing buffer to {new_capacity}");
                    buffer.grow(new_capacity);
                    if buffer.available_space() < head_room {
                        eprintln!("buffer shift");
                        buffer.shift();
                    }
                    break;
                }
                Err(ErrMode::Incomplete(Needed::Unknown)) => {
                    let new_capacity = buffer_growth_factor * buffer.capacity();
                    eprintln!("growing buffer to {new_capacity}");
                    buffer.grow(new_capacity);
                    break;
                }
            }
        }
    }

    Ok(())
}

#[derive(Default)]
struct Args {
    input: Option<std::path::PathBuf>,
}

impl Args {
    fn parse() -> Result<Self, lexopt::Error> {
        use lexopt::prelude::*;

        let mut res = Args::default();

        let mut args = lexopt::Parser::from_env();
        while let Some(arg) = args.next()? {
            match arg {
                Value(input) => {
                    res.input = Some(input.into());
                }
                _ => return Err(arg.unexpected()),
            }
        }
        Ok(res)
    }
}

fn to_lexopt(e: impl std::error::Error + Send + Sync + 'static) -> lexopt::Error {
    lexopt::Error::Custom(Box::new(e))
}

fn fmt_lexopt(e: String) -> lexopt::Error {
    lexopt::Error::Custom(e.into())
}
