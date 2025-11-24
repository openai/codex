#[cfg(feature = "alloc")]
use alloc::format;
use core::fmt;
use yansi::Color::{Green, Red};
use yansi::{Paint, Style};

macro_rules! paint {
    ($f:expr, $style:expr, $fmt:expr, $($args:tt)*) => (
        write!($f, "{}", format!($fmt, $($args)*).paint($style))
    )
}

const SIGN_RIGHT: char = '>'; // + > →
const SIGN_LEFT: char = '<'; // - < ←

/// Present the diff output for two mutliline strings in a pretty, colorised manner.
pub(crate) fn write_header(f: &mut fmt::Formatter) -> fmt::Result {
    writeln!(
        f,
        "{} {} {} / {} {} :",
        "Diff".bold(),
        SIGN_LEFT.red().linger(),
        "left".resetting(),
        "right".green().linger(),
        SIGN_RIGHT.resetting(),
    )
}

/// Delay formatting this deleted chunk until later.
///
/// It can be formatted as a whole chunk by calling `flush`, or the inner value
/// obtained with `take` for further processing (such as an inline diff).
#[derive(Default)]
struct LatentDeletion<'a> {
    // The most recent deleted line we've seen
    value: Option<&'a str>,
    // The number of deleted lines we've seen, including the current value
    count: usize,
}

impl<'a> LatentDeletion<'a> {
    /// Set the chunk value.
    fn set(&mut self, value: &'a str) {
        self.value = Some(value);
        self.count += 1;
    }

    /// Take the underlying chunk value, if it's suitable for inline diffing.
    ///
    /// If there is no value or we've seen more than one line, return `None`.
    fn take(&mut self) -> Option<&'a str> {
        if self.count == 1 {
            self.value.take()
        } else {
            None
        }
    }

    /// If a value is set, print it as a whole chunk, using the given formatter.
    ///
    /// If a value is not set, reset the count to zero (as we've called `flush` twice,
    /// without seeing another deletion. Therefore the line in the middle was something else).
    fn flush<TWrite: fmt::Write>(&mut self, f: &mut TWrite) -> fmt::Result {
        if let Some(value) = self.value {
            paint!(f, Red, "{}{}", SIGN_LEFT, value)?;
            writeln!(f)?;
            self.value = None;
        } else {
            self.count = 0;
        }

        Ok(())
    }
}

// Adapted from:
// https://github.com/johannhof/difference.rs/blob/c5749ad7d82aa3d480c15cb61af9f6baa08f116f/examples/github-style.rs
// Credits johannhof (MIT License)

/// Present the diff output for two mutliline strings in a pretty, colorised manner.
pub(crate) fn write_lines<TWrite: fmt::Write>(
    f: &mut TWrite,
    left: &str,
    right: &str,
) -> fmt::Result {
    let diff = ::diff::lines(left, right);

    let mut changes = diff.into_iter().peekable();
    let mut previous_deletion = LatentDeletion::default();

    while let Some(change) = changes.next() {
        match (change, changes.peek()) {
            // If the text is unchanged, just print it plain
            (::diff::Result::Both(value, _), _) => {
                previous_deletion.flush(f)?;
                writeln!(f, " {}", value)?;
            }
            // Defer any deletions to next loop
            (::diff::Result::Left(deleted), _) => {
                previous_deletion.flush(f)?;
                previous_deletion.set(deleted);
            }
            // If we're being followed by more insertions, don't inline diff
            (::diff::Result::Right(inserted), Some(::diff::Result::Right(_))) => {
                previous_deletion.flush(f)?;
                paint!(f, Green, "{}{}", SIGN_RIGHT, inserted)?;
                writeln!(f)?;
            }
            // Otherwise, check if we need to inline diff with the previous line (if it was a deletion)
            (::diff::Result::Right(inserted), _) => {
                if let Some(deleted) = previous_deletion.take() {
                    write_inline_diff(f, deleted, inserted)?;
                } else {
                    previous_deletion.flush(f)?;
                    paint!(f, Green, "{}{}", SIGN_RIGHT, inserted)?;
                    writeln!(f)?;
                }
            }
        };
    }

    previous_deletion.flush(f)?;
    Ok(())
}

/// Group character styling for an inline diff, to prevent wrapping each single
/// character in terminal styling codes.
///
/// Styles are applied automatically each time a new style is given in `write_with_style`.
struct InlineWriter<'a, Writer> {
    f: &'a mut Writer,
    style: Style,
}

