use tui::style::Color;

/// This enum stores most types of ansi escape sequences  
///
/// You can turn an escape sequence to this enum variant using
/// AnsiCode::from(code: u8)
/// This doesn't support all of them but does support most of them.  

#[derive(Debug, PartialEq, Clone)]
#[non_exhaustive]
pub enum AnsiCode {
    /// Reset the terminal
    Reset,
    /// Set font to bold
    Bold,
    /// Set font to faint
    Faint,
    /// Set font to italic
    Italic,
    /// Set font to underline
    Underline,
    /// Set cursor to slowblink
    SlowBlink,
    /// Set cursor to rapidblink
    RapidBlink,
    /// Invert the colors
    Reverse,
    /// Conceal text
    Conceal,
    /// Display crossed out text
    CrossedOut,
    /// Choose primary font
    PrimaryFont,
    /// Choose alternate font
    AlternateFont,
    /// Choose alternate fonts 1-9
    #[allow(dead_code)]
    AlternateFonts(u8), // = 11..19, // from 11 to 19
    /// Fraktur ? No clue
    Fraktur,
    /// Turn off bold
    BoldOff,
    /// Set text to normal
    Normal,
    /// Turn off Italic
    NotItalic,
    /// Turn off underline
    UnderlineOff,
    /// Turn off blinking
    BlinkOff,
    // 26 ?
    /// Don't invert colors
    InvertOff,
    /// Reveal text
    Reveal,
    /// Turn off Crossedout text
    CrossedOutOff,
    /// Set foreground color (4-bit)
    ForegroundColor(Color), //, 31..37//Issue 60553 https://github.com/rust-lang/rust/issues/60553
    /// Set foreground color (8-bit and 24-bit)
    SetForegroundColor,
    /// Default foreground color
    DefaultForegroundColor,
    /// Set background color (4-bit)
    BackgroundColor(Color), // 41..47
    /// Set background color (8-bit and 24-bit)
    SetBackgroundColor,
    /// Default background color
    DefaultBackgroundColor, // 49
    /// Other / non supported escape codes
    Code(Vec<u8>),
}

impl From<u8> for AnsiCode {
    fn from(code: u8) -> Self {
        match code {
            0 => AnsiCode::Reset,
            1 => AnsiCode::Bold,
            2 => AnsiCode::Faint,
            3 => AnsiCode::Italic,
            4 => AnsiCode::Underline,
            5 => AnsiCode::SlowBlink,
            6 => AnsiCode::RapidBlink,
            7 => AnsiCode::Reverse,
            8 => AnsiCode::Conceal,
            9 => AnsiCode::CrossedOut,
            10 => AnsiCode::PrimaryFont,
            11 => AnsiCode::AlternateFont,
            // AnsiCode::// AlternateFont = 11..19, // from 11 to 19
            20 => AnsiCode::Fraktur,
            21 => AnsiCode::BoldOff,
            22 => AnsiCode::Normal,
            23 => AnsiCode::NotItalic,
            24 => AnsiCode::UnderlineOff,
            25 => AnsiCode::BlinkOff,
            // 26 ?
            27 => AnsiCode::InvertOff,
            28 => AnsiCode::Reveal,
            29 => AnsiCode::CrossedOutOff,
            30 => AnsiCode::ForegroundColor(Color::Black),
            31 => AnsiCode::ForegroundColor(Color::Red),
            32 => AnsiCode::ForegroundColor(Color::Green),
            33 => AnsiCode::ForegroundColor(Color::Yellow),
            34 => AnsiCode::ForegroundColor(Color::Blue),
            35 => AnsiCode::ForegroundColor(Color::Magenta),
            36 => AnsiCode::ForegroundColor(Color::Cyan),
            37 => AnsiCode::ForegroundColor(Color::Gray),
            38 => AnsiCode::SetForegroundColor,
            39 => AnsiCode::DefaultForegroundColor,
            40 => AnsiCode::BackgroundColor(Color::Black),
            41 => AnsiCode::BackgroundColor(Color::Red),
            42 => AnsiCode::BackgroundColor(Color::Green),
            43 => AnsiCode::BackgroundColor(Color::Yellow),
            44 => AnsiCode::BackgroundColor(Color::Blue),
            45 => AnsiCode::BackgroundColor(Color::Magenta),
            46 => AnsiCode::BackgroundColor(Color::Cyan),
            47 => AnsiCode::BackgroundColor(Color::Gray),
            48 => AnsiCode::SetBackgroundColor,
            49 => AnsiCode::DefaultBackgroundColor,
            90 => AnsiCode::ForegroundColor(Color::DarkGray),
            91 => AnsiCode::ForegroundColor(Color::LightRed),
            92 => AnsiCode::ForegroundColor(Color::LightGreen),
            93 => AnsiCode::ForegroundColor(Color::LightYellow),
            94 => AnsiCode::ForegroundColor(Color::LightBlue),
            95 => AnsiCode::ForegroundColor(Color::LightMagenta),
            96 => AnsiCode::ForegroundColor(Color::LightCyan),
            97 => AnsiCode::ForegroundColor(Color::White),
            100 => AnsiCode::BackgroundColor(Color::DarkGray),
            101 => AnsiCode::BackgroundColor(Color::LightRed),
            102 => AnsiCode::BackgroundColor(Color::LightGreen),
            103 => AnsiCode::BackgroundColor(Color::LightYellow),
            104 => AnsiCode::BackgroundColor(Color::LightBlue),
            105 => AnsiCode::BackgroundColor(Color::LightMagenta),
            106 => AnsiCode::BackgroundColor(Color::LightCyan),
            107 => AnsiCode::ForegroundColor(Color::White),
            code => AnsiCode::Code(vec![code]),
        }
    }
}
