mod format;
mod parse;

pub use format::PatchFormatter;
pub use parse::ParsePatchError;

use std::{borrow::Cow, fmt, ops};

const NO_NEWLINE_AT_EOF: &str = "\\ No newline at end of file";

/// Representation of all the differences between two files
#[derive(PartialEq, Eq)]
pub struct Patch<'a, T: ToOwned + ?Sized> {
    // TODO GNU patch is able to parse patches without filename headers.
    // This should be changed to an `Option` type to reflect this instead of setting this to ""
    // when they're missing
    original: Option<Filename<'a, T>>,
    modified: Option<Filename<'a, T>>,
    hunks: Vec<Hunk<'a, T>>,
}

impl<'a, T: ToOwned + ?Sized> Patch<'a, T> {
    pub(crate) fn new<O, M>(
        original: Option<O>,
        modified: Option<M>,
        hunks: Vec<Hunk<'a, T>>,
    ) -> Self
    where
        O: Into<Cow<'a, T>>,
        M: Into<Cow<'a, T>>,
    {
        let original = original.map(|o| Filename(o.into()));
        let modified = modified.map(|m| Filename(m.into()));
        Self {
            original,
            modified,
            hunks,
        }
    }

    /// Return the name of the old file
    pub fn original(&self) -> Option<&T> {
        self.original.as_ref().map(AsRef::as_ref)
    }

    /// Return the name of the new file
    pub fn modified(&self) -> Option<&T> {
        self.modified.as_ref().map(AsRef::as_ref)
    }

    /// Returns the hunks in the patch
    pub fn hunks(&self) -> &[Hunk<'_, T>] {
        &self.hunks
    }

    pub fn reverse(&self) -> Patch<'_, T> {
        let hunks = self.hunks.iter().map(Hunk::reverse).collect();
        Patch {
            original: self.modified.clone(),
            modified: self.original.clone(),
            hunks,
        }
    }
}

impl<T: AsRef<[u8]> + ToOwned + ?Sized> Patch<'_, T> {
    /// Convert a `Patch` into bytes
    ///
    /// This is the equivalent of the `to_string` function but for
    /// potentially non-utf8 patches.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        PatchFormatter::new()
            .write_patch_into(self, &mut bytes)
            .unwrap();
        bytes
    }
}

impl<'a> Patch<'a, str> {
    /// Parse a `Patch` from a string
    ///
    /// ```
    /// use diffy::Patch;
    ///
    /// let s = "\
    /// --- a/ideals
    /// +++ b/ideals
    /// @@ -1,4 +1,6 @@
    ///  First:
    ///      Life before death,
    ///      strength before weakness,
    ///      journey before destination.
    /// +Second:
    /// +    I will protect those who cannot protect themselves.
    /// ";
    ///
    /// let patch = Patch::from_str(s).unwrap();
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &'a str) -> Result<Patch<'a, str>, ParsePatchError> {
        parse::parse(s)
    }
}

impl<'a> Patch<'a, [u8]> {
    /// Parse a `Patch` from bytes
    pub fn from_bytes(s: &'a [u8]) -> Result<Patch<'a, [u8]>, ParsePatchError> {
        parse::parse_bytes(s)
    }
}

impl<T: ToOwned + ?Sized> Clone for Patch<'_, T> {
    fn clone(&self) -> Self {
        Self {
            original: self.original.clone(),
            modified: self.modified.clone(),
            hunks: self.hunks.clone(),
        }
    }
}

impl fmt::Display for Patch<'_, str> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", PatchFormatter::new().fmt_patch(self))
    }
}

impl<T: ?Sized, O> fmt::Debug for Patch<'_, T>
where
    T: ToOwned<Owned = O> + fmt::Debug,
    O: std::borrow::Borrow<T> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Patch")
            .field("original", &self.original)
            .field("modified", &self.modified)
            .field("hunks", &self.hunks)
            .finish()
    }
}

#[derive(PartialEq, Eq)]
struct Filename<'a, T: ToOwned + ?Sized>(Cow<'a, T>);

const ESCAPED_CHARS: &[char] = &['\n', '\t', '\0', '\r', '\"', '\\'];
#[allow(clippy::byte_char_slices)]
const ESCAPED_CHARS_BYTES: &[u8] = &[b'\n', b'\t', b'\0', b'\r', b'\"', b'\\'];

impl Filename<'_, str> {
    fn needs_to_be_escaped(&self) -> bool {
        self.0.contains(ESCAPED_CHARS)
    }
}

impl<T: ToOwned + AsRef<[u8]> + ?Sized> Filename<'_, T> {
    fn needs_to_be_escaped_bytes(&self) -> bool {
        self.0
            .as_ref()
            .as_ref()
            .iter()
            .any(|b| ESCAPED_CHARS_BYTES.contains(b))
    }

    fn write_into<W: std::io::Write>(&self, mut w: W) -> std::io::Result<()> {
        if self.needs_to_be_escaped_bytes() {
            w.write_all(b"\"")?;
            for b in self.0.as_ref().as_ref() {
                if ESCAPED_CHARS_BYTES.contains(b) {
                    w.write_all(b"\\")?;
                }
                w.write_all(&[*b])?;
            }
            w.write_all(b"\"")?;
        } else {
            w.write_all(self.0.as_ref().as_ref())?;
        }

        Ok(())
    }
}

