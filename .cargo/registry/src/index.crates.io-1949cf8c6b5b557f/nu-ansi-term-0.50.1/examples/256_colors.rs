use nu_ansi_term::Color;

// This example prints out the 256 colors.
// They're arranged like this:
//
// - 0 to 8 are the eight standard colors.
// - 9 to 15 are the eight bold colors.
// - 16 to 231 are six blocks of six-by-six color squares.
// - 232 to 255 are shades of grey.

fn main() {
    #[cfg(windows)]
    nu_ansi_term::enable_ansi_support().unwrap();

    // First two lines
    for c in 0..8 {
        glow(c, c != 0);
        print!(" ");
    }
    println!();
    for c in 8..16 {
        glow(c, c != 8);
        print!(" ");
    }
    println!("\n");

    // Six lines of the first three squares
    for row in 0..6 {
        for square in 0..3 {
            for column in 0..6 {
                glow(16 + square * 36 + row * 6 + column, row >= 3);
                print!(" ");
            }

            print!("  ");
        }

        println!();
    }
    println!();

    // Six more lines of the other three squares
    for row in 0..6 {
        for square in 0..3 {
            for column in 0..6 {
                glow(124 + square * 36 + row * 6 + column, row >= 3);
                print!(" ");
            }

            print!("  ");
        }

        println!();
    }
    println!();

    // The last greyscale lines
    for c in 232..=243 {
        glow(c, false);
        print!(" ");
    }
    println!();
    for c in 244..=255 {
        glow(c, true);
        print!(" ");
    }
    println!();
}

fn glow(c: u8, light_bg: bool) {
    let base = if light_bg { Color::Black } else { Color::White };
    let style = base.on(Color::Fixed(c));
    print!("{}", style.paint(&format!(" {:3} ", c)));
}
