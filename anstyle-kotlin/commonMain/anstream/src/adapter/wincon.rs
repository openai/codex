/// Incrementally convert to wincon calls for non-contiguous data
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct WinconBytes {
    parser: anstyle_parse::Parser,
    capture: WinconCapture,
}

impl WinconBytes {
    /// Initial state
    pub fn new() -> Self {
        Default::default()
    }

    /// Strip the next segment of data
    pub fn extract_next<'s>(&'s mut self, bytes: &'s [u8]) -> WinconBytesIter<'s> {
        self.capture.reset();
        self.capture.printable.reserve(bytes.len());
        WinconBytesIter {
            bytes,
            parser: &mut self.parser,
            capture: &mut self.capture,
        }
    }
}

/// See [`WinconBytes`]
#[derive(Debug, PartialEq, Eq)]
pub struct WinconBytesIter<'s> {
    bytes: &'s [u8],
    parser: &'s mut anstyle_parse::Parser,
    capture: &'s mut WinconCapture,
}

impl Iterator for WinconBytesIter<'_> {
    type Item = (anstyle::Style, String);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        next_bytes(&mut self.bytes, self.parser, self.capture)
    }
}

#[inline]
fn next_bytes(
    bytes: &mut &[u8],
    parser: &mut anstyle_parse::Parser,
    capture: &mut WinconCapture,
) -> Option<(anstyle::Style, String)> {
    capture.reset();
    while capture.ready.is_none() {
        let byte = if let Some((byte, remainder)) = (*bytes).split_first() {
            *bytes = remainder;
            *byte
        } else {
            break;
        };
        parser.advance(capture, byte);
    }
    if capture.printable.is_empty() {
        return None;
    }

    let style = capture.ready.unwrap_or(capture.style);
    Some((style, std::mem::take(&mut capture.printable)))
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
struct WinconCapture {
    style: anstyle::Style,
    printable: String,
    ready: Option<anstyle::Style>,
}

impl WinconCapture {
    fn reset(&mut self) {
        self.ready = None;
    }
}

impl anstyle_parse::Perform for WinconCapture {
    /// Draw a character to the screen and update states.
    fn print(&mut self, c: char) {
        self.printable.push(c);
    }

    /// Execute a C0 or C1 control function.
    fn execute(&mut self, byte: u8) {
        if byte.is_ascii_whitespace() {
            self.printable.push(byte as char);
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &anstyle_parse::Params,
        _intermediates: &[u8],
        ignore: bool,
        action: u8,
    ) {
        if ignore {
            return;
        }
        if action != b'm' {
            return;
        }

        let mut style = self.style;
        // param/value differences are dependent on the escape code
        let mut state = CsiState::Normal;
        let mut r = None;
        let mut g = None;
        let mut color_target = ColorTarget::Fg;
        for param in params {
            for value in param {
                match (state, *value) {
                    (CsiState::Normal, 0) => {
                        style = anstyle::Style::default();
                        break;
                    }
                    (CsiState::Normal, 1) => {
                        style = style.bold();
                        break;
                    }
                    (CsiState::Normal, 2) => {
                        style = style.dimmed();
                        break;
                    }
                    (CsiState::Normal, 3) => {
                        style = style.italic();
                        break;
                    }
                    (CsiState::Normal, 4) => {
                        style = style.underline();
                        state = CsiState::Underline;
                    }
                    (CsiState::Normal, 21) => {
                        style |= anstyle::Effects::DOUBLE_UNDERLINE;
                        break;
                    }
                    (CsiState::Normal, 7) => {
                        style = style.invert();
                        break;
                    }
                    (CsiState::Normal, 8) => {
                        style = style.hidden();
                        break;
                    }
                    (CsiState::Normal, 9) => {
                        style = style.strikethrough();
                        break;
                    }
                    (CsiState::Normal, 30..=37) => {
                        let color = to_ansi_color(value - 30).expect("within 4-bit range");
                        style = style.fg_color(Some(color.into()));
                        break;
                    }
                    (CsiState::Normal, 38) => {
                        color_target = ColorTarget::Fg;
                        state = CsiState::PrepareCustomColor;
                    }
                    (CsiState::Normal, 39) => {
                        style = style.fg_color(None);
                        break;
                    }
                    (CsiState::Normal, 40..=47) => {
                        let color = to_ansi_color(value - 40).expect("within 4-bit range");
                        style = style.bg_color(Some(color.into()));
                        break;
                    }
                    (CsiState::Normal, 48) => {
                        color_target = ColorTarget::Bg;
                        state = CsiState::PrepareCustomColor;
                    }
                    (CsiState::Normal, 49) => {
                        style = style.bg_color(None);
                        break;
                    }
                    (CsiState::Normal, 58) => {
                        color_target = ColorTarget::Underline;
                        state = CsiState::PrepareCustomColor;
                    }
                    (CsiState::Normal, 90..=97) => {
                        let color = to_ansi_color(value - 90)
                            .expect("within 4-bit range")
                            .bright(true);
                        style = style.fg_color(Some(color.into()));
                        break;
                    }
                    (CsiState::Normal, 100..=107) => {
                        let color = to_ansi_color(value - 100)
                            .expect("within 4-bit range")
                            .bright(true);
                        style = style.bg_color(Some(color.into()));
                        break;
                    }
                    (CsiState::PrepareCustomColor, 5) => {
                        state = CsiState::Ansi256;
                    }
                    (CsiState::PrepareCustomColor, 2) => {
                        state = CsiState::Rgb;
                        r = None;
                        g = None;
                    }
                    (CsiState::Ansi256, n) => {
                        let color = anstyle::Ansi256Color(n as u8);
                        style = match color_target {
                            ColorTarget::Fg => style.fg_color(Some(color.into())),
                            ColorTarget::Bg => style.bg_color(Some(color.into())),
                            ColorTarget::Underline => style.underline_color(Some(color.into())),
                        };
                        break;
                    }
                    (CsiState::Rgb, b) => match (r, g) {
                        (None, _) => {
                            r = Some(b);
                        }
                        (Some(_), None) => {
                            g = Some(b);
                        }
                        (Some(r), Some(g)) => {
                            let color = anstyle::RgbColor(r as u8, g as u8, b as u8);
                            style = match color_target {
                                ColorTarget::Fg => style.fg_color(Some(color.into())),
                                ColorTarget::Bg => style.bg_color(Some(color.into())),
                                ColorTarget::Underline => style.underline_color(Some(color.into())),
                            };
                            break;
                        }
                    },
                    (CsiState::Underline, 0) => {
                        style =
                            style.effects(style.get_effects().remove(anstyle::Effects::UNDERLINE));
                    }
                    (CsiState::Underline, 1) => {
                        // underline already set
                    }
                    (CsiState::Underline, 2) => {
                        style = style
                            .effects(style.get_effects().remove(anstyle::Effects::UNDERLINE))
                            | anstyle::Effects::DOUBLE_UNDERLINE;
                    }
                    (CsiState::Underline, 3) => {
                        style = style
                            .effects(style.get_effects().remove(anstyle::Effects::UNDERLINE))
                            | anstyle::Effects::CURLY_UNDERLINE;
                    }
                    (CsiState::Underline, 4) => {
                        style = style
                            .effects(style.get_effects().remove(anstyle::Effects::UNDERLINE))
                            | anstyle::Effects::DOTTED_UNDERLINE;
                    }
                    (CsiState::Underline, 5) => {
                        style = style
                            .effects(style.get_effects().remove(anstyle::Effects::UNDERLINE))
                            | anstyle::Effects::DASHED_UNDERLINE;
                    }
                    _ => {
                        break;
                    }
                }
            }
        }

        if style != self.style && !self.printable.is_empty() {
            self.ready = Some(self.style);
        }
        self.style = style;
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum CsiState {
    Normal,
    PrepareCustomColor,
    Ansi256,
    Rgb,
    Underline,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum ColorTarget {
    Fg,
    Bg,
    Underline,
}

fn to_ansi_color(digit: u16) -> Option<anstyle::AnsiColor> {
    match digit {
        0 => Some(anstyle::AnsiColor::Black),
        1 => Some(anstyle::AnsiColor::Red),
        2 => Some(anstyle::AnsiColor::Green),
        3 => Some(anstyle::AnsiColor::Yellow),
        4 => Some(anstyle::AnsiColor::Blue),
        5 => Some(anstyle::AnsiColor::Magenta),
        6 => Some(anstyle::AnsiColor::Cyan),
        7 => Some(anstyle::AnsiColor::White),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;

    #[track_caller]
    fn verify(input: &str, expected: Vec<(anstyle::Style, &str)>) {
        let expected = expected
            .into_iter()
            .map(|(style, value)| (style, value.to_owned()))
            .collect::<Vec<_>>();
        let mut state = WinconBytes::new();
        let actual = state.extract_next(input.as_bytes()).collect::<Vec<_>>();
        assert_eq!(expected, actual, "{input:?}");
    }

    #[test]
    fn start() {
        let green_on_red = anstyle::AnsiColor::Green.on(anstyle::AnsiColor::Red);
        let input = format!("{green_on_red}Hello{green_on_red:#} world!");
        let expected = vec![
            (green_on_red, "Hello"),
            (anstyle::Style::default(), " world!"),
        ];
        verify(&input, expected);
    }

    #[test]
    fn middle() {
        let green_on_red = anstyle::AnsiColor::Green.on(anstyle::AnsiColor::Red);
        let input = format!("Hello {green_on_red}world{green_on_red:#}!");
        let expected = vec![
            (anstyle::Style::default(), "Hello "),
            (green_on_red, "world"),
            (anstyle::Style::default(), "!"),
        ];
        verify(&input, expected);
    }

    #[test]
    fn end() {
        let green_on_red = anstyle::AnsiColor::Green.on(anstyle::AnsiColor::Red);
        let input = format!("Hello {green_on_red}world!{green_on_red:#}");
        let expected = vec![
            (anstyle::Style::default(), "Hello "),
            (green_on_red, "world!"),
        ];
        verify(&input, expected);
    }

    #[test]
    fn ansi256_colors() {
        let ansi_11 = anstyle::Ansi256Color(11).on_default();
        // termcolor only supports "brights" via these
        let input = format!("Hello {ansi_11}world{ansi_11:#}!");
        let expected = vec![
            (anstyle::Style::default(), "Hello "),
            (ansi_11, "world"),
            (anstyle::Style::default(), "!"),
        ];
        verify(&input, expected);
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn wincon_no_escapes(s in "\\PC*") {
            let expected = if s.is_empty() {
                vec![]
            } else {
                vec![(anstyle::Style::default(), s.clone())]
            };
            let mut state = WinconBytes::new();
            let actual = state.extract_next(s.as_bytes()).collect::<Vec<_>>();
            assert_eq!(expected, actual);
        }
    }
}
