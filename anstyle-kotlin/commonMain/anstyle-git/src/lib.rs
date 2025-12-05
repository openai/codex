//! `anstyle_git::parse` parses a color configuration string (in Git syntax)
//! into an `anstyle::Style`:
//!
//! # Examples
//!
//! ```rust
//! let style = anstyle_git::parse("bold red blue").unwrap();
//! assert_eq!(style, anstyle::AnsiColor::Red.on(anstyle::AnsiColor::Blue) | anstyle::Effects::BOLD);
//!
//! let hyperlink_style = anstyle_git::parse("#0000ee ul").unwrap();
//! assert_eq!(hyperlink_style, anstyle::RgbColor(0x00, 0x00, 0xee).on_default() | anstyle::Effects::UNDERLINE);
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

/// Parse a string in Git's color configuration syntax into an
/// [`anstyle::Style`].
pub fn parse(s: &str) -> Result<anstyle::Style, Error> {
    let mut style = anstyle::Style::new();
    let mut num_colors = 0;
    let mut effects = anstyle::Effects::new();
    for word in s.split_whitespace() {
        match word.to_lowercase().as_ref() {
            "nobold" | "no-bold" => {
                effects = effects.remove(anstyle::Effects::BOLD);
            }
            "bold" => {
                effects = effects.insert(anstyle::Effects::BOLD);
            }
            "nodim" | "no-dim" => {
                effects = effects.remove(anstyle::Effects::DIMMED);
            }
            "dim" => {
                effects = effects.insert(anstyle::Effects::DIMMED);
            }
            "noul" | "no-ul" => {
                effects = effects.remove(anstyle::Effects::UNDERLINE);
            }
            "ul" => {
                effects = effects.insert(anstyle::Effects::UNDERLINE);
            }
            "noblink" | "no-blink" => {
                effects = effects.remove(anstyle::Effects::BLINK);
            }
            "blink" => {
                effects = effects.insert(anstyle::Effects::BLINK);
            }
            "noreverse" | "no-reverse" => {
                effects = effects.remove(anstyle::Effects::INVERT);
            }
            "reverse" => {
                effects = effects.insert(anstyle::Effects::INVERT);
            }
            "noitalic" | "no-italic" => {
                effects = effects.remove(anstyle::Effects::ITALIC);
            }
            "italic" => {
                effects = effects.insert(anstyle::Effects::ITALIC);
            }
            "nostrike" | "no-strike" => {
                effects = effects.remove(anstyle::Effects::STRIKETHROUGH);
            }
            "strike" => {
                effects = effects.insert(anstyle::Effects::STRIKETHROUGH);
            }
            w => {
                if let Ok(color) = parse_color(w) {
                    match num_colors {
                        0 => {
                            style = style.fg_color(color);
                            num_colors += 1;
                        }
                        1 => {
                            style = style.bg_color(color);
                            num_colors += 1;
                        }
                        _ => {
                            return Err(Error::ExtraColor {
                                style: s.to_owned(),
                                word: word.to_owned(),
                            });
                        }
                    }
                } else {
                    return Err(Error::UnknownWord {
                        style: s.to_owned(),
                        word: word.to_owned(),
                    });
                }
            }
        }
    }
    style |= effects;
    Ok(style)
}

fn parse_color(word: &str) -> Result<Option<anstyle::Color>, ()> {
    let color = match word {
        "normal" => None,
        "-1" => None,
        "black" => Some(anstyle::AnsiColor::Black.into()),
        "red" => Some(anstyle::AnsiColor::Red.into()),
        "green" => Some(anstyle::AnsiColor::Green.into()),
        "yellow" => Some(anstyle::AnsiColor::Yellow.into()),
        "blue" => Some(anstyle::AnsiColor::Blue.into()),
        "magenta" => Some(anstyle::AnsiColor::Magenta.into()),
        "cyan" => Some(anstyle::AnsiColor::Cyan.into()),
        "white" => Some(anstyle::AnsiColor::White.into()),
        _ => {
            if let Some(hex) = word.strip_prefix('#') {
                let l = hex.len();
                if l != 3 && l != 6 {
                    return Err(());
                }
                let l = l / 3;
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..l], 16),
                    u8::from_str_radix(&hex[l..(2 * l)], 16),
                    u8::from_str_radix(&hex[(2 * l)..(3 * l)], 16),
                ) {
                    Some(anstyle::Color::from((r, g, b)))
                } else {
                    return Err(());
                }
            } else if let Ok(n) = word.parse::<u8>() {
                Some(anstyle::Color::from(n))
            } else {
                return Err(());
            }
        }
    };
    Ok(color)
}

