//! Provide tools for generating anstyle stylings from text

use anstyle::{AnsiColor, Color as AColor, Effects, Style};
use cansi::{v3::CategorisedSlice, Color, Intensity};

/// Produce a stream of [`StyledStr`] from text that contains ansi escape sequences
pub(crate) fn styled_stream(text: &str) -> impl Iterator<Item = StyledStr<'_>> {
    let categorized = cansi::v3::categorise_text(text);
    categorized.into_iter().map(|x| x.into())
}

/// Represents a Section of text, along with the desired styling for it
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct StyledStr<'text> {
    pub(crate) text: &'text str,
    pub(crate) style: Style,
}

impl<'text> From<CategorisedSlice<'text>> for StyledStr<'text> {
    fn from(category: CategorisedSlice<'text>) -> Self {
        let mut style = Style::new();
        style = style
            .fg_color(cansi_to_anstyle_color(category.fg))
            .bg_color(cansi_to_anstyle_color(category.bg));

        let effects = create_effects(&category);
        style = style.effects(effects);

        Self {
            text: category.text,
            style,
        }
    }
}

fn create_effects(category: &CategorisedSlice<'_>) -> Effects {
    Effects::new()
        .set(Effects::ITALIC, category.italic.unwrap_or(false))
        .set(Effects::BLINK, category.blink.unwrap_or(false))
        .set(Effects::INVERT, category.reversed.unwrap_or(false))
        .set(Effects::HIDDEN, category.hidden.unwrap_or(false))
        .set(
            Effects::STRIKETHROUGH,
            category.strikethrough.unwrap_or(false),
        )
        .set(Effects::UNDERLINE, category.underline.unwrap_or(false))
        .set(Effects::BOLD, is_bold(category.intensity))
        .set(Effects::DIMMED, is_faint(category.intensity))
}

fn is_bold(intensity: Option<Intensity>) -> bool {
    matches!(intensity, Some(Intensity::Bold))
}

fn is_faint(intensity: Option<Intensity>) -> bool {
    matches!(intensity, Some(Intensity::Faint))
}

