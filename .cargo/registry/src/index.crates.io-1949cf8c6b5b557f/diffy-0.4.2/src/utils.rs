//! Common utilities

use std::{
    collections::{hash_map::Entry, HashMap},
    hash::Hash,
};

/// Classifies lines, converting lines into unique `u64`s for quicker comparison
pub struct Classifier<'a, T: ?Sized> {
    next_id: u64,
    unique_ids: HashMap<&'a T, u64>,
}

impl<'a, T: ?Sized + Eq + Hash> Classifier<'a, T> {
    fn classify(&mut self, record: &'a T) -> u64 {
        match self.unique_ids.entry(record) {
            Entry::Occupied(o) => *o.get(),
            Entry::Vacant(v) => {
                let id = self.next_id;
                self.next_id += 1;
                *v.insert(id)
            }
        }
    }
}

impl<'a, T: ?Sized + Text> Classifier<'a, T> {
    pub fn classify_lines(&mut self, text: &'a T) -> (Vec<&'a T>, Vec<u64>) {
        LineIter::new(text)
            .map(|line| (line, self.classify(line)))
            .unzip()
    }
}

impl<T: Eq + Hash + ?Sized> Default for Classifier<'_, T> {
    fn default() -> Self {
        Self {
            next_id: 0,
            unique_ids: HashMap::default(),
        }
    }
}

/// Iterator over the lines of a string, including the `\n` character.
pub struct LineIter<'a, T: ?Sized>(&'a T);

impl<'a, T: ?Sized> LineIter<'a, T> {
    pub fn new(text: &'a T) -> Self {
        Self(text)
    }
}

impl<'a, T: Text + ?Sized> Iterator for LineIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }

        let end = if let Some(idx) = self.0.find("\n") {
            idx + 1
        } else {
            self.0.len()
        };

        let (line, remaining) = self.0.split_at(end);
        self.0 = remaining;
        Some(line)
    }
}

/// A helper trait for processing text like `str` and `[u8]`
/// Useful for abstracting over those types for parsing as well as breaking input into lines
pub trait Text: Eq + Hash {
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
    fn starts_with(&self, prefix: &str) -> bool;
    #[allow(unused)]
    fn ends_with(&self, suffix: &str) -> bool;
    fn strip_prefix(&self, prefix: &str) -> Option<&Self>;
    fn strip_suffix(&self, suffix: &str) -> Option<&Self>;
    fn split_at_exclusive(&self, needle: &str) -> Option<(&Self, &Self)>;
    fn find(&self, needle: &str) -> Option<usize>;
    fn split_at(&self, mid: usize) -> (&Self, &Self);
    fn as_str(&self) -> Option<&str>;
    fn as_bytes(&self) -> &[u8];
    #[allow(unused)]
    fn lines(&self) -> LineIter<Self>;

    fn parse<T: std::str::FromStr>(&self) -> Option<T> {
        self.as_str().and_then(|s| s.parse().ok())
    }
}

impl Text for str {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.starts_with(prefix)
    }

    fn ends_with(&self, suffix: &str) -> bool {
        self.ends_with(suffix)
    }

    fn strip_prefix(&self, prefix: &str) -> Option<&Self> {
        self.strip_prefix(prefix)
    }

    fn strip_suffix(&self, suffix: &str) -> Option<&Self> {
        self.strip_suffix(suffix)
    }

    fn split_at_exclusive(&self, needle: &str) -> Option<(&Self, &Self)> {
        self.find(needle)
            .map(|idx| (&self[..idx], &self[idx + needle.len()..]))
    }

    fn find(&self, needle: &str) -> Option<usize> {
        self.find(needle)
    }

    fn split_at(&self, mid: usize) -> (&Self, &Self) {
        self.split_at(mid)
    }

    fn as_str(&self) -> Option<&str> {
        Some(self)
    }

    fn as_bytes(&self) -> &[u8] {
        self.as_bytes()
    }

    fn lines(&self) -> LineIter<Self> {
        LineIter::new(self)
    }
}

impl Text for [u8] {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.starts_with(prefix.as_bytes())
    }

    fn ends_with(&self, suffix: &str) -> bool {
        self.ends_with(suffix.as_bytes())
    }

    fn strip_prefix(&self, prefix: &str) -> Option<&Self> {
        self.strip_prefix(prefix.as_bytes())
    }

    fn strip_suffix(&self, suffix: &str) -> Option<&Self> {
        self.strip_suffix(suffix.as_bytes())
    }

    fn split_at_exclusive(&self, needle: &str) -> Option<(&Self, &Self)> {
        find_bytes(self, needle.as_bytes()).map(|idx| (&self[..idx], &self[idx + needle.len()..]))
    }

    fn find(&self, needle: &str) -> Option<usize> {
        find_bytes(self, needle.as_bytes())
    }

    fn split_at(&self, mid: usize) -> (&Self, &Self) {
        self.split_at(mid)
    }

    fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(self).ok()
    }

    fn as_bytes(&self) -> &[u8] {
        self
    }

    fn lines(&self) -> LineIter<Self> {
        LineIter::new(self)
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    match needle.len() {
        0 => Some(0),
        1 => find_byte(haystack, needle[0]),
        len if len > haystack.len() => None,
        needle_len => {
            let mut offset = 0;
            let mut haystack = haystack;

            while let Some(position) = find_byte(haystack, needle[0]) {
                offset += position;

                if let Some(haystack) = haystack.get(position..position + needle_len) {
                    if haystack == needle {
                        return Some(offset);
                    }
                } else {
                    return None;
                }

                haystack = &haystack[position + 1..];
                offset += 1;
            }

            None
        }
    }
}

// XXX Maybe use `memchr`?
fn find_byte(haystack: &[u8], byte: u8) -> Option<usize> {
    haystack.iter().position(|&b| b == byte)
}
