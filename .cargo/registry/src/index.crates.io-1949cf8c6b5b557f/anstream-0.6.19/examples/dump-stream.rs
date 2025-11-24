//! Write colored text, adapting to the terminals capabilities

use std::io::Write;

fn main() -> Result<(), lexopt::Error> {
    let args = Args::parse()?;
    let stdout = anstream::stdout();
    let mut stdout = stdout.lock();

    for fixed in 0..16 {
        let color = anstyle::Ansi256Color(fixed)
            .into_ansi()
            .expect("within 4-bit color range");
        let style = style(color, args.layer, args.effects);
        let _ = print_number(&mut stdout, fixed, style);
        if fixed == 7 || fixed == 15 {
            let _ = writeln!(&mut stdout);
        }
    }

    for fixed in 16..232 {
        let col = (fixed - 16) % 36;
        if col == 0 {
            let _ = writeln!(stdout);
        }
        let color = anstyle::Ansi256Color(fixed);
        let style = style(color, args.layer, args.effects);
        let _ = print_number(&mut stdout, fixed, style);
    }

    let _ = writeln!(stdout);
    let _ = writeln!(stdout);
    for fixed in 232..=255 {
        let color = anstyle::Ansi256Color(fixed);
        let style = style(color, args.layer, args.effects);
        let _ = print_number(&mut stdout, fixed, style);
    }

    let _ = writeln!(stdout);

    Ok(())
}

fn style(
    color: impl Into<anstyle::Color>,
    layer: Layer,
    effects: anstyle::Effects,
) -> anstyle::Style {
    let color = color.into();
    (match layer {
        Layer::Fg => anstyle::Style::new().fg_color(Some(color)),
        Layer::Bg => anstyle::Style::new().bg_color(Some(color)),
        Layer::Underline => anstyle::Style::new().underline_color(Some(color)),
    }) | effects
}

fn print_number(stdout: &mut impl Write, fixed: u8, style: anstyle::Style) -> std::io::Result<()> {
    write!(stdout, "{style}{fixed:>3X}{style:#}",)
}

#[derive(Default)]
struct Args {
    effects: anstyle::Effects,
    layer: Layer,
}

#[derive(Copy, Clone, Default)]
enum Layer {
    #[default]
    Fg,
    Bg,
    Underline,
}

impl Args {
    fn parse() -> Result<Self, lexopt::Error> {
        use lexopt::prelude::*;

        let mut res = Args::default();

        let mut args = lexopt::Parser::from_env();
        while let Some(arg) = args.next()? {
            match arg {
                Long("layer") => {
                    res.layer = args.value()?.parse_with(|s| match s {
                        "fg" => Ok(Layer::Fg),
                        "bg" => Ok(Layer::Bg),
                        "underline" => Ok(Layer::Underline),
                        _ => Err("expected values fg, bg, underline"),
                    })?;
                }
                Long("effect") => {
                    const EFFECTS: [(&str, anstyle::Effects); 12] = [
                        ("bold", anstyle::Effects::BOLD),
                        ("dimmed", anstyle::Effects::DIMMED),
                        ("italic", anstyle::Effects::ITALIC),
                        ("underline", anstyle::Effects::UNDERLINE),
                        ("double_underline", anstyle::Effects::DOUBLE_UNDERLINE),
                        ("curly_underline", anstyle::Effects::CURLY_UNDERLINE),
                        ("dotted_underline", anstyle::Effects::DOTTED_UNDERLINE),
                        ("dashed_underline", anstyle::Effects::DASHED_UNDERLINE),
                        ("blink", anstyle::Effects::BLINK),
                        ("invert", anstyle::Effects::INVERT),
                        ("hidden", anstyle::Effects::HIDDEN),
                        ("strikethrough", anstyle::Effects::STRIKETHROUGH),
                    ];
                    let effect = args.value()?.parse_with(|s| {
                        EFFECTS
                            .into_iter()
                            .find(|(name, _)| *name == s)
                            .map(|(_, effect)| effect)
                            .ok_or_else(|| {
                                format!(
                                    "expected one of {}",
                                    EFFECTS
                                        .into_iter()
                                        .map(|(n, _)| n)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            })
                    })?;
                    res.effects = res.effects.insert(effect);
                }
                _ => return Err(arg.unexpected()),
            }
        }
        Ok(res)
    }
}
