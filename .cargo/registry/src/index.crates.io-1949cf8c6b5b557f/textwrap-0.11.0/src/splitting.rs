//! Word splitting functionality.
//!
//! To wrap text into lines, long words sometimes need to be split
//! across lines. The [`WordSplitter`] trait defines this
//! functionality. [`HyphenSplitter`] is the default implementation of
//! this treat: it will simply split words on existing hyphens.

#[cfg(feature = "hyphenation")]
use hyphenation::{Hyphenator, Standard};

/// An interface for splitting words.
///
/// When the [`wrap_iter`] method will try to fit text into a line, it
/// will eventually find a word that it too large the current text
/// width. It will then call the currently configured `WordSplitter` to
/// have it attempt to split the word into smaller parts. This trait
/// describes that functionality via the [`split`] method.
///
/// If the `textwrap` crate has been compiled with the `hyphenation`
/// feature enabled, you will find an implementation of `WordSplitter`
/// by the `hyphenation::language::Corpus` struct. Use this struct for
/// language-aware hyphenation. See the [`hyphenation` documentation]
/// for details.
///
/// [`wrap_iter`]: ../struct.Wrapper.html#method.wrap_iter
/// [`split`]: #tymethod.split
/// [`hyphenation` documentation]: https://docs.rs/hyphenation/
pub trait WordSplitter {
    /// Return all possible splits of word. Each split is a triple
    /// with a head, a hyphen, and a tail where `head + &hyphen +
    /// &tail == word`. The hyphen can be empty if there is already a
    /// hyphen in the head.
    ///
    /// The splits should go from smallest to longest and should
    /// include no split at all. So the word "technology" could be
    /// split into
    ///
    /// ```no_run
    /// vec![("tech", "-", "nology"),
    ///      ("technol", "-", "ogy"),
    ///      ("technolo", "-", "gy"),
    ///      ("technology", "", "")];
    /// ```
    fn split<'w>(&self, word: &'w str) -> Vec<(&'w str, &'w str, &'w str)>;
}

/// Use this as a [`Wrapper.splitter`] to avoid any kind of
/// hyphenation:
///
/// ```
/// use textwrap::{Wrapper, NoHyphenation};
///
/// let wrapper = Wrapper::with_splitter(8, NoHyphenation);
/// assert_eq!(wrapper.wrap("foo bar-baz"), vec!["foo", "bar-baz"]);
/// ```
///
/// [`Wrapper.splitter`]: ../struct.Wrapper.html#structfield.splitter
#[derive(Clone, Debug)]
pub struct NoHyphenation;

/// `NoHyphenation` implements `WordSplitter` by not splitting the
/// word at all.
impl WordSplitter for NoHyphenation {
    fn split<'w>(&self, word: &'w str) -> Vec<(&'w str, &'w str, &'w str)> {
        vec![(word, "", "")]
    }
}

/// Simple and default way to split words: splitting on existing
/// hyphens only.
///
/// You probably don't need to use this type since it's already used
/// by default by `Wrapper::new`.
#[derive(Clone, Debug)]
pub struct HyphenSplitter;

/// `HyphenSplitter` is the default `WordSplitter` used by
/// `Wrapper::new`. It will split words on any existing hyphens in the
/// word.
///
/// It will only use hyphens that are surrounded by alphanumeric
/// characters, which prevents a word like "--foo-bar" from being
/// split on the first or second hyphen.
impl WordSplitter for HyphenSplitter {
    fn split<'w>(&self, word: &'w str) -> Vec<(&'w str, &'w str, &'w str)> {
        let mut triples = Vec::new();
        // Split on hyphens, smallest split first. We only use hyphens
        // that are surrounded by alphanumeric characters. This is to
        // avoid splitting on repeated hyphens, such as those found in
        // --foo-bar.
        let mut char_indices = word.char_indices();
        // Early return if the word is empty.
        let mut prev = match char_indices.next() {
            None => return vec![(word, "", "")],
            Some((_, ch)) => ch,
        };

        // Find current word, or return early if the word only has a
        // single character.
        let (mut idx, mut cur) = match char_indices.next() {
            None => return vec![(word, "", "")],
            Some((idx, cur)) => (idx, cur),
        };

        for (i, next) in char_indices {
            if prev.is_alphanumeric() && cur == '-' && next.is_alphanumeric() {
                let (head, tail) = word.split_at(idx + 1);
                triples.push((head, "", tail));
            }
            prev = cur;
            idx = i;
            cur = next;
        }

        // Finally option is no split at all.
        triples.push((word, "", ""));

        triples
    }
}

/// A hyphenation dictionary can be used to do language-specific
/// hyphenation using patterns from the hyphenation crate.
#[cfg(feature = "hyphenation")]
impl WordSplitter for Standard {
    fn split<'w>(&self, word: &'w str) -> Vec<(&'w str, &'w str, &'w str)> {
        // Find splits based on language dictionary.
        let mut triples = Vec::new();
        for n in self.hyphenate(word).breaks {
            let (head, tail) = word.split_at(n);
            let hyphen = if head.ends_with('-') { "" } else { "-" };
            triples.push((head, hyphen, tail));
        }
        // Finally option is no split at all.
        triples.push((word, "", ""));

        triples
    }
}