impl<'a, Writer> InlineWriter<'a, Writer>
where
    Writer: fmt::Write,
{
    fn new(f: &'a mut Writer) -> Self {
        InlineWriter {
            f,
            style: Style::new(),
        }
    }

    /// Push a new character into the buffer, specifying the style it should be written in.
    fn write_with_style<T: Into<Style>>(&mut self, c: &char, style: T) -> fmt::Result {
        // If the style is the same as previously, just write character
        let style = style.into();
        if style == self.style {
            write!(self.f, "{}", c)?;
        } else {
            // Close out previous style
            self.style.fmt_suffix(self.f)?;

            // Store new style and start writing it
            style.fmt_prefix(self.f)?;
            write!(self.f, "{}", c)?;
            self.style = style;
        }
        Ok(())
    }

    /// Finish any existing style and reset to default state.
    fn finish(&mut self) -> fmt::Result {
        // Close out previous style
        self.style.fmt_suffix(self.f)?;
        writeln!(self.f)?;
        self.style = Style::new();
        Ok(())
    }
}

/// Format a single line to show an inline diff of the two strings given.
///
/// The given strings should not have a trailing newline.
///
/// The output of this function will be two lines, each with a trailing newline.
fn write_inline_diff<TWrite: fmt::Write>(f: &mut TWrite, left: &str, right: &str) -> fmt::Result {
    let diff = ::diff::chars(left, right);
    let mut writer = InlineWriter::new(f);

    // Print the left string on one line, with differences highlighted
    let light = Red;
    let heavy = Red.on_fixed(52).bold();
    writer.write_with_style(&SIGN_LEFT, light)?;
    for change in diff.iter() {
        match change {
            ::diff::Result::Both(value, _) => writer.write_with_style(value, light)?,
            ::diff::Result::Left(value) => writer.write_with_style(value, heavy)?,
            _ => (),
        }
    }
    writer.finish()?;

    // Print the right string on one line, with differences highlighted
    let light = Green;
    let heavy = Green.on_fixed(22).bold();
    writer.write_with_style(&SIGN_RIGHT, light)?;
    for change in diff.iter() {
        match change {
            ::diff::Result::Both(value, _) => writer.write_with_style(value, light)?,
            ::diff::Result::Right(value) => writer.write_with_style(value, heavy)?,
            _ => (),
        }
    }
    writer.finish()
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(feature = "alloc")]
    use alloc::string::String;

    // ANSI terminal codes used in our outputs.
    //
    // Interpolate these into test strings to make expected values easier to read.
    const RED_LIGHT: &str = "\u{1b}[31m";
    const GREEN_LIGHT: &str = "\u{1b}[32m";
    const RED_HEAVY: &str = "\u{1b}[1;48;5;52;31m";
    const GREEN_HEAVY: &str = "\u{1b}[1;48;5;22;32m";
    const RESET: &str = "\u{1b}[0m";

    /// Given that both of our diff printing functions have the same
    /// type signature, we can reuse the same test code for them.
    ///
    /// This could probably be nicer with traits!
    fn check_printer<TPrint>(printer: TPrint, left: &str, right: &str, expected: &str)
    where
        TPrint: Fn(&mut String, &str, &str) -> fmt::Result,
    {
        let mut actual = String::new();
        printer(&mut actual, left, right).expect("printer function failed");

        // Cannot use IO without stdlib
        #[cfg(feature = "std")]
        println!(
            "## left ##\n\
             {}\n\
             ## right ##\n\
             {}\n\
             ## actual diff ##\n\
             {}\n\
             ## expected diff ##\n\
             {}",
            left, right, actual, expected
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn write_inline_diff_empty() {
        let left = "";
        let right = "";
        let expected = format!(
            "{red_light}<{reset}\n\
             {green_light}>{reset}\n",
            red_light = RED_LIGHT,
            green_light = GREEN_LIGHT,
            reset = RESET,
        );

        check_printer(write_inline_diff, left, right, &expected);
    }

    #[test]
    fn write_inline_diff_added() {
        let left = "";
        let right = "polymerase";
        let expected = format!(
            "{red_light}<{reset}\n\
             {green_light}>{reset}{green_heavy}polymerase{reset}\n",
            red_light = RED_LIGHT,
            green_light = GREEN_LIGHT,
            green_heavy = GREEN_HEAVY,
            reset = RESET,
        );

        check_printer(write_inline_diff, left, right, &expected);
    }

    #[test]
    fn write_inline_diff_removed() {
        let left = "polyacrylamide";
        let right = "";
        let expected = format!(
            "{red_light}<{reset}{red_heavy}polyacrylamide{reset}\n\
             {green_light}>{reset}\n",
            red_light = RED_LIGHT,
            green_light = GREEN_LIGHT,
            red_heavy = RED_HEAVY,
            reset = RESET,
        );

        check_printer(write_inline_diff, left, right, &expected);
    }

    #[test]
    fn write_inline_diff_changed() {
        let left = "polymerase";
        let right = "polyacrylamide";
        let expected = format!(
            "{red_light}<poly{reset}{red_heavy}me{reset}{red_light}ra{reset}{red_heavy}s{reset}{red_light}e{reset}\n\
             {green_light}>poly{reset}{green_heavy}ac{reset}{green_light}r{reset}{green_heavy}yl{reset}{green_light}a{reset}{green_heavy}mid{reset}{green_light}e{reset}\n",
            red_light = RED_LIGHT,
            green_light = GREEN_LIGHT,
            red_heavy = RED_HEAVY,
            green_heavy = GREEN_HEAVY,
            reset = RESET,
        );

        check_printer(write_inline_diff, left, right, &expected);
    }

    /// If one of our strings is empty, it should not be shown at all in the output.
    #[test]
    fn write_lines_empty_string() {
        let left = "";
        let right = "content";
        let expected = format!(
            "{green_light}>content{reset}\n",
            green_light = GREEN_LIGHT,
            reset = RESET,
        );

        check_printer(write_lines, left, right, &expected);
    }

    /// Realistic multiline struct diffing case.
    #[test]
    fn write_lines_struct() {
        let left = r#"Some(
    Foo {
        lorem: "Hello World!",
        ipsum: 42,
        dolor: Ok(
            "hey",
        ),
    },
)"#;
        let right = r#"Some(
    Foo {
        lorem: "Hello Wrold!",
        ipsum: 42,
        dolor: Ok(
            "hey ho!",
        ),
    },
)"#;
        let expected = format!(
            r#" Some(
     Foo {{
{red_light}<        lorem: "Hello W{reset}{red_heavy}o{reset}{red_light}rld!",{reset}
{green_light}>        lorem: "Hello Wr{reset}{green_heavy}o{reset}{green_light}ld!",{reset}
         ipsum: 42,
         dolor: Ok(
{red_light}<            "hey",{reset}
{green_light}>            "hey{reset}{green_heavy} ho!{reset}{green_light}",{reset}
         ),
     }},
 )
"#,
            red_light = RED_LIGHT,
            red_heavy = RED_HEAVY,
            green_light = GREEN_LIGHT,
            green_heavy = GREEN_HEAVY,
            reset = RESET,
        );

        check_printer(write_lines, left, right, &expected);
    }

    /// Relistic multiple line chunks
    ///
    /// We can't support realistic line diffing in large blocks
    /// (also, it's unclear how usefult this is)
    ///
    /// So if we have more than one line in a single removal chunk, disable inline diffing.
    #[test]
    fn write_lines_multiline_block() {
        let left = r#"Proboscis
Cabbage"#;
        let right = r#"Probed
Caravaggio"#;
        let expected = format!(
            r#"{red_light}<Proboscis{reset}
{red_light}<Cabbage{reset}
{green_light}>Probed{reset}
{green_light}>Caravaggio{reset}
"#,
            red_light = RED_LIGHT,
            green_light = GREEN_LIGHT,
            reset = RESET,
        );

        check_printer(write_lines, left, right, &expected);
    }

    /// Single deletion line, multiple insertions - no inline diffing.
    #[test]
    fn write_lines_multiline_insert() {
        let left = r#"Cabbage"#;
        let right = r#"Probed
Caravaggio"#;
        let expected = format!(
            r#"{red_light}<Cabbage{reset}
{green_light}>Probed{reset}
{green_light}>Caravaggio{reset}
"#,
            red_light = RED_LIGHT,
            green_light = GREEN_LIGHT,
            reset = RESET,
        );

        check_printer(write_lines, left, right, &expected);
    }

    /// Multiple deletion, single insertion - no inline diffing.
    #[test]
    fn write_lines_multiline_delete() {
        let left = r#"Proboscis
Cabbage"#;
        let right = r#"Probed"#;
        let expected = format!(
            r#"{red_light}<Proboscis{reset}
{red_light}<Cabbage{reset}
{green_light}>Probed{reset}
"#,
            red_light = RED_LIGHT,
            green_light = GREEN_LIGHT,
            reset = RESET,
        );

        check_printer(write_lines, left, right, &expected);
    }

    /// Regression test for multiline highlighting issue
    #[test]
    fn write_lines_issue12() {
        let left = r#"[
    0,
    0,
    0,
    128,
    10,
    191,
    5,
    64,
]"#;
        let right = r#"[
    84,
    248,
    45,
    64,
]"#;
        let expected = format!(
            r#" [
{red_light}<    0,{reset}
{red_light}<    0,{reset}
{red_light}<    0,{reset}
{red_light}<    128,{reset}
{red_light}<    10,{reset}
{red_light}<    191,{reset}
{red_light}<    5,{reset}
{green_light}>    84,{reset}
{green_light}>    248,{reset}
{green_light}>    45,{reset}
     64,
 ]
