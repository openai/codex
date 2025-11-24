use core::fmt;

#[cfg(feature = "supports-colors")]
/// A display wrapper which applies a transformation based on if the given stream supports
/// colored terminal output
pub struct SupportsColorsDisplay<'a, InVal, Out, ApplyFn>(
    pub(crate) &'a InVal,
    pub(crate) ApplyFn,
    pub(crate) Stream,
)
where
    InVal: ?Sized,
    ApplyFn: Fn(&'a InVal) -> Out;

/// A possible stream source.
#[derive(Clone, Copy, Debug)]
pub enum Stream {
    /// Standard output.
    Stdout,

    /// Standard error.
    Stderr,
}

use crate::OVERRIDE;

impl From<supports_color::Stream> for Stream {
    fn from(stream: supports_color::Stream) -> Self {
        match stream {
            supports_color::Stream::Stdout => Self::Stdout,
            supports_color::Stream::Stderr => Self::Stderr,
        }
    }
}

impl From<supports_color_2::Stream> for Stream {
    fn from(stream: supports_color_2::Stream) -> Self {
        match stream {
            supports_color_2::Stream::Stdout => Self::Stdout,
            supports_color_2::Stream::Stderr => Self::Stderr,
        }
    }
}

macro_rules! impl_fmt_for {
    ($($trait:path),* $(,)?) => {
        $(
            impl<'a, In, Out, F> $trait for SupportsColorsDisplay<'a, In, Out, F>
                where In: $trait,
                      Out: $trait,
                      F: Fn(&'a In) -> Out,
            {
                #[inline(always)]
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    let (force_enabled, force_disabled) = OVERRIDE.is_force_enabled_or_disabled();
                    if force_enabled || (on_cached(self.2) && !force_disabled) {
                        <Out as $trait>::fmt(&self.1(self.0), f)
                    } else {
                        <In as $trait>::fmt(self.0, f)
                    }
                }
            }
        )*
    };
}

fn on_cached(stream: Stream) -> bool {
    let stream = match stream {
        Stream::Stdout => supports_color::Stream::Stdout,
        Stream::Stderr => supports_color::Stream::Stderr,
    };
    supports_color::on_cached(stream)
        .map(|level| level.has_basic)
        .unwrap_or(false)
}

impl_fmt_for! {
    fmt::Display,
    fmt::Debug,
    fmt::UpperHex,
    fmt::LowerHex,
    fmt::Binary,
    fmt::UpperExp,
    fmt::LowerExp,
    fmt::Octal,
    fmt::Pointer,
}

#[cfg(test)]
mod test {
    use crate::OwoColorize;

    #[test]
    fn test_supports_color_versions() {
        println!(
            "{}",
            "This might be green"
                .if_supports_color(supports_color_2::Stream::Stdout, |x| x.green())
        );

        println!(
            "{}",
            "This might be red".if_supports_color(supports_color::Stream::Stdout, |x| x.red())
        );
    }
}
