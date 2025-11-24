//! This library implements string similarity metrics.

#![forbid(unsafe_code)]

use std::char;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::hash::Hash;
use std::str::Chars;

#[derive(Debug, PartialEq)]
pub enum StrSimError {
    DifferentLengthArgs,
}

impl Display for StrSimError {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        let text = match self {
            StrSimError::DifferentLengthArgs => "Differing length arguments provided",
        };

        write!(fmt, "{}", text)
    }
}

impl Error for StrSimError {}

pub type HammingResult = Result<usize, StrSimError>;

/// Calculates the number of positions in the two sequences where the elements
/// differ. Returns an error if the sequences have different lengths.
pub fn generic_hamming<Iter1, Iter2, Elem1, Elem2>(a: Iter1, b: Iter2) -> HammingResult
    where Iter1: IntoIterator<Item=Elem1>,
          Iter2: IntoIterator<Item=Elem2>,
          Elem1: PartialEq<Elem2> {
    let (mut ita, mut itb) = (a.into_iter(), b.into_iter());
    let mut count = 0;
    loop {
        match (ita.next(), itb.next()){
            (Some(x), Some(y)) => if x != y { count += 1 },
            (None, None) => return Ok(count),
            _ => return Err(StrSimError::DifferentLengthArgs),
        }
    }
}

/// Calculates the number of positions in the two strings where the characters
/// differ. Returns an error if the strings have different lengths.
///
/// ```
/// use strsim::{hamming, StrSimError::DifferentLengthArgs};
///
/// assert_eq!(Ok(3), hamming("hamming", "hammers"));
///
/// assert_eq!(Err(DifferentLengthArgs), hamming("hamming", "ham"));
/// ```
pub fn hamming(a: &str, b: &str) -> HammingResult {
    generic_hamming(a.chars(), b.chars())
}

/// Calculates the Jaro similarity between two sequences. The returned value
/// is between 0.0 and 1.0 (higher value means more similar).
pub fn generic_jaro<'a, 'b, Iter1, Iter2, Elem1, Elem2>(a: &'a Iter1, b: &'b Iter2) -> f64
    where &'a Iter1: IntoIterator<Item=Elem1>,
          &'b Iter2: IntoIterator<Item=Elem2>,
          Elem1: PartialEq<Elem2> {
    let a_len = a.into_iter().count();
    let b_len = b.into_iter().count();

    // The check for lengths of one here is to prevent integer overflow when
    // calculating the search range.
    if a_len == 0 && b_len == 0 {
        return 1.0;
    } else if a_len == 0 || b_len == 0 {
        return 0.0;
    } else if a_len == 1 && b_len == 1 {
        return if a.into_iter().eq(b.into_iter()) { 1.0} else { 0.0 };
    }

    let search_range = (max(a_len, b_len) / 2) - 1;

    let mut b_consumed = Vec::with_capacity(b_len);
    for _ in 0..b_len {
        b_consumed.push(false);
    }
    let mut matches = 0.0;

    let mut transpositions = 0.0;
    let mut b_match_index = 0;

    for (i, a_elem) in a.into_iter().enumerate() {
        let min_bound =
            // prevent integer wrapping
            if i > search_range {
                max(0, i - search_range)
            } else {
                0
            };

        let max_bound = min(b_len - 1, i + search_range);

        if min_bound > max_bound {
            continue;
        }

        for (j, b_elem) in b.into_iter().enumerate() {
            if min_bound <= j && j <= max_bound && a_elem == b_elem &&
                !b_consumed[j] {
                b_consumed[j] = true;
                matches += 1.0;

                if j < b_match_index {
                    transpositions += 1.0;
                }
                b_match_index = j;

                break;
            }
        }
    }

    if matches == 0.0 {
        0.0
    } else {
        (1.0 / 3.0) * ((matches / a_len as f64) +
            (matches / b_len as f64) +
            ((matches - transpositions) / matches))
    }
}

struct StringWrapper<'a>(&'a str);

impl<'a, 'b> IntoIterator for &'a StringWrapper<'b> {
    type Item = char;
    type IntoIter = Chars<'b>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.chars()
    }
}

