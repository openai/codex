//! Word splitting functionality.
//!
//! To wrap text into lines, long words sometimes need to be split
//! across lines. The [`WordSplitter`] enum defines this
//! functionality.

use crate::core::{display_width, Word};

/// The `WordSplitter` enum describes where words can be split.
///
/// If the textwrap crate has been compiled with the `hyphenation`
/// Cargo feature enabled, you will find a
/// [`WordSplitter::Hyphenation`] variant. Use this struct for
/// language-aware hyphenation:
///
/// ```
/// #[cfg(feature = "hyphenation")] {
///     use hyphenation::{Language, Load, Standard};
///     use textwrap::{wrap, Options, WordSplitter};
///
///     let text = "Oxidation is the loss of electrons.";
///     let dictionary = Standard::from_embedded(Language::EnglishUS).unwrap();
///     let options = Options::new(8).word_splitter(WordSplitter::Hyphenation(dictionary));
///     assert_eq!(wrap(text, &options), vec!["Oxida-",
///                                           "tion is",
///                                           "the loss",
///                                           "of elec-",
///                                           "trons."]);
/// }
/// ```
///
/// Please see the documentation for the [hyphenation] crate for more
/// details.
///
/// [hyphenation]: https://docs.rs/hyphenation/
#[derive(Debug, Clone)]
pub enum WordSplitter {
    /// Use this as a [`Options.word_splitter`] to avoid any kind of
    /// hyphenation:
    ///
    /// ```
    /// use textwrap::{wrap, Options, WordSplitter};
    ///
    /// let options = Options::new(8).word_splitter(WordSplitter::NoHyphenation);
    /// assert_eq!(wrap("foo bar-baz", &options),
    ///            vec!["foo", "bar-baz"]);
    /// ```
    ///
    /// [`Options.word_splitter`]: super::Options::word_splitter
    NoHyphenation,

    /// `HyphenSplitter` is the default `WordSplitter` used by
    /// [`Options::new`](super::Options::new). It will split words on
    /// existing hyphens in the word.
    ///
    /// It will only use hyphens that are surrounded by alphanumeric
    /// characters, which prevents a word like `"--foo-bar"` from
    /// being split into `"--"` and `"foo-bar"`.
    ///
    /// # Examples
    ///
    /// ```
    /// use textwrap::WordSplitter;
    ///
    /// assert_eq!(WordSplitter::HyphenSplitter.split_points("--foo-bar"),
    ///            vec![6]);
    /// ```
    HyphenSplitter,

    /// Use a custom function as the word splitter.
    ///
    /// This variant lets you implement a custom word splitter using
    /// your own function.
    ///
    /// # Examples
    ///
    /// ```
    /// use textwrap::WordSplitter;
    ///
    /// fn split_at_underscore(word: &str) -> Vec<usize> {
    ///     word.match_indices('_').map(|(idx, _)| idx + 1).collect()
    /// }
    ///
    /// let word_splitter = WordSplitter::Custom(split_at_underscore);
    /// assert_eq!(word_splitter.split_points("a_long_identifier"),
    ///            vec![2, 7]);
    /// ```
    Custom(fn(word: &str) -> Vec<usize>),

    /// A hyphenation dictionary can be used to do language-specific
    /// hyphenation using patterns from the [hyphenation] crate.
    ///
    /// **Note:** Only available when the `hyphenation` Cargo feature is
    /// enabled.
    ///
    /// [hyphenation]: https://docs.rs/hyphenation/
    #[cfg(feature = "hyphenation")]
    Hyphenation(hyphenation::Standard),
}

impl PartialEq<WordSplitter> for WordSplitter {
    fn eq(&self, other: &WordSplitter) -> bool {
        match (self, other) {
            (WordSplitter::NoHyphenation, WordSplitter::NoHyphenation) => true,
            (WordSplitter::HyphenSplitter, WordSplitter::HyphenSplitter) => true,
            #[cfg(feature = "hyphenation")]
            (WordSplitter::Hyphenation(this_dict), WordSplitter::Hyphenation(other_dict)) => {
                this_dict.language() == other_dict.language()
            }
            (_, _) => false,
        }
    }
}

