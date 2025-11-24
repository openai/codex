use std::fmt::{Display, Error, Formatter};

pub use std::collections::btree_map as map;
pub struct Sep<S>(pub &'static str, pub S);

impl<'a, S: Display> Display for Sep<&'a Vec<S>> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        let &Sep(sep, vec) = self;
        let mut elems = vec.iter();
        if let Some(elem) = elems.next() {
            write!(fmt, "{}", elem)?;
            for elem in elems {
                write!(fmt, "{}{}", sep, elem)?;
            }
        }
        Ok(())
    }
}

pub struct Escape<S>(pub S);

impl<S: Display> Display for Escape<S> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        let tmp = format!("{}", self.0);
        for c in tmp.chars() {
            match c {
                'a'..='z' | '0'..='9' | 'A'..='Z' => write!(fmt, "{}", c)?,
                '_' => write!(fmt, "__")?,
                _ => write!(fmt, "_{:x}", c as usize)?,
            }
        }
        Ok(())
    }
}

pub struct Prefix<S>(pub &'static str, pub S);

impl<'a, S: Display> Display for Prefix<&'a [S]> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        let &Prefix(prefix, vec) = self;
        for elem in vec.iter() {
            write!(fmt, "{}{}", prefix, elem)?;
        }
        Ok(())
    }
}

/// Strip leading and trailing whitespace.
pub fn strip(s: &str) -> &str {
    s.trim_matches(char::is_whitespace)
}
