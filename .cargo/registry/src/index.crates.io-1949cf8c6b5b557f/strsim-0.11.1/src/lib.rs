//! This library implements string similarity metrics.

#![forbid(unsafe_code)]
#![allow(
    // these casts are sometimes needed. They restrict the length of input iterators
    // but there isn't really any way around this except for always working with
    // 128 bit types
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    // not practical
    clippy::needless_pass_by_value,
    clippy::similar_names,
    // noisy
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    // todo https://github.com/rapidfuzz/strsim-rs/issues/59
    clippy::range_plus_one
)]

use std::char;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::hash::Hash;
use std::mem;
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
where
    Iter1: IntoIterator<Item = Elem1>,
    Iter2: IntoIterator<Item = Elem2>,
    Elem1: PartialEq<Elem2>,
{
    let (mut ita, mut itb) = (a.into_iter(), b.into_iter());
    let mut count = 0;
    loop {
        match (ita.next(), itb.next()) {
            (Some(x), Some(y)) => {
                if x != y {
                    count += 1;
                }
            }
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
where
    &'a Iter1: IntoIterator<Item = Elem1>,
    &'b Iter2: IntoIterator<Item = Elem2>,
    Elem1: PartialEq<Elem2>,
{
    let a_len = a.into_iter().count();
    let b_len = b.into_iter().count();

    if a_len == 0 && b_len == 0 {
        return 1.0;
    } else if a_len == 0 || b_len == 0 {
        return 0.0;
    }

    let mut search_range = max(a_len, b_len) / 2;
    search_range = search_range.saturating_sub(1);

    // combine memory allocations to reduce runtime
    let mut flags_memory = vec![false; a_len + b_len];
    let (a_flags, b_flags) = flags_memory.split_at_mut(a_len);

    let mut matches = 0_usize;

    for (i, a_elem) in a.into_iter().enumerate() {
        // prevent integer wrapping
        let min_bound = if i > search_range {
            i - search_range
        } else {
            0
        };

        let max_bound = min(b_len, i + search_range + 1);

        for (j, b_elem) in b.into_iter().enumerate().take(max_bound) {
            if min_bound <= j && a_elem == b_elem && !b_flags[j] {
                a_flags[i] = true;
                b_flags[j] = true;
                matches += 1;
                break;
            }
        }
    }

    let mut transpositions = 0_usize;
    if matches != 0 {
        let mut b_iter = b_flags.iter().zip(b);
        for (a_flag, ch1) in a_flags.iter().zip(a) {
            if *a_flag {
                loop {
                    if let Some((b_flag, ch2)) = b_iter.next() {
                        if !*b_flag {
                            continue;
                        }

                        if ch1 != ch2 {
                            transpositions += 1;
                        }
                        break;
                    }
                }
            }
        }
    }
    transpositions /= 2;

    if matches == 0 {
        0.0
    } else {
        ((matches as f64 / a_len as f64)
            + (matches as f64 / b_len as f64)
            + ((matches - transpositions) as f64 / matches as f64))
            / 3.0
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
where
    &'a Iter1: IntoIterator<Item = Elem1>,
    &'b Iter2: IntoIterator<Item = Elem2>,
    Elem1: PartialEq<Elem2>,
{
    let sim = generic_jaro(a, b);

    if sim > 0.7 {
        let prefix_length = a
            .into_iter()
            .take(4)
            .zip(b)
            .take_while(|(a_elem, b_elem)| a_elem == b_elem)
            .count();

        sim + 0.1 * prefix_length as f64 * (1.0 - sim)
    } else {
        sim
    }
}

/// Like Jaro but gives a boost to strings that have a common prefix.
///
/// ```
/// use strsim::jaro_winkler;
///
/// assert!((0.866 - jaro_winkler("cheeseburger", "cheese fries")).abs() <
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
where
    &'a Iter1: IntoIterator<Item = Elem1>,
    &'b Iter2: IntoIterator<Item = Elem2>,
    Elem1: PartialEq<Elem2>,
{
    let b_len = b.into_iter().count();

    let mut cache: Vec<usize> = (1..b_len + 1).collect();

    let mut result = b_len;

    for (i, a_elem) in a.into_iter().enumerate() {
        result = i + 1;
        let mut distance_b = i;

        for (j, b_elem) in b.into_iter().enumerate() {
            let cost = usize::from(a_elem != b_elem);
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
    let b_len = b.chars().count();
    // 0..=b_len behaves like 0..b_len.saturating_add(1) which could be a different size
    // this leads to significantly worse code gen when swapping the vectors below
    let mut prev_two_distances: Vec<usize> = (0..b_len + 1).collect();
    let mut prev_distances: Vec<usize> = (0..b_len + 1).collect();
    let mut curr_distances: Vec<usize> = vec![0; b_len + 1];

    let mut prev_a_char = char::MAX;
    let mut prev_b_char = char::MAX;

    for (i, a_char) in a.chars().enumerate() {
        curr_distances[0] = i + 1;

        for (j, b_char) in b.chars().enumerate() {
            let cost = usize::from(a_char != b_char);
            curr_distances[j + 1] = min(
                curr_distances[j] + 1,
                min(prev_distances[j + 1] + 1, prev_distances[j] + cost),
            );
            if i > 0 && j > 0 && a_char != b_char && a_char == prev_b_char && b_char == prev_a_char
            {
                curr_distances[j + 1] = min(curr_distances[j + 1], prev_two_distances[j - 1] + 1);
            }

            prev_b_char = b_char;
        }

        mem::swap(&mut prev_two_distances, &mut prev_distances);
        mem::swap(&mut prev_distances, &mut curr_distances);
        prev_a_char = a_char;
    }

    // access prev_distances instead of curr_distances since we swapped
    // them above. In case a is empty this would still contain the correct value
    // from initializing the last element to b_len
    prev_distances[b_len]
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
where
    Elem: Eq + Hash + Clone,
{
    let a_len = a_elems.len();
    let b_len = b_elems.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

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
                None => 0,
            };

            let insertion_cost = distances[flat_index(i, j + 1, width)] + 1;
            let deletion_cost = distances[flat_index(i + 1, j, width)] + 1;
            let transposition_cost =
                distances[flat_index(k, db, width)] + (i - k - 1) + 1 + (j - db - 1);

            let mut substitution_cost = distances[flat_index(i, j, width)] + 1;
            if a_elems[i - 1] == b_elems[j - 1] {
                db = j;
                substitution_cost -= 1;
            }

            distances[flat_index(i + 1, j + 1, width)] = min(
                substitution_cost,
                min(insertion_cost, min(deletion_cost, transposition_cost)),
            );
        }

        elems.insert(a_elems[i - 1].clone(), i);
    }

    distances[flat_index(a_len + 1, b_len + 1, width)]
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RowId {
    val: isize,
}

impl Default for RowId {
    fn default() -> Self {
        Self { val: -1 }
    }
}

#[derive(Default, Clone)]
struct GrowingHashmapMapElemChar<ValueType> {
    key: u32,
    value: ValueType,
}

/// specialized hashmap to store user provided types
/// this implementation relies on a couple of base assumptions in order to simplify the implementation
/// - the hashmap does not have an upper limit of included items
/// - the default value for the `ValueType` can be used as a dummy value to indicate an empty cell
/// - elements can't be removed
/// - only allocates memory on first write access.
///   This improves performance for hashmaps that are never written to
struct GrowingHashmapChar<ValueType> {
    used: i32,
    fill: i32,
    mask: i32,
    map: Option<Vec<GrowingHashmapMapElemChar<ValueType>>>,
}

impl<ValueType> Default for GrowingHashmapChar<ValueType>
where
    ValueType: Default + Clone + Eq,
{
    fn default() -> Self {
        Self {
            used: 0,
            fill: 0,
            mask: -1,
            map: None,
        }
    }
}

impl<ValueType> GrowingHashmapChar<ValueType>
where
    ValueType: Default + Clone + Eq + Copy,
{
    fn get(&self, key: u32) -> ValueType {
        self.map
            .as_ref()
            .map_or_else(|| Default::default(), |map| map[self.lookup(key)].value)
    }

    fn get_mut(&mut self, key: u32) -> &mut ValueType {
        if self.map.is_none() {
            self.allocate();
        }

        let mut i = self.lookup(key);
        if self
            .map
            .as_ref()
            .expect("map should have been created above")[i]
            .value
            == Default::default()
        {
            self.fill += 1;
            // resize when 2/3 full
            if self.fill * 3 >= (self.mask + 1) * 2 {
                self.grow((self.used + 1) * 2);
                i = self.lookup(key);
            }

            self.used += 1;
        }

        let elem = &mut self
            .map
            .as_mut()
            .expect("map should have been created above")[i];
        elem.key = key;
        &mut elem.value
    }

    fn allocate(&mut self) {
        self.mask = 8 - 1;
        self.map = Some(vec![GrowingHashmapMapElemChar::default(); 8]);
    }

    /// lookup key inside the hashmap using a similar collision resolution
    /// strategy to `CPython` and `Ruby`
    fn lookup(&self, key: u32) -> usize {
        let hash = key;
        let mut i = hash as usize & self.mask as usize;

        let map = self
            .map
            .as_ref()
            .expect("callers have to ensure map is allocated");

        if map[i].value == Default::default() || map[i].key == key {
            return i;
        }

        let mut perturb = key;
        loop {
            i = (i * 5 + perturb as usize + 1) & self.mask as usize;

            if map[i].value == Default::default() || map[i].key == key {
                return i;
            }

            perturb >>= 5;
        }
    }

    fn grow(&mut self, min_used: i32) {
        let mut new_size = self.mask + 1;
        while new_size <= min_used {
            new_size <<= 1;
        }

        self.fill = self.used;
        self.mask = new_size - 1;

        let old_map = std::mem::replace(
            self.map
                .as_mut()
                .expect("callers have to ensure map is allocated"),
            vec![GrowingHashmapMapElemChar::<ValueType>::default(); new_size as usize],
        );

        for elem in old_map {
            if elem.value != Default::default() {
                let j = self.lookup(elem.key);
                let new_elem = &mut self.map.as_mut().expect("map created above")[j];
                new_elem.key = elem.key;
                new_elem.value = elem.value;
                self.used -= 1;
                if self.used == 0 {
                    break;
                }
            }
        }

        self.used = self.fill;
    }
}

struct HybridGrowingHashmapChar<ValueType> {
    map: GrowingHashmapChar<ValueType>,
    extended_ascii: [ValueType; 256],
}

impl<ValueType> HybridGrowingHashmapChar<ValueType>
where
    ValueType: Default + Clone + Copy + Eq,
{
    fn get(&self, key: char) -> ValueType {
        let value = key as u32;
        if value <= 255 {
            let val_u8 = u8::try_from(value).expect("we check the bounds above");
            self.extended_ascii[usize::from(val_u8)]
        } else {
            self.map.get(value)
        }
    }

    fn get_mut(&mut self, key: char) -> &mut ValueType {
        let value = key as u32;
        if value <= 255 {
            let val_u8 = u8::try_from(value).expect("we check the bounds above");
            &mut self.extended_ascii[usize::from(val_u8)]
        } else {
            self.map.get_mut(value)
        }
    }
}

impl<ValueType> Default for HybridGrowingHashmapChar<ValueType>
where
    ValueType: Default + Clone + Copy + Eq,
{
    fn default() -> Self {
        HybridGrowingHashmapChar {
            map: GrowingHashmapChar::default(),
            extended_ascii: [Default::default(); 256],
        }
    }
}

fn damerau_levenshtein_impl<Iter1, Iter2>(s1: Iter1, len1: usize, s2: Iter2, len2: usize) -> usize
where
    Iter1: Iterator<Item = char> + Clone,
    Iter2: Iterator<Item = char> + Clone,
{
    // The implementations is based on the paper
    // `Linear space string correction algorithm using the Damerau-Levenshtein distance`
    // from Chunchun Zhao and Sartaj Sahni
    //
    // It has a runtime complexity of `O(N*M)` and a memory usage of `O(N+M)`.
    let max_val = max(len1, len2) as isize + 1;

    let mut last_row_id = HybridGrowingHashmapChar::<RowId>::default();

    let size = len2 + 2;
    let mut fr = vec![max_val; size];
    let mut r1 = vec![max_val; size];
    let mut r: Vec<isize> = (max_val..max_val + 1)
        .chain(0..(size - 1) as isize)
        .collect();

    for (i, ch1) in s1.enumerate().map(|(i, ch1)| (i + 1, ch1)) {
        mem::swap(&mut r, &mut r1);
        let mut last_col_id: isize = -1;
        let mut last_i2l1 = r[1];
        r[1] = i as isize;
        let mut t = max_val;

        for (j, ch2) in s2.clone().enumerate().map(|(j, ch2)| (j + 1, ch2)) {
            let diag = r1[j] + isize::from(ch1 != ch2);
            let left = r[j] + 1;
            let up = r1[j + 1] + 1;
            let mut temp = min(diag, min(left, up));

            if ch1 == ch2 {
                last_col_id = j as isize; // last occurence of s1_i
                fr[j + 1] = r1[j - 1]; // save H_k-1,j-2
                t = last_i2l1; // save H_i-2,l-1
            } else {
                let k = last_row_id.get(ch2).val;
                let l = last_col_id;

                if j as isize - l == 1 {
                    let transpose = fr[j + 1] + (i as isize - k);
                    temp = min(temp, transpose);
                } else if i as isize - k == 1 {
                    let transpose = t + (j as isize - l);
                    temp = min(temp, transpose);
                }
            }

            last_i2l1 = r[j + 1];
            r[j + 1] = temp;
        }
        last_row_id.get_mut(ch1).val = i as isize;
    }

    r[len2 + 1] as usize
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
    damerau_levenshtein_impl(a.chars(), a.chars().count(), b.chars(), b.chars().count())
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

    let len1 = a.chars().count();
    let len2 = b.chars().count();
    let dist = damerau_levenshtein_impl(a.chars(), len1, b.chars(), len2);
    1.0 - (dist as f64) / (max(len1, len2) as f64)
}

/// Returns an Iterator of char tuples.
fn bigrams(s: &str) -> impl Iterator<Item = (char, char)> + '_ {
    s.chars().zip(s.chars().skip(1))
}

/// Calculates a Sørensen-Dice similarity distance using bigrams.
/// See <https://en.wikipedia.org/wiki/S%C3%B8rensen%E2%80%93Dice_coefficient>.
///
/// ```
/// use strsim::sorensen_dice;
///
/// assert_eq!(1.0, sorensen_dice("", ""));
/// assert_eq!(0.0, sorensen_dice("", "a"));
/// assert_eq!(0.0, sorensen_dice("french", "quebec"));
/// assert_eq!(1.0, sorensen_dice("ferris", "ferris"));
/// assert_eq!(0.8888888888888888, sorensen_dice("feris", "ferris"));
/// ```
pub fn sorensen_dice(a: &str, b: &str) -> f64 {
    // implementation guided by
    // https://github.com/aceakash/string-similarity/blob/f83ba3cd7bae874c20c429774e911ae8cff8bced/src/index.js#L6

    let a: String = a.chars().filter(|&x| !char::is_whitespace(x)).collect();
    let b: String = b.chars().filter(|&x| !char::is_whitespace(x)).collect();

    if a == b {
        return 1.0;
    }

    if a.len() < 2 || b.len() < 2 {
        return 0.0;
    }

    let mut a_bigrams: HashMap<(char, char), usize> = HashMap::new();

    for bigram in bigrams(&a) {
        *a_bigrams.entry(bigram).or_insert(0) += 1;
    }

    let mut intersection_size = 0_usize;

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

    macro_rules! assert_delta {
        ($x:expr, $y:expr) => {
            assert_delta!($x, $y, 1e-5);
        };
        ($x:expr, $y:expr, $d:expr) => {
            if ($x - $y).abs() > $d {
                panic!(
                    "assertion failed: actual: `{}`, expected: `{}`: \
                    actual not within < {} of expected",
                    $x, $y, $d
                );
            }
        };
    }

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
        assert_delta!(0.818, jaro("testabctest", "testöঙ香test"), 0.001);
        assert_delta!(0.818, jaro("testöঙ香test", "testabctest"), 0.001);
    }

    #[test]
    fn jaro_diff_short() {
        assert_delta!(0.767, jaro("dixon", "dicksonx"), 0.001);
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
        assert_delta!(0.83, jaro("a", "ab"), 0.01);
    }

    #[test]
    fn jaro_diff_two_and_one() {
        assert_delta!(0.83, jaro("ab", "a"), 0.01);
    }

    #[test]
    fn jaro_diff_no_transposition() {
        assert_delta!(0.822, jaro("dwayne", "duane"), 0.001);
    }

    #[test]
    fn jaro_diff_with_transposition() {
        assert_delta!(0.944, jaro("martha", "marhta"), 0.001);
        assert_delta!(0.6, jaro("a jke", "jane a k"), 0.001);
    }

    #[test]
    fn jaro_names() {
        assert_delta!(
            0.392,
            jaro("Friedrich Nietzsche", "Jean-Paul Sartre"),
            0.001
        );
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
        assert_delta!(0.89, jaro_winkler("testabctest", "testöঙ香test"), 0.001);
        assert_delta!(0.89, jaro_winkler("testöঙ香test", "testabctest"), 0.001);
    }

    #[test]
    fn jaro_winkler_diff_short() {
        assert_delta!(0.813, jaro_winkler("dixon", "dicksonx"), 0.001);
        assert_delta!(0.813, jaro_winkler("dicksonx", "dixon"), 0.001);
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
        assert_delta!(0.84, jaro_winkler("dwayne", "duane"), 0.001);
    }

    #[test]
    fn jaro_winkler_diff_with_transposition() {
        assert_delta!(0.961, jaro_winkler("martha", "marhta"), 0.001);
        assert_delta!(0.6, jaro_winkler("a jke", "jane a k"), 0.001);
    }

    #[test]
    fn jaro_winkler_names() {
        assert_delta!(
            0.452,
            jaro_winkler("Friedrich Nietzsche", "Fran-Paul Sartre"),
            0.001
        );
    }

    #[test]
    fn jaro_winkler_long_prefix() {
        assert_delta!(0.866, jaro_winkler("cheeseburger", "cheese fries"), 0.001);
    }

    #[test]
    fn jaro_winkler_more_names() {
        assert_delta!(0.868, jaro_winkler("Thorkel", "Thorgier"), 0.001);
    }

    #[test]
    fn jaro_winkler_length_of_one() {
        assert_delta!(0.738, jaro_winkler("Dinsdale", "D"), 0.001);
    }

    #[test]
    fn jaro_winkler_very_long_prefix() {
        assert_delta!(
            0.98519,
            jaro_winkler("thequickbrownfoxjumpedoverx", "thequickbrownfoxjumpedovery")
        );
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
        assert_delta!(0.57142, normalized_levenshtein("kitten", "sitting"));
    }

    #[test]
    fn normalized_levenshtein_for_empty_strings() {
        assert_delta!(1.0, normalized_levenshtein("", ""));
    }

    #[test]
    fn normalized_levenshtein_first_empty() {
        assert_delta!(0.0, normalized_levenshtein("", "second"));
    }

    #[test]
    fn normalized_levenshtein_second_empty() {
        assert_delta!(0.0, normalized_levenshtein("first", ""));
    }

    #[test]
    fn normalized_levenshtein_identical_strings() {
        assert_delta!(1.0, normalized_levenshtein("identical", "identical"));
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
        assert_delta!(
            0.27272,
            normalized_damerau_levenshtein("levenshtein", "löwenbräu")
        );
    }

    #[test]
    fn normalized_damerau_levenshtein_for_empty_strings() {
        assert_delta!(1.0, normalized_damerau_levenshtein("", ""));
    }

    #[test]
    fn normalized_damerau_levenshtein_first_empty() {
        assert_delta!(0.0, normalized_damerau_levenshtein("", "flower"));
    }

    #[test]
    fn normalized_damerau_levenshtein_second_empty() {
        assert_delta!(0.0, normalized_damerau_levenshtein("tree", ""));
    }

    #[test]
    fn normalized_damerau_levenshtein_identical_strings() {
        assert_delta!(
            1.0,
            normalized_damerau_levenshtein("sunglasses", "sunglasses")
        );
    }

    #[test]
    fn sorensen_dice_all() {
        // test cases taken from
        // https://github.com/aceakash/string-similarity/blob/f83ba3cd7bae874c20c429774e911ae8cff8bced/src/spec/index.spec.js#L11

        assert_delta!(1.0, sorensen_dice("a", "a"));
        assert_delta!(0.0, sorensen_dice("a", "b"));
        assert_delta!(1.0, sorensen_dice("", ""));
        assert_delta!(0.0, sorensen_dice("a", ""));
        assert_delta!(0.0, sorensen_dice("", "a"));
        assert_delta!(1.0, sorensen_dice("apple event", "apple    event"));
        assert_delta!(0.90909, sorensen_dice("iphone", "iphone x"));
        assert_delta!(0.0, sorensen_dice("french", "quebec"));
        assert_delta!(1.0, sorensen_dice("france", "france"));
        assert_delta!(0.2, sorensen_dice("fRaNce", "france"));
        assert_delta!(0.8, sorensen_dice("healed", "sealed"));
        assert_delta!(
            0.78788,
            sorensen_dice("web applications", "applications of the web")
        );
        assert_delta!(
            0.92,
            sorensen_dice(
                "this will have a typo somewhere",
                "this will huve a typo somewhere"
            )
        );
        assert_delta!(
            0.60606,
            sorensen_dice(
                "Olive-green table for sale, in extremely good condition.",
                "For sale: table in very good  condition, olive green in colour."
            )
        );
        assert_delta!(
            0.25581,
            sorensen_dice(
                "Olive-green table for sale, in extremely good condition.",
                "For sale: green Subaru Impreza, 210,000 miles"
            )
        );
        assert_delta!(
            0.14118,
            sorensen_dice(
                "Olive-green table for sale, in extremely good condition.",
                "Wanted: mountain bike with at least 21 gears."
            )
        );
        assert_delta!(
            0.77419,
            sorensen_dice("this has one extra word", "this has one word")
        );
    }
}
