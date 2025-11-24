//! Text diffing utilities.
use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Duration;

mod abstraction;
#[cfg(feature = "inline")]
mod inline;
mod utils;

pub use self::abstraction::{DiffableStr, DiffableStrRef};
#[cfg(feature = "inline")]
pub use self::inline::InlineChange;

use self::utils::{upper_seq_ratio, QuickSeqRatio};
use crate::algorithms::IdentifyDistinct;
use crate::deadline_support::{duration_to_deadline, Instant};
use crate::iter::{AllChangesIter, ChangesIter};
use crate::udiff::UnifiedDiff;
use crate::{capture_diff_deadline, get_diff_ratio, group_diff_ops, Algorithm, DiffOp};

#[derive(Debug, Clone, Copy)]
enum Deadline {
    Absolute(Instant),
    Relative(Duration),
}

impl Deadline {
    fn into_instant(self) -> Option<Instant> {
        match self {
            Deadline::Absolute(instant) => Some(instant),
            Deadline::Relative(duration) => duration_to_deadline(duration),
        }
    }
}

/// A builder type config for more complex uses of [`TextDiff`].
///
/// Requires the `text` feature.
#[derive(Clone, Debug, Default)]
pub struct TextDiffConfig {
    algorithm: Algorithm,
    newline_terminated: Option<bool>,
    deadline: Option<Deadline>,
}

impl TextDiffConfig {
    /// Changes the algorithm.
    ///
    /// The default algorithm is [`Algorithm::Myers`].
    pub fn algorithm(&mut self, alg: Algorithm) -> &mut Self {
        self.algorithm = alg;
        self
    }

    /// Sets a deadline for the diff operation.
    ///
    /// By default a diff will take as long as it takes.  For certain diff
    /// algorithms like Myer's and Patience a maximum running time can be
    /// defined after which the algorithm gives up and approximates.
    pub fn deadline(&mut self, deadline: Instant) -> &mut Self {
        self.deadline = Some(Deadline::Absolute(deadline));
        self
    }

