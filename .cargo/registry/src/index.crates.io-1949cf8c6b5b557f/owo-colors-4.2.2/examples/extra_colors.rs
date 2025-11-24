use owo_colors::{OwoColorize, colors::xterm};

fn main() {
    println!("{}", "Electric violet".fg::<xterm::ElectricViolet>());
    println!("{}", "Matrix".fg::<xterm::MatrixPink>());
    println!("{}", "Flirt".fg::<xterm::Flirt>());
    println!("{}", "Cyan2".fg::<xterm::Cyan>());
    println!("{}", "Cyan".fg::<xterm::UserCyan>());
    println!("{}", "Lime".fg::<xterm::Lime>());
    println!("{}", "Jade".fg::<xterm::Jade>());
    println!("{}", "Reef".fg::<xterm::Mauve>());
}
