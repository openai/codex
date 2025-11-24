use winnow::prelude::*;

mod parser;

use parser::hex_color;

fn main() -> Result<(), lexopt::Error> {
    let args = Args::parse()?;

    let input = args.input.as_deref().unwrap_or("#AAAAAA");

    println!("{input} =");
    match hex_color.parse(input) {
        Ok(result) => {
            println!("  {result:?}");
        }
        Err(err) => {
            println!("  {err}");
        }
    }

    Ok(())
}

#[derive(Default)]
struct Args {
    input: Option<String>,
}

impl Args {
    fn parse() -> Result<Self, lexopt::Error> {
        use lexopt::prelude::*;

        let mut res = Args::default();

        let mut args = lexopt::Parser::from_env();
        while let Some(arg) = args.next()? {
            match arg {
                Value(input) => {
                    res.input = Some(input.string()?);
                }
                _ => return Err(arg.unexpected()),
            }
        }
        Ok(res)
    }
}

#[test]
fn parse_color() {
    assert_eq!(
        hex_color.parse_peek("#2F14DF"),
        Ok((
            "",
            parser::Color {
                red: 47,
                green: 20,
                blue: 223,
            }
        ))
    );
}