    /// Sets a timeout for thediff operation.
    ///
    /// This is like [`deadline`](Self::deadline) but accepts a duration.
    pub fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.deadline = Some(Deadline::Relative(timeout));
        self
    }

    /// Changes the newline termination flag.
    ///
    /// The default is automatic based on input.  This flag controls the
    /// behavior of [`TextDiff::iter_changes`] and unified diff generation
    /// with regards to newlines.  When the flag is set to `false` (which
    /// is the default) then newlines are added.  Otherwise the newlines
    /// from the source sequences are reused.
    pub fn newline_terminated(&mut self, yes: bool) -> &mut Self {
        self.newline_terminated = Some(yes);
        self
    }

    /// Creates a diff of lines.
    ///
    /// This splits the text `old` and `new` into lines preserving newlines
    /// in the input.  Line diffs are very common and because of that enjoy
    /// special handling in similar.  When a line diff is created with this
    /// method the `newline_terminated` flag is flipped to `true` and will
    /// influence the behavior of unified diff generation.
    ///
    /// ```rust
    /// use similar::{TextDiff, ChangeTag};
    ///
    /// let diff = TextDiff::configure().diff_lines("a\nb\nc", "a\nb\nC");
    /// let changes: Vec<_> = diff
    ///     .iter_all_changes()
    ///     .map(|x| (x.tag(), x.value()))
    ///     .collect();
    ///
    /// assert_eq!(changes, vec![
    ///    (ChangeTag::Equal, "a\n"),
    ///    (ChangeTag::Equal, "b\n"),
    ///    (ChangeTag::Delete, "c"),
    ///    (ChangeTag::Insert, "C"),
    /// ]);
    /// ```
    pub fn diff_lines<'old, 'new, 'bufs, T: DiffableStrRef + ?Sized>(
        &self,
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        self.diff(
            Cow::Owned(old.as_diffable_str().tokenize_lines()),
            Cow::Owned(new.as_diffable_str().tokenize_lines()),
            true,
        )
    }

    /// Creates a diff of words.
    ///
    /// This splits the text into words and whitespace.
    ///
    /// Note on word diffs: because the text differ will tokenize the strings
    /// into small segments it can be inconvenient to work with the results
    /// depending on the use case.  You might also want to combine word level
    /// diffs with the [`TextDiffRemapper`](crate::utils::TextDiffRemapper)
    /// which lets you remap the diffs back to the original input strings.
    ///
    /// ```rust
    /// use similar::{TextDiff, ChangeTag};
    ///
    /// let diff = TextDiff::configure().diff_words("foo bar baz", "foo BAR baz");
    /// let changes: Vec<_> = diff
    ///     .iter_all_changes()
    ///     .map(|x| (x.tag(), x.value()))
    ///     .collect();
    ///
    /// assert_eq!(changes, vec![
    ///    (ChangeTag::Equal, "foo"),
    ///    (ChangeTag::Equal, " "),
    ///    (ChangeTag::Delete, "bar"),
    ///    (ChangeTag::Insert, "BAR"),
    ///    (ChangeTag::Equal, " "),
    ///    (ChangeTag::Equal, "baz"),
    /// ]);
    /// ```
    pub fn diff_words<'old, 'new, 'bufs, T: DiffableStrRef + ?Sized>(
        &self,
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        self.diff(
            Cow::Owned(old.as_diffable_str().tokenize_words()),
            Cow::Owned(new.as_diffable_str().tokenize_words()),
            false,
        )
    }

    /// Creates a diff of characters.
    ///
    /// Note on character diffs: because the text differ will tokenize the strings
    /// into small segments it can be inconvenient to work with the results
    /// depending on the use case.  You might also want to combine word level
    /// diffs with the [`TextDiffRemapper`](crate::utils::TextDiffRemapper)
    /// which lets you remap the diffs back to the original input strings.
    ///
    /// ```rust
    /// use similar::{TextDiff, ChangeTag};
    ///
    /// let diff = TextDiff::configure().diff_chars("abcdef", "abcDDf");
    /// let changes: Vec<_> = diff
    ///     .iter_all_changes()
    ///     .map(|x| (x.tag(), x.value()))
    ///     .collect();
    ///
    /// assert_eq!(changes, vec![
    ///    (ChangeTag::Equal, "a"),
    ///    (ChangeTag::Equal, "b"),
    ///    (ChangeTag::Equal, "c"),
    ///    (ChangeTag::Delete, "d"),
    ///    (ChangeTag::Delete, "e"),
    ///    (ChangeTag::Insert, "D"),
    ///    (ChangeTag::Insert, "D"),
    ///    (ChangeTag::Equal, "f"),
    /// ]);
    /// ```
    pub fn diff_chars<'old, 'new, 'bufs, T: DiffableStrRef + ?Sized>(
        &self,
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        self.diff(
            Cow::Owned(old.as_diffable_str().tokenize_chars()),
            Cow::Owned(new.as_diffable_str().tokenize_chars()),
            false,
        )
    }

    /// Creates a diff of unicode words.
    ///
    /// This splits the text into words according to unicode rules.  This is
    /// generally recommended over [`TextDiffConfig::diff_words`] but
    /// requires a dependency.
    ///
    /// This requires the `unicode` feature.
    ///
    /// Note on word diffs: because the text differ will tokenize the strings
    /// into small segments it can be inconvenient to work with the results
    /// depending on the use case.  You might also want to combine word level
    /// diffs with the [`TextDiffRemapper`](crate::utils::TextDiffRemapper)
    /// which lets you remap the diffs back to the original input strings.
    ///
    /// ```rust
    /// use similar::{TextDiff, ChangeTag};
    ///
    /// let diff = TextDiff::configure().diff_unicode_words("ah(be)ce", "ah(ah)ce");
    /// let changes: Vec<_> = diff
    ///     .iter_all_changes()
    ///     .map(|x| (x.tag(), x.value()))
    ///     .collect();
    ///
    /// assert_eq!(changes, vec![
    ///    (ChangeTag::Equal, "ah"),
    ///    (ChangeTag::Equal, "("),
    ///    (ChangeTag::Delete, "be"),
    ///    (ChangeTag::Insert, "ah"),
    ///    (ChangeTag::Equal, ")"),
    ///    (ChangeTag::Equal, "ce"),
    /// ]);
    /// ```
    #[cfg(feature = "unicode")]
    pub fn diff_unicode_words<'old, 'new, 'bufs, T: DiffableStrRef + ?Sized>(
        &self,
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        self.diff(
            Cow::Owned(old.as_diffable_str().tokenize_unicode_words()),
            Cow::Owned(new.as_diffable_str().tokenize_unicode_words()),
            false,
        )
    }

    /// Creates a diff of graphemes.
    ///
    /// This requires the `unicode` feature.
    ///
    /// Note on grapheme diffs: because the text differ will tokenize the strings
    /// into small segments it can be inconvenient to work with the results
    /// depending on the use case.  You might also want to combine word level
    /// diffs with the [`TextDiffRemapper`](crate::utils::TextDiffRemapper)
    /// which lets you remap the diffs back to the original input strings.
    ///
    /// ```rust
    /// use similar::{TextDiff, ChangeTag};
    ///
    /// let diff = TextDiff::configure().diff_graphemes("💩🇦🇹🦠", "💩🇦🇱❄️");
    /// let changes: Vec<_> = diff
    ///     .iter_all_changes()
    ///     .map(|x| (x.tag(), x.value()))
    ///     .collect();
    ///
    /// assert_eq!(changes, vec![
    ///    (ChangeTag::Equal, "💩"),
    ///    (ChangeTag::Delete, "🇦🇹"),
    ///    (ChangeTag::Delete, "🦠"),
    ///    (ChangeTag::Insert, "🇦🇱"),
    ///    (ChangeTag::Insert, "❄️"),
    /// ]);
    /// ```
    #[cfg(feature = "unicode")]
    pub fn diff_graphemes<'old, 'new, 'bufs, T: DiffableStrRef + ?Sized>(
        &self,
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        self.diff(
            Cow::Owned(old.as_diffable_str().tokenize_graphemes()),
            Cow::Owned(new.as_diffable_str().tokenize_graphemes()),
            false,
        )
    }

    /// Creates a diff of arbitrary slices.
    ///
    /// ```rust
    /// use similar::{TextDiff, ChangeTag};
    ///
    /// let old = &["foo", "bar", "baz"];
    /// let new = &["foo", "BAR", "baz"];
    /// let diff = TextDiff::configure().diff_slices(old, new);
    /// let changes: Vec<_> = diff
    ///     .iter_all_changes()
    ///     .map(|x| (x.tag(), x.value()))
    ///     .collect();
    ///
    /// assert_eq!(changes, vec![
    ///    (ChangeTag::Equal, "foo"),
    ///    (ChangeTag::Delete, "bar"),
    ///    (ChangeTag::Insert, "BAR"),
    ///    (ChangeTag::Equal, "baz"),
    /// ]);
    /// ```
    pub fn diff_slices<'old, 'new, 'bufs, T: DiffableStr + ?Sized>(
        &self,
        old: &'bufs [&'old T],
        new: &'bufs [&'new T],
    ) -> TextDiff<'old, 'new, 'bufs, T> {
        self.diff(Cow::Borrowed(old), Cow::Borrowed(new), false)
    }

    fn diff<'old, 'new, 'bufs, T: DiffableStr + ?Sized>(
        &self,
        old: Cow<'bufs, [&'old T]>,
        new: Cow<'bufs, [&'new T]>,
        newline_terminated: bool,
    ) -> TextDiff<'old, 'new, 'bufs, T> {
        let deadline = self.deadline.and_then(|x| x.into_instant());
        let ops = if old.len() > 100 || new.len() > 100 {
            let ih = IdentifyDistinct::<u32>::new(&old[..], 0..old.len(), &new[..], 0..new.len());
            capture_diff_deadline(
                self.algorithm,
                ih.old_lookup(),
                ih.old_range(),
                ih.new_lookup(),
                ih.new_range(),
                deadline,
            )
        } else {
            capture_diff_deadline(
                self.algorithm,
                &old[..],
                0..old.len(),
                &new[..],
                0..new.len(),
                deadline,
            )
        };
        TextDiff {
            old,
            new,
            ops,
            newline_terminated: self.newline_terminated.unwrap_or(newline_terminated),
            algorithm: self.algorithm,
        }
    }
}

