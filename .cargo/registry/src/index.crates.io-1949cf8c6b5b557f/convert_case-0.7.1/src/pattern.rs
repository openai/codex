use std::iter;

#[cfg(feature = "random")]
use rand::prelude::*;

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
enum WordCase {
    Lower,
    Upper,
    Capital,
    Toggle,
}

impl WordCase {
    fn mutate(&self, word: &str) -> String {
        use WordCase::*;
        match self {
            Lower => word.to_lowercase(),
            Upper => word.to_uppercase(),
            Capital => {
                let mut chars = word.graphemes(true);
                if let Some(c) = chars.next() {
                    [c.to_uppercase(), chars.as_str().to_lowercase()].concat()
                } else {
                    String::new()
                }
            }
            Toggle => {
                let mut chars = word.graphemes(true);
                if let Some(c) = chars.next() {
                    [c.to_lowercase(), chars.as_str().to_uppercase()].concat()
                } else {
                    String::new()
                }
            }
        }
    }
}

/// A pattern is how a set of words is mutated before joining with
/// a delimeter.
///
/// The `Random` and `PseudoRandom` patterns are used for their respective cases
/// and are only available in the "random" feature.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Pattern {
    /// Lowercase patterns make all words lowercase.
    /// ```
    /// # use convert_case::Pattern;
    /// assert_eq!(
    ///     vec!["case", "conversion", "library"],
    ///     Pattern::Lowercase.mutate(&["Case", "CONVERSION", "library"])
    /// );
    /// ```
    Lowercase,

    /// Uppercase patterns make all words uppercase.
    /// ```
    /// # use convert_case::Pattern;
    /// assert_eq!(
    ///     vec!["CASE", "CONVERSION", "LIBRARY"],
    ///     Pattern::Uppercase.mutate(&["Case", "CONVERSION", "library"])
    /// );
    /// ```
    Uppercase,

    /// Capital patterns makes the first letter of each word uppercase
    /// and the remaining letters of each word lowercase.
    /// ```
    /// # use convert_case::Pattern;
    /// assert_eq!(
    ///     vec!["Case", "Conversion", "Library"],
    ///     Pattern::Capital.mutate(&["Case", "CONVERSION", "library"])
    /// );
    /// ```
    Capital,

    /// Capital patterns make the first word capitalized and the
    /// remaining lowercase.
    /// ```
    /// # use convert_case::Pattern;
    /// assert_eq!(
    ///     vec!["Case", "conversion", "library"],
    ///     Pattern::Sentence.mutate(&["Case", "CONVERSION", "library"])
    /// );
    /// ```
    Sentence,

    /// Camel patterns make the first word lowercase and the remaining
    /// capitalized.
    /// ```
    /// # use convert_case::Pattern;
    /// assert_eq!(
    ///     vec!["case", "Conversion", "Library"],
    ///     Pattern::Camel.mutate(&["Case", "CONVERSION", "library"])
    /// );
    /// ```
    Camel,

    /// Alternating patterns make each letter of each word alternate
    /// between lowercase and uppercase.  They alternate across words,
    /// which means the last letter of one word and the first letter of the
    /// next will not be the same letter casing.
    /// ```
    /// # use convert_case::Pattern;
    /// assert_eq!(
    ///     vec!["cAsE", "cOnVeRsIoN", "lIbRaRy"],
    ///     Pattern::Alternating.mutate(&["Case", "CONVERSION", "library"])
    /// );
    /// assert_eq!(
    ///     vec!["aNoThEr", "ExAmPlE"],
    ///     Pattern::Alternating.mutate(&["Another", "Example"]),
    /// );
    /// ```
    Alternating,

    /// Toggle patterns have the first letter of each word uppercase
    /// and the remaining letters of each word uppercase.
    /// ```
    /// # use convert_case::Pattern;
    /// assert_eq!(
    ///     vec!["cASE", "cONVERSION", "lIBRARY"],
    ///     Pattern::Toggle.mutate(&["Case", "CONVERSION", "library"])
    /// );
    /// ```
    Toggle,

    /// Random patterns will lowercase or uppercase each letter
    /// uniformly randomly.  This uses the `rand` crate and is only available with the "random"
    /// feature.  This example will not pass the assertion due to randomness, but it used as an
    /// example of what output is possible.
    /// ```should_panic
    /// # use convert_case::Pattern;
    /// # #[cfg(any(doc, feature = "random"))]
    /// assert_eq!(
    ///     vec!["Case", "coNVeRSiOn", "lIBraRY"],
    ///     Pattern::Random.mutate(&["Case", "CONVERSION", "library"])
    /// );
    /// ```
    #[cfg(any(doc, feature = "random"))]
    #[cfg(feature = "random")]
    Random,

    /// PseudoRandom patterns are random-like patterns.  Instead of randomizing
    /// each letter individually, it mutates each pair of characters
    /// as either (Lowercase, Uppercase) or (Uppercase, Lowercase).  This generates
    /// more "random looking" words.  A consequence of this algorithm for randomization
    /// is that there will never be three consecutive letters that are all lowercase
    /// or all uppercase.  This uses the `rand` crate and is only available with the "random"
    /// feature.  This example will not pass the assertion due to randomness, but it used as an
    /// example of what output is possible.
    /// ```should_panic
    /// # use convert_case::Pattern;
    /// # #[cfg(any(doc, feature = "random"))]
    /// assert_eq!(
    ///     vec!["cAsE", "cONveRSioN", "lIBrAry"],
    ///     Pattern::Random.mutate(&["Case", "CONVERSION", "library"]),
    /// );
    /// ```
    #[cfg(any(doc, feature = "random"))]
    #[cfg(feature = "random")]
    PseudoRandom,
}