/// Calculates the Jaro similarity between two strings. The returned value
/// is between 0.0 and 1.0 (higher value means more similar).
///
/// ```
/// use strsim::jaro;
///
/// assert!((0.392 - jaro("Friedrich Nietzsche", "Jean-Paul Sartre")).abs() <
///         0.001);
/// ```
pub fn jaro(a: &str, b: &str) -> f64 {
    generic_jaro(&StringWrapper(a), &StringWrapper(b))
}

/// Like Jaro but gives a boost to sequences that have a common prefix.
pub fn generic_jaro_winkler<'a, 'b, Iter1, Iter2, Elem1, Elem2>(a: &'a Iter1, b: &'b Iter2) -> f64
    where &'a Iter1: IntoIterator<Item=Elem1>,
          &'b Iter2: IntoIterator<Item=Elem2>,
          Elem1: PartialEq<Elem2> {
    let jaro_distance = generic_jaro(a, b);

    // Don't limit the length of the common prefix
    let prefix_length = a.into_iter()
        .zip(b.into_iter())
        .take_while(|&(ref a_elem, ref b_elem)| a_elem == b_elem)
        .count();

    let jaro_winkler_distance =
        jaro_distance + (0.1 * prefix_length as f64 * (1.0 - jaro_distance));

    if jaro_winkler_distance <= 1.0 {
        jaro_winkler_distance
    } else {
        1.0
    }
}

/// Like Jaro but gives a boost to strings that have a common prefix.
///
/// ```
/// use strsim::jaro_winkler;
///
/// assert!((0.911 - jaro_winkler("cheeseburger", "cheese fries")).abs() <
///         0.001);
/// ```
pub fn jaro_winkler(a: &str, b: &str) -> f64 {
    generic_jaro_winkler(&StringWrapper(a), &StringWrapper(b))
}

/// Calculates the minimum number of insertions, deletions, and substitutions
/// required to change one sequence into the other.
///
/// ```
/// use strsim::generic_levenshtein;
///
/// assert_eq!(3, generic_levenshtein(&[1,2,3], &[1,2,3,4,5,6]));
/// ```
pub fn generic_levenshtein<'a, 'b, Iter1, Iter2, Elem1, Elem2>(a: &'a Iter1, b: &'b Iter2) -> usize
    where &'a Iter1: IntoIterator<Item=Elem1>,
          &'b Iter2: IntoIterator<Item=Elem2>,
          Elem1: PartialEq<Elem2> {
    let b_len = b.into_iter().count();

    if a.into_iter().next().is_none() { return b_len; }

    let mut cache: Vec<usize> = (1..b_len+1).collect();

    let mut result = 0;

    for (i, a_elem) in a.into_iter().enumerate() {
        result = i + 1;
        let mut distance_b = i;

        for (j, b_elem) in b.into_iter().enumerate() {
            let cost = if a_elem == b_elem { 0usize } else { 1usize };
            let distance_a = distance_b + cost;
            distance_b = cache[j];
            result = min(result + 1, min(distance_a, distance_b + 1));
            cache[j] = result;
        }
    }

    result
}

/// Calculates the minimum number of insertions, deletions, and substitutions
/// required to change one string into the other.
///
/// ```
/// use strsim::levenshtein;
///
/// assert_eq!(3, levenshtein("kitten", "sitting"));
/// ```
pub fn levenshtein(a: &str, b: &str) -> usize {
    generic_levenshtein(&StringWrapper(a), &StringWrapper(b))
}

/// Calculates a normalized score of the Levenshtein algorithm between 0.0 and
/// 1.0 (inclusive), where 1.0 means the strings are the same.
///
/// ```
/// use strsim::normalized_levenshtein;
///
/// assert!((normalized_levenshtein("kitten", "sitting") - 0.57142).abs() < 0.00001);
/// assert!((normalized_levenshtein("", "") - 1.0).abs() < 0.00001);
/// assert!(normalized_levenshtein("", "second").abs() < 0.00001);
/// assert!(normalized_levenshtein("first", "").abs() < 0.00001);
/// assert!((normalized_levenshtein("string", "string") - 1.0).abs() < 0.00001);
/// ```
pub fn normalized_levenshtein(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    1.0 - (levenshtein(a, b) as f64) / (a.chars().count().max(b.chars().count()) as f64)
}