/// Captures diff op codes for textual diffs.
///
/// The exact diff behavior is depending on the underlying [`DiffableStr`].
/// For instance diffs on bytes and strings are slightly different.  You can
/// create a text diff from constructors such as [`TextDiff::from_lines`] or
/// the [`TextDiffConfig`] created by [`TextDiff::configure`].
///
/// Requires the `text` feature.
pub struct TextDiff<'old, 'new, 'bufs, T: DiffableStr + ?Sized> {
    old: Cow<'bufs, [&'old T]>,
    new: Cow<'bufs, [&'new T]>,
    ops: Vec<DiffOp>,
    newline_terminated: bool,
    algorithm: Algorithm,
}

impl<'old, 'new, 'bufs> TextDiff<'old, 'new, 'bufs, str> {
    /// Configures a text differ before diffing.
    pub fn configure() -> TextDiffConfig {
        TextDiffConfig::default()
    }

    /// Creates a diff of lines.
    ///
    /// For more information see [`TextDiffConfig::diff_lines`].
    pub fn from_lines<T: DiffableStrRef + ?Sized>(
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        TextDiff::configure().diff_lines(old, new)
    }

    /// Creates a diff of words.
    ///
    /// For more information see [`TextDiffConfig::diff_words`].
    pub fn from_words<T: DiffableStrRef + ?Sized>(
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        TextDiff::configure().diff_words(old, new)
    }

