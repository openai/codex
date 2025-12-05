use anstyle::Style;
use snapbox::assert_data_eq;
use snapbox::file;

#[test]
fn test_ansi_color_output() {
    // HACK: cansi doesn't properly merge separate sequences
    // let style = AnsiColor::Red.on(AnsiColor::Blue);
    // let text = format!("{}{}", style.render(), "test");
    let text = "\u{1b}[31;44mtest".to_owned();
    let roff_doc = anstyle_roff::to_roff(&text);
    assert_data_eq!(roff_doc.to_roff(), file!["roffs/ansi_color.roff"].raw());
}

#[test]
fn test_bold_output() {
    let style = Style::new().bold();
    let text = format!("{}{}", style.render(), "test");
    dbg!(&text);
    let roff_doc = anstyle_roff::to_roff(&text);

    assert_data_eq!(roff_doc.to_roff(), file!["roffs/bold.roff"].raw());
}

#[test]
fn test_italic_output() {
    let style = Style::new().italic();
    let text = format!("{}{}", style.render(), "test");
    dbg!(&text);

    let roff_doc = anstyle_roff::to_roff(&text);
    assert_data_eq!(roff_doc.to_roff(), file!["roffs/italic.roff"].raw());
}

#[test]
fn test_bright_color_output_as_bold() {
    // HACK: cansi doesn't properly merge separate sequences
    // let style = AnsiColor::BrightRed.on(AnsiColor::Blue);
    // let text = format!("{}{}", style.render(), "test");
    let text = "\u{1b}[91;44mtest".to_owned();
    dbg!(&text);
    let roff_doc = anstyle_roff::to_roff(&text);
    assert_data_eq!(
        roff_doc.to_roff(),
        file!["roffs/bright_ansi_colors.roff"].raw()
    );
}
