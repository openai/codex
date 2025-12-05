use anstyle::RgbColor;

#[test]
fn rainbow() {
    let input = std::fs::read_to_string("tests/rainbow.vte").unwrap();
    let actual = anstyle_svg::Term::new().render_svg(&input);
    snapbox::assert_data_eq!(actual, snapbox::file!["rainbow.svg": Text].raw());
}

#[test]
fn rg_linus() {
    let input = std::fs::read_to_string("tests/rg_linus.vte").unwrap();
    let actual = anstyle_svg::Term::new().render_svg(&input);
    snapbox::assert_data_eq!(actual, snapbox::file!["rg_linus.svg": Text].raw());
}

#[test]
fn hyperlink_demo() {
    let bytes = std::fs::read("tests/hyperlink-demo.vte").unwrap();
    String::from_utf8(bytes).unwrap();
    let input = std::fs::read_to_string("tests/hyperlink-demo.vte").unwrap();
    let actual = anstyle_svg::Term::new().render_svg(&input);
    snapbox::assert_data_eq!(actual, snapbox::file!["hyperlink-demo.svg": Text].raw());
}

#[test]
fn custom_background_color() {
    let bytes = std::fs::read("tests/custom_background_color.vte").unwrap();
    String::from_utf8(bytes).unwrap();
    let input = std::fs::read_to_string("tests/custom_background_color.vte").unwrap();
    let actual = anstyle_svg::Term::new()
        .bg_color(anstyle::Color::Rgb(RgbColor(0x18, 0x18, 0x18)))
        .render_svg(&input);
    snapbox::assert_data_eq!(
        actual,
        snapbox::file!["custom_background_color.svg": Text].raw()
    );
}

#[test]
fn rainbow_html() {
    let input = std::fs::read_to_string("tests/rainbow.vte").unwrap();
    let actual = anstyle_svg::Term::new().render_html(&input);
    snapbox::assert_data_eq!(actual, snapbox::file!["rainbow.html": Text].raw());
}

#[test]
fn rg_linus_html() {
    let input = std::fs::read_to_string("tests/rg_linus.vte").unwrap();
    let actual = anstyle_svg::Term::new().render_html(&input);
    snapbox::assert_data_eq!(actual, snapbox::file!["rg_linus.html": Text].raw());
}

#[test]
fn hyperlink_demo_html() {
    let bytes = std::fs::read("tests/hyperlink-demo.vte").unwrap();
    String::from_utf8(bytes).unwrap();
    let input = std::fs::read_to_string("tests/hyperlink-demo.vte").unwrap();
    let actual = anstyle_svg::Term::new().render_html(&input);
    snapbox::assert_data_eq!(actual, snapbox::file!["hyperlink-demo.html": Text].raw());
}

#[test]
fn custom_background_color_html() {
    let bytes = std::fs::read("tests/custom_background_color.vte").unwrap();
    String::from_utf8(bytes).unwrap();
    let input = std::fs::read_to_string("tests/custom_background_color.vte").unwrap();
    let actual = anstyle_svg::Term::new()
        .bg_color(anstyle::Color::Rgb(RgbColor(0x18, 0x18, 0x18)))
        .render_html(&input);
    snapbox::assert_data_eq!(
        actual,
        snapbox::file!["custom_background_color.html": Text].raw()
    );
}