impl Pattern {
    /// Generates a vector of new `String`s in the right pattern given
    /// the input strings.
    /// ```
    /// # use convert_case::Pattern;
    /// assert_eq!(
    ///     vec!["crack", "the", "skye"],
    ///     Pattern::Lowercase.mutate(&vec!["CRACK", "the", "Skye"]),
    /// )
    /// ```
    pub fn mutate(&self, words: &[&str]) -> Vec<String> {
        use Pattern::*;
        match self {
            Lowercase => words
                .iter()
                .map(|word| WordCase::Lower.mutate(word))
                .collect(),
            Uppercase => words
                .iter()
                .map(|word| WordCase::Upper.mutate(word))
                .collect(),
            Capital => words
                .iter()
                .map(|word| WordCase::Capital.mutate(word))
                .collect(),
            Toggle => words
                .iter()
                .map(|word| WordCase::Toggle.mutate(word))
                .collect(),
            Sentence => {
                let word_cases =
                    iter::once(WordCase::Capital).chain(iter::once(WordCase::Lower).cycle());
                words
                    .iter()
                    .zip(word_cases)
                    .map(|(word, word_case)| word_case.mutate(word))
                    .collect()
            }
            Camel => {
                let word_cases =
                    iter::once(WordCase::Lower).chain(iter::once(WordCase::Capital).cycle());
                words
                    .iter()
                    .zip(word_cases)
                    .map(|(word, word_case)| word_case.mutate(word))
                    .collect()
            }
            Alternating => alternating(words),
            #[cfg(feature = "random")]
            Random => randomize(words),
            #[cfg(feature = "random")]
            PseudoRandom => pseudo_randomize(words),
        }
    }
}

fn alternating(words: &[&str]) -> Vec<String> {
    let mut upper = false;
    words
        .iter()
        .map(|word| {
            word.chars()
                .map(|letter| {
                    if letter.is_uppercase() || letter.is_lowercase() {
                        if upper {
                            upper = false;
                            letter.to_uppercase().to_string()
                        } else {
                            upper = true;
                            letter.to_lowercase().to_string()
                        }
                    } else {
                        letter.to_string()
                    }
                })
                .collect()
        })
        .collect()
}

/// Randomly picks whether to be upper case or lower case
#[cfg(feature = "random")]
fn randomize(words: &[&str]) -> Vec<String> {
    let mut rng = rand::thread_rng();
    words
        .iter()
        .map(|word| {
            word.chars()
                .map(|letter| {
                    if rng.gen::<f32>() > 0.5 {
                        letter.to_uppercase().to_string()
                    } else {
                        letter.to_lowercase().to_string()
                    }
                })
                .collect()
        })
        .collect()
}

/// Randomly selects patterns: [upper, lower] or [lower, upper]
/// for a more random feeling pattern.
#[cfg(feature = "random")]
fn pseudo_randomize(words: &[&str]) -> Vec<String> {
    let mut rng = rand::thread_rng();

    // Keeps track of when to alternate
    let mut alt: Option<bool> = None;
    words
        .iter()
        .map(|word| {
            word.chars()
                .map(|letter| {
                    match alt {
                        // No existing pattern, start one
                        None => {
                            if rng.gen::<f32>() > 0.5 {
                                alt = Some(false); // Make the next char lower
                                letter.to_uppercase().to_string()
                            } else {
                                alt = Some(true); // Make the next char upper
                                letter.to_lowercase().to_string()
                            }
                        }
                        // Existing pattern, do what it says
                        Some(upper) => {
                            alt = None;
                            if upper {
                                letter.to_uppercase().to_string()
                            } else {
                                letter.to_lowercase().to_string()
                            }
                        }
                    }
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(feature = "random")]
    #[test]
    fn pseudo_no_triples() {
        let words = vec!["abcdefg", "hijklmnop", "qrstuv", "wxyz"];
        for _ in 0..5 {
            let new = pseudo_randomize(&words).join("");
            let mut iter = new
                .chars()
                .zip(new.chars().skip(1))
                .zip(new.chars().skip(2));
            assert!(!iter
                .clone()
                .any(|((a, b), c)| a.is_lowercase() && b.is_lowercase() && c.is_lowercase()));
            assert!(
                !iter.any(|((a, b), c)| a.is_uppercase() && b.is_uppercase() && c.is_uppercase())
            );
        }
    }

    #[cfg(feature = "random")]
    #[test]
    fn randoms_are_random() {
        let words = vec!["abcdefg", "hijklmnop", "qrstuv", "wxyz"];

        for _ in 0..5 {
            let transformed = pseudo_randomize(&words);
            assert_ne!(words, transformed);
            let transformed = randomize(&words);
            assert_ne!(words, transformed);
        }
    }

    #[test]
    fn mutate_empty_strings() {
        for wcase in [
            WordCase::Lower,
            WordCase::Upper,
            WordCase::Capital,
            WordCase::Toggle,
        ] {
            assert_eq!(String::new(), wcase.mutate(&String::new()))
        }
    }
}