/// Type for errors returned by the parser.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// An extra color appeared after the foreground and background colors.
    ExtraColor {
        /// Original style
        style: String,
        /// Extra color
        word: String,
    },
    /// An unknown word appeared.
    UnknownWord {
        /// Original style
        style: String,
        /// Unknown word
        word: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExtraColor { style, word } => {
                write!(
                    fmt,
                    "Error parsing style \"{style}\": extra color \"{word}\""
                )
            }
            Self::UnknownWord { style, word } => {
                write!(
                    fmt,
                    "Error parsing style \"{style}\": unknown word: \"{word}\""
                )
            }
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::Error::*;
    use super::*;

    use anstyle::AnsiColor::*;
    use anstyle::*;

    #[test]
    fn test_parse_style() {
        macro_rules! test {
            ($s:expr => $style:expr) => {
                assert_eq!(parse($s).unwrap(), $style);
            };
        }

        test!("" => Style::new());
        test!("  " => Style::new());
        test!("normal" => Style::new());
        test!("normal normal" => Style::new());
        test!("-1 normal" => Style::new());
        test!("red" => Red.on_default());
        test!("red blue" => Red.on(Blue));
        test!("   red blue   " => Red.on(Blue));
        test!("red\tblue" => Red.on(Blue));
        test!("red\n blue" => Red.on(Blue));
        test!("red\r\n blue" => Red.on(Blue));
        test!("blue red" => Blue.on(Red));
        test!("yellow green" => Yellow.on(Green));
        test!("white magenta" => White.on(Magenta));
        test!("black cyan" => Black.on(Cyan));
        test!("red normal" => Red.on_default());
        test!("normal red" => Style::new().bg_color(Some(Red.into())));
        test!("0" => Ansi256Color(0).on_default());
        test!("8 3" => Ansi256Color(8).on(Ansi256Color(3)));
        test!("255" => Ansi256Color(255).on_default());
        test!("255 -1" => Ansi256Color(255).on_default());
        test!("#000000" => RgbColor(0,0,0).on_default());
        test!("#204060" => RgbColor(0x20,0x40,0x60).on_default());
        test!("#1a2b3c" => RgbColor(0x1a,0x2b,0x3c).on_default());
        test!("#000" => RgbColor(0,0,0).on_default());
        test!("#cba" => RgbColor(0xc,0xb,0xa).on_default());
        test!("#cba   " => RgbColor(0xc,0xb,0xa).on_default());
        test!("#987 #135" => RgbColor(9,8,7).on(RgbColor(1, 3, 5)));
        test!("#987    #135   " => RgbColor(9,8,7).on(RgbColor(1, 3, 5)));
        test!("#123 #abcdef" => RgbColor(1,2,3).on(RgbColor(0xab, 0xcd, 0xef)));
        test!("#654321 #a9b" => RgbColor(0x65,0x43,0x21).on(RgbColor(0xa, 0x9, 0xb)));

        test!("bold cyan white" => Cyan.on(White).bold());
        test!("bold cyan nobold white" => Cyan.on(White));
        test!("bold cyan reverse white nobold" => Cyan.on(White).invert());
        test!("bold cyan ul white dim" => Cyan.on(White).bold().underline().dimmed());
        test!("ul cyan white no-ul" => Cyan.on(White));
        test!("italic cyan white" => Cyan.on(White).italic());
        test!("strike cyan white" => Cyan.on(White).strikethrough());
        test!("blink #050505 white" => RgbColor(5,5,5).on(White).blink());
        test!("bold #987 green" => RgbColor(9,8,7).on(Green).bold());
        test!("strike #147 #cba" => RgbColor(1,4,7).on(RgbColor(0xc, 0xb, 0xa)).strikethrough());
    }

    #[test]
    fn test_parse_style_err() {
        macro_rules! test {
            ($s:expr => $err:ident $word:expr) => {
                assert_eq!(
                    parse($s),
                    Err($err {
                        style: $s.to_owned(),
                        word: $word.to_owned()
                    })
                );
            };
        }

        test!("red blue green" => ExtraColor "green");
        test!("red blue 123" => ExtraColor "123");
        test!("123 red blue" => ExtraColor "blue");
        test!("red blue normal" => ExtraColor "normal");
        test!("red blue -1" => ExtraColor "-1");
        test!("yellow green #abcdef" => ExtraColor "#abcdef");
        test!("#123456 #654321 #abcdef" => ExtraColor "#abcdef");
        test!("#123 #654 #abc" => ExtraColor "#abc");
        test!("#123 #654 #abcdef" => ExtraColor "#abcdef");
        test!("#123456 #654321 #abc" => ExtraColor "#abc");
        test!("bold red blue green" => ExtraColor "green");
        test!("red bold blue green" => ExtraColor "green");
        test!("red blue bold green" => ExtraColor "green");
        test!("red blue green bold" => ExtraColor "green");

        test!("256" => UnknownWord "256");
        test!("-2" => UnknownWord "-2");
        test!("-" => UnknownWord "-");
        test!("- 1" => UnknownWord "-");
        test!("123-1" => UnknownWord "123-1");
        test!("blue1" => UnknownWord "blue1");
        test!("blue-1" => UnknownWord "blue-1");
        test!("no" => UnknownWord "no");
        test!("nou" => UnknownWord "nou");
        test!("noblue" => UnknownWord "noblue");
        test!("no#123456" => UnknownWord "no#123456");
        test!("no-" => UnknownWord "no-");
        test!("no-u" => UnknownWord "no-u");
        test!("no-green" => UnknownWord "no-green");
        test!("no-#123456" => UnknownWord "no-#123456");
        test!("#" => UnknownWord "#");
        test!("#1" => UnknownWord "#1");
        test!("#12" => UnknownWord "#12");
        test!("#1234" => UnknownWord "#1234");
        test!("#12345" => UnknownWord "#12345");
        test!("#1234567" => UnknownWord "#1234567");
        test!("#12345678" => UnknownWord "#12345678");
        test!("#123456789" => UnknownWord "#123456789");
        test!("#123456789abc" => UnknownWord "#123456789abc");
        test!("#bcdefg" => UnknownWord "#bcdefg");
        test!("#blue" => UnknownWord "#blue");
        test!("blue#123456" => UnknownWord "blue#123456");
    }
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