impl<T: ToOwned + ?Sized> AsRef<T> for Filename<'_, T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: ToOwned + ?Sized> ops::Deref for Filename<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ToOwned + ?Sized> Clone for Filename<'_, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl fmt::Display for Filename<'_, str> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;
        if self.needs_to_be_escaped() {
            f.write_char('\"')?;
            for c in self.0.chars() {
                if ESCAPED_CHARS.contains(&c) {
                    f.write_char('\\')?;
                }
                f.write_char(c)?;
            }
            f.write_char('\"')?;
        } else {
            f.write_str(&self.0)?;
        }

        Ok(())
    }
}

impl<T: ?Sized, O> fmt::Debug for Filename<'_, T>
where
    T: ToOwned<Owned = O> + fmt::Debug,
    O: std::borrow::Borrow<T> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Filename").field(&self.0).finish()
    }
}

/// Represents a group of differing lines between two files
#[derive(Debug, PartialEq, Eq)]
pub struct Hunk<'a, T: ?Sized> {
    old_range: HunkRange,
    new_range: HunkRange,

    function_context: Option<&'a T>,

    lines: Vec<Line<'a, T>>,
}

fn hunk_lines_count<T: ?Sized>(lines: &[Line<'_, T>]) -> (usize, usize) {
    lines.iter().fold((0, 0), |count, line| match line {
        Line::Context(_) => (count.0 + 1, count.1 + 1),
        Line::Delete(_) => (count.0 + 1, count.1),
        Line::Insert(_) => (count.0, count.1 + 1),
    })
}

impl<'a, T: ?Sized> Hunk<'a, T> {
    pub(crate) fn new(
        old_range: HunkRange,
        new_range: HunkRange,
        function_context: Option<&'a T>,
        lines: Vec<Line<'a, T>>,
    ) -> Self {
        let (old_count, new_count) = hunk_lines_count(&lines);

        assert_eq!(old_range.len, old_count);
        assert_eq!(new_range.len, new_count);

        Self {
            old_range,
            new_range,
            function_context,
            lines,
        }
    }

    /// Returns the corresponding range for the old file in the hunk
    pub fn old_range(&self) -> HunkRange {
        self.old_range
    }

    /// Returns the corresponding range for the new file in the hunk
    pub fn new_range(&self) -> HunkRange {
        self.new_range
    }

    /// Returns the function context (if any) for the hunk
    pub fn function_context(&self) -> Option<&T> {
        self.function_context
    }

    /// Returns the lines in the hunk
    pub fn lines(&self) -> &[Line<'a, T>] {
        &self.lines
    }

    /// Creates a reverse patch for the hunk.  This is equivalent to what
    /// XDL_PATCH_REVERSE would apply in libxdiff.
    pub fn reverse(&self) -> Self {
        let lines = self.lines.iter().map(Line::reverse).collect();
        Self {
            old_range: self.new_range,
            new_range: self.old_range,
            function_context: self.function_context,
            lines,
        }
    }
}

impl<T: ?Sized> Clone for Hunk<'_, T> {
    fn clone(&self) -> Self {
        Self {
            old_range: self.old_range,
            new_range: self.new_range,
            function_context: self.function_context,
            lines: self.lines.clone(),
        }
    }
}

/// The range of lines in a file for a particular `Hunk`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct HunkRange {
    /// The starting line number of a hunk
    start: usize,
    /// The hunk size (number of lines)
    len: usize,
}

impl HunkRange {
    pub(crate) fn new(start: usize, len: usize) -> Self {
        Self { start, len }
    }

    /// Returns the range as a `ops::Range`
    pub fn range(&self) -> ops::Range<usize> {
        self.start..self.end()
    }

    /// Returns the starting line number of the range (inclusive)
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the ending line number of the range (exclusive)
    pub fn end(&self) -> usize {
        self.start + self.len
    }

    /// Returns the number of lines in the range
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the range is empty (has a length of `0`)
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Display for HunkRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.start)?;
        if self.len != 1 {
            write!(f, ",{}", self.len)?;
        }
        Ok(())
    }
}

/// A line in either the old file, new file, or both.
///
/// A `Line` contains the terminating newline character `\n` unless it is the final
/// line in the file and the file does not end with a newline character.
#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a, T: ?Sized> {
    /// A line providing context in the diff which is present in both the old and new file
    Context(&'a T),
    /// A line deleted from the old file
    Delete(&'a T),
    /// A line inserted to the new file
    Insert(&'a T),
}

impl<T: ?Sized> Copy for Line<'_, T> {}

impl<T: ?Sized> Clone for Line<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Line<'_, T> {
    pub fn reverse(&self) -> Self {
        match self {
            Line::Context(s) => Line::Context(s),
            Line::Delete(s) => Line::Insert(s),
            Line::Insert(s) => Line::Delete(s),
        }
    }
}