    /// Creates a diff of chars.
    ///
    /// For more information see [`TextDiffConfig::diff_chars`].
    pub fn from_chars<T: DiffableStrRef + ?Sized>(
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        TextDiff::configure().diff_chars(old, new)
    }

    /// Creates a diff of unicode words.
    ///
    /// For more information see [`TextDiffConfig::diff_unicode_words`].
    ///
    /// This requires the `unicode` feature.
    #[cfg(feature = "unicode")]
    pub fn from_unicode_words<T: DiffableStrRef + ?Sized>(
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        TextDiff::configure().diff_unicode_words(old, new)
    }

    /// Creates a diff of graphemes.
    ///
    /// For more information see [`TextDiffConfig::diff_graphemes`].
    ///
    /// This requires the `unicode` feature.
    #[cfg(feature = "unicode")]
    pub fn from_graphemes<T: DiffableStrRef + ?Sized>(
        old: &'old T,
        new: &'new T,
    ) -> TextDiff<'old, 'new, 'bufs, T::Output> {
        TextDiff::configure().diff_graphemes(old, new)
    }
}

impl<'old, 'new, 'bufs, T: DiffableStr + ?Sized + 'old + 'new> TextDiff<'old, 'new, 'bufs, T> {
    /// Creates a diff of arbitrary slices.
    ///
    /// For more information see [`TextDiffConfig::diff_slices`].
    pub fn from_slices(
        old: &'bufs [&'old T],
        new: &'bufs [&'new T],
    ) -> TextDiff<'old, 'new, 'bufs, T> {
        TextDiff::configure().diff_slices(old, new)
    }

    /// The name of the algorithm that created the diff.
    pub fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    /// Returns `true` if items in the slice are newline terminated.
    ///
    /// This flag is used by the unified diff writer to determine if extra
    /// newlines have to be added.
    pub fn newline_terminated(&self) -> bool {
        self.newline_terminated
    }

    /// Returns all old slices.
    pub fn old_slices(&self) -> &[&'old T] {
        &self.old
    }

    /// Returns all new slices.
    pub fn new_slices(&self) -> &[&'new T] {
        &self.new
    }

    /// Return a measure of the sequences' similarity in the range `0..=1`.
    ///
    /// A ratio of `1.0` means the two sequences are a complete match, a
    /// ratio of `0.0` would indicate completely distinct sequences.
    ///
    /// ```rust
    /// # use similar::TextDiff;
    /// let diff = TextDiff::from_chars("abcd", "bcde");
    /// assert_eq!(diff.ratio(), 0.75);
    /// ```
    pub fn ratio(&self) -> f32 {
        get_diff_ratio(self.ops(), self.old.len(), self.new.len())
    }

    /// Iterates over the changes the op expands to.
    ///
    /// This method is a convenient way to automatically resolve the different
    /// ways in which a change could be encoded (insert/delete vs replace), look
    /// up the value from the appropriate slice and also handle correct index
    /// handling.
    pub fn iter_changes<'x, 'slf>(
        &'slf self,
        op: &DiffOp,
    ) -> ChangesIter<'slf, [&'x T], [&'x T], &'x T>
    where
        'x: 'slf,
        'old: 'x,
        'new: 'x,
    {
        op.iter_changes(self.old_slices(), self.new_slices())
    }

    /// Returns the captured diff ops.
    pub fn ops(&self) -> &[DiffOp] {
        &self.ops
    }

    /// Isolate change clusters by eliminating ranges with no changes.
    ///
    /// This is equivalent to calling [`group_diff_ops`] on [`TextDiff::ops`].
    pub fn grouped_ops(&self, n: usize) -> Vec<Vec<DiffOp>> {
        group_diff_ops(self.ops().to_vec(), n)
    }

    /// Flattens out the diff into all changes.
    ///
    /// This is a shortcut for combining [`TextDiff::ops`] with
    /// [`TextDiff::iter_changes`].
    pub fn iter_all_changes<'x, 'slf>(&'slf self) -> AllChangesIter<'slf, 'x, T>
    where
        'x: 'slf + 'old + 'new,
        'old: 'x,
        'new: 'x,
    {
        AllChangesIter::new(&self.old[..], &self.new[..], self.ops())
    }

    /// Utility to return a unified diff formatter.
    pub fn unified_diff<'diff>(&'diff self) -> UnifiedDiff<'diff, 'old, 'new, 'bufs, T> {
        UnifiedDiff::from_text_diff(self)
    }

    /// Iterates over the changes the op expands to with inline emphasis.
    ///
    /// This is very similar to [`TextDiff::iter_changes`] but it performs a second
    /// level diff on adjacent line replacements.  The exact behavior of
    /// this function with regards to how it detects those inline changes
    /// is currently not defined and will likely change over time.
    ///
    /// This method has a hardcoded 500ms deadline which is often not ideal.  For
    /// fine tuning use [`iter_inline_changes_deadline`](Self::iter_inline_changes_deadline).
    ///
    /// As of similar 1.2.0 the behavior of this function changes depending on
    /// if the `unicode` feature is enabled or not.  It will prefer unicode word
    /// splitting over word splitting depending on the feature flag.
    ///
    /// Requires the `inline` feature.
    #[cfg(feature = "inline")]
    pub fn iter_inline_changes<'slf>(
        &'slf self,
        op: &DiffOp,
    ) -> impl Iterator<Item = InlineChange<'slf, T>> + 'slf
    where
        'slf: 'old + 'new,
    {
        use crate::deadline_support::duration_to_deadline;

        inline::iter_inline_changes(self, op, duration_to_deadline(Duration::from_millis(500)))
    }

    /// Iterates over the changes the op expands to with inline emphasis with a deadline.
    ///
    /// Like [`iter_inline_changes`](Self::iter_inline_changes) but with an explicit deadline.
    #[cfg(feature = "inline")]
    pub fn iter_inline_changes_deadline<'slf>(
        &'slf self,
        op: &DiffOp,
        deadline: Option<Instant>,
    ) -> impl Iterator<Item = InlineChange<'slf, T>> + 'slf
    where
        'slf: 'old + 'new,
    {
        inline::iter_inline_changes(self, op, deadline)
    }
}

