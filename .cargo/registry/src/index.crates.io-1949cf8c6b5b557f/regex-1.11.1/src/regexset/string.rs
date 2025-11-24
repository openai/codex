use alloc::string::String;

use regex_automata::{meta, Input, PatternID, PatternSet, PatternSetIter};

use crate::{Error, RegexSetBuilder};

/// Match multiple, possibly overlapping, regexes in a single search.
///
/// A regex set corresponds to the union of zero or more regular expressions.
/// That is, a regex set will match a haystack when at least one of its
/// constituent regexes matches. A regex set as its formulated here provides a
/// touch more power: it will also report *which* regular expressions in the
/// set match. Indeed, this is the key difference between regex sets and a
/// single `Regex` with many alternates, since only one alternate can match at
/// a time.
///
/// For example, consider regular expressions to match email addresses and
/// domains: `[a-z]+@[a-z]+\.(com|org|net)` and `[a-z]+\.(com|org|net)`. If a
/// regex set is constructed from those regexes, then searching the haystack
/// `foo@example.com` will report both regexes as matching. Of course, one
/// could accomplish this by compiling each regex on its own and doing two
/// searches over the haystack. The key advantage of using a regex set is
/// that it will report the matching regexes using a *single pass through the
/// haystack*. If one has hundreds or thousands of regexes to match repeatedly
/// (like a URL router for a complex web application or a user agent matcher),
/// then a regex set *can* realize huge performance gains.
///
/// # Limitations
///
/// Regex sets are limited to answering the following two questions:
///
/// 1. Does any regex in the set match?
/// 2. If so, which regexes in the set match?
///
/// As with the main [`Regex`][crate::Regex] type, it is cheaper to ask (1)
/// instead of (2) since the matching engines can stop after the first match
/// is found.
///
/// You cannot directly extract [`Match`][crate::Match] or
/// [`Captures`][crate::Captures] objects from a regex set. If you need these
/// operations, the recommended approach is to compile each pattern in the set
/// independently and scan the exact same haystack a second time with those
/// independently compiled patterns:
///
/// ```
/// use regex::{Regex, RegexSet};
///
/// let patterns = ["foo", "bar"];
/// // Both patterns will match different ranges of this string.
/// let hay = "barfoo";
///
/// // Compile a set matching any of our patterns.
/// let set = RegexSet::new(patterns).unwrap();
/// // Compile each pattern independently.
/// let regexes: Vec<_> = set
///     .patterns()
///     .iter()
///     .map(|pat| Regex::new(pat).unwrap())
///     .collect();
///
/// // Match against the whole set first and identify the individual
/// // matching patterns.
/// let matches: Vec<&str> = set
///     .matches(hay)
///     .into_iter()
///     // Dereference the match index to get the corresponding
///     // compiled pattern.
///     .map(|index| &regexes[index])
///     // To get match locations or any other info, we then have to search the
///     // exact same haystack again, using our separately-compiled pattern.
///     .map(|re| re.find(hay).unwrap().as_str())
///     .collect();
///
/// // Matches arrive in the order the constituent patterns were declared,
/// // not the order they appear in the haystack.
/// assert_eq!(vec!["foo", "bar"], matches);
/// ```
///
/// # Performance
///
/// A `RegexSet` has the same performance characteristics as `Regex`. Namely,
/// search takes `O(m * n)` time, where `m` is proportional to the size of the
/// regex set and `n` is proportional to the length of the haystack.
///
/// # Trait implementations
///
/// The `Default` trait is implemented for `RegexSet`. The default value
/// is an empty set. An empty set can also be explicitly constructed via
/// [`RegexSet::empty`].
///
/// # Example
///
/// This shows how the above two regexes (for matching email addresses and
/// domains) might work:
///
/// ```
/// use regex::RegexSet;
///
/// let set = RegexSet::new(&[
///     r"[a-z]+@[a-z]+\.(com|org|net)",
///     r"[a-z]+\.(com|org|net)",
/// ]).unwrap();
///
/// // Ask whether any regexes in the set match.
/// assert!(set.is_match("foo@example.com"));
///
/// // Identify which regexes in the set match.
/// let matches: Vec<_> = set.matches("foo@example.com").into_iter().collect();
/// assert_eq!(vec![0, 1], matches);
///
/// // Try again, but with a haystack that only matches one of the regexes.
/// let matches: Vec<_> = set.matches("example.com").into_iter().collect();
/// assert_eq!(vec![1], matches);
///
/// // Try again, but with a haystack that doesn't match any regex in the set.
/// let matches: Vec<_> = set.matches("example").into_iter().collect();
/// assert!(matches.is_empty());
/// ```
///
/// Note that it would be possible to adapt the above example to using `Regex`
/// with an expression like:
///
/// ```text
/// (?P<email>[a-z]+@(?P<email_domain>[a-z]+[.](com|org|net)))|(?P<domain>[a-z]+[.](com|org|net))
/// ```
///
/// After a match, one could then inspect the capture groups to figure out
/// which alternates matched. The problem is that it is hard to make this
/// approach scale when there are many regexes since the overlap between each
/// alternate isn't always obvious to reason about.
#[derive(Clone)]
pub struct RegexSet {
    pub(crate) meta: meta::Regex,
    pub(crate) patterns: alloc::sync::Arc<[String]>,
}

