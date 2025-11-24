use super::Style;

/// When printing out one colored string followed by another, use one of
/// these rules to figure out which *extra* control codes need to be sent.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum Difference {
    /// Print out the control codes specified by this style to end up looking
    /// like the second string's styles.
    ExtraStyles(Style),

    /// Converting between these two is impossible, so just send a reset
    /// command and then the second string's styles.
    Reset,

    /// The before style is exactly the same as the after style, so no further
    /// control codes need to be printed.
    Empty,
}

impl Difference {
    /// Compute the 'style difference' required to turn an existing style into
    /// the given, second style.
    ///
    /// For example, to turn green text into green bold text, it's redundant
    /// to write a reset command then a second green+bold command, instead of
    /// just writing one bold command. This method should see that both styles
    /// use the foreground color green, and reduce it to a single command.
    ///
    /// This method returns an enum value because it's not actually always
    /// possible to turn one style into another: for example, text could be
    /// made bold and underlined, but you can't remove the bold property
    /// without also removing the underline property. So when this has to
    /// happen, this function returns None, meaning that the entire set of
    /// styles should be reset and begun again.
    pub fn between(first: &Style, next: &Style) -> Difference {
        use self::Difference::*;

        // XXX(Havvy): This algorithm is kind of hard to replicate without
        // having the Plain/Foreground enum variants, so I'm just leaving
        // it commented out for now, and defaulting to Reset.

        if first == next {
            return Empty;
        }

        // Cannot un-bold, so must Reset.
        if first.is_bold && !next.is_bold {
            return Reset;
        }

        if first.is_dimmed && !next.is_dimmed {
            return Reset;
        }

        if first.is_italic && !next.is_italic {
            return Reset;
        }

        // Cannot un-underline, so must Reset.
        if first.is_underline && !next.is_underline {
            return Reset;
        }

        if first.is_blink && !next.is_blink {
            return Reset;
        }

        if first.is_reverse && !next.is_reverse {
            return Reset;
        }

        if first.is_hidden && !next.is_hidden {
            return Reset;
        }

        if first.is_strikethrough && !next.is_strikethrough {
            return Reset;
        }

        // Cannot go from foreground to no foreground, so must Reset.
        if first.foreground.is_some() && next.foreground.is_none() {
            return Reset;
        }

        // Cannot go from background to no background, so must Reset.
        if first.background.is_some() && next.background.is_none() {
            return Reset;
        }

        let mut extra_styles = Style::default();

        if first.is_bold != next.is_bold {
            extra_styles.is_bold = true;
        }

        if first.is_dimmed != next.is_dimmed {
            extra_styles.is_dimmed = true;
        }

        if first.is_italic != next.is_italic {
            extra_styles.is_italic = true;
        }

        if first.is_underline != next.is_underline {
            extra_styles.is_underline = true;
        }

        if first.is_blink != next.is_blink {
            extra_styles.is_blink = true;
        }

        if first.is_reverse != next.is_reverse {
            extra_styles.is_reverse = true;
        }

        if first.is_hidden != next.is_hidden {
            extra_styles.is_hidden = true;
        }

        if first.is_strikethrough != next.is_strikethrough {
            extra_styles.is_strikethrough = true;
        }

        if first.foreground != next.foreground {
            extra_styles.foreground = next.foreground;
        }

        if first.background != next.background {
            extra_styles.background = next.background;
        }

        ExtraStyles(extra_styles)
    }
}

#[cfg(test)]
mod test {
    use super::Difference::*;
    use super::*;
    use crate::style::Color::*;
    use crate::style::Style;

    fn style() -> Style {
        Style::new()
    }

    macro_rules! test {
        ($name: ident: $first: expr; $next: expr => $result: expr) => {
            #[test]
            fn $name() {
                assert_eq!($result, Difference::between(&$first, &$next));
            }
        };
    }

    test!(nothing:    Green.normal(); Green.normal()  => Empty);
    test!(uppercase:  Green.normal(); Green.bold()    => ExtraStyles(style().bold()));
    test!(lowercase:  Green.bold();   Green.normal()  => Reset);
    test!(nothing2:   Green.bold();   Green.bold()    => Empty);

    test!(color_change: Red.normal(); Blue.normal() => ExtraStyles(Blue.normal()));

    test!(addition_of_blink:          style(); style().blink()          => ExtraStyles(style().blink()));
    test!(addition_of_dimmed:         style(); style().dimmed()         => ExtraStyles(style().dimmed()));
    test!(addition_of_hidden:         style(); style().hidden()         => ExtraStyles(style().hidden()));
    test!(addition_of_reverse:        style(); style().reverse()        => ExtraStyles(style().reverse()));
    test!(addition_of_strikethrough:  style(); style().strikethrough()  => ExtraStyles(style().strikethrough()));

    test!(removal_of_strikethrough:   style().strikethrough(); style()  => Reset);
    test!(removal_of_reverse:         style().reverse();       style()  => Reset);
    test!(removal_of_hidden:          style().hidden();        style()  => Reset);
    test!(removal_of_dimmed:          style().dimmed();        style()  => Reset);
    test!(removal_of_blink:           style().blink();         style()  => Reset);
}