/// Use the text differ to find `n` close matches.
///
/// `cutoff` defines the threshold which needs to be reached for a word
/// to be considered similar.  See [`TextDiff::ratio`] for more information.
///
/// ```
/// # use similar::get_close_matches;
/// let matches = get_close_matches(
///     "appel",
///     &["ape", "apple", "peach", "puppy"][..],
///     3,
///     0.6
/// );
/// assert_eq!(matches, vec!["apple", "ape"]);
/// ```
///
/// Requires the `text` feature.
pub fn get_close_matches<'a, T: DiffableStr + ?Sized>(
    word: &T,
    possibilities: &[&'a T],
    n: usize,
    cutoff: f32,
) -> Vec<&'a T> {
    let mut matches = BinaryHeap::new();
    let seq1 = word.tokenize_chars();
    let quick_ratio = QuickSeqRatio::new(&seq1);

    for &possibility in possibilities {
        let seq2 = possibility.tokenize_chars();

        if upper_seq_ratio(&seq1, &seq2) < cutoff || quick_ratio.calc(&seq2) < cutoff {
            continue;
        }

        let diff = TextDiff::from_slices(&seq1, &seq2);
        let ratio = diff.ratio();
        if ratio >= cutoff {
            // we're putting the word itself in reverse in so that matches with
            // the same ratio are ordered lexicographically.
            matches.push(((ratio * u32::MAX as f32) as u32, Reverse(possibility)));
        }
    }

    let mut rv = vec![];
    for _ in 0..n {
        if let Some((_, elt)) = matches.pop() {
            rv.push(elt.0);
        } else {
            break;
        }
    }

    rv
}

