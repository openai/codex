/// Opaque string storage for raw TOML; internal to `toml_edit`
#[derive(PartialEq, Eq, Clone, Hash)]
pub struct RawString(RawStringInner);

#[derive(PartialEq, Eq, Clone, Hash)]
enum RawStringInner {
    Empty,
    Explicit(String),
    Spanned(std::ops::Range<usize>),
}

impl RawString {
    pub(crate) fn with_span(span: std::ops::Range<usize>) -> Self {
        Self(RawStringInner::Spanned(span))
    }

    /// Access the underlying string
    ///
    /// This generally requires a [`DocumentMut`][crate::DocumentMut].
    pub fn as_str(&self) -> Option<&str> {
        match &self.0 {
            RawStringInner::Empty => Some(""),
            RawStringInner::Explicit(s) => Some(s.as_str()),
            RawStringInner::Spanned(_) => None,
        }
    }

    /// The location within the original document
    ///
    /// This generally requires a [`Document`][crate::Document].
    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        match &self.0 {
            RawStringInner::Empty => None,
            RawStringInner::Explicit(_) => None,
            RawStringInner::Spanned(span) => Some(span.clone()),
        }
    }

    pub(crate) fn to_str<'s>(&'s self, input: &'s str) -> &'s str {
        match &self.0 {
            RawStringInner::Empty => "",
            RawStringInner::Explicit(s) => s.as_str(),
            RawStringInner::Spanned(span) => input
                .get(span.clone())
                .unwrap_or_else(|| panic!("span {span:?} should be in input:\n```\n{input}\n```")),
        }
    }

    pub(crate) fn to_str_with_default<'s>(
        &'s self,
        input: Option<&'s str>,
        default: &'s str,
    ) -> &'s str {
        match &self.0 {
            RawStringInner::Empty => "",
            RawStringInner::Explicit(s) => s.as_str(),
            RawStringInner::Spanned(span) => {
                if let Some(input) = input {
                    input.get(span.clone()).unwrap_or_else(|| {
                        panic!("span {span:?} should be in input:\n```\n{input}\n```")
                    })
                } else {
                    default
                }
            }
        }
    }

    pub(crate) fn despan(&mut self, input: &str) {
        match &self.0 {
            RawStringInner::Empty => {}
            RawStringInner::Explicit(_) => {}
            RawStringInner::Spanned(span) => {
                if span.start == span.end {
                    *self = Self(RawStringInner::Empty);
                } else {
                    *self = Self::from(input.get(span.clone()).unwrap_or_else(|| {
                        panic!("span {span:?} should be in input:\n```\n{input}\n```")
                    }));
                }
            }
        }
    }

    #[cfg(feature = "display")]
    pub(crate) fn encode(&self, buf: &mut dyn std::fmt::Write, input: &str) -> std::fmt::Result {
        let raw = self.to_str(input);
        for part in raw.split('\r') {
            write!(buf, "{part}")?;
        }
        Ok(())
    }

    #[cfg(feature = "display")]
    pub(crate) fn encode_with_default(
        &self,
        buf: &mut dyn std::fmt::Write,
        input: Option<&str>,
        default: &str,
    ) -> std::fmt::Result {
        let raw = self.to_str_with_default(input, default);
        for part in raw.split('\r') {
            write!(buf, "{part}")?;
        }
        Ok(())
    }
}

impl Default for RawString {
    fn default() -> Self {
        Self(RawStringInner::Empty)
    }
}

impl std::fmt::Debug for RawString {
    #[inline]
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self.0 {
            RawStringInner::Empty => write!(formatter, "empty"),
            RawStringInner::Explicit(s) => write!(formatter, "{s:?}"),
            RawStringInner::Spanned(s) => write!(formatter, "{s:?}"),
        }
    }
}

impl From<&str> for RawString {
    #[inline]
    fn from(s: &str) -> Self {
        if s.is_empty() {
            Self(RawStringInner::Empty)
        } else {
            String::from(s).into()
        }
    }
}

impl From<String> for RawString {
    #[inline]
    fn from(s: String) -> Self {
        if s.is_empty() {
            Self(RawStringInner::Empty)
        } else {
            Self(RawStringInner::Explicit(s))
        }
    }
}

impl From<&String> for RawString {
    #[inline]
    fn from(s: &String) -> Self {
        if s.is_empty() {
            Self(RawStringInner::Empty)
        } else {
            String::from(s).into()
        }
    }
}

impl From<Box<str>> for RawString {
    #[inline]
    fn from(s: Box<str>) -> Self {
        if s.is_empty() {
            Self(RawStringInner::Empty)
        } else {
            String::from(s).into()
        }
    }
}