impl WordSplitter {
    /// Return all possible indices where `word` can be split.
    ///
    /// The indices are in the range `0..word.len()`. They point to
    /// the index _after_ the split point, i.e., after `-` if
    /// splitting on hyphens. This way, `word.split_at(idx)` will
    /// break the word into two well-formed pieces.
    ///
    /// # Examples
    ///
    /// ```
    /// use textwrap::WordSplitter;
    /// assert_eq!(WordSplitter::NoHyphenation.split_points("cannot-be-split"), vec![]);
    /// assert_eq!(WordSplitter::HyphenSplitter.split_points("can-be-split"), vec![4, 7]);
    /// assert_eq!(WordSplitter::Custom(|word| vec![word.len()/2]).split_points("middle"), vec![3]);
    /// ```
    pub fn split_points(&self, word: &str) -> Vec<usize> {
        match self {
            WordSplitter::NoHyphenation => Vec::new(),
            WordSplitter::HyphenSplitter => {
                let mut splits = Vec::new();

                for (idx, _) in word.match_indices('-') {
                    // We only use hyphens that are surrounded by alphanumeric
                    // characters. This is to avoid splitting on repeated hyphens,
                    // such as those found in --foo-bar.
                    let prev = word[..idx].chars().next_back();
                    let next = word[idx + 1..].chars().next();

                    if prev.filter(|ch| ch.is_alphanumeric()).is_some()
                        && next.filter(|ch| ch.is_alphanumeric()).is_some()
                    {
                        splits.push(idx + 1); // +1 due to width of '-'.
                    }
                }

                splits
            }
            WordSplitter::Custom(splitter_func) => splitter_func(word),
            #[cfg(feature = "hyphenation")]
            WordSplitter::Hyphenation(dictionary) => {
                use hyphenation::Hyphenator;
                dictionary.hyphenate(word).breaks
            }
        }
    }
}

/// Split words into smaller words according to the split points given
/// by `word_splitter`.
///
/// Note that we split all words, regardless of their length. This is
/// to more cleanly separate the business of splitting (including
/// automatic hyphenation) from the business of word wrapping.
pub fn split_words<'a, I>(
    words: I,
    word_splitter: &'a WordSplitter,
) -> impl Iterator<Item = Word<'a>>
where
    I: IntoIterator<Item = Word<'a>>,
{
    words.into_iter().flat_map(move |word| {
        let mut prev = 0;
        let mut split_points = word_splitter.split_points(&word).into_iter();
        std::iter::from_fn(move || {
            if let Some(idx) = split_points.next() {
                let need_hyphen = !word[..idx].ends_with('-');
                let w = Word {
                    word: &word.word[prev..idx],
                    width: display_width(&word[prev..idx]),
                    whitespace: "",
                    penalty: if need_hyphen { "-" } else { "" },
                };
                prev = idx;
                return Some(w);
            }

            if prev < word.word.len() || prev == 0 {
                let w = Word {
                    word: &word.word[prev..],
                    width: display_width(&word[prev..]),
                    whitespace: word.whitespace,
                    penalty: word.penalty,
                };
                prev = word.word.len() + 1;
                return Some(w);
            }

            None
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Like assert_eq!, but the left expression is an iterator.
    macro_rules! assert_iter_eq {
        ($left:expr, $right:expr) => {
            assert_eq!($left.collect::<Vec<_>>(), $right);
        };
    }

    #[test]
    fn split_words_no_words() {
        assert_iter_eq!(split_words(vec![], &WordSplitter::HyphenSplitter), vec![]);
    }

    #[test]
    fn split_words_empty_word() {
        assert_iter_eq!(
            split_words(vec![Word::from("   ")], &WordSplitter::HyphenSplitter),
            vec![Word::from("   ")]
        );
    }

    #[test]
    fn split_words_single_word() {
        assert_iter_eq!(
            split_words(vec![Word::from("foobar")], &WordSplitter::HyphenSplitter),
            vec![Word::from("foobar")]
        );
    }

    #[test]
    fn split_words_hyphen_splitter() {
        assert_iter_eq!(
            split_words(vec![Word::from("foo-bar")], &WordSplitter::HyphenSplitter),
            vec![Word::from("foo-"), Word::from("bar")]
        );
    }

    #[test]
    fn split_words_no_hyphenation() {
        assert_iter_eq!(
            split_words(vec![Word::from("foo-bar")], &WordSplitter::NoHyphenation),
            vec![Word::from("foo-bar")]
        );
    }

    #[test]
    fn split_words_adds_penalty() {
        let fixed_split_point = |_: &str| vec![3];

        assert_iter_eq!(
            split_words(
                vec![Word::from("foobar")].into_iter(),
                &WordSplitter::Custom(fixed_split_point)
            ),
            vec![
                Word {
                    word: "foo",
                    width: 3,
                    whitespace: "",
                    penalty: "-"
                },
                Word {
                    word: "bar",
                    width: 3,
                    whitespace: "",
                    penalty: ""
                }
            ]
        );

        assert_iter_eq!(
            split_words(
                vec![Word::from("fo-bar")].into_iter(),
                &WordSplitter::Custom(fixed_split_point)
            ),
            vec![
                Word {
                    word: "fo-",
                    width: 3,
                    whitespace: "",
                    penalty: ""
                },
                Word {
                    word: "bar",
                    width: 3,
                    whitespace: "",
                    penalty: ""
                }
            ]
        );
    }
}
