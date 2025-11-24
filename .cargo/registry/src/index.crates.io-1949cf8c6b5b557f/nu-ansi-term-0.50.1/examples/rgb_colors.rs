use nu_ansi_term::{Color, Style};

// This example prints out a color gradient in a grid by calculating each
// character’s red, green, and blue components, and using 24-bit color codes
// to display them.

const WIDTH: i32 = 80;
const HEIGHT: i32 = 24;

fn main() {
    #[cfg(windows)]
    nu_ansi_term::enable_ansi_support().unwrap();

    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            let r = (row * 255 / HEIGHT) as u8;
            let g = (col * 255 / WIDTH) as u8;
            let b = 128;

            print!("{}", Style::default().on(Color::Rgb(r, g, b)).paint(" "));
        }

        println!();
    }
}