/// Like Levenshtein but allows for adjacent transpositions. Each substring can
/// only be edited once.
///
/// ```
/// use strsim::osa_distance;
///
/// assert_eq!(3, osa_distance("ab", "bca"));
/// ```
pub fn osa_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();
    if a == b { return 0; }
    else if a_len == 0 { return b_len; }
    else if b_len == 0 { return a_len; }

    let mut prev_two_distances: Vec<usize> = Vec::with_capacity(b_len + 1);
    let mut prev_distances: Vec<usize> = Vec::with_capacity(b_len + 1);
    let mut curr_distances: Vec<usize> = Vec::with_capacity(b_len + 1);

    let mut prev_a_char = char::MAX;
    let mut prev_b_char = char::MAX;

    for i in 0..(b_len + 1) {
        prev_two_distances.push(i);
        prev_distances.push(i);
        curr_distances.push(0);
    }

    for (i, a_char) in a.chars().enumerate() {
        curr_distances[0] = i + 1;

        for (j, b_char) in b.chars().enumerate() {
            let cost = if a_char == b_char { 0 } else { 1 };
            curr_distances[j + 1] = min(curr_distances[j] + 1,
                                        min(prev_distances[j + 1] + 1,
                                            prev_distances[j] + cost));
            if i > 0 && j > 0 && a_char != b_char &&
                a_char == prev_b_char && b_char == prev_a_char {
                curr_distances[j + 1] = min(curr_distances[j + 1],
                                            prev_two_distances[j - 1] + 1);
            }

            prev_b_char = b_char;
        }

        prev_two_distances.clone_from(&prev_distances);
        prev_distances.clone_from(&curr_distances);
        prev_a_char = a_char;
    }

    curr_distances[b_len]

}

/* Returns the final index for a value in a single vector that represents a fixed
   2d grid */
fn flat_index(i: usize, j: usize, width: usize) -> usize {
    j * width + i
}

/// Like optimal string alignment, but substrings can be edited an unlimited
/// number of times, and the triangle inequality holds.
///
/// ```
/// use strsim::generic_damerau_levenshtein;
///
/// assert_eq!(2, generic_damerau_levenshtein(&[1,2], &[2,3,1]));
/// ```
pub fn generic_damerau_levenshtein<Elem>(a_elems: &[Elem], b_elems: &[Elem]) -> usize
    where Elem: Eq + Hash + Clone {
    let a_len = a_elems.len();
    let b_len = b_elems.len();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let width = a_len + 2;
    let mut distances = vec![0; (a_len + 2) * (b_len + 2)];
    let max_distance = a_len + b_len;
    distances[0] = max_distance;

    for i in 0..(a_len + 1) {
        distances[flat_index(i + 1, 0, width)] = max_distance;
        distances[flat_index(i + 1, 1, width)] = i;
    }

    for j in 0..(b_len + 1) {
        distances[flat_index(0, j + 1, width)] = max_distance;
        distances[flat_index(1, j + 1, width)] = j;
    }

    let mut elems: HashMap<Elem, usize> = HashMap::with_capacity(64);

    for i in 1..(a_len + 1) {
        let mut db = 0;

        for j in 1..(b_len + 1) {
            let k = match elems.get(&b_elems[j - 1]) {
                Some(&value) => value,
                None => 0
            };

            let insertion_cost = distances[flat_index(i, j + 1, width)] + 1;
            let deletion_cost = distances[flat_index(i + 1, j, width)] + 1;
            let transposition_cost = distances[flat_index(k, db, width)] +
                (i - k - 1) + 1 + (j - db - 1);

            let mut substitution_cost = distances[flat_index(i, j, width)] + 1;
            if a_elems[i - 1] == b_elems[j - 1] {
                db = j;
                substitution_cost -= 1;
            }

            distances[flat_index(i + 1, j + 1, width)] = min(substitution_cost,
                min(insertion_cost, min(deletion_cost, transposition_cost)));
        }

        elems.insert(a_elems[i - 1].clone(), i);
    }

    distances[flat_index(a_len + 1, b_len + 1, width)]
}

