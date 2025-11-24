use std::fmt;

#[derive(Debug)]
pub(crate) struct FilterOp {
    #[cfg(feature = "regex")]
    inner: regex::Regex,
    #[cfg(not(feature = "regex"))]
    inner: String,
}

#[cfg(feature = "regex")]
impl FilterOp {
    pub(crate) fn new(spec: &str) -> Result<Self, String> {
        match regex::Regex::new(spec) {
            Ok(r) => Ok(Self { inner: r }),
            Err(e) => Err(e.to_string()),
        }
    }

    pub(crate) fn is_match(&self, s: &str) -> bool {
        self.inner.is_match(s)
    }
}

#[cfg(not(feature = "regex"))]
impl FilterOp {
    pub fn new(spec: &str) -> Result<Self, String> {
        Ok(Self {
            inner: spec.to_string(),
        })
    }

    pub fn is_match(&self, s: &str) -> bool {
        s.contains(&self.inner)
    }
}

impl fmt::Display for FilterOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
