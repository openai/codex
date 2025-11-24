use std::{fmt, ops::Range};

/// An unescaped route that keeps track of the position of escaped characters ('{{' or '}}').
///
/// Note that this type dereferences to `&[u8]`.
#[derive(Clone, Default)]
pub struct UnescapedRoute {
    // The raw unescaped route.
    inner: Vec<u8>,
    escaped: Vec<usize>,
}

impl UnescapedRoute {
    /// Unescapes escaped brackets ('{{' or '}}') in a route.
    pub fn new(mut inner: Vec<u8>) -> UnescapedRoute {
        let mut escaped = Vec::new();
        let mut i = 0;

        while let Some(&c) = inner.get(i) {
            if (c == b'{' && inner.get(i + 1) == Some(&b'{'))
                || (c == b'}' && inner.get(i + 1) == Some(&b'}'))
            {
                inner.remove(i);
                escaped.push(i);
            }

            i += 1;
        }

        UnescapedRoute { inner, escaped }
    }

    /// Returns true if the character at the given index was escaped.
    pub fn is_escaped(&self, i: usize) -> bool {
        self.escaped.contains(&i)
    }

    /// Replaces the characters in the given range.
    pub fn splice(
        &mut self,
        range: Range<usize>,
        replace: Vec<u8>,
    ) -> impl Iterator<Item = u8> + '_ {
        // Ignore any escaped characters in the range being replaced.
        self.escaped.retain(|x| !range.contains(x));

        // Update the escaped indices.
        let offset = (replace.len() as isize) - (range.len() as isize);
        for i in &mut self.escaped {
            if *i > range.end {
                *i = i.checked_add_signed(offset).unwrap();
            }
        }

        self.inner.splice(range, replace)
    }

    /// Appends another route to the end of this one.
    pub fn append(&mut self, other: &UnescapedRoute) {
        for i in &other.escaped {
            self.escaped.push(self.inner.len() + i);
        }

        self.inner.extend_from_slice(&other.inner);
    }

    /// Truncates the route to the given length.
    pub fn truncate(&mut self, to: usize) {
        self.escaped.retain(|&x| x < to);
        self.inner.truncate(to);
    }

    /// Returns a reference to this route.
    pub fn as_ref(&self) -> UnescapedRef<'_> {
        UnescapedRef {
            inner: &self.inner,
            escaped: &self.escaped,
            offset: 0,
        }
    }

    /// Returns a reference to the unescaped slice.
    pub fn unescaped(&self) -> &[u8] {
        &self.inner
    }

    /// Returns the unescaped route.
    pub fn into_unescaped(self) -> Vec<u8> {
        self.inner
    }
}

impl std::ops::Deref for UnescapedRoute {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl fmt::Debug for UnescapedRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(std::str::from_utf8(&self.inner).unwrap(), f)
    }
}

/// A reference to an `UnescapedRoute`.
#[derive(Copy, Clone)]
pub struct UnescapedRef<'a> {
    pub inner: &'a [u8],
    escaped: &'a [usize],
    // An offset applied to each escaped index.
    offset: isize,
}

impl<'a> UnescapedRef<'a> {
    /// Converts this reference into an owned route.
    pub fn to_owned(self) -> UnescapedRoute {
        let mut escaped = Vec::new();
        for &i in self.escaped {
            let i = i.checked_add_signed(self.offset);

            match i {
                Some(i) if i < self.inner.len() => escaped.push(i),
                _ => {}
            }
        }

        UnescapedRoute {
            escaped,
            inner: self.inner.to_owned(),
        }
    }

    /// Returns `true` if the character at the given index was escaped.
    pub fn is_escaped(&self, i: usize) -> bool {
        if let Some(i) = i.checked_add_signed(-self.offset) {
            return self.escaped.contains(&i);
        }

        false
    }

    /// Slices the route with `start..`.
    pub fn slice_off(&self, start: usize) -> UnescapedRef<'a> {
        UnescapedRef {
            inner: &self.inner[start..],
            escaped: self.escaped,
            offset: self.offset - (start as isize),
        }
    }

    /// Slices the route with `..end`.
    pub fn slice_until(&self, end: usize) -> UnescapedRef<'a> {
        UnescapedRef {
            inner: &self.inner[..end],
            escaped: self.escaped,
            offset: self.offset,
        }
    }

    /// Returns a reference to the unescaped slice.
    pub fn unescaped(&self) -> &[u8] {
        self.inner
    }
}

impl<'a> std::ops::Deref for UnescapedRef<'a> {
    type Target = &'a [u8];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> fmt::Debug for UnescapedRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnescapedRef")
            .field("inner", &std::str::from_utf8(self.inner))
            .field("escaped", &self.escaped)
            .field("offset", &self.offset)
            .finish()
    }
}