/// Like optimal string alignment, but substrings can be edited an unlimited
/// number of times, and the triangle inequality holds.
///
/// ```
/// use strsim::damerau_levenshtein;
///
/// assert_eq!(2, damerau_levenshtein("ab", "bca"));
/// ```
pub fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let (x, y): (Vec<_>, Vec<_>) = (a.chars().collect(), b.chars().collect());
    generic_damerau_levenshtein(x.as_slice(), y.as_slice())
}

/// Calculates a normalized score of the Damerau–Levenshtein algorithm between
/// 0.0 and 1.0 (inclusive), where 1.0 means the strings are the same.
///
/// ```
/// use strsim::normalized_damerau_levenshtein;
///
/// assert!((normalized_damerau_levenshtein("levenshtein", "löwenbräu") - 0.27272).abs() < 0.00001);
/// assert!((normalized_damerau_levenshtein("", "") - 1.0).abs() < 0.00001);
/// assert!(normalized_damerau_levenshtein("", "flower").abs() < 0.00001);
/// assert!(normalized_damerau_levenshtein("tree", "").abs() < 0.00001);
/// assert!((normalized_damerau_levenshtein("sunglasses", "sunglasses") - 1.0).abs() < 0.00001);
/// ```
pub fn normalized_damerau_levenshtein(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    1.0 - (damerau_levenshtein(a, b) as f64) / (a.chars().count().max(b.chars().count()) as f64)
}

/// Returns an Iterator of char tuples.
fn bigrams(s: &str) -> impl Iterator<Item=(char, char)> + '_ {
    s.chars().zip(s.chars().skip(1))
}


