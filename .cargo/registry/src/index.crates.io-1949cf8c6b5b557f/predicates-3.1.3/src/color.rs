#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Palette {
    description: anstyle::Style,
    var: anstyle::Style,
    expected: anstyle::Style,
}

impl Palette {
    pub(crate) fn new(alternate: bool) -> Self {
        if alternate && cfg!(feature = "color") {
            Self {
                description: anstyle::AnsiColor::Blue.on_default() | anstyle::Effects::BOLD,
                var: anstyle::AnsiColor::Red.on_default() | anstyle::Effects::BOLD,
                expected: anstyle::AnsiColor::Green.on_default() | anstyle::Effects::BOLD,
            }
        } else {
            Self::plain()
        }
    }

    pub(crate) fn plain() -> Self {
        Self {
            description: Default::default(),
            var: Default::default(),
            expected: Default::default(),
        }
    }

    pub(crate) fn description<D: std::fmt::Display>(self, display: D) -> Styled<D> {
        Styled::new(display, self.description)
    }

    pub(crate) fn var<D: std::fmt::Display>(self, display: D) -> Styled<D> {
        Styled::new(display, self.var)
    }

    pub(crate) fn expected<D: std::fmt::Display>(self, display: D) -> Styled<D> {
        Styled::new(display, self.expected)
    }
}

#[derive(Debug)]
pub(crate) struct Styled<D> {
    display: D,
    style: anstyle::Style,
}

impl<D: std::fmt::Display> Styled<D> {
    pub(crate) fn new(display: D, style: anstyle::Style) -> Self {
        Self { display, style }
    }
}

impl<D: std::fmt::Display> std::fmt::Display for Styled<D> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "{}", self.style.render())?;
            self.display.fmt(f)?;
            write!(f, "{}", self.style.render_reset())?;
            Ok(())
        } else {
            self.display.fmt(f)
        }
    }
}