impl RegexSet {
    /// Create a new regex set with the given regular expressions.
    ///
    /// This takes an iterator of `S`, where `S` is something that can produce
    /// a `&str`. If any of the strings in the iterator are not valid regular
    /// expressions, then an error is returned.
    ///
    /// # Example
    ///
    /// Create a new regex set from an iterator of strings:
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new([r"\w+", r"\d+"]).unwrap();
    /// assert!(set.is_match("foo"));
    /// ```
    pub fn new<I, S>(exprs: I) -> Result<RegexSet, Error>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        RegexSetBuilder::new(exprs).build()
    }

    /// Create a new empty regex set.
    ///
    /// An empty regex never matches anything.
    ///
    /// This is a convenience function for `RegexSet::new([])`, but doesn't
    /// require one to specify the type of the input.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::empty();
    /// assert!(set.is_empty());
    /// // an empty set matches nothing
    /// assert!(!set.is_match(""));
    /// ```
    pub fn empty() -> RegexSet {
        let empty: [&str; 0] = [];
        RegexSetBuilder::new(empty).build().unwrap()
    }

    /// Returns true if and only if one of the regexes in this set matches
    /// the haystack given.
    ///
    /// This method should be preferred if you only need to test whether any
    /// of the regexes in the set should match, but don't care about *which*
    /// regexes matched. This is because the underlying matching engine will
    /// quit immediately after seeing the first match instead of continuing to
    /// find all matches.
    ///
    /// Note that as with searches using [`Regex`](crate::Regex), the
    /// expression is unanchored by default. That is, if the regex does not
    /// start with `^` or `\A`, or end with `$` or `\z`, then it is permitted
    /// to match anywhere in the haystack.
    ///
    /// # Example
    ///
    /// Tests whether a set matches somewhere in a haystack:
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new([r"\w+", r"\d+"]).unwrap();
    /// assert!(set.is_match("foo"));
    /// assert!(!set.is_match("☃"));
    /// ```
    #[inline]
    pub fn is_match(&self, haystack: &str) -> bool {
        self.is_match_at(haystack, 0)
    }

    /// Returns true if and only if one of the regexes in this set matches the
    /// haystack given, with the search starting at the offset given.
    ///
    /// The significance of the starting point is that it takes the surrounding
    /// context into consideration. For example, the `\A` anchor can only
    /// match when `start == 0`.
    ///
    /// # Panics
    ///
    /// This panics when `start >= haystack.len() + 1`.
    ///
    /// # Example
    ///
    /// This example shows the significance of `start`. Namely, consider a
    /// haystack `foobar` and a desire to execute a search starting at offset
    /// `3`. You could search a substring explicitly, but then the look-around
    /// assertions won't work correctly. Instead, you can use this method to
    /// specify the start position of a search.
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new([r"\bbar\b", r"(?m)^bar$"]).unwrap();
    /// let hay = "foobar";
    /// // We get a match here, but it's probably not intended.
    /// assert!(set.is_match(&hay[3..]));
    /// // No match because the  assertions take the context into account.
    /// assert!(!set.is_match_at(hay, 3));
    /// ```
    #[inline]
    pub fn is_match_at(&self, haystack: &str, start: usize) -> bool {
        self.meta.is_match(Input::new(haystack).span(start..haystack.len()))
    }

    /// Returns the set of regexes that match in the given haystack.
    ///
    /// The set returned contains the index of each regex that matches in
    /// the given haystack. The index is in correspondence with the order of
    /// regular expressions given to `RegexSet`'s constructor.
    ///
    /// The set can also be used to iterate over the matched indices. The order
    /// of iteration is always ascending with respect to the matching indices.
    ///
    /// Note that as with searches using [`Regex`](crate::Regex), the
    /// expression is unanchored by default. That is, if the regex does not
    /// start with `^` or `\A`, or end with `$` or `\z`, then it is permitted
    /// to match anywhere in the haystack.
    ///
    /// # Example
    ///
    /// Tests which regular expressions match the given haystack:
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new([
    ///     r"\w+",
    ///     r"\d+",
    ///     r"\pL+",
    ///     r"foo",
    ///     r"bar",
    ///     r"barfoo",
    ///     r"foobar",
    /// ]).unwrap();
    /// let matches: Vec<_> = set.matches("foobar").into_iter().collect();
    /// assert_eq!(matches, vec![0, 2, 3, 4, 6]);
    ///
    /// // You can also test whether a particular regex matched:
    /// let matches = set.matches("foobar");
    /// assert!(!matches.matched(5));
    /// assert!(matches.matched(6));
    /// ```
    #[inline]
    pub fn matches(&self, haystack: &str) -> SetMatches {
        self.matches_at(haystack, 0)
    }

    /// Returns the set of regexes that match in the given haystack.
    ///
    /// The set returned contains the index of each regex that matches in
    /// the given haystack. The index is in correspondence with the order of
    /// regular expressions given to `RegexSet`'s constructor.
    ///
    /// The set can also be used to iterate over the matched indices. The order
    /// of iteration is always ascending with respect to the matching indices.
    ///
    /// The significance of the starting point is that it takes the surrounding
    /// context into consideration. For example, the `\A` anchor can only
    /// match when `start == 0`.
    ///
    /// # Panics
    ///
    /// This panics when `start >= haystack.len() + 1`.
    ///
    /// # Example
    ///
    /// Tests which regular expressions match the given haystack:
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new([r"\bbar\b", r"(?m)^bar$"]).unwrap();
    /// let hay = "foobar";
    /// // We get matches here, but it's probably not intended.
    /// let matches: Vec<_> = set.matches(&hay[3..]).into_iter().collect();
    /// assert_eq!(matches, vec![0, 1]);
    /// // No matches because the  assertions take the context into account.
    /// let matches: Vec<_> = set.matches_at(hay, 3).into_iter().collect();
    /// assert_eq!(matches, vec![]);
    /// ```
    #[inline]
    pub fn matches_at(&self, haystack: &str, start: usize) -> SetMatches {
        let input = Input::new(haystack).span(start..haystack.len());
        let mut patset = PatternSet::new(self.meta.pattern_len());
        self.meta.which_overlapping_matches(&input, &mut patset);
        SetMatches(patset)
    }

    /// Returns the same as matches, but starts the search at the given
    /// offset and stores the matches into the slice given.
    ///
    /// The significance of the starting point is that it takes the surrounding
    /// context into consideration. For example, the `\A` anchor can only
    /// match when `start == 0`.
    ///
    /// `matches` must have a length that is at least the number of regexes
    /// in this set.
    ///
    /// This method returns true if and only if at least one member of
    /// `matches` is true after executing the set against `haystack`.
    #[doc(hidden)]
    #[inline]
    pub fn matches_read_at(
        &self,
        matches: &mut [bool],
        haystack: &str,
        start: usize,
    ) -> bool {
        // This is pretty dumb. We should try to fix this, but the
        // regex-automata API doesn't provide a way to store matches in an
        // arbitrary &mut [bool]. Thankfully, this API is doc(hidden) and
        // thus not public... But regex-capi currently uses it. We should
        // fix regex-capi to use a PatternSet, maybe? Not sure... PatternSet
        // is in regex-automata, not regex. So maybe we should just accept a
        // 'SetMatches', which is basically just a newtype around PatternSet.
        let mut patset = PatternSet::new(self.meta.pattern_len());
        let mut input = Input::new(haystack);
        input.set_start(start);
        self.meta.which_overlapping_matches(&input, &mut patset);
        for pid in patset.iter() {
            matches[pid] = true;
        }
        !patset.is_empty()
    }

    /// An alias for `matches_read_at` to preserve backward compatibility.
    ///
    /// The `regex-capi` crate used this method, so to avoid breaking that
    /// crate, we continue to export it as an undocumented API.
    #[doc(hidden)]
    #[inline]
    pub fn read_matches_at(
        &self,
        matches: &mut [bool],
        haystack: &str,
        start: usize,
    ) -> bool {
        self.matches_read_at(matches, haystack, start)
    }

    /// Returns the total number of regexes in this set.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// assert_eq!(0, RegexSet::empty().len());
    /// assert_eq!(1, RegexSet::new([r"[0-9]"]).unwrap().len());
    /// assert_eq!(2, RegexSet::new([r"[0-9]", r"[a-z]"]).unwrap().len());
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.meta.pattern_len()
    }

    /// Returns `true` if this set contains no regexes.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// assert!(RegexSet::empty().is_empty());
    /// assert!(!RegexSet::new([r"[0-9]"]).unwrap().is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.meta.pattern_len() == 0
    }

    /// Returns the regex patterns that this regex set was constructed from.
    ///
    /// This function can be used to determine the pattern for a match. The
    /// slice returned has exactly as many patterns givens to this regex set,
    /// and the order of the slice is the same as the order of the patterns
    /// provided to the set.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new(&[
    ///     r"\w+",
    ///     r"\d+",
    ///     r"\pL+",
    ///     r"foo",
    ///     r"bar",
    ///     r"barfoo",
    ///     r"foobar",
    /// ]).unwrap();
    /// let matches: Vec<_> = set
    ///     .matches("foobar")
    ///     .into_iter()
    ///     .map(|index| &set.patterns()[index])
    ///     .collect();
    /// assert_eq!(matches, vec![r"\w+", r"\pL+", r"foo", r"bar", r"foobar"]);
    /// ```
    #[inline]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

