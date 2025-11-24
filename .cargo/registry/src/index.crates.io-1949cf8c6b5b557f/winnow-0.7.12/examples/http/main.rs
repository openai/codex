mod parser;

fn main() -> Result<(), lexopt::Error> {
    let args = Args::parse()?;

    let input = args.input.as_deref().unwrap_or(
        "GET / HTTP/1.1
Host: www.reddit.com
User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.8; rv:15.0) Gecko/20100101 Firefox/15.0.1
Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8
Accept-Language: en-us,en;q=0.5
Accept-Encoding: gzip, deflate
Connection: keep-alive

",
    );

    if let Some(result) = parser::parse(input.as_bytes()) {
        println!("  {result:#?}");
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
