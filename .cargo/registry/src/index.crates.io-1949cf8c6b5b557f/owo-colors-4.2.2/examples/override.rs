use owo_colors::{OwoColorize, Stream::Stdout};

fn main() {
    println!("Override color=always");
    owo_colors::set_override(true);
    println!("{}", "blue".if_supports_color(Stdout, |text| text.blue()));

    println!("Override color=never");
    owo_colors::set_override(false);
    println!("{}", "green".if_supports_color(Stdout, |text| text.green()));

    println!("Override color=auto");
    owo_colors::unset_override();
    println!(
        "{}",
        "yellow".if_supports_color(Stdout, |text| text.bright_yellow())
    );
}
