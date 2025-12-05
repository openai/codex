//! Report a terminal's capabilities

fn main() {
    println!("clicolor: {:?}", anstyle_query::clicolor());
    println!("clicolor_force: {}", anstyle_query::clicolor_force());
    println!("no_color: {}", anstyle_query::no_color());
    println!(
        "term_supports_ansi_color: {}",
        anstyle_query::term_supports_ansi_color()
    );
    println!(
        "term_supports_color: {}",
        anstyle_query::term_supports_color()
    );
    println!("truecolor: {}", anstyle_query::truecolor());
    println!(
        "enable_ansi_colors: {:?}",
        anstyle_query::windows::enable_ansi_colors()
    );
    #[cfg(windows)]
    println!(
        "  enable_virtual_terminal_processing: {:?}",
        anstyle_query::windows::enable_virtual_terminal_processing()
    );
    println!("is_ci: {:?}", anstyle_query::is_ci());
}
