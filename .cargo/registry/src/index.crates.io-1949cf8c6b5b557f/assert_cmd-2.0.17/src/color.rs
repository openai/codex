#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Palette {
    key: anstyle::Style,
    value: anstyle::Style,
}

impl Palette {
    pub(crate) fn color() -> Self {
        if cfg!(feature = "color") {
            Self {
                key: anstyle::AnsiColor::Blue.on_default() | anstyle::Effects::BOLD,
                value: anstyle::AnsiColor::Yellow.on_default() | anstyle::Effects::BOLD,
            }
        } else {
            Self::plain()
        }
    }

    pub(crate) fn plain() -> Self {
        Self::default()
    }

    pub(crate) fn key<D: std::fmt::Display>(self, display: D) -> Styled<D> {
        Styled::new(display, self.key)
    }

    pub(crate) fn value<D: std::fmt::Display>(self, display: D) -> Styled<D> {
        Styled::new(display, self.value)
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
