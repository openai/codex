//! Interactively manipulate wincon colors

#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(not(windows))]
fn main() {
    panic!("unsupported");
}

#[cfg(windows)]
fn main() -> Result<(), lexopt::Error> {
    use anstyle_wincon::WinconStream as _;

    let args = Args::parse()?;
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    let fg = args.fg.and_then(|c| c.into_ansi());
    let bg = args.bg.and_then(|c| c.into_ansi());

    let _ = stdout.write_colored(fg, bg, b"");

    #[allow(clippy::mem_forget)]
    std::mem::forget(stdout);

    Ok(())
}

#[derive(Default)]
struct Args {
    fg: Option<anstyle::Ansi256Color>,
    bg: Option<anstyle::Ansi256Color>,
}

impl Args {
    fn parse() -> Result<Self, lexopt::Error> {
        use lexopt::prelude::*;

        let mut res = Args::default();

        let mut args = lexopt::Parser::from_env();
        while let Some(arg) = args.next()? {
            match arg {
                Long("fg") => {
                    res.fg = Some(
                        args.value()?
                            .parse_with(|s| s.parse::<u8>().map(anstyle::Ansi256Color))?,
                    );
                }
                Long("bg") => {
                    res.fg = Some(
                        args.value()?
                            .parse_with(|s| s.parse::<u8>().map(anstyle::Ansi256Color))?,
                    );
                }
                _ => return Err(arg.unexpected()),
            }
        }
        Ok(res)
    }
}