"#,
            red_light = RED_LIGHT,
            green_light = GREEN_LIGHT,
            reset = RESET,
        );

        check_printer(write_lines, left, right, &expected);
    }

    mod write_lines_edge_newlines {
        use super::*;

        #[test]
        fn both_trailing() {
            let left = "fan\n";
            let right = "mug\n";
            // Note the additional space at the bottom is caused by a trailing newline
            // adding an additional line with zero content to both sides of the diff
            let expected = format!(
                r#"{red_light}<{reset}{red_heavy}fan{reset}
{green_light}>{reset}{green_heavy}mug{reset}
 
"#,
                red_light = RED_LIGHT,
                red_heavy = RED_HEAVY,
                green_light = GREEN_LIGHT,
                green_heavy = GREEN_HEAVY,
                reset = RESET,
            );

            check_printer(write_lines, left, right, &expected);
        }

        #[test]
        fn both_leading() {
            let left = "\nfan";
            let right = "\nmug";
            // Note the additional space at the top is caused by a leading newline
            // adding an additional line with zero content to both sides of the diff
            let expected = format!(
                r#" 
{red_light}<{reset}{red_heavy}fan{reset}
{green_light}>{reset}{green_heavy}mug{reset}
"#,
                red_light = RED_LIGHT,
                red_heavy = RED_HEAVY,
                green_light = GREEN_LIGHT,
                green_heavy = GREEN_HEAVY,
                reset = RESET,
            );

            check_printer(write_lines, left, right, &expected);
        }

        #[test]
        fn leading_added() {
            let left = "fan";
            let right = "\nmug";
            let expected = format!(
                r#"{red_light}<fan{reset}
{green_light}>{reset}
{green_light}>mug{reset}
"#,
                red_light = RED_LIGHT,
                green_light = GREEN_LIGHT,
                reset = RESET,
            );

            check_printer(write_lines, left, right, &expected);
        }

        #[test]
        fn leading_deleted() {
            let left = "\nfan";
            let right = "mug";
            let expected = format!(
                r#"{red_light}<{reset}
{red_light}<fan{reset}
{green_light}>mug{reset}
"#,
                red_light = RED_LIGHT,
                green_light = GREEN_LIGHT,
                reset = RESET,
            );

            check_printer(write_lines, left, right, &expected);
        }

        #[test]
        fn trailing_added() {
            let left = "fan";
            let right = "mug\n";
            let expected = format!(
                r#"{red_light}<fan{reset}
{green_light}>mug{reset}
{green_light}>{reset}
"#,
                red_light = RED_LIGHT,
                green_light = GREEN_LIGHT,
                reset = RESET,
            );

            check_printer(write_lines, left, right, &expected);
        }

        /// Regression test for double abort
        ///
        /// See: https://github.com/rust-pretty-assertions/rust-pretty-assertions/issues/96
        #[test]
        fn trailing_deleted() {
            // The below inputs caused an abort via double panic
            // we panicked at 'insertion followed by deletion'
            let left = "fan\n";
            let right = "mug";
            let expected = format!(
                r#"{red_light}<{reset}{red_heavy}fan{reset}
{green_light}>{reset}{green_heavy}mug{reset}
{red_light}<{reset}
"#,
                red_light = RED_LIGHT,
                red_heavy = RED_HEAVY,
                green_light = GREEN_LIGHT,
                green_heavy = GREEN_HEAVY,
                reset = RESET,
            );

            check_printer(write_lines, left, right, &expected);
        }
    }
}
