use std::fmt;

/// Represents something surrounded by opening and closing strings.
pub(crate) struct Quoted<T> {
    open: &'static str,
    body: T,
    close: &'static str,
}

impl<T> Quoted<T> {
    /// Creates a new instance with matching open and close strings.
    pub fn new(quote: &'static str, body: T) -> Self {
        Self {
            open: quote,
            body,
            close: quote,
        }
    }

    /// Creates a new instance using a backtick as the open and close string.
    pub fn backticks(body: T) -> Self {
        Self::new("`", body)
    }
}

impl<T: fmt::Display> fmt::Display for Quoted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.open, self.body, self.close)
    }
}

/// Iterate through a list of items, writing each of them to the target separated
/// by a delimiter string.
pub(crate) fn write_delimited<T: fmt::Display>(
    f: &mut impl fmt::Write,
    items: impl IntoIterator<Item = T>,
    delimiter: &str,
) -> fmt::Result {
    let mut first = true;
    for item in items {
        if !first {
            write!(f, "{}", delimiter)?;
        }
        first = false;
        write!(f, "{}", item)?;
    }

    Ok(())
}
