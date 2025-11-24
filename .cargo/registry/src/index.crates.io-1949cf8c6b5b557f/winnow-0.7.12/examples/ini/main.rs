use winnow::prelude::*;

mod parser;
mod parser_str;

fn main() -> Result<(), lexopt::Error> {
    let args = Args::parse()?;

    let input = args.input.as_deref().unwrap_or("1 + 1");

    if args.binary {
        match parser::categories.parse(input.as_bytes()) {
            Ok(result) => {
                println!("  {result:?}");
            }
            Err(err) => {
                println!("  {err:?}");
            }
        }
    } else {
        match parser_str::categories.parse(input) {
            Ok(result) => {
                println!("  {result:?}");
            }
            Err(err) => {
                println!("  {err}");
            }
        }
    }

    Ok(())
}

#[derive(Default)]
struct Args {
    input: Option<String>,
    binary: bool,
}

impl Args {
    fn parse() -> Result<Self, lexopt::Error> {
        use lexopt::prelude::*;

        let mut res = Args::default();

        let mut args = lexopt::Parser::from_env();
        while let Some(arg) = args.next()? {
            match arg {
                Long("binary") => {
                    res.binary = true;
                }
                Value(input) => {
                    res.input = Some(input.string()?);
                }
                _ => return Err(arg.unexpected()),
            }
        }
        Ok(res)
    }
}
