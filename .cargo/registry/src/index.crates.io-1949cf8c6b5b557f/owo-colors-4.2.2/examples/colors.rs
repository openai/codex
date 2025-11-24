use owo_colors::{OwoColorize, colors::*};

fn main() {
    // normal usage
    println!("{}", "green".green());
    println!("{}", "yellow".yellow());
    println!("{}", "blue".blue());
    println!("{}", "black".black());

    // generic examples
    println!("{}", "red".fg::<Red>());
    println!("{}", "magenta".fg::<Magenta>());
    println!("{}", "white".fg::<White>());
    println!("{}", "cyan".fg::<Cyan>());

    println!("\nBrights\n-------");
    println!("{}", "green".fg::<BrightGreen>());
    println!("{}", "yellow".fg::<BrightYellow>());
    println!("{}", "blue".fg::<BrightBlue>());
    println!("{}", "black".fg::<BrightBlack>());
    println!("{}", "red".fg::<BrightRed>());
    println!("{}", "magenta".fg::<BrightMagenta>());
    println!("{}", "white".fg::<BrightWhite>());
    println!("{}", "cyan".fg::<BrightCyan>());

    println!("\nStyles\n-------");
    println!("{}", "underline".underline());
    println!("{}", "bold".bold());
    println!("{}", "italic".italic());
    println!("{}", "strikethrough".strikethrough());
    println!("{}", "reverse".reversed());
    println!("1{}3", "2".hidden());
    println!("{}", "blink".blink());
    println!("{}", "blink fast".blink_fast());

    // foreground and background
    let red_on_white = "red on white".red().on_white();
    println!("{}", red_on_white);
}
