use owo_colors::{AnsiColors, DynColors, OwoColorize, Rgb, XtermColors};

fn random_number() -> u32 {
    2
}

fn main() {
    let mut color = AnsiColors::Red;
    println!("{}", "red".color(color));

    color = AnsiColors::Blue;
    println!("{}", "blue".color(color));

    let color = XtermColors::Fuchsia;
    println!("{}", "fuchsia".color(color));

    let color = Rgb(141, 59, 212);
    println!("{}", "custom purple".color(color));

    let color = match random_number() {
        1 => DynColors::Rgb(141, 59, 212),
        2 => DynColors::Ansi(AnsiColors::BrightGreen),
        3 => "#F3F3F3".parse().unwrap(),
        _ => DynColors::Xterm(XtermColors::Aqua),
    };

    println!("{}", "mystery color".color(color));
}