#[test]
fn test_captured_ops() {
    let diff = TextDiff::from_lines(
        "Hello World\nsome stuff here\nsome more stuff here\n",
        "Hello World\nsome amazing stuff here\nsome more stuff here\n",
    );
    insta::assert_debug_snapshot!(&diff.ops());
}

#[test]
fn test_captured_word_ops() {
    let diff = TextDiff::from_words(
        "Hello World\nsome stuff here\nsome more stuff here\n",
        "Hello World\nsome amazing stuff here\nsome more stuff here\n",
    );
    let changes = diff
        .ops()
        .iter()
        .flat_map(|op| diff.iter_changes(op))
        .collect::<Vec<_>>();
    insta::assert_debug_snapshot!(&changes);
}

#[test]
fn test_unified_diff() {
    let diff = TextDiff::from_lines(
        "Hello World\nsome stuff here\nsome more stuff here\n",
        "Hello World\nsome amazing stuff here\nsome more stuff here\n",
    );
    assert!(diff.newline_terminated());
    insta::assert_snapshot!(&diff
        .unified_diff()
        .context_radius(3)
        .header("old", "new")
        .to_string());
}

#[test]
fn test_line_ops() {
    let a = "Hello World\nsome stuff here\nsome more stuff here\n";
    let b = "Hello World\nsome amazing stuff here\nsome more stuff here\n";
    let diff = TextDiff::from_lines(a, b);
    assert!(diff.newline_terminated());
    let changes = diff
        .ops()
        .iter()
        .flat_map(|op| diff.iter_changes(op))
        .collect::<Vec<_>>();
    insta::assert_debug_snapshot!(&changes);

    #[cfg(feature = "bytes")]
    {
        let byte_diff = TextDiff::from_lines(a.as_bytes(), b.as_bytes());
        let byte_changes = byte_diff
            .ops()
            .iter()
            .flat_map(|op| byte_diff.iter_changes(op))
            .collect::<Vec<_>>();
        for (change, byte_change) in changes.iter().zip(byte_changes.iter()) {
            assert_eq!(change.to_string_lossy(), byte_change.to_string_lossy());
        }
    }
}

