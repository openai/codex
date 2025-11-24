use nu_ansi_term::{Color::*, Style};

// This example prints out the 16 basic colors.

fn main() {
    #[cfg(windows)]
    nu_ansi_term::enable_ansi_support().unwrap();

    let normal = Style::default();

    println!("{} {}", normal.paint("Normal"), normal.bold().paint("bold"));
    println!("{} {}", Black.paint("Black"), Black.bold().paint("bold"));
    println!("{} {}", Red.paint("Red"), Red.bold().paint("bold"));
    println!("{} {}", Green.paint("Green"), Green.bold().paint("bold"));
    println!("{} {}", Yellow.paint("Yellow"), Yellow.bold().paint("bold"));
    println!("{} {}", Blue.paint("Blue"), Blue.bold().paint("bold"));
    println!("{} {}", Purple.paint("Purple"), Purple.bold().paint("bold"));
    println!("{} {}", Cyan.paint("Cyan"), Cyan.bold().paint("bold"));
    println!("{} {}", White.paint("White"), White.bold().paint("bold"));
    println!("\nreset_before_style at work:");
    println!(
        "\x1b[33mReset {} \x1b[33mand {}\x1b[0m",
        Style::new().reset_before_style().bold().paint("bold"),
        Style::new()
            .reset_before_style()
            .underline()
            .paint("underline")
    );
    println!(
        "\x1b[33mDo not reset {} \x1b[33mand {}\x1b[0m",
        Style::new().bold().paint("bold"),
        Style::new().underline().paint("underline")
    );
}