fn cansi_to_anstyle_color(color: Option<Color>) -> Option<AColor> {
    match color {
        Some(Color::Black) => Some(AColor::Ansi(AnsiColor::Black)),
        Some(Color::Red) => Some(AColor::Ansi(AnsiColor::Red)),
        Some(Color::Green) => Some(AColor::Ansi(AnsiColor::Green)),
        Some(Color::Yellow) => Some(AColor::Ansi(AnsiColor::Yellow)),
        Some(Color::Blue) => Some(AColor::Ansi(AnsiColor::Blue)),
        Some(Color::Magenta) => Some(AColor::Ansi(AnsiColor::Magenta)),
        Some(Color::Cyan) => Some(AColor::Ansi(AnsiColor::Cyan)),
        Some(Color::White) => Some(AColor::Ansi(AnsiColor::White)),
        Some(Color::BrightBlack) => Some(AColor::Ansi(AnsiColor::BrightBlack)),
        Some(Color::BrightRed) => Some(AColor::Ansi(AnsiColor::BrightRed)),
        Some(Color::BrightGreen) => Some(AColor::Ansi(AnsiColor::BrightGreen)),
        Some(Color::BrightYellow) => Some(AColor::Ansi(AnsiColor::BrightYellow)),
        Some(Color::BrightBlue) => Some(AColor::Ansi(AnsiColor::BrightBlue)),
        Some(Color::BrightMagenta) => Some(AColor::Ansi(AnsiColor::BrightMagenta)),
        Some(Color::BrightCyan) => Some(AColor::Ansi(AnsiColor::BrightCyan)),
        Some(Color::BrightWhite) => Some(AColor::Ansi(AnsiColor::BrightWhite)),
        None => None,
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    /// Creates a [`CategorisedSlice`] for Testing
    ///
    /// ```rust
    /// styled_str!(Text, [Color:COLOR_SET] [Intensity:INTENSITY_SET] [Effects:EFFECTS_SET])
    /// ```
    ///
    /// Where:
    ///     `COLOR_SET={fg|bg}:<cansi::Color>`
    ///     `INTENSITY_SET=<cansi::Intensity>`
    ///     `EFFECTS_SET={"underline"|"italic"|"blink"|"reversed"|"strikethrough"|"hidden"};+`
    macro_rules! styled_str {
        ($text: literal, $(Color:$color_key:literal:$color_val:expr;)* $(Intensity:$intensity:expr;)? $(Effects:$($key:literal;)+)? ) => {
            {
            let mut cat_text = CategorisedSlice {
                text: $text,
                start: 0,
                end: 5,
                fg: None,
                bg: None,
                intensity: Some(Intensity::Normal),
                italic: None,
                underline: None,
                blink: None,
                reversed: None,
                strikethrough: None,
                hidden: None,
            };

            $(
            match $color_key {
                "fg" => cat_text.fg = Some($color_val),
                "bg" => cat_text.bg = Some($color_val),
                _ => panic!("Not A Valid key for color")
            };
            )*
            $(
                cat_text.intensity = Some($intensity);
            )?
            $($(
            match $key {
                "underline" => cat_text.underline = Some(true),
                "italic" => cat_text.italic= Some(true),
                "blink" => cat_text.blink= Some(true),
                "reversed" => cat_text.reversed = Some(true),
                "strikethrough" => cat_text.strikethrough= Some(true),
                "hidden" => cat_text.hidden= Some(true),
                _ => panic!("Not A Valid key for effects")
            };
            )+)?
            cat_text
        }}
    }

    #[test]
    fn from_categorized_underlined() {
        let categorised = styled_str!("Hello", Effects:"underline";);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(styled_str.style.get_effects().contains(Effects::UNDERLINE));
    }

    #[test]
    fn from_categorized_underlined_striketrhough() {
        let categorised = styled_str!("Hello", Effects:"underline";"strikethrough";);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(styled_str.style.get_effects().contains(Effects::UNDERLINE));
        assert!(styled_str
            .style
            .get_effects()
            .contains(Effects::STRIKETHROUGH));
    }

    #[test]
    fn from_categorized_blink() {
        let categorised = styled_str!("Hello", Effects:"blink";);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(styled_str.style.get_effects().contains(Effects::BLINK));
    }

    #[test]
    fn from_categorized_reversed() {
        let categorised = styled_str!("Hello", Effects:"reversed";);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(styled_str.style.get_effects().contains(Effects::INVERT));
    }

    #[test]
    fn from_categorized_strikthrough() {
        let categorised = styled_str!("Hello", Effects:"strikethrough";);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(styled_str
            .style
            .get_effects()
            .contains(Effects::STRIKETHROUGH));
    }

    #[test]
    fn from_categorized_hidden() {
        let categorised = styled_str!("Hello", Effects:"hidden";);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(styled_str.style.get_effects().contains(Effects::HIDDEN));
    }

    #[test]
    fn from_categorized_bg() {
        let categorised = styled_str!("Hello", Color:"bg":Color::Blue;);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(matches!(
            styled_str.style.get_bg_color(),
            Some(AColor::Ansi(AnsiColor::Blue))
        ));
    }

    #[test]
    fn from_categorized_fg() {
        let categorised = styled_str!("Hello", Color:"fg":Color::Blue;);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(matches!(
            styled_str.style.get_fg_color(),
            Some(AColor::Ansi(AnsiColor::Blue))
        ));
    }

    #[test]
    fn from_categorized_bold() {
        let categorised = styled_str!("Hello", Intensity:Intensity::Bold;);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(styled_str.style.get_effects().contains(Effects::BOLD));
    }

    #[test]
    fn from_categorized_faint() {
        let categorised = styled_str!("Hello", Intensity:Intensity::Faint;);
        let styled_str: StyledStr<'_> = categorised.into();
        assert!(styled_str.style.get_effects().contains(Effects::DIMMED));
    }
}