#[test]
fn test_virtual_newlines() {
    let diff = TextDiff::from_lines("a\nb", "a\nc\n");
    assert!(diff.newline_terminated());
    let changes = diff
        .ops()
        .iter()
        .flat_map(|op| diff.iter_changes(op))
        .collect::<Vec<_>>();
    insta::assert_debug_snapshot!(&changes);
}

#[test]
fn test_char_diff() {
    let diff = TextDiff::from_chars("Hello World", "Hallo Welt");
    insta::assert_debug_snapshot!(diff.ops());

    #[cfg(feature = "bytes")]
    {
        let byte_diff = TextDiff::from_chars("Hello World".as_bytes(), "Hallo Welt".as_bytes());
        assert_eq!(diff.ops(), byte_diff.ops());
    }
}

#[test]
fn test_ratio() {
    let diff = TextDiff::from_chars("abcd", "bcde");
    assert_eq!(diff.ratio(), 0.75);
    let diff = TextDiff::from_chars("", "");
    assert_eq!(diff.ratio(), 1.0);
}

#[test]
fn test_get_close_matches() {
    let matches = get_close_matches("appel", &["ape", "apple", "peach", "puppy"][..], 3, 0.6);
    assert_eq!(matches, vec!["apple", "ape"]);
    let matches = get_close_matches(
        "hulo",
        &[
            "hi", "hulu", "hali", "hoho", "amaz", "zulo", "blah", "hopp", "uulo", "aulo",
        ][..],
        5,
        0.7,
    );
    assert_eq!(matches, vec!["aulo", "hulu", "uulo", "zulo"]);
}

#[test]
fn test_lifetimes_on_iter() {
    use crate::Change;

    fn diff_lines<'x, T>(old: &'x T, new: &'x T) -> Vec<Change<&'x T::Output>>
    where
        T: DiffableStrRef + ?Sized,
    {
        TextDiff::from_lines(old, new).iter_all_changes().collect()
    }

    let a = "1\n2\n3\n".to_string();
    let b = "1\n99\n3\n".to_string();
    let changes = diff_lines(&a, &b);
    insta::assert_debug_snapshot!(&changes);
}

#[test]
#[cfg(feature = "serde")]
fn test_serde() {
    let diff = TextDiff::from_lines(
        "Hello World\nsome stuff here\nsome more stuff here\n\nAha stuff here\nand more stuff",
        "Stuff\nHello World\nsome amazing stuff here\nsome more stuff here\n",
    );
    let changes = diff
        .ops()
        .iter()
        .flat_map(|op| diff.iter_changes(op))
        .collect::<Vec<_>>();
    let json = serde_json::to_string_pretty(&changes).unwrap();
    insta::assert_snapshot!(&json);
}

#[test]
#[cfg(feature = "serde")]
fn test_serde_ops() {
    let diff = TextDiff::from_lines(
        "Hello World\nsome stuff here\nsome more stuff here\n\nAha stuff here\nand more stuff",
        "Stuff\nHello World\nsome amazing stuff here\nsome more stuff here\n",
    );
    let changes = diff.ops();
    let json = serde_json::to_string_pretty(&changes).unwrap();
    insta::assert_snapshot!(&json);
}

#[test]
fn test_regression_issue_37() {
    let config = TextDiffConfig::default();
    let diff = config.diff_lines("\u{18}\n\n", "\n\n\r");
    let mut output = diff.unified_diff();
    assert_eq!(
        output.context_radius(0).to_string(),
        "@@ -1 +1,0 @@\n-\u{18}\n@@ -2,0 +2,2 @@\n+\n+\r"
    );
}
