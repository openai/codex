use nu_ansi_term::{build_all_gradient_text, Color, Gradient, Rgb, TargetGround};

fn main() {
    #[cfg(windows)]
    nu_ansi_term::enable_ansi_support().unwrap();

    let text = "lorem ipsum quia dolor sit amet, consectetur, adipisci velit";

    // a gradient from hex colors
    let start = Rgb::from_hex(0x40c9ff);
    let end = Rgb::from_hex(0xe81cff);
    let grad0 = Gradient::new(start, end);

    // a gradient from color::rgb()
    let start = Color::Rgb(64, 201, 255);
    let end = Color::Rgb(232, 28, 255);
    let gradient = Gradient::from_color_rgb(start, end);

    // a slightly different gradient
    let start2 = Color::Rgb(128, 64, 255);
    let end2 = Color::Rgb(0, 28, 255);
    let gradient2 = Gradient::from_color_rgb(start2, end2);

    // reverse the gradient
    let gradient3 = gradient.reverse();

    let build_fg = gradient.build(text, TargetGround::Foreground);
    println!("{}", build_fg);
    let build_bg = gradient.build(text, TargetGround::Background);
    println!("{}", build_bg);
    let bgt = build_all_gradient_text(text, gradient, gradient2);
    println!("{}", bgt);
    let bgt2 = build_all_gradient_text(text, gradient, gradient3);
    println!("{}", bgt2);

    println!(
        "{}",
        grad0.build("nushell is awesome", TargetGround::Foreground)
    );
}