impl Default for RegexSet {
    fn default() -> Self {
        RegexSet::empty()
    }
}

/// A set of matches returned by a regex set.
///
/// Values of this type are constructed by [`RegexSet::matches`].
#[derive(Clone, Debug)]
pub struct SetMatches(PatternSet);

impl SetMatches {
    /// Whether this set contains any matches.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new(&[
    ///     r"[a-z]+@[a-z]+\.(com|org|net)",
    ///     r"[a-z]+\.(com|org|net)",
    /// ]).unwrap();
    /// let matches = set.matches("foo@example.com");
    /// assert!(matches.matched_any());
    /// ```
    #[inline]
    pub fn matched_any(&self) -> bool {
        !self.0.is_empty()
    }

    /// Whether all patterns in this set matched.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new(&[
    ///     r"^foo",
    ///     r"[a-z]+\.com",
    /// ]).unwrap();
    /// let matches = set.matches("foo.example.com");
    /// assert!(matches.matched_all());
    /// ```
    pub fn matched_all(&self) -> bool {
        self.0.is_full()
    }

    /// Whether the regex at the given index matched.
    ///
    /// The index for a regex is determined by its insertion order upon the
    /// initial construction of a `RegexSet`, starting at `0`.
    ///
    /// # Panics
    ///
    /// If `index` is greater than or equal to the number of regexes in the
    /// original set that produced these matches. Equivalently, when `index`
    /// is greater than or equal to [`SetMatches::len`].
    ///
    /// # Example
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new([
    ///     r"[a-z]+@[a-z]+\.(com|org|net)",
    ///     r"[a-z]+\.(com|org|net)",
    /// ]).unwrap();
    /// let matches = set.matches("example.com");
    /// assert!(!matches.matched(0));
    /// assert!(matches.matched(1));
    /// ```
    #[inline]
    pub fn matched(&self, index: usize) -> bool {
        self.0.contains(PatternID::new_unchecked(index))
    }

    /// The total number of regexes in the set that created these matches.
    ///
    /// **WARNING:** This always returns the same value as [`RegexSet::len`].
    /// In particular, it does *not* return the number of elements yielded by
    /// [`SetMatches::iter`]. The only way to determine the total number of
    /// matched regexes is to iterate over them.
    ///
    /// # Example
    ///
    /// Notice that this method returns the total number of regexes in the
    /// original set, and *not* the total number of regexes that matched.
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new([
    ///     r"[a-z]+@[a-z]+\.(com|org|net)",
    ///     r"[a-z]+\.(com|org|net)",
    /// ]).unwrap();
    /// let matches = set.matches("example.com");
    /// // Total number of patterns that matched.
    /// assert_eq!(1, matches.iter().count());
    /// // Total number of patterns in the set.
    /// assert_eq!(2, matches.len());
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.0.capacity()
    }

    /// Returns an iterator over the indices of the regexes that matched.
    ///
    /// This will always produces matches in ascending order, where the index
    /// yielded corresponds to the index of the regex that matched with respect
    /// to its position when initially building the set.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new([
    ///     r"[0-9]",
    ///     r"[a-z]",
    ///     r"[A-Z]",
    ///     r"\p{Greek}",
    /// ]).unwrap();
    /// let hay = "βa1";
    /// let matches: Vec<_> = set.matches(hay).iter().collect();
    /// assert_eq!(matches, vec![0, 1, 3]);
    /// ```
    ///
    /// Note that `SetMatches` also implemnets the `IntoIterator` trait, so
    /// this method is not always needed. For example:
    ///
    /// ```
    /// use regex::RegexSet;
    ///
    /// let set = RegexSet::new([
    ///     r"[0-9]",
    ///     r"[a-z]",
    ///     r"[A-Z]",
    ///     r"\p{Greek}",
    /// ]).unwrap();
    /// let hay = "βa1";
    /// let mut matches = vec![];
    /// for index in set.matches(hay) {
    ///     matches.push(index);
    /// }
    /// assert_eq!(matches, vec![0, 1, 3]);
    /// ```
    #[inline]
    pub fn iter(&self) -> SetMatchesIter<'_> {
        SetMatchesIter(self.0.iter())
    }
}

impl IntoIterator for SetMatches {
    type IntoIter = SetMatchesIntoIter;
    type Item = usize;

    fn into_iter(self) -> Self::IntoIter {
        let it = 0..self.0.capacity();
        SetMatchesIntoIter { patset: self.0, it }
    }
}

impl<'a> IntoIterator for &'a SetMatches {
    type IntoIter = SetMatchesIter<'a>;
    type Item = usize;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An owned iterator over the set of matches from a regex set.
///
/// This will always produces matches in ascending order of index, where the
/// index corresponds to the index of the regex that matched with respect to
/// its position when initially building the set.
///
/// This iterator is created by calling `SetMatches::into_iter` via the
/// `IntoIterator` trait. This is automatically done in `for` loops.
///
/// # Example
///
/// ```
/// use regex::RegexSet;
///
/// let set = RegexSet::new([
///     r"[0-9]",
///     r"[a-z]",
///     r"[A-Z]",
///     r"\p{Greek}",
/// ]).unwrap();
/// let hay = "βa1";
/// let mut matches = vec![];
/// for index in set.matches(hay) {
///     matches.push(index);
/// }
/// assert_eq!(matches, vec![0, 1, 3]);
/// ```
#[derive(Debug)]
pub struct SetMatchesIntoIter {
    patset: PatternSet,
    it: core::ops::Range<usize>,
}

impl Iterator for SetMatchesIntoIter {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        loop {
            let id = self.it.next()?;
            if self.patset.contains(PatternID::new_unchecked(id)) {
                return Some(id);
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl DoubleEndedIterator for SetMatchesIntoIter {
    fn next_back(&mut self) -> Option<usize> {
        loop {
            let id = self.it.next_back()?;
            if self.patset.contains(PatternID::new_unchecked(id)) {
                return Some(id);
            }
        }
    }
}

impl core::iter::FusedIterator for SetMatchesIntoIter {}

/// A borrowed iterator over the set of matches from a regex set.
///
/// The lifetime `'a` refers to the lifetime of the [`SetMatches`] value that
/// created this iterator.
///
/// This will always produces matches in ascending order, where the index
/// corresponds to the index of the regex that matched with respect to its
/// position when initially building the set.
///
/// This iterator is created by the [`SetMatches::iter`] method.
#[derive(Clone, Debug)]
pub struct SetMatchesIter<'a>(PatternSetIter<'a>);

impl<'a> Iterator for SetMatchesIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        self.0.next().map(|pid| pid.as_usize())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a> DoubleEndedIterator for SetMatchesIter<'a> {
    fn next_back(&mut self) -> Option<usize> {
        self.0.next_back().map(|pid| pid.as_usize())
    }
}

impl<'a> core::iter::FusedIterator for SetMatchesIter<'a> {}

impl core::fmt::Debug for RegexSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "RegexSet({:?})", self.patterns())
    }
}