/// Calculates a Sørensen-Dice similarity distance using bigrams.
/// See http://en.wikipedia.org/wiki/S%C3%B8rensen%E2%80%93Dice_coefficient.
///
/// ```
/// use strsim::sorensen_dice;
///
/// assert_eq!(1.0, sorensen_dice("", ""));
/// assert_eq!(0.0, sorensen_dice("", "a"));
/// assert_eq!(0.0, sorensen_dice("french", "quebec"));
/// assert_eq!(1.0, sorensen_dice("ferris", "ferris"));
/// assert_eq!(1.0, sorensen_dice("ferris", "ferris"));
/// assert_eq!(0.8888888888888888, sorensen_dice("feris", "ferris"));
/// ```
pub fn sorensen_dice(a: &str, b: &str) -> f64 {
    // implementation guided by
    // https://github.com/aceakash/string-similarity/blob/f83ba3cd7bae874c20c429774e911ae8cff8bced/src/index.js#L6

    let a: String = a.chars().filter(|&x| !char::is_whitespace(x)).collect();
    let b: String = b.chars().filter(|&x| !char::is_whitespace(x)).collect();

    if a.len() == 0 && b.len() == 0 {
        return 1.0;
    }

    if a.len() == 0 || b.len() == 0 {
        return 0.0;
    }

    if a == b {
        return 1.0;
    }

    if a.len() == 1 && b.len() == 1 {
        return 0.0;
    }

    if a.len() < 2 || b.len() < 2 {
        return 0.0;
    }

    let mut a_bigrams: HashMap<(char, char), usize> = HashMap::new();

    for bigram in bigrams(&a) {
        *a_bigrams.entry(bigram).or_insert(0) += 1;
    }

    let mut intersection_size = 0;

    for bigram in bigrams(&b) {
        a_bigrams.entry(bigram).and_modify(|bi| {
            if *bi > 0 {
                *bi -= 1;
                intersection_size += 1;
            }
        });
    }

    (2 * intersection_size) as f64 / (a.len() + b.len() - 2) as f64
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bigrams_iterator() {
        let mut bi = bigrams("abcde");

        assert_eq!(Some(('a', 'b')), bi.next());
        assert_eq!(Some(('b', 'c')), bi.next());
        assert_eq!(Some(('c', 'd')), bi.next());
        assert_eq!(Some(('d', 'e')), bi.next());
        assert_eq!(None, bi.next());
    }

    fn assert_hamming_dist(dist: usize, str1: &str, str2: &str) {
        assert_eq!(Ok(dist), hamming(str1, str2));
    }

    #[test]
    fn hamming_empty() {
        assert_hamming_dist(0, "", "")
    }

    #[test]
    fn hamming_same() {
        assert_hamming_dist(0, "hamming", "hamming")
    }

    #[test]
    fn hamming_numbers() {
        assert_eq!(Ok(1), generic_hamming(&[1, 2, 4], &[1, 2, 3]));
    }

    #[test]
    fn hamming_diff() {
        assert_hamming_dist(3, "hamming", "hammers")
    }

    #[test]
    fn hamming_diff_multibyte() {
        assert_hamming_dist(2, "hamming", "h香mmüng");
    }

    #[test]
    fn hamming_unequal_length() {
        assert_eq!(
            Err(StrSimError::DifferentLengthArgs),
            generic_hamming("ham".chars(), "hamming".chars())
        );
    }

    #[test]
    fn hamming_names() {
        assert_hamming_dist(14, "Friedrich Nietzs", "Jean-Paul Sartre")
    }

    #[test]
    fn jaro_both_empty() {
        assert_eq!(1.0, jaro("", ""));
    }

    #[test]
    fn jaro_first_empty() {
        assert_eq!(0.0, jaro("", "jaro"));
    }

    #[test]
    fn jaro_second_empty() {
        assert_eq!(0.0, jaro("distance", ""));
    }

    #[test]
    fn jaro_same() {
        assert_eq!(1.0, jaro("jaro", "jaro"));
    }

    #[test]
    fn jaro_multibyte() {
        assert!((0.818 - jaro("testabctest", "testöঙ香test")) < 0.001);
        assert!((0.818 - jaro("testöঙ香test", "testabctest")) < 0.001);
    }

    #[test]
    fn jaro_diff_short() {
        assert!((0.767 - jaro("dixon", "dicksonx")).abs() < 0.001);
    }

    #[test]
    fn jaro_diff_one_character() {
        assert_eq!(0.0, jaro("a", "b"));
    }

    #[test]
    fn jaro_same_one_character() {
        assert_eq!(1.0, jaro("a", "a"));
    }

    #[test]
    fn generic_jaro_diff() {
        assert_eq!(0.0, generic_jaro(&[1, 2], &[3, 4]));
    }

    #[test]
    fn jaro_diff_one_and_two() {
        assert!((0.83 - jaro("a", "ab")).abs() < 0.01);
    }

    #[test]
    fn jaro_diff_two_and_one() {
        assert!((0.83 - jaro("ab", "a")).abs() < 0.01);
    }

    #[test]
    fn jaro_diff_no_transposition() {
        assert!((0.822 - jaro("dwayne", "duane")).abs() < 0.001);
    }

    #[test]
    fn jaro_diff_with_transposition() {
        assert!((0.944 - jaro("martha", "marhta")).abs() < 0.001);
    }

    #[test]
    fn jaro_names() {
        assert!((0.392 - jaro("Friedrich Nietzsche",
                              "Jean-Paul Sartre")).abs() < 0.001);
    }

    #[test]
    fn jaro_winkler_both_empty() {
        assert_eq!(1.0, jaro_winkler("", ""));
    }

    #[test]
    fn jaro_winkler_first_empty() {
        assert_eq!(0.0, jaro_winkler("", "jaro-winkler"));
    }

    #[test]
    fn jaro_winkler_second_empty() {
        assert_eq!(0.0, jaro_winkler("distance", ""));
    }

    #[test]
    fn jaro_winkler_same() {
        assert_eq!(1.0, jaro_winkler("Jaro-Winkler", "Jaro-Winkler"));
    }

    #[test]
    fn jaro_winkler_multibyte() {
        assert!((0.89 - jaro_winkler("testabctest", "testöঙ香test")).abs() <
            0.001);
        assert!((0.89 - jaro_winkler("testöঙ香test", "testabctest")).abs() <
            0.001);
    }

    #[test]
    fn jaro_winkler_diff_short() {
        assert!((0.813 - jaro_winkler("dixon", "dicksonx")).abs() < 0.001);
        assert!((0.813 - jaro_winkler("dicksonx", "dixon")).abs() < 0.001);
    }

    #[test]
    fn jaro_winkler_diff_one_character() {
        assert_eq!(0.0, jaro_winkler("a", "b"));
    }

    #[test]
    fn jaro_winkler_same_one_character() {
        assert_eq!(1.0, jaro_winkler("a", "a"));
    }

    #[test]
    fn jaro_winkler_diff_no_transposition() {
        assert!((0.840 - jaro_winkler("dwayne", "duane")).abs() < 0.001);
    }

    #[test]
    fn jaro_winkler_diff_with_transposition() {
        assert!((0.961 - jaro_winkler("martha", "marhta")).abs() < 0.001);
    }

    #[test]
    fn jaro_winkler_names() {
        assert!((0.562 - jaro_winkler("Friedrich Nietzsche",
                                      "Fran-Paul Sartre")).abs() < 0.001);
    }

    #[test]
    fn jaro_winkler_long_prefix() {
        assert!((0.911 - jaro_winkler("cheeseburger", "cheese fries")).abs() <
            0.001);
    }

    #[test]
    fn jaro_winkler_more_names() {
        assert!((0.868 - jaro_winkler("Thorkel", "Thorgier")).abs() < 0.001);
    }

    #[test]
    fn jaro_winkler_length_of_one() {
        assert!((0.738 - jaro_winkler("Dinsdale", "D")).abs() < 0.001);
    }

    #[test]
    fn jaro_winkler_very_long_prefix() {
        assert!((1.0 - jaro_winkler("thequickbrownfoxjumpedoverx",
                                    "thequickbrownfoxjumpedovery")).abs() <
            0.001);
    }

    #[test]
    fn levenshtein_empty() {
        assert_eq!(0, levenshtein("", ""));
    }

    #[test]
    fn levenshtein_same() {
        assert_eq!(0, levenshtein("levenshtein", "levenshtein"));
    }

    #[test]
    fn levenshtein_diff_short() {
        assert_eq!(3, levenshtein("kitten", "sitting"));
    }

    #[test]
    fn levenshtein_diff_with_space() {
        assert_eq!(5, levenshtein("hello, world", "bye, world"));
    }

    #[test]
    fn levenshtein_diff_multibyte() {
        assert_eq!(3, levenshtein("öঙ香", "abc"));
        assert_eq!(3, levenshtein("abc", "öঙ香"));
    }

    #[test]
    fn levenshtein_diff_longer() {
        let a = "The quick brown fox jumped over the angry dog.";
        let b = "Lorem ipsum dolor sit amet, dicta latine an eam.";
        assert_eq!(37, levenshtein(a, b));
    }

    #[test]
    fn levenshtein_first_empty() {
        assert_eq!(7, levenshtein("", "sitting"));
    }

    #[test]
    fn levenshtein_second_empty() {
        assert_eq!(6, levenshtein("kitten", ""));
    }

    #[test]
    fn normalized_levenshtein_diff_short() {
        assert!((normalized_levenshtein("kitten", "sitting") - 0.57142).abs() < 0.00001);
    }

    #[test]
    fn normalized_levenshtein_for_empty_strings() {
        assert!((normalized_levenshtein("", "") - 1.0).abs() < 0.00001);
    }

    #[test]
    fn normalized_levenshtein_first_empty() {
        assert!(normalized_levenshtein("", "second").abs() < 0.00001);
    }

    #[test]
    fn normalized_levenshtein_second_empty() {
        assert!(normalized_levenshtein("first", "").abs() < 0.00001);
    }

    #[test]
    fn normalized_levenshtein_identical_strings() {
        assert!((normalized_levenshtein("identical", "identical") - 1.0).abs() < 0.00001);
    }

    #[test]
    fn osa_distance_empty() {
        assert_eq!(0, osa_distance("", ""));
    }

    #[test]
    fn osa_distance_same() {
        assert_eq!(0, osa_distance("damerau", "damerau"));
    }

    #[test]
    fn osa_distance_first_empty() {
        assert_eq!(7, osa_distance("", "damerau"));
    }

    #[test]
    fn osa_distance_second_empty() {
        assert_eq!(7, osa_distance("damerau", ""));
    }

    #[test]
    fn osa_distance_diff() {
        assert_eq!(3, osa_distance("ca", "abc"));
    }

    #[test]
    fn osa_distance_diff_short() {
        assert_eq!(3, osa_distance("damerau", "aderua"));
    }

    #[test]
    fn osa_distance_diff_reversed() {
        assert_eq!(3, osa_distance("aderua", "damerau"));
    }

    #[test]
    fn osa_distance_diff_multibyte() {
        assert_eq!(3, osa_distance("öঙ香", "abc"));
        assert_eq!(3, osa_distance("abc", "öঙ香"));
    }

    #[test]
    fn osa_distance_diff_unequal_length() {
        assert_eq!(6, osa_distance("damerau", "aderuaxyz"));
    }

    #[test]
    fn osa_distance_diff_unequal_length_reversed() {
        assert_eq!(6, osa_distance("aderuaxyz", "damerau"));
    }

    #[test]
    fn osa_distance_diff_comedians() {
        assert_eq!(5, osa_distance("Stewart", "Colbert"));
    }

    #[test]
    fn osa_distance_many_transpositions() {
        assert_eq!(4, osa_distance("abcdefghijkl", "bacedfgihjlk"));
    }

    #[test]
    fn osa_distance_diff_longer() {
        let a = "The quick brown fox jumped over the angry dog.";
        let b = "Lehem ipsum dolor sit amet, dicta latine an eam.";
        assert_eq!(36, osa_distance(a, b));
    }

    #[test]
    fn osa_distance_beginning_transposition() {
        assert_eq!(1, osa_distance("foobar", "ofobar"));
    }

    #[test]
    fn osa_distance_end_transposition() {
        assert_eq!(1, osa_distance("specter", "spectre"));
    }

    #[test]
    fn osa_distance_restricted_edit() {
        assert_eq!(4, osa_distance("a cat", "an abct"));
    }

    #[test]
    fn damerau_levenshtein_empty() {
        assert_eq!(0, damerau_levenshtein("", ""));
    }

    #[test]
    fn damerau_levenshtein_same() {
        assert_eq!(0, damerau_levenshtein("damerau", "damerau"));
    }

    #[test]
    fn damerau_levenshtein_first_empty() {
        assert_eq!(7, damerau_levenshtein("", "damerau"));
    }

    #[test]
    fn damerau_levenshtein_second_empty() {
        assert_eq!(7, damerau_levenshtein("damerau", ""));
    }

    #[test]
    fn damerau_levenshtein_diff() {
        assert_eq!(2, damerau_levenshtein("ca", "abc"));
    }

    #[test]
    fn damerau_levenshtein_diff_short() {
        assert_eq!(3, damerau_levenshtein("damerau", "aderua"));
    }

    #[test]
    fn damerau_levenshtein_diff_reversed() {
        assert_eq!(3, damerau_levenshtein("aderua", "damerau"));
    }

    #[test]
    fn damerau_levenshtein_diff_multibyte() {
        assert_eq!(3, damerau_levenshtein("öঙ香", "abc"));
        assert_eq!(3, damerau_levenshtein("abc", "öঙ香"));
    }

    #[test]
    fn damerau_levenshtein_diff_unequal_length() {
        assert_eq!(6, damerau_levenshtein("damerau", "aderuaxyz"));
    }

    #[test]
    fn damerau_levenshtein_diff_unequal_length_reversed() {
        assert_eq!(6, damerau_levenshtein("aderuaxyz", "damerau"));
    }

    #[test]
    fn damerau_levenshtein_diff_comedians() {
        assert_eq!(5, damerau_levenshtein("Stewart", "Colbert"));
    }

    #[test]
    fn damerau_levenshtein_many_transpositions() {
        assert_eq!(4, damerau_levenshtein("abcdefghijkl", "bacedfgihjlk"));
    }

    #[test]
    fn damerau_levenshtein_diff_longer() {
        let a = "The quick brown fox jumped over the angry dog.";
        let b = "Lehem ipsum dolor sit amet, dicta latine an eam.";
        assert_eq!(36, damerau_levenshtein(a, b));
    }

    #[test]
    fn damerau_levenshtein_beginning_transposition() {
        assert_eq!(1, damerau_levenshtein("foobar", "ofobar"));
    }

    #[test]
    fn damerau_levenshtein_end_transposition() {
        assert_eq!(1, damerau_levenshtein("specter", "spectre"));
    }

    #[test]
    fn damerau_levenshtein_unrestricted_edit() {
        assert_eq!(3, damerau_levenshtein("a cat", "an abct"));
    }

    #[test]
    fn normalized_damerau_levenshtein_diff_short() {
        assert!((normalized_damerau_levenshtein("levenshtein", "löwenbräu") - 0.27272).abs() < 0.00001);
    }

    #[test]
    fn normalized_damerau_levenshtein_for_empty_strings() {
        assert!((normalized_damerau_levenshtein("", "") - 1.0).abs() < 0.00001);
    }

    #[test]
    fn normalized_damerau_levenshtein_first_empty() {
        assert!(normalized_damerau_levenshtein("", "flower").abs() < 0.00001);
    }

    #[test]
    fn normalized_damerau_levenshtein_second_empty() {
        assert!(normalized_damerau_levenshtein("tree", "").abs() < 0.00001);
    }

    #[test]
    fn normalized_damerau_levenshtein_identical_strings() {
        assert!((normalized_damerau_levenshtein("sunglasses", "sunglasses") - 1.0).abs() < 0.00001);
    }

    #[test]
    fn sorensen_dice_all() {
        // test cases taken from
        // https://github.com/aceakash/string-similarity/blob/f83ba3cd7bae874c20c429774e911ae8cff8bced/src/spec/index.spec.js#L11

        assert_eq!(1.0, sorensen_dice("a", "a"));
        assert_eq!(0.0, sorensen_dice("a", "b"));
        assert_eq!(1.0, sorensen_dice("", ""));
        assert_eq!(0.0, sorensen_dice("a", ""));
        assert_eq!(0.0, sorensen_dice("", "a"));
        assert_eq!(1.0, sorensen_dice("apple event", "apple    event"));
        assert_eq!(0.9090909090909091, sorensen_dice("iphone", "iphone x"));
        assert_eq!(0.0, sorensen_dice("french", "quebec"));
        assert_eq!(1.0, sorensen_dice("france", "france"));
        assert_eq!(0.2, sorensen_dice("fRaNce", "france"));
        assert_eq!(0.8, sorensen_dice("healed", "sealed"));
        assert_eq!(
            0.7878787878787878,
            sorensen_dice("web applications", "applications of the web")
        );
        assert_eq!(
            0.92,
            sorensen_dice(
                "this will have a typo somewhere",
                "this will huve a typo somewhere"
            )
        );
        assert_eq!(
            0.6060606060606061,
            sorensen_dice(
                "Olive-green table for sale, in extremely good condition.",
                "For sale: table in very good  condition, olive green in colour."
            )
        );
        assert_eq!(
            0.2558139534883721,
            sorensen_dice(
                "Olive-green table for sale, in extremely good condition.",
                "For sale: green Subaru Impreza, 210,000 miles"
            )
        );
        assert_eq!(
            0.1411764705882353,
            sorensen_dice(
                "Olive-green table for sale, in extremely good condition.",
                "Wanted: mountain bike with at least 21 gears."
            )
        );
        assert_eq!(
            0.7741935483870968,
            sorensen_dice("this has one extra word", "this has one word")
        );
    }
}
