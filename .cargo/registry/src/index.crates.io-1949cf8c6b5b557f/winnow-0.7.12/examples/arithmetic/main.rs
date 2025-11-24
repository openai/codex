use winnow::prelude::*;

mod parser;
mod parser_ast;
mod parser_lexer;
#[cfg(test)]
mod test_parser;
#[cfg(test)]
mod test_parser_ast;
#[cfg(test)]
mod test_parser_lexer;

fn main() -> Result<(), lexopt::Error> {
    let args = Args::parse()?;

    let input = args.input.as_deref().unwrap_or("1 + 1");
    if let Err(err) = calc(input, args.implementation) {
        println!("FAILED");
        println!("{err}");
    }

    Ok(())
}

fn calc(
    input: &str,
    imp: Impl,
) -> Result<(), winnow::error::ParseError<&str, winnow::error::ContextError>> {
    println!("{input} =");
    match imp {
        Impl::Eval => {
            let result = parser::expr.parse(input)?;
            println!("  {result}");
        }
        Impl::Ast => {
            let result = parser_ast::expr.parse(input)?;
            println!("  {:#?}={}", result, result.eval());
        }
        Impl::Lexer => {
            let tokens = parser_lexer::tokens.parse(input)?;
            println!("  {tokens:#?}");
            let tokens = parser_lexer::Tokens::new(&tokens);
            let result = parser_lexer::expr.parse(tokens).unwrap();
            println!("  {:#?}={}", result, result.eval());
        }
    }
    Ok(())
}

#[derive(Default)]
struct Args {
    input: Option<String>,
    implementation: Impl,
}

enum Impl {
    Eval,
    Ast,
    Lexer,
}

impl Default for Impl {
    fn default() -> Self {
        Self::Eval
    }
}

impl Args {
    fn parse() -> Result<Self, lexopt::Error> {
        use lexopt::prelude::*;

        let mut res = Args::default();

        let mut args = lexopt::Parser::from_env();
        while let Some(arg) = args.next()? {
            match arg {
                Long("impl") => {
                    res.implementation = args.value()?.parse_with(|s| match s {
                        "eval" => Ok(Impl::Eval),
                        "ast" => Ok(Impl::Ast),
                        "lexer" => Ok(Impl::Lexer),
                        _ => Err("expected `eval`, `ast`"),
                    })?;
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
