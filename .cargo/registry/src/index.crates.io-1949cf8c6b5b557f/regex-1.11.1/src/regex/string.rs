use alloc::{borrow::Cow, string::String, sync::Arc};

use regex_automata::{meta, util::captures, Input, PatternID};

use crate::{error::Error, RegexBuilder};

/// A compiled regular expression for searching Unicode haystacks.
///
/// A `Regex` can be used to search haystacks, split haystacks into substrings
/// or replace substrings in a haystack with a different substring. All
/// searching is done with an implicit `(?s:.)*?` at the beginning and end of
/// an pattern. To force an expression to match the whole string (or a prefix
/// or a suffix), you must use an anchor like `^` or `$` (or `\A` and `\z`).
///
/// While this crate will handle Unicode strings (whether in the regular
/// expression or in the haystack), all positions returned are **byte
/// offsets**. Every byte offset is guaranteed to be at a Unicode code point
/// boundary. That is, all offsets returned by the `Regex` API are guaranteed
/// to be ranges that can slice a `&str` without panicking. If you want to
/// relax this requirement, then you must search `&[u8]` haystacks with a
/// [`bytes::Regex`](crate::bytes::Regex).
///
/// The only methods that allocate new strings are the string replacement
/// methods. All other methods (searching and splitting) return borrowed
/// references into the haystack given.
///
/// # Example
///
/// Find the offsets of a US phone number:
///
/// ```
/// use regex::Regex;
///
/// let re = Regex::new("[0-9]{3}-[0-9]{3}-[0-9]{4}").unwrap();
/// let m = re.find("phone: 111-222-3333").unwrap();
/// assert_eq!(7..19, m.range());
/// ```
///
/// # Example: extracting capture groups
///
/// A common way to use regexes is with capture groups. That is, instead of
/// just looking for matches of an entire regex, parentheses are used to create
/// groups that represent part of the match.
///
/// For example, consider a haystack with multiple lines, and each line has
/// three whitespace delimited fields where the second field is expected to be
/// a number and the third field a boolean. To make this convenient, we use
/// the [`Captures::extract`] API to put the strings that match each group
/// into a fixed size array:
///
/// ```
/// use regex::Regex;
///
/// let hay = "
/// rabbit         54 true
/// groundhog 2 true
/// does not match
/// fox   109    false
/// ";
/// let re = Regex::new(r"(?m)^\s*(\S+)\s+([0-9]+)\s+(true|false)\s*$").unwrap();
/// let mut fields: Vec<(&str, i64, bool)> = vec![];
/// for (_, [f1, f2, f3]) in re.captures_iter(hay).map(|caps| caps.extract()) {
///     fields.push((f1, f2.parse()?, f3.parse()?));
/// }
/// assert_eq!(fields, vec![
///     ("rabbit", 54, true),
///     ("groundhog", 2, true),
///     ("fox", 109, false),
/// ]);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: searching with the `Pattern` trait
///
/// **Note**: This section requires that this crate is compiled with the
/// `pattern` Cargo feature enabled, which **requires nightly Rust**.
///
/// Since `Regex` implements `Pattern` from the standard library, one can
/// use regexes with methods defined on `&str`. For example, `is_match`,
/// `find`, `find_iter` and `split` can, in some cases, be replaced with
/// `str::contains`, `str::find`, `str::match_indices` and `str::split`.
///
/// Here are some examples:
///
/// ```ignore
/// use regex::Regex;
///
/// let re = Regex::new(r"\d+").unwrap();
/// let hay = "a111b222c";
///
/// assert!(hay.contains(&re));
/// assert_eq!(hay.find(&re), Some(1));
/// assert_eq!(hay.match_indices(&re).collect::<Vec<_>>(), vec![
///     (1, "111"),
///     (5, "222"),
/// ]);
/// assert_eq!(hay.split(&re).collect::<Vec<_>>(), vec!["a", "b", "c"]);
/// ```
#[derive(Clone)]
pub struct Regex {
    pub(crate) meta: meta::Regex,
    pub(crate) pattern: Arc<str>,
}

impl core::fmt::Display for Regex {
    /// Shows the original regular expression.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl core::fmt::Debug for Regex {
    /// Shows the original regular expression.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Regex").field(&self.as_str()).finish()
    }
}

impl core::str::FromStr for Regex {
    type Err = Error;

    /// Attempts to parse a string into a regular expression
    fn from_str(s: &str) -> Result<Regex, Error> {
        Regex::new(s)
    }
}

impl TryFrom<&str> for Regex {
    type Error = Error;

    /// Attempts to parse a string into a regular expression
    fn try_from(s: &str) -> Result<Regex, Error> {
        Regex::new(s)
    }
}

impl TryFrom<String> for Regex {
    type Error = Error;

    /// Attempts to parse a string into a regular expression
    fn try_from(s: String) -> Result<Regex, Error> {
        Regex::new(&s)
    }
}

/// Core regular expression methods.
impl Regex {
    /// Compiles a regular expression. Once compiled, it can be used repeatedly
    /// to search, split or replace substrings in a haystack.
    ///
    /// Note that regex compilation tends to be a somewhat expensive process,
    /// and unlike higher level environments, compilation is not automatically
    /// cached for you. One should endeavor to compile a regex once and then
    /// reuse it. For example, it's a bad idea to compile the same regex
    /// repeatedly in a loop.
    ///
    /// # Errors
    ///
    /// If an invalid pattern is given, then an error is returned.
    /// An error is also returned if the pattern is valid, but would
    /// produce a regex that is bigger than the configured size limit via
    /// [`RegexBuilder::size_limit`]. (A reasonable size limit is enabled by
    /// default.)
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// // An Invalid pattern because of an unclosed parenthesis
    /// assert!(Regex::new(r"foo(bar").is_err());
    /// // An invalid pattern because the regex would be too big
    /// // because Unicode tends to inflate things.
    /// assert!(Regex::new(r"\w{1000}").is_err());
    /// // Disabling Unicode can make the regex much smaller,
    /// // potentially by up to or more than an order of magnitude.
    /// assert!(Regex::new(r"(?-u:\w){1000}").is_ok());
    /// ```
    pub fn new(re: &str) -> Result<Regex, Error> {
        RegexBuilder::new(re).build()
    }

    /// Returns true if and only if there is a match for the regex anywhere
    /// in the haystack given.
    ///
    /// It is recommended to use this method if all you need to do is test
    /// whether a match exists, since the underlying matching engine may be
    /// able to do less work.
    ///
    /// # Example
    ///
    /// Test if some haystack contains at least one word with exactly 13
    /// Unicode word characters:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"\b\w{13}\b").unwrap();
    /// let hay = "I categorically deny having triskaidekaphobia.";
    /// assert!(re.is_match(hay));
    /// ```
    #[inline]
    pub fn is_match(&self, haystack: &str) -> bool {
        self.is_match_at(haystack, 0)
    }

    /// This routine searches for the first match of this regex in the
    /// haystack given, and if found, returns a [`Match`]. The `Match`
    /// provides access to both the byte offsets of the match and the actual
    /// substring that matched.
    ///
    /// Note that this should only be used if you want to find the entire
    /// match. If instead you just want to test the existence of a match,
    /// it's potentially faster to use `Regex::is_match(hay)` instead of
    /// `Regex::find(hay).is_some()`.
    ///
    /// # Example
    ///
    /// Find the first word with exactly 13 Unicode word characters:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"\b\w{13}\b").unwrap();
    /// let hay = "I categorically deny having triskaidekaphobia.";
    /// let mat = re.find(hay).unwrap();
    /// assert_eq!(2..15, mat.range());
    /// assert_eq!("categorically", mat.as_str());
    /// ```
    #[inline]
    pub fn find<'h>(&self, haystack: &'h str) -> Option<Match<'h>> {
        self.find_at(haystack, 0)
    }

    /// Returns an iterator that yields successive non-overlapping matches in
    /// the given haystack. The iterator yields values of type [`Match`].
    ///
    /// # Time complexity
    ///
    /// Note that since `find_iter` runs potentially many searches on the
    /// haystack and since each search has worst case `O(m * n)` time
    /// complexity, the overall worst case time complexity for iteration is
    /// `O(m * n^2)`.
    ///
    /// # Example
    ///
    /// Find every word with exactly 13 Unicode word characters:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"\b\w{13}\b").unwrap();
    /// let hay = "Retroactively relinquishing remunerations is reprehensible.";
    /// let matches: Vec<_> = re.find_iter(hay).map(|m| m.as_str()).collect();
    /// assert_eq!(matches, vec![
    ///     "Retroactively",
    ///     "relinquishing",
    ///     "remunerations",
    ///     "reprehensible",
    /// ]);
    /// ```
    #[inline]
    pub fn find_iter<'r, 'h>(&'r self, haystack: &'h str) -> Matches<'r, 'h> {
        Matches { haystack, it: self.meta.find_iter(haystack) }
    }

    /// This routine searches for the first match of this regex in the haystack
    /// given, and if found, returns not only the overall match but also the
    /// matches of each capture group in the regex. If no match is found, then
    /// `None` is returned.
    ///
    /// Capture group `0` always corresponds to an implicit unnamed group that
    /// includes the entire match. If a match is found, this group is always
    /// present. Subsequent groups may be named and are numbered, starting
    /// at 1, by the order in which the opening parenthesis appears in the
    /// pattern. For example, in the pattern `(?<a>.(?<b>.))(?<c>.)`, `a`,
    /// `b` and `c` correspond to capture group indices `1`, `2` and `3`,
    /// respectively.
    ///
    /// You should only use `captures` if you need access to the capture group
    /// matches. Otherwise, [`Regex::find`] is generally faster for discovering
    /// just the overall match.
    ///
    /// # Example
    ///
    /// Say you have some haystack with movie names and their release years,
    /// like "'Citizen Kane' (1941)". It'd be nice if we could search for
    /// substrings looking like that, while also extracting the movie name and
    /// its release year separately. The example below shows how to do that.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"'([^']+)'\s+\((\d{4})\)").unwrap();
    /// let hay = "Not my favorite movie: 'Citizen Kane' (1941).";
    /// let caps = re.captures(hay).unwrap();
    /// assert_eq!(caps.get(0).unwrap().as_str(), "'Citizen Kane' (1941)");
    /// assert_eq!(caps.get(1).unwrap().as_str(), "Citizen Kane");
    /// assert_eq!(caps.get(2).unwrap().as_str(), "1941");
    /// // You can also access the groups by index using the Index notation.
    /// // Note that this will panic on an invalid index. In this case, these
    /// // accesses are always correct because the overall regex will only
    /// // match when these capture groups match.
    /// assert_eq!(&caps[0], "'Citizen Kane' (1941)");
    /// assert_eq!(&caps[1], "Citizen Kane");
    /// assert_eq!(&caps[2], "1941");
    /// ```
    ///
    /// Note that the full match is at capture group `0`. Each subsequent
    /// capture group is indexed by the order of its opening `(`.
    ///
    /// We can make this example a bit clearer by using *named* capture groups:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"'(?<title>[^']+)'\s+\((?<year>\d{4})\)").unwrap();
    /// let hay = "Not my favorite movie: 'Citizen Kane' (1941).";
    /// let caps = re.captures(hay).unwrap();
    /// assert_eq!(caps.get(0).unwrap().as_str(), "'Citizen Kane' (1941)");
    /// assert_eq!(caps.name("title").unwrap().as_str(), "Citizen Kane");
    /// assert_eq!(caps.name("year").unwrap().as_str(), "1941");
    /// // You can also access the groups by name using the Index notation.
    /// // Note that this will panic on an invalid group name. In this case,
    /// // these accesses are always correct because the overall regex will
    /// // only match when these capture groups match.
    /// assert_eq!(&caps[0], "'Citizen Kane' (1941)");
    /// assert_eq!(&caps["title"], "Citizen Kane");
    /// assert_eq!(&caps["year"], "1941");
    /// ```
    ///
    /// Here we name the capture groups, which we can access with the `name`
    /// method or the `Index` notation with a `&str`. Note that the named
    /// capture groups are still accessible with `get` or the `Index` notation
    /// with a `usize`.
    ///
    /// The `0`th capture group is always unnamed, so it must always be
    /// accessed with `get(0)` or `[0]`.
    ///
    /// Finally, one other way to to get the matched substrings is with the
    /// [`Captures::extract`] API:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"'([^']+)'\s+\((\d{4})\)").unwrap();
    /// let hay = "Not my favorite movie: 'Citizen Kane' (1941).";
    /// let (full, [title, year]) = re.captures(hay).unwrap().extract();
    /// assert_eq!(full, "'Citizen Kane' (1941)");
    /// assert_eq!(title, "Citizen Kane");
    /// assert_eq!(year, "1941");
    /// ```
    #[inline]
    pub fn captures<'h>(&self, haystack: &'h str) -> Option<Captures<'h>> {
        self.captures_at(haystack, 0)
    }

    /// Returns an iterator that yields successive non-overlapping matches in
    /// the given haystack. The iterator yields values of type [`Captures`].
    ///
    /// This is the same as [`Regex::find_iter`], but instead of only providing
    /// access to the overall match, each value yield includes access to the
    /// matches of all capture groups in the regex. Reporting this extra match
    /// data is potentially costly, so callers should only use `captures_iter`
    /// over `find_iter` when they actually need access to the capture group
    /// matches.
    ///
    /// # Time complexity
    ///
    /// Note that since `captures_iter` runs potentially many searches on the
    /// haystack and since each search has worst case `O(m * n)` time
    /// complexity, the overall worst case time complexity for iteration is
    /// `O(m * n^2)`.
    ///
    /// # Example
    ///
    /// We can use this to find all movie titles and their release years in
    /// some haystack, where the movie is formatted like "'Title' (xxxx)":
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"'([^']+)'\s+\(([0-9]{4})\)").unwrap();
    /// let hay = "'Citizen Kane' (1941), 'The Wizard of Oz' (1939), 'M' (1931).";
    /// let mut movies = vec![];
    /// for (_, [title, year]) in re.captures_iter(hay).map(|c| c.extract()) {
    ///     movies.push((title, year.parse::<i64>()?));
    /// }
    /// assert_eq!(movies, vec![
    ///     ("Citizen Kane", 1941),
    ///     ("The Wizard of Oz", 1939),
    ///     ("M", 1931),
    /// ]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Or with named groups:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"'(?<title>[^']+)'\s+\((?<year>[0-9]{4})\)").unwrap();
    /// let hay = "'Citizen Kane' (1941), 'The Wizard of Oz' (1939), 'M' (1931).";
    /// let mut it = re.captures_iter(hay);
    ///
    /// let caps = it.next().unwrap();
    /// assert_eq!(&caps["title"], "Citizen Kane");
    /// assert_eq!(&caps["year"], "1941");
    ///
    /// let caps = it.next().unwrap();
    /// assert_eq!(&caps["title"], "The Wizard of Oz");
    /// assert_eq!(&caps["year"], "1939");
    ///
    /// let caps = it.next().unwrap();
    /// assert_eq!(&caps["title"], "M");
    /// assert_eq!(&caps["year"], "1931");
    /// ```
    #[inline]
    pub fn captures_iter<'r, 'h>(
        &'r self,
        haystack: &'h str,
    ) -> CaptureMatches<'r, 'h> {
        CaptureMatches { haystack, it: self.meta.captures_iter(haystack) }
    }

    /// Returns an iterator of substrings of the haystack given, delimited by a
    /// match of the regex. Namely, each element of the iterator corresponds to
    /// a part of the haystack that *isn't* matched by the regular expression.
    ///
    /// # Time complexity
    ///
    /// Since iterators over all matches requires running potentially many
    /// searches on the haystack, and since each search has worst case
    /// `O(m * n)` time complexity, the overall worst case time complexity for
    /// this routine is `O(m * n^2)`.
    ///
    /// # Example
    ///
    /// To split a string delimited by arbitrary amounts of spaces or tabs:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"[ \t]+").unwrap();
    /// let hay = "a b \t  c\td    e";
    /// let fields: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(fields, vec!["a", "b", "c", "d", "e"]);
    /// ```
    ///
    /// # Example: more cases
    ///
    /// Basic usage:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r" ").unwrap();
    /// let hay = "Mary had a little lamb";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec!["Mary", "had", "a", "little", "lamb"]);
    ///
    /// let re = Regex::new(r"X").unwrap();
    /// let hay = "";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec![""]);
    ///
    /// let re = Regex::new(r"X").unwrap();
    /// let hay = "lionXXtigerXleopard";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec!["lion", "", "tiger", "leopard"]);
    ///
    /// let re = Regex::new(r"::").unwrap();
    /// let hay = "lion::tiger::leopard";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec!["lion", "tiger", "leopard"]);
    /// ```
    ///
    /// If a haystack contains multiple contiguous matches, you will end up
    /// with empty spans yielded by the iterator:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"X").unwrap();
    /// let hay = "XXXXaXXbXc";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec!["", "", "", "", "a", "", "b", "c"]);
    ///
    /// let re = Regex::new(r"/").unwrap();
    /// let hay = "(///)";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec!["(", "", "", ")"]);
    /// ```
    ///
    /// Separators at the start or end of a haystack are neighbored by empty
    /// substring.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"0").unwrap();
    /// let hay = "010";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec!["", "1", ""]);
    /// ```
    ///
    /// When the empty string is used as a regex, it splits at every valid
    /// UTF-8 boundary by default (which includes the beginning and end of the
    /// haystack):
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"").unwrap();
    /// let hay = "rust";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec!["", "r", "u", "s", "t", ""]);
    ///
    /// // Splitting by an empty string is UTF-8 aware by default!
    /// let re = Regex::new(r"").unwrap();
    /// let hay = "☃";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec!["", "☃", ""]);
    /// ```
    ///
    /// Contiguous separators (commonly shows up with whitespace), can lead to
    /// possibly surprising behavior. For example, this code is correct:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r" ").unwrap();
    /// let hay = "    a  b c";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// assert_eq!(got, vec!["", "", "", "", "a", "", "b", "c"]);
    /// ```
    ///
    /// It does *not* give you `["a", "b", "c"]`. For that behavior, you'd want
    /// to match contiguous space characters:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r" +").unwrap();
    /// let hay = "    a  b c";
    /// let got: Vec<&str> = re.split(hay).collect();
    /// // N.B. This does still include a leading empty span because ' +'
    /// // matches at the beginning of the haystack.
    /// assert_eq!(got, vec!["", "a", "b", "c"]);
    /// ```
    #[inline]
    pub fn split<'r, 'h>(&'r self, haystack: &'h str) -> Split<'r, 'h> {
        Split { haystack, it: self.meta.split(haystack) }
    }

    /// Returns an iterator of at most `limit` substrings of the haystack
    /// given, delimited by a match of the regex. (A `limit` of `0` will return
    /// no substrings.) Namely, each element of the iterator corresponds to a
    /// part of the haystack that *isn't* matched by the regular expression.
    /// The remainder of the haystack that is not split will be the last
    /// element in the iterator.
    ///
    /// # Time complexity
    ///
    /// Since iterators over all matches requires running potentially many
    /// searches on the haystack, and since each search has worst case
    /// `O(m * n)` time complexity, the overall worst case time complexity for
    /// this routine is `O(m * n^2)`.
    ///
    /// Although note that the worst case time here has an upper bound given
    /// by the `limit` parameter.
    ///
    /// # Example
    ///
    /// Get the first two words in some haystack:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"\W+").unwrap();
    /// let hay = "Hey! How are you?";
    /// let fields: Vec<&str> = re.splitn(hay, 3).collect();
    /// assert_eq!(fields, vec!["Hey", "How", "are you?"]);
    /// ```
    ///
    /// # Examples: more cases
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r" ").unwrap();
    /// let hay = "Mary had a little lamb";
    /// let got: Vec<&str> = re.splitn(hay, 3).collect();
    /// assert_eq!(got, vec!["Mary", "had", "a little lamb"]);
    ///
    /// let re = Regex::new(r"X").unwrap();
    /// let hay = "";
    /// let got: Vec<&str> = re.splitn(hay, 3).collect();
    /// assert_eq!(got, vec![""]);
    ///
    /// let re = Regex::new(r"X").unwrap();
    /// let hay = "lionXXtigerXleopard";
    /// let got: Vec<&str> = re.splitn(hay, 3).collect();
    /// assert_eq!(got, vec!["lion", "", "tigerXleopard"]);
    ///
    /// let re = Regex::new(r"::").unwrap();
    /// let hay = "lion::tiger::leopard";
    /// let got: Vec<&str> = re.splitn(hay, 2).collect();
    /// assert_eq!(got, vec!["lion", "tiger::leopard"]);
    ///
    /// let re = Regex::new(r"X").unwrap();
    /// let hay = "abcXdef";
    /// let got: Vec<&str> = re.splitn(hay, 1).collect();
    /// assert_eq!(got, vec!["abcXdef"]);
    ///
    /// let re = Regex::new(r"X").unwrap();
    /// let hay = "abcdef";
    /// let got: Vec<&str> = re.splitn(hay, 2).collect();
    /// assert_eq!(got, vec!["abcdef"]);
    ///
    /// let re = Regex::new(r"X").unwrap();
    /// let hay = "abcXdef";
    /// let got: Vec<&str> = re.splitn(hay, 0).collect();
    /// assert!(got.is_empty());
    /// ```
    #[inline]
    pub fn splitn<'r, 'h>(
        &'r self,
        haystack: &'h str,
        limit: usize,
    ) -> SplitN<'r, 'h> {
        SplitN { haystack, it: self.meta.splitn(haystack, limit) }
    }

    /// Replaces the leftmost-first match in the given haystack with the
    /// replacement provided. The replacement can be a regular string (where
    /// `$N` and `$name` are expanded to match capture groups) or a function
    /// that takes a [`Captures`] and returns the replaced string.
    ///
    /// If no match is found, then the haystack is returned unchanged. In that
    /// case, this implementation will likely return a `Cow::Borrowed` value
    /// such that no allocation is performed.
    ///
    /// When a `Cow::Borrowed` is returned, the value returned is guaranteed
    /// to be equivalent to the `haystack` given.
    ///
    /// # Replacement string syntax
    ///
    /// All instances of `$ref` in the replacement string are replaced with
    /// the substring corresponding to the capture group identified by `ref`.
    ///
    /// `ref` may be an integer corresponding to the index of the capture group
    /// (counted by order of opening parenthesis where `0` is the entire match)
    /// or it can be a name (consisting of letters, digits or underscores)
    /// corresponding to a named capture group.
    ///
    /// If `ref` isn't a valid capture group (whether the name doesn't exist or
    /// isn't a valid index), then it is replaced with the empty string.
    ///
    /// The longest possible name is used. For example, `$1a` looks up the
    /// capture group named `1a` and not the capture group at index `1`. To
    /// exert more precise control over the name, use braces, e.g., `${1}a`.
    ///
    /// To write a literal `$` use `$$`.
    ///
    /// # Example
    ///
    /// Note that this function is polymorphic with respect to the replacement.
    /// In typical usage, this can just be a normal string:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"[^01]+").unwrap();
    /// assert_eq!(re.replace("1078910", ""), "1010");
    /// ```
    ///
    /// But anything satisfying the [`Replacer`] trait will work. For example,
    /// a closure of type `|&Captures| -> String` provides direct access to the
    /// captures corresponding to a match. This allows one to access capturing
    /// group matches easily:
    ///
    /// ```
    /// use regex::{Captures, Regex};
    ///
    /// let re = Regex::new(r"([^,\s]+),\s+(\S+)").unwrap();
    /// let result = re.replace("Springsteen, Bruce", |caps: &Captures| {
    ///     format!("{} {}", &caps[2], &caps[1])
    /// });
    /// assert_eq!(result, "Bruce Springsteen");
    /// ```
    ///
    /// But this is a bit cumbersome to use all the time. Instead, a simple
    /// syntax is supported (as described above) that expands `$name` into the
    /// corresponding capture group. Here's the last example, but using this
    /// expansion technique with named capture groups:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(?<last>[^,\s]+),\s+(?<first>\S+)").unwrap();
    /// let result = re.replace("Springsteen, Bruce", "$first $last");
    /// assert_eq!(result, "Bruce Springsteen");
    /// ```
    ///
    /// Note that using `$2` instead of `$first` or `$1` instead of `$last`
    /// would produce the same result. To write a literal `$` use `$$`.
    ///
    /// Sometimes the replacement string requires use of curly braces to
    /// delineate a capture group replacement when it is adjacent to some other
    /// literal text. For example, if we wanted to join two words together with
    /// an underscore:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(?<first>\w+)\s+(?<second>\w+)").unwrap();
    /// let result = re.replace("deep fried", "${first}_$second");
    /// assert_eq!(result, "deep_fried");
    /// ```
    ///
    /// Without the curly braces, the capture group name `first_` would be
    /// used, and since it doesn't exist, it would be replaced with the empty
    /// string.
    ///
    /// Finally, sometimes you just want to replace a literal string with no
    /// regard for capturing group expansion. This can be done by wrapping a
    /// string with [`NoExpand`]:
    ///
    /// ```
    /// use regex::{NoExpand, Regex};
    ///
    /// let re = Regex::new(r"(?<last>[^,\s]+),\s+(\S+)").unwrap();
    /// let result = re.replace("Springsteen, Bruce", NoExpand("$2 $last"));
    /// assert_eq!(result, "$2 $last");
    /// ```
    ///
    /// Using `NoExpand` may also be faster, since the replacement string won't
    /// need to be parsed for the `$` syntax.
    #[inline]
    pub fn replace<'h, R: Replacer>(
        &self,
        haystack: &'h str,
        rep: R,
    ) -> Cow<'h, str> {
        self.replacen(haystack, 1, rep)
    }

    /// Replaces all non-overlapping matches in the haystack with the
    /// replacement provided. This is the same as calling `replacen` with
    /// `limit` set to `0`.
    ///
    /// If no match is found, then the haystack is returned unchanged. In that
    /// case, this implementation will likely return a `Cow::Borrowed` value
    /// such that no allocation is performed.
    ///
    /// When a `Cow::Borrowed` is returned, the value returned is guaranteed
    /// to be equivalent to the `haystack` given.
    ///
    /// The documentation for [`Regex::replace`] goes into more detail about
    /// what kinds of replacement strings are supported.
    ///
    /// # Time complexity
    ///
    /// Since iterators over all matches requires running potentially many
    /// searches on the haystack, and since each search has worst case
    /// `O(m * n)` time complexity, the overall worst case time complexity for
    /// this routine is `O(m * n^2)`.
    ///
    /// # Fallibility
    ///
    /// If you need to write a replacement routine where any individual
    /// replacement might "fail," doing so with this API isn't really feasible
    /// because there's no way to stop the search process if a replacement
    /// fails. Instead, if you need this functionality, you should consider
    /// implementing your own replacement routine:
    ///
    /// ```
    /// use regex::{Captures, Regex};
    ///
    /// fn replace_all<E>(
    ///     re: &Regex,
    ///     haystack: &str,
    ///     replacement: impl Fn(&Captures) -> Result<String, E>,
    /// ) -> Result<String, E> {
    ///     let mut new = String::with_capacity(haystack.len());
    ///     let mut last_match = 0;
    ///     for caps in re.captures_iter(haystack) {
    ///         let m = caps.get(0).unwrap();
    ///         new.push_str(&haystack[last_match..m.start()]);
    ///         new.push_str(&replacement(&caps)?);
    ///         last_match = m.end();
    ///     }
    ///     new.push_str(&haystack[last_match..]);
    ///     Ok(new)
    /// }
    ///
    /// // Let's replace each word with the number of bytes in that word.
    /// // But if we see a word that is "too long," we'll give up.
    /// let re = Regex::new(r"\w+").unwrap();
    /// let replacement = |caps: &Captures| -> Result<String, &'static str> {
    ///     if caps[0].len() >= 5 {
    ///         return Err("word too long");
    ///     }
    ///     Ok(caps[0].len().to_string())
    /// };
    /// assert_eq!(
    ///     Ok("2 3 3 3?".to_string()),
    ///     replace_all(&re, "hi how are you?", &replacement),
    /// );
    /// assert!(replace_all(&re, "hi there", &replacement).is_err());
    /// ```
    ///
    /// # Example
    ///
    /// This example shows how to flip the order of whitespace (excluding line
    /// terminators) delimited fields, and normalizes the whitespace that
    /// delimits the fields:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(?m)^(\S+)[\s--\r\n]+(\S+)$").unwrap();
    /// let hay = "
    /// Greetings  1973
    /// Wild\t1973
    /// BornToRun\t\t\t\t1975
    /// Darkness                    1978
    /// TheRiver 1980
    /// ";
    /// let new = re.replace_all(hay, "$2 $1");
    /// assert_eq!(new, "
    /// 1973 Greetings
    /// 1973 Wild
    /// 1975 BornToRun
    /// 1978 Darkness
    /// 1980 TheRiver
    /// ");
    /// ```
    #[inline]
    pub fn replace_all<'h, R: Replacer>(
        &self,
        haystack: &'h str,
        rep: R,
    ) -> Cow<'h, str> {
        self.replacen(haystack, 0, rep)
    }

    /// Replaces at most `limit` non-overlapping matches in the haystack with
    /// the replacement provided. If `limit` is `0`, then all non-overlapping
    /// matches are replaced. That is, `Regex::replace_all(hay, rep)` is
    /// equivalent to `Regex::replacen(hay, 0, rep)`.
    ///
    /// If no match is found, then the haystack is returned unchanged. In that
    /// case, this implementation will likely return a `Cow::Borrowed` value
    /// such that no allocation is performed.
    ///
    /// When a `Cow::Borrowed` is returned, the value returned is guaranteed
    /// to be equivalent to the `haystack` given.
    ///
    /// The documentation for [`Regex::replace`] goes into more detail about
    /// what kinds of replacement strings are supported.
    ///
    /// # Time complexity
    ///
    /// Since iterators over all matches requires running potentially many
    /// searches on the haystack, and since each search has worst case
    /// `O(m * n)` time complexity, the overall worst case time complexity for
    /// this routine is `O(m * n^2)`.
    ///
    /// Although note that the worst case time here has an upper bound given
    /// by the `limit` parameter.
    ///
    /// # Fallibility
    ///
    /// See the corresponding section in the docs for [`Regex::replace_all`]
    /// for tips on how to deal with a replacement routine that can fail.
    ///
    /// # Example
    ///
    /// This example shows how to flip the order of whitespace (excluding line
    /// terminators) delimited fields, and normalizes the whitespace that
    /// delimits the fields. But we only do it for the first two matches.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(?m)^(\S+)[\s--\r\n]+(\S+)$").unwrap();
    /// let hay = "
    /// Greetings  1973
    /// Wild\t1973
    /// BornToRun\t\t\t\t1975
    /// Darkness                    1978
    /// TheRiver 1980
    /// ";
    /// let new = re.replacen(hay, 2, "$2 $1");
    /// assert_eq!(new, "
    /// 1973 Greetings
    /// 1973 Wild
    /// BornToRun\t\t\t\t1975
    /// Darkness                    1978
    /// TheRiver 1980
    /// ");
    /// ```
    #[inline]
    pub fn replacen<'h, R: Replacer>(
        &self,
        haystack: &'h str,
        limit: usize,
        mut rep: R,
    ) -> Cow<'h, str> {
        // If we know that the replacement doesn't have any capture expansions,
        // then we can use the fast path. The fast path can make a tremendous
        // difference:
        //
        //   1) We use `find_iter` instead of `captures_iter`. Not asking for
        //      captures generally makes the regex engines faster.
        //   2) We don't need to look up all of the capture groups and do
        //      replacements inside the replacement string. We just push it
        //      at each match and be done with it.
        if let Some(rep) = rep.no_expansion() {
            let mut it = self.find_iter(haystack).enumerate().peekable();
            if it.peek().is_none() {
                return Cow::Borrowed(haystack);
            }
            let mut new = String::with_capacity(haystack.len());
            let mut last_match = 0;
            for (i, m) in it {
                new.push_str(&haystack[last_match..m.start()]);
                new.push_str(&rep);
                last_match = m.end();
                if limit > 0 && i >= limit - 1 {
                    break;
                }
            }
            new.push_str(&haystack[last_match..]);
            return Cow::Owned(new);
        }

        // The slower path, which we use if the replacement may need access to
        // capture groups.
        let mut it = self.captures_iter(haystack).enumerate().peekable();
        if it.peek().is_none() {
            return Cow::Borrowed(haystack);
        }
        let mut new = String::with_capacity(haystack.len());
        let mut last_match = 0;
        for (i, cap) in it {
            // unwrap on 0 is OK because captures only reports matches
            let m = cap.get(0).unwrap();
            new.push_str(&haystack[last_match..m.start()]);
            rep.replace_append(&cap, &mut new);
            last_match = m.end();
            if limit > 0 && i >= limit - 1 {
                break;
            }
        }
        new.push_str(&haystack[last_match..]);
        Cow::Owned(new)
    }
}

/// A group of advanced or "lower level" search methods. Some methods permit
/// starting the search at a position greater than `0` in the haystack. Other
/// methods permit reusing allocations, for example, when extracting the
/// matches for capture groups.
impl Regex {
    /// Returns the end byte offset of the first match in the haystack given.
    ///
    /// This method may have the same performance characteristics as
    /// `is_match`. Behaviorlly, it doesn't just report whether it match
    /// occurs, but also the end offset for a match. In particular, the offset
    /// returned *may be shorter* than the proper end of the leftmost-first
    /// match that you would find via [`Regex::find`].
    ///
    /// Note that it is not guaranteed that this routine finds the shortest or
    /// "earliest" possible match. Instead, the main idea of this API is that
    /// it returns the offset at the point at which the internal regex engine
    /// has determined that a match has occurred. This may vary depending on
    /// which internal regex engine is used, and thus, the offset itself may
    /// change based on internal heuristics.
    ///
    /// # Example
    ///
    /// Typically, `a+` would match the entire first sequence of `a` in some
    /// haystack, but `shortest_match` *may* give up as soon as it sees the
    /// first `a`.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"a+").unwrap();
    /// let offset = re.shortest_match("aaaaa").unwrap();
    /// assert_eq!(offset, 1);
    /// ```
    #[inline]
    pub fn shortest_match(&self, haystack: &str) -> Option<usize> {
        self.shortest_match_at(haystack, 0)
    }

    /// Returns the same as [`Regex::shortest_match`], but starts the search at
    /// the given offset.
    ///
    /// The significance of the starting point is that it takes the surrounding
    /// context into consideration. For example, the `\A` anchor can only match
    /// when `start == 0`.
    ///
    /// If a match is found, the offset returned is relative to the beginning
    /// of the haystack, not the beginning of the search.
    ///
    /// # Panics
    ///
    /// This panics when `start >= haystack.len() + 1`.
    ///
    /// # Example
    ///
    /// This example shows the significance of `start` by demonstrating how it
    /// can be used to permit look-around assertions in a regex to take the
    /// surrounding context into account.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"\bchew\b").unwrap();
    /// let hay = "eschew";
    /// // We get a match here, but it's probably not intended.
    /// assert_eq!(re.shortest_match(&hay[2..]), Some(4));
    /// // No match because the  assertions take the context into account.
    /// assert_eq!(re.shortest_match_at(hay, 2), None);
    /// ```
    #[inline]
    pub fn shortest_match_at(
        &self,
        haystack: &str,
        start: usize,
    ) -> Option<usize> {
        let input =
            Input::new(haystack).earliest(true).span(start..haystack.len());
        self.meta.search_half(&input).map(|hm| hm.offset())
    }

    /// Returns the same as [`Regex::is_match`], but starts the search at the
    /// given offset.
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
    /// This example shows the significance of `start` by demonstrating how it
    /// can be used to permit look-around assertions in a regex to take the
    /// surrounding context into account.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"\bchew\b").unwrap();
    /// let hay = "eschew";
    /// // We get a match here, but it's probably not intended.
    /// assert!(re.is_match(&hay[2..]));
    /// // No match because the  assertions take the context into account.
    /// assert!(!re.is_match_at(hay, 2));
    /// ```
    #[inline]
    pub fn is_match_at(&self, haystack: &str, start: usize) -> bool {
        let input =
            Input::new(haystack).earliest(true).span(start..haystack.len());
        self.meta.search_half(&input).is_some()
    }

    /// Returns the same as [`Regex::find`], but starts the search at the given
    /// offset.
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
    /// This example shows the significance of `start` by demonstrating how it
    /// can be used to permit look-around assertions in a regex to take the
    /// surrounding context into account.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"\bchew\b").unwrap();
    /// let hay = "eschew";
    /// // We get a match here, but it's probably not intended.
    /// assert_eq!(re.find(&hay[2..]).map(|m| m.range()), Some(0..4));
    /// // No match because the  assertions take the context into account.
    /// assert_eq!(re.find_at(hay, 2), None);
    /// ```
    #[inline]
    pub fn find_at<'h>(
        &self,
        haystack: &'h str,
        start: usize,
    ) -> Option<Match<'h>> {
        let input = Input::new(haystack).span(start..haystack.len());
        self.meta
            .search(&input)
            .map(|m| Match::new(haystack, m.start(), m.end()))
    }

    /// Returns the same as [`Regex::captures`], but starts the search at the
    /// given offset.
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
    /// This example shows the significance of `start` by demonstrating how it
    /// can be used to permit look-around assertions in a regex to take the
    /// surrounding context into account.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"\bchew\b").unwrap();
    /// let hay = "eschew";
    /// // We get a match here, but it's probably not intended.
    /// assert_eq!(&re.captures(&hay[2..]).unwrap()[0], "chew");
    /// // No match because the  assertions take the context into account.
    /// assert!(re.captures_at(hay, 2).is_none());
    /// ```
    #[inline]
    pub fn captures_at<'h>(
        &self,
        haystack: &'h str,
        start: usize,
    ) -> Option<Captures<'h>> {
        let input = Input::new(haystack).span(start..haystack.len());
        let mut caps = self.meta.create_captures();
        self.meta.search_captures(&input, &mut caps);
        if caps.is_match() {
            let static_captures_len = self.static_captures_len();
            Some(Captures { haystack, caps, static_captures_len })
        } else {
            None
        }
    }

    /// This is like [`Regex::captures`], but writes the byte offsets of each
    /// capture group match into the locations given.
    ///
    /// A [`CaptureLocations`] stores the same byte offsets as a [`Captures`],
    /// but does *not* store a reference to the haystack. This makes its API
    /// a bit lower level and less convenient. But in exchange, callers
    /// may allocate their own `CaptureLocations` and reuse it for multiple
    /// searches. This may be helpful if allocating a `Captures` shows up in a
    /// profile as too costly.
    ///
    /// To create a `CaptureLocations` value, use the
    /// [`Regex::capture_locations`] method.
    ///
    /// This also returns the overall match if one was found. When a match is
    /// found, its offsets are also always stored in `locs` at index `0`.
    ///
    /// # Panics
    ///
    /// This routine may panic if the given `CaptureLocations` was not created
    /// by this regex.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"^([a-z]+)=(\S*)$").unwrap();
    /// let mut locs = re.capture_locations();
    /// assert!(re.captures_read(&mut locs, "id=foo123").is_some());
    /// assert_eq!(Some((0, 9)), locs.get(0));
    /// assert_eq!(Some((0, 2)), locs.get(1));
    /// assert_eq!(Some((3, 9)), locs.get(2));
    /// ```
    #[inline]
    pub fn captures_read<'h>(
        &self,
        locs: &mut CaptureLocations,
        haystack: &'h str,
    ) -> Option<Match<'h>> {
        self.captures_read_at(locs, haystack, 0)
    }

    /// Returns the same as [`Regex::captures_read`], but starts the search at
    /// the given offset.
    ///
    /// The significance of the starting point is that it takes the surrounding
    /// context into consideration. For example, the `\A` anchor can only
    /// match when `start == 0`.
    ///
    /// # Panics
    ///
    /// This panics when `start >= haystack.len() + 1`.
    ///
    /// This routine may also panic if the given `CaptureLocations` was not
    /// created by this regex.
    ///
    /// # Example
    ///
    /// This example shows the significance of `start` by demonstrating how it
    /// can be used to permit look-around assertions in a regex to take the
    /// surrounding context into account.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"\bchew\b").unwrap();
    /// let hay = "eschew";
    /// let mut locs = re.capture_locations();
    /// // We get a match here, but it's probably not intended.
    /// assert!(re.captures_read(&mut locs, &hay[2..]).is_some());
    /// // No match because the  assertions take the context into account.
    /// assert!(re.captures_read_at(&mut locs, hay, 2).is_none());
    /// ```
    #[inline]
    pub fn captures_read_at<'h>(
        &self,
        locs: &mut CaptureLocations,
        haystack: &'h str,
        start: usize,
    ) -> Option<Match<'h>> {
        let input = Input::new(haystack).span(start..haystack.len());
        self.meta.search_captures(&input, &mut locs.0);
        locs.0.get_match().map(|m| Match::new(haystack, m.start(), m.end()))
    }

    /// An undocumented alias for `captures_read_at`.
    ///
    /// The `regex-capi` crate previously used this routine, so to avoid
    /// breaking that crate, we continue to provide the name as an undocumented
    /// alias.
    #[doc(hidden)]
    #[inline]
    pub fn read_captures_at<'h>(
        &self,
        locs: &mut CaptureLocations,
        haystack: &'h str,
        start: usize,
    ) -> Option<Match<'h>> {
        self.captures_read_at(locs, haystack, start)
    }
}

/// Auxiliary methods.
impl Regex {
    /// Returns the original string of this regex.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"foo\w+bar").unwrap();
    /// assert_eq!(re.as_str(), r"foo\w+bar");
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.pattern
    }

    /// Returns an iterator over the capture names in this regex.
    ///
    /// The iterator returned yields elements of type `Option<&str>`. That is,
    /// the iterator yields values for all capture groups, even ones that are
    /// unnamed. The order of the groups corresponds to the order of the group's
    /// corresponding opening parenthesis.
    ///
    /// The first element of the iterator always yields the group corresponding
    /// to the overall match, and this group is always unnamed. Therefore, the
    /// iterator always yields at least one group.
    ///
    /// # Example
    ///
    /// This shows basic usage with a mix of named and unnamed capture groups:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(?<a>.(?<b>.))(.)(?:.)(?<c>.)").unwrap();
    /// let mut names = re.capture_names();
    /// assert_eq!(names.next(), Some(None));
    /// assert_eq!(names.next(), Some(Some("a")));
    /// assert_eq!(names.next(), Some(Some("b")));
    /// assert_eq!(names.next(), Some(None));
    /// // the '(?:.)' group is non-capturing and so doesn't appear here!
    /// assert_eq!(names.next(), Some(Some("c")));
    /// assert_eq!(names.next(), None);
    /// ```
    ///
    /// The iterator always yields at least one element, even for regexes with
    /// no capture groups and even for regexes that can never match:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"").unwrap();
    /// let mut names = re.capture_names();
    /// assert_eq!(names.next(), Some(None));
    /// assert_eq!(names.next(), None);
    ///
    /// let re = Regex::new(r"[a&&b]").unwrap();
    /// let mut names = re.capture_names();
    /// assert_eq!(names.next(), Some(None));
    /// assert_eq!(names.next(), None);
    /// ```
    #[inline]
    pub fn capture_names(&self) -> CaptureNames<'_> {
        CaptureNames(self.meta.group_info().pattern_names(PatternID::ZERO))
    }

    /// Returns the number of captures groups in this regex.
    ///
    /// This includes all named and unnamed groups, including the implicit
    /// unnamed group that is always present and corresponds to the entire
    /// match.
    ///
    /// Since the implicit unnamed group is always included in this length, the
    /// length returned is guaranteed to be greater than zero.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"foo").unwrap();
    /// assert_eq!(1, re.captures_len());
    ///
    /// let re = Regex::new(r"(foo)").unwrap();
    /// assert_eq!(2, re.captures_len());
    ///
    /// let re = Regex::new(r"(?<a>.(?<b>.))(.)(?:.)(?<c>.)").unwrap();
    /// assert_eq!(5, re.captures_len());
    ///
    /// let re = Regex::new(r"[a&&b]").unwrap();
    /// assert_eq!(1, re.captures_len());
    /// ```
    #[inline]
    pub fn captures_len(&self) -> usize {
        self.meta.group_info().group_len(PatternID::ZERO)
    }

    /// Returns the total number of capturing groups that appear in every
    /// possible match.
    ///
    /// If the number of capture groups can vary depending on the match, then
    /// this returns `None`. That is, a value is only returned when the number
    /// of matching groups is invariant or "static."
    ///
    /// Note that like [`Regex::captures_len`], this **does** include the
    /// implicit capturing group corresponding to the entire match. Therefore,
    /// when a non-None value is returned, it is guaranteed to be at least `1`.
    /// Stated differently, a return value of `Some(0)` is impossible.
    ///
    /// # Example
    ///
    /// This shows a few cases where a static number of capture groups is
    /// available and a few cases where it is not.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let len = |pattern| {
    ///     Regex::new(pattern).map(|re| re.static_captures_len())
    /// };
    ///
    /// assert_eq!(Some(1), len("a")?);
    /// assert_eq!(Some(2), len("(a)")?);
    /// assert_eq!(Some(2), len("(a)|(b)")?);
    /// assert_eq!(Some(3), len("(a)(b)|(c)(d)")?);
    /// assert_eq!(None, len("(a)|b")?);
    /// assert_eq!(None, len("a|(b)")?);
    /// assert_eq!(None, len("(b)*")?);
    /// assert_eq!(Some(2), len("(b)+")?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn static_captures_len(&self) -> Option<usize> {
        self.meta.static_captures_len()
    }

    /// Returns a fresh allocated set of capture locations that can
    /// be reused in multiple calls to [`Regex::captures_read`] or
    /// [`Regex::captures_read_at`].
    ///
    /// The returned locations can be used for any subsequent search for this
    /// particular regex. There is no guarantee that it is correct to use for
    /// other regexes, even if they have the same number of capture groups.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(.)(.)(\w+)").unwrap();
    /// let mut locs = re.capture_locations();
    /// assert!(re.captures_read(&mut locs, "Padron").is_some());
    /// assert_eq!(locs.get(0), Some((0, 6)));
    /// assert_eq!(locs.get(1), Some((0, 1)));
    /// assert_eq!(locs.get(2), Some((1, 2)));
    /// assert_eq!(locs.get(3), Some((2, 6)));
    /// ```
    #[inline]
    pub fn capture_locations(&self) -> CaptureLocations {
        CaptureLocations(self.meta.create_captures())
    }

    /// An alias for `capture_locations` to preserve backward compatibility.
    ///
    /// The `regex-capi` crate used this method, so to avoid breaking that
    /// crate, we continue to export it as an undocumented API.
    #[doc(hidden)]
    #[inline]
    pub fn locations(&self) -> CaptureLocations {
        self.capture_locations()
    }
}

/// Represents a single match of a regex in a haystack.
///
/// A `Match` contains both the start and end byte offsets of the match and the
/// actual substring corresponding to the range of those byte offsets. It is
/// guaranteed that `start <= end`. When `start == end`, the match is empty.
///
/// Since this `Match` can only be produced by the top-level `Regex` APIs
/// that only support searching UTF-8 encoded strings, the byte offsets for a
/// `Match` are guaranteed to fall on valid UTF-8 codepoint boundaries. That
/// is, slicing a `&str` with [`Match::range`] is guaranteed to never panic.
///
/// Values with this type are created by [`Regex::find`] or
/// [`Regex::find_iter`]. Other APIs can create `Match` values too. For
/// example, [`Captures::get`].
///
/// The lifetime parameter `'h` refers to the lifetime of the matched of the
/// haystack that this match was produced from.
///
/// # Numbering
///
/// The byte offsets in a `Match` form a half-open interval. That is, the
/// start of the range is inclusive and the end of the range is exclusive.
/// For example, given a haystack `abcFOOxyz` and a match of `FOO`, its byte
/// offset range starts at `3` and ends at `6`. `3` corresponds to `F` and
/// `6` corresponds to `x`, which is one past the end of the match. This
/// corresponds to the same kind of slicing that Rust uses.
///
/// For more on why this was chosen over other schemes (aside from being
/// consistent with how Rust the language works), see [this discussion] and
/// [Dijkstra's note on a related topic][note].
///
/// [this discussion]: https://github.com/rust-lang/regex/discussions/866
/// [note]: https://www.cs.utexas.edu/users/EWD/transcriptions/EWD08xx/EWD831.html
///
/// # Example
///
/// This example shows the value of each of the methods on `Match` for a
/// particular search.
///
/// ```
/// use regex::Regex;
///
/// let re = Regex::new(r"\p{Greek}+").unwrap();
/// let hay = "Greek: αβγδ";
/// let m = re.find(hay).unwrap();
/// assert_eq!(7, m.start());
/// assert_eq!(15, m.end());
/// assert!(!m.is_empty());
/// assert_eq!(8, m.len());
/// assert_eq!(7..15, m.range());
/// assert_eq!("αβγδ", m.as_str());
/// ```
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Match<'h> {
    haystack: &'h str,
    start: usize,
    end: usize,
}

impl<'h> Match<'h> {
    /// Returns the byte offset of the start of the match in the haystack. The
    /// start of the match corresponds to the position where the match begins
    /// and includes the first byte in the match.
    ///
    /// It is guaranteed that `Match::start() <= Match::end()`.
    ///
    /// This is guaranteed to fall on a valid UTF-8 codepoint boundary. That
    /// is, it will never be an offset that appears between the UTF-8 code
    /// units of a UTF-8 encoded Unicode scalar value. Consequently, it is
    /// always safe to slice the corresponding haystack using this offset.
    #[inline]
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the byte offset of the end of the match in the haystack. The
    /// end of the match corresponds to the byte immediately following the last
    /// byte in the match. This means that `&slice[start..end]` works as one
    /// would expect.
    ///
    /// It is guaranteed that `Match::start() <= Match::end()`.
    ///
    /// This is guaranteed to fall on a valid UTF-8 codepoint boundary. That
    /// is, it will never be an offset that appears between the UTF-8 code
    /// units of a UTF-8 encoded Unicode scalar value. Consequently, it is
    /// always safe to slice the corresponding haystack using this offset.
    #[inline]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Returns true if and only if this match has a length of zero.
    ///
    /// Note that an empty match can only occur when the regex itself can
    /// match the empty string. Here are some examples of regexes that can
    /// all match the empty string: `^`, `^$`, `\b`, `a?`, `a*`, `a{0}`,
    /// `(foo|\d+|quux)?`.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the length, in bytes, of this match.
    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns the range over the starting and ending byte offsets of the
    /// match in the haystack.
    ///
    /// It is always correct to slice the original haystack searched with this
    /// range. That is, because the offsets are guaranteed to fall on valid
    /// UTF-8 boundaries, the range returned is always valid.
    #[inline]
    pub fn range(&self) -> core::ops::Range<usize> {
        self.start..self.end
    }

    /// Returns the substring of the haystack that matched.
    #[inline]
    pub fn as_str(&self) -> &'h str {
        &self.haystack[self.range()]
    }

    /// Creates a new match from the given haystack and byte offsets.
    #[inline]
    fn new(haystack: &'h str, start: usize, end: usize) -> Match<'h> {
        Match { haystack, start, end }
    }
}

impl<'h> core::fmt::Debug for Match<'h> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Match")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("string", &self.as_str())
            .finish()
    }
}

impl<'h> From<Match<'h>> for &'h str {
    fn from(m: Match<'h>) -> &'h str {
        m.as_str()
    }
}

impl<'h> From<Match<'h>> for core::ops::Range<usize> {
    fn from(m: Match<'h>) -> core::ops::Range<usize> {
        m.range()
    }
}

/// Represents the capture groups for a single match.
///
/// Capture groups refer to parts of a regex enclosed in parentheses. They
/// can be optionally named. The purpose of capture groups is to be able to
/// reference different parts of a match based on the original pattern. In
/// essence, a `Captures` is a container of [`Match`] values for each group
/// that participated in a regex match. Each `Match` can be looked up by either
/// its capture group index or name (if it has one).
///
/// For example, say you want to match the individual letters in a 5-letter
/// word:
///
/// ```text
/// (?<first>\w)(\w)(?:\w)\w(?<last>\w)
/// ```
///
/// This regex has 4 capture groups:
///
/// * The group at index `0` corresponds to the overall match. It is always
/// present in every match and never has a name.
/// * The group at index `1` with name `first` corresponding to the first
/// letter.
/// * The group at index `2` with no name corresponding to the second letter.
/// * The group at index `3` with name `last` corresponding to the fifth and
/// last letter.
///
/// Notice that `(?:\w)` was not listed above as a capture group despite it
/// being enclosed in parentheses. That's because `(?:pattern)` is a special
/// syntax that permits grouping but *without* capturing. The reason for not
/// treating it as a capture is that tracking and reporting capture groups
/// requires additional state that may lead to slower searches. So using as few
/// capture groups as possible can help performance. (Although the difference
/// in performance of a couple of capture groups is likely immaterial.)
///
/// Values with this type are created by [`Regex::captures`] or
/// [`Regex::captures_iter`].
///
/// `'h` is the lifetime of the haystack that these captures were matched from.
///
/// # Example
///
/// ```
/// use regex::Regex;
///
/// let re = Regex::new(r"(?<first>\w)(\w)(?:\w)\w(?<last>\w)").unwrap();
/// let caps = re.captures("toady").unwrap();
/// assert_eq!("toady", &caps[0]);
/// assert_eq!("t", &caps["first"]);
/// assert_eq!("o", &caps[2]);
/// assert_eq!("y", &caps["last"]);
/// ```
pub struct Captures<'h> {
    haystack: &'h str,
    caps: captures::Captures,
    static_captures_len: Option<usize>,
}

impl<'h> Captures<'h> {
    /// Returns the `Match` associated with the capture group at index `i`. If
    /// `i` does not correspond to a capture group, or if the capture group did
    /// not participate in the match, then `None` is returned.
    ///
    /// When `i == 0`, this is guaranteed to return a non-`None` value.
    ///
    /// # Examples
    ///
    /// Get the substring that matched with a default of an empty string if the
    /// group didn't participate in the match:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"[a-z]+(?:([0-9]+)|([A-Z]+))").unwrap();
    /// let caps = re.captures("abc123").unwrap();
    ///
    /// let substr1 = caps.get(1).map_or("", |m| m.as_str());
    /// let substr2 = caps.get(2).map_or("", |m| m.as_str());
    /// assert_eq!(substr1, "123");
    /// assert_eq!(substr2, "");
    /// ```
    #[inline]
    pub fn get(&self, i: usize) -> Option<Match<'h>> {
        self.caps
            .get_group(i)
            .map(|sp| Match::new(self.haystack, sp.start, sp.end))
    }

    /// Returns the `Match` associated with the capture group named `name`. If
    /// `name` isn't a valid capture group or it refers to a group that didn't
    /// match, then `None` is returned.
    ///
    /// Note that unlike `caps["name"]`, this returns a `Match` whose lifetime
    /// matches the lifetime of the haystack in this `Captures` value.
    /// Conversely, the substring returned by `caps["name"]` has a lifetime
    /// of the `Captures` value, which is likely shorter than the lifetime of
    /// the haystack. In some cases, it may be necessary to use this method to
    /// access the matching substring instead of the `caps["name"]` notation.
    ///
    /// # Examples
    ///
    /// Get the substring that matched with a default of an empty string if the
    /// group didn't participate in the match:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(
    ///     r"[a-z]+(?:(?<numbers>[0-9]+)|(?<letters>[A-Z]+))",
    /// ).unwrap();
    /// let caps = re.captures("abc123").unwrap();
    ///
    /// let numbers = caps.name("numbers").map_or("", |m| m.as_str());
    /// let letters = caps.name("letters").map_or("", |m| m.as_str());
    /// assert_eq!(numbers, "123");
    /// assert_eq!(letters, "");
    /// ```
    #[inline]
    pub fn name(&self, name: &str) -> Option<Match<'h>> {
        self.caps
            .get_group_by_name(name)
            .map(|sp| Match::new(self.haystack, sp.start, sp.end))
    }

    /// This is a convenience routine for extracting the substrings
    /// corresponding to matching capture groups.
    ///
    /// This returns a tuple where the first element corresponds to the full
    /// substring of the haystack that matched the regex. The second element is
    /// an array of substrings, with each corresponding to the substring that
    /// matched for a particular capture group.
    ///
    /// # Panics
    ///
    /// This panics if the number of possible matching groups in this
    /// `Captures` value is not fixed to `N` in all circumstances.
    /// More precisely, this routine only works when `N` is equivalent to
    /// [`Regex::static_captures_len`].
    ///
    /// Stated more plainly, if the number of matching capture groups in a
    /// regex can vary from match to match, then this function always panics.
    ///
    /// For example, `(a)(b)|(c)` could produce two matching capture groups
    /// or one matching capture group for any given match. Therefore, one
    /// cannot use `extract` with such a pattern.
    ///
    /// But a pattern like `(a)(b)|(c)(d)` can be used with `extract` because
    /// the number of capture groups in every match is always equivalent,
    /// even if the capture _indices_ in each match are not.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"([0-9]{4})-([0-9]{2})-([0-9]{2})").unwrap();
    /// let hay = "On 2010-03-14, I became a Tenneessee lamb.";
    /// let Some((full, [year, month, day])) =
    ///     re.captures(hay).map(|caps| caps.extract()) else { return };
    /// assert_eq!("2010-03-14", full);
    /// assert_eq!("2010", year);
    /// assert_eq!("03", month);
    /// assert_eq!("14", day);
    /// ```
    ///
    /// # Example: iteration
    ///
    /// This example shows how to use this method when iterating over all
    /// `Captures` matches in a haystack.
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"([0-9]{4})-([0-9]{2})-([0-9]{2})").unwrap();
    /// let hay = "1973-01-05, 1975-08-25 and 1980-10-18";
    ///
    /// let mut dates: Vec<(&str, &str, &str)> = vec![];
    /// for (_, [y, m, d]) in re.captures_iter(hay).map(|c| c.extract()) {
    ///     dates.push((y, m, d));
    /// }
    /// assert_eq!(dates, vec![
    ///     ("1973", "01", "05"),
    ///     ("1975", "08", "25"),
    ///     ("1980", "10", "18"),
    /// ]);
    /// ```
    ///
    /// # Example: parsing different formats
    ///
    /// This API is particularly useful when you need to extract a particular
    /// value that might occur in a different format. Consider, for example,
    /// an identifier that might be in double quotes or single quotes:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r#"id:(?:"([^"]+)"|'([^']+)')"#).unwrap();
    /// let hay = r#"The first is id:"foo" and the second is id:'bar'."#;
    /// let mut ids = vec![];
    /// for (_, [id]) in re.captures_iter(hay).map(|c| c.extract()) {
    ///     ids.push(id);
    /// }
    /// assert_eq!(ids, vec!["foo", "bar"]);
    /// ```
    pub fn extract<const N: usize>(&self) -> (&'h str, [&'h str; N]) {
        let len = self
            .static_captures_len
            .expect("number of capture groups can vary in a match")
            .checked_sub(1)
            .expect("number of groups is always greater than zero");
        assert_eq!(N, len, "asked for {} groups, but must ask for {}", N, len);
        // The regex-automata variant of extract is a bit more permissive.
        // It doesn't require the number of matching capturing groups to be
        // static, and you can even request fewer groups than what's there. So
        // this is guaranteed to never panic because we've asserted above that
        // the user has requested precisely the number of groups that must be
        // present in any match for this regex.
        self.caps.extract(self.haystack)
    }

    /// Expands all instances of `$ref` in `replacement` to the corresponding
    /// capture group, and writes them to the `dst` buffer given. A `ref` can
    /// be a capture group index or a name. If `ref` doesn't refer to a capture
    /// group that participated in the match, then it is replaced with the
    /// empty string.
    ///
    /// # Format
    ///
    /// The format of the replacement string supports two different kinds of
    /// capture references: unbraced and braced.
    ///
    /// For the unbraced format, the format supported is `$ref` where `name`
    /// can be any character in the class `[0-9A-Za-z_]`. `ref` is always
    /// the longest possible parse. So for example, `$1a` corresponds to the
    /// capture group named `1a` and not the capture group at index `1`. If
    /// `ref` matches `^[0-9]+$`, then it is treated as a capture group index
    /// itself and not a name.
    ///
    /// For the braced format, the format supported is `${ref}` where `ref` can
    /// be any sequence of bytes except for `}`. If no closing brace occurs,
    /// then it is not considered a capture reference. As with the unbraced
    /// format, if `ref` matches `^[0-9]+$`, then it is treated as a capture
    /// group index and not a name.
    ///
    /// The braced format is useful for exerting precise control over the name
    /// of the capture reference. For example, `${1}a` corresponds to the
    /// capture group reference `1` followed by the letter `a`, where as `$1a`
    /// (as mentioned above) corresponds to the capture group reference `1a`.
    /// The braced format is also useful for expressing capture group names
    /// that use characters not supported by the unbraced format. For example,
    /// `${foo[bar].baz}` refers to the capture group named `foo[bar].baz`.
    ///
    /// If a capture group reference is found and it does not refer to a valid
    /// capture group, then it will be replaced with the empty string.
    ///
    /// To write a literal `$`, use `$$`.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(
    ///     r"(?<day>[0-9]{2})-(?<month>[0-9]{2})-(?<year>[0-9]{4})",
    /// ).unwrap();
    /// let hay = "On 14-03-2010, I became a Tenneessee lamb.";
    /// let caps = re.captures(hay).unwrap();
    ///
    /// let mut dst = String::new();
    /// caps.expand("year=$year, month=$month, day=$day", &mut dst);
    /// assert_eq!(dst, "year=2010, month=03, day=14");
    /// ```
    #[inline]
    pub fn expand(&self, replacement: &str, dst: &mut String) {
        self.caps.interpolate_string_into(self.haystack, replacement, dst);
    }

    /// Returns an iterator over all capture groups. This includes both
    /// matching and non-matching groups.
    ///
    /// The iterator always yields at least one matching group: the first group
    /// (at index `0`) with no name. Subsequent groups are returned in the order
    /// of their opening parenthesis in the regex.
    ///
    /// The elements yielded have type `Option<Match<'h>>`, where a non-`None`
    /// value is present if the capture group matches.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(\w)(\d)?(\w)").unwrap();
    /// let caps = re.captures("AZ").unwrap();
    ///
    /// let mut it = caps.iter();
    /// assert_eq!(it.next().unwrap().map(|m| m.as_str()), Some("AZ"));
    /// assert_eq!(it.next().unwrap().map(|m| m.as_str()), Some("A"));
    /// assert_eq!(it.next().unwrap().map(|m| m.as_str()), None);
    /// assert_eq!(it.next().unwrap().map(|m| m.as_str()), Some("Z"));
    /// assert_eq!(it.next(), None);
    /// ```
    #[inline]
    pub fn iter<'c>(&'c self) -> SubCaptureMatches<'c, 'h> {
        SubCaptureMatches { haystack: self.haystack, it: self.caps.iter() }
    }

    /// Returns the total number of capture groups. This includes both
    /// matching and non-matching groups.
    ///
    /// The length returned is always equivalent to the number of elements
    /// yielded by [`Captures::iter`]. Consequently, the length is always
    /// greater than zero since every `Captures` value always includes the
    /// match for the entire regex.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(\w)(\d)?(\w)").unwrap();
    /// let caps = re.captures("AZ").unwrap();
    /// assert_eq!(caps.len(), 4);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.caps.group_len()
    }
}

impl<'h> core::fmt::Debug for Captures<'h> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        /// A little helper type to provide a nice map-like debug
        /// representation for our capturing group spans.
        ///
        /// regex-automata has something similar, but it includes the pattern
        /// ID in its debug output, which is confusing. It also doesn't include
        /// that strings that match because a regex-automata `Captures` doesn't
        /// borrow the haystack.
        struct CapturesDebugMap<'a> {
            caps: &'a Captures<'a>,
        }

        impl<'a> core::fmt::Debug for CapturesDebugMap<'a> {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                let mut map = f.debug_map();
                let names =
                    self.caps.caps.group_info().pattern_names(PatternID::ZERO);
                for (group_index, maybe_name) in names.enumerate() {
                    let key = Key(group_index, maybe_name);
                    match self.caps.get(group_index) {
                        None => map.entry(&key, &None::<()>),
                        Some(mat) => map.entry(&key, &Value(mat)),
                    };
                }
                map.finish()
            }
        }

        struct Key<'a>(usize, Option<&'a str>);

        impl<'a> core::fmt::Debug for Key<'a> {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                write!(f, "{}", self.0)?;
                if let Some(name) = self.1 {
                    write!(f, "/{:?}", name)?;
                }
                Ok(())
            }
        }

        struct Value<'a>(Match<'a>);

        impl<'a> core::fmt::Debug for Value<'a> {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                write!(
                    f,
                    "{}..{}/{:?}",
                    self.0.start(),
                    self.0.end(),
                    self.0.as_str()
                )
            }
        }

        f.debug_tuple("Captures")
            .field(&CapturesDebugMap { caps: self })
            .finish()
    }
}

/// Get a matching capture group's haystack substring by index.
///
/// The haystack substring returned can't outlive the `Captures` object if this
/// method is used, because of how `Index` is defined (normally `a[i]` is part
/// of `a` and can't outlive it). To work around this limitation, do that, use
/// [`Captures::get`] instead.
///
/// `'h` is the lifetime of the matched haystack, but the lifetime of the
/// `&str` returned by this implementation is the lifetime of the `Captures`
/// value itself.
///
/// # Panics
///
/// If there is no matching group at the given index.
impl<'h> core::ops::Index<usize> for Captures<'h> {
    type Output = str;

    // The lifetime is written out to make it clear that the &str returned
    // does NOT have a lifetime equivalent to 'h.
    fn index<'a>(&'a self, i: usize) -> &'a str {
        self.get(i)
            .map(|m| m.as_str())
            .unwrap_or_else(|| panic!("no group at index '{}'", i))
    }
}

/// Get a matching capture group's haystack substring by name.
///
/// The haystack substring returned can't outlive the `Captures` object if this
/// method is used, because of how `Index` is defined (normally `a[i]` is part
/// of `a` and can't outlive it). To work around this limitation, do that, use
/// [`Captures::name`] instead.
///
/// `'h` is the lifetime of the matched haystack, but the lifetime of the
/// `&str` returned by this implementation is the lifetime of the `Captures`
/// value itself.
///
/// `'n` is the lifetime of the group name used to index the `Captures` value.
///
/// # Panics
///
/// If there is no matching group at the given name.
impl<'h, 'n> core::ops::Index<&'n str> for Captures<'h> {
    type Output = str;

    fn index<'a>(&'a self, name: &'n str) -> &'a str {
        self.name(name)
            .map(|m| m.as_str())
            .unwrap_or_else(|| panic!("no group named '{}'", name))
    }
}

/// A low level representation of the byte offsets of each capture group.
///
/// You can think of this as a lower level [`Captures`], where this type does
/// not support named capturing groups directly and it does not borrow the
/// haystack that these offsets were matched on.
///
/// Primarily, this type is useful when using the lower level `Regex` APIs such
/// as [`Regex::captures_read`], which permits amortizing the allocation in
/// which capture match offsets are stored.
///
/// In order to build a value of this type, you'll need to call the
/// [`Regex::capture_locations`] method. The value returned can then be reused
/// in subsequent searches for that regex. Using it for other regexes may
/// result in a panic or otherwise incorrect results.
///
/// # Example
///
/// This example shows how to create and use `CaptureLocations` in a search.
///
/// ```
/// use regex::Regex;
///
/// let re = Regex::new(r"(?<first>\w+)\s+(?<last>\w+)").unwrap();
/// let mut locs = re.capture_locations();
/// let m = re.captures_read(&mut locs, "Bruce Springsteen").unwrap();
/// assert_eq!(0..17, m.range());
/// assert_eq!(Some((0, 17)), locs.get(0));
/// assert_eq!(Some((0, 5)), locs.get(1));
/// assert_eq!(Some((6, 17)), locs.get(2));
///
/// // Asking for an invalid capture group always returns None.
/// assert_eq!(None, locs.get(3));
/// # // literals are too big for 32-bit usize: #1041
/// # #[cfg(target_pointer_width = "64")]
/// assert_eq!(None, locs.get(34973498648));
/// # #[cfg(target_pointer_width = "64")]
/// assert_eq!(None, locs.get(9944060567225171988));
/// ```
#[derive(Clone, Debug)]
pub struct CaptureLocations(captures::Captures);

/// A type alias for `CaptureLocations` for backwards compatibility.
///
/// Previously, we exported `CaptureLocations` as `Locations` in an
/// undocumented API. To prevent breaking that code (e.g., in `regex-capi`),
/// we continue re-exporting the same undocumented API.
#[doc(hidden)]
pub type Locations = CaptureLocations;

impl CaptureLocations {
    /// Returns the start and end byte offsets of the capture group at index
    /// `i`. This returns `None` if `i` is not a valid capture group or if the
    /// capture group did not match.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(?<first>\w+)\s+(?<last>\w+)").unwrap();
    /// let mut locs = re.capture_locations();
    /// re.captures_read(&mut locs, "Bruce Springsteen").unwrap();
    /// assert_eq!(Some((0, 17)), locs.get(0));
    /// assert_eq!(Some((0, 5)), locs.get(1));
    /// assert_eq!(Some((6, 17)), locs.get(2));
    /// ```
    #[inline]
    pub fn get(&self, i: usize) -> Option<(usize, usize)> {
        self.0.get_group(i).map(|sp| (sp.start, sp.end))
    }

    /// Returns the total number of capture groups (even if they didn't match).
    /// That is, the length returned is unaffected by the result of a search.
    ///
    /// This is always at least `1` since every regex has at least `1`
    /// capturing group that corresponds to the entire match.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"(?<first>\w+)\s+(?<last>\w+)").unwrap();
    /// let mut locs = re.capture_locations();
    /// assert_eq!(3, locs.len());
    /// re.captures_read(&mut locs, "Bruce Springsteen").unwrap();
    /// assert_eq!(3, locs.len());
    /// ```
    ///
    /// Notice that the length is always at least `1`, regardless of the regex:
    ///
    /// ```
    /// use regex::Regex;
    ///
    /// let re = Regex::new(r"").unwrap();
    /// let locs = re.capture_locations();
    /// assert_eq!(1, locs.len());
    ///
    /// // [a&&b] is a regex that never matches anything.
    /// let re = Regex::new(r"[a&&b]").unwrap();
    /// let locs = re.capture_locations();
    /// assert_eq!(1, locs.len());
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        // self.0.group_len() returns 0 if the underlying captures doesn't
        // represent a match, but the behavior guaranteed for this method is
        // that the length doesn't change based on a match or not.
        self.0.group_info().group_len(PatternID::ZERO)
    }

    /// An alias for the `get` method for backwards compatibility.
    ///
    /// Previously, we exported `get` as `pos` in an undocumented API. To
    /// prevent breaking that code (e.g., in `regex-capi`), we continue
    /// re-exporting the same undocumented API.
    #[doc(hidden)]
    #[inline]
    pub fn pos(&self, i: usize) -> Option<(usize, usize)> {
        self.get(i)
    }
}

/// An iterator over all non-overlapping matches in a haystack.
///
/// This iterator yields [`Match`] values. The iterator stops when no more
/// matches can be found.
///
/// `'r` is the lifetime of the compiled regular expression and `'h` is the
/// lifetime of the haystack.
///
/// This iterator is created by [`Regex::find_iter`].
///
/// # Time complexity
///
/// Note that since an iterator runs potentially many searches on the haystack
/// and since each search has worst case `O(m * n)` time complexity, the
/// overall worst case time complexity for iteration is `O(m * n^2)`.
#[derive(Debug)]
pub struct Matches<'r, 'h> {
    haystack: &'h str,
    it: meta::FindMatches<'r, 'h>,
}

impl<'r, 'h> Iterator for Matches<'r, 'h> {
    type Item = Match<'h>;

    #[inline]
    fn next(&mut self) -> Option<Match<'h>> {
        self.it
            .next()
            .map(|sp| Match::new(self.haystack, sp.start(), sp.end()))
    }

    #[inline]
    fn count(self) -> usize {
        // This can actually be up to 2x faster than calling `next()` until
        // completion, because counting matches when using a DFA only requires
        // finding the end of each match. But returning a `Match` via `next()`
        // requires the start of each match which, with a DFA, requires a
        // reverse forward scan to find it.
        self.it.count()
    }
}

impl<'r, 'h> core::iter::FusedIterator for Matches<'r, 'h> {}

/// An iterator over all non-overlapping capture matches in a haystack.
///
/// This iterator yields [`Captures`] values. The iterator stops when no more
/// matches can be found.
///
/// `'r` is the lifetime of the compiled regular expression and `'h` is the
/// lifetime of the matched string.
///
/// This iterator is created by [`Regex::captures_iter`].
///
/// # Time complexity
///
/// Note that since an iterator runs potentially many searches on the haystack
/// and since each search has worst case `O(m * n)` time complexity, the
/// overall worst case time complexity for iteration is `O(m * n^2)`.
#[derive(Debug)]
pub struct CaptureMatches<'r, 'h> {
    haystack: &'h str,
    it: meta::CapturesMatches<'r, 'h>,
}

impl<'r, 'h> Iterator for CaptureMatches<'r, 'h> {
    type Item = Captures<'h>;

    #[inline]
    fn next(&mut self) -> Option<Captures<'h>> {
        let static_captures_len = self.it.regex().static_captures_len();
        self.it.next().map(|caps| Captures {
            haystack: self.haystack,
            caps,
            static_captures_len,
        })
    }

    #[inline]
    fn count(self) -> usize {
        // This can actually be up to 2x faster than calling `next()` until
        // completion, because counting matches when using a DFA only requires
        // finding the end of each match. But returning a `Match` via `next()`
        // requires the start of each match which, with a DFA, requires a
        // reverse forward scan to find it.
        self.it.count()
    }
}

impl<'r, 'h> core::iter::FusedIterator for CaptureMatches<'r, 'h> {}

/// An iterator over all substrings delimited by a regex match.
///
/// `'r` is the lifetime of the compiled regular expression and `'h` is the
/// lifetime of the byte string being split.
///
/// This iterator is created by [`Regex::split`].
///
/// # Time complexity
///
/// Note that since an iterator runs potentially many searches on the haystack
/// and since each search has worst case `O(m * n)` time complexity, the
/// overall worst case time complexity for iteration is `O(m * n^2)`.
#[derive(Debug)]
pub struct Split<'r, 'h> {
    haystack: &'h str,
    it: meta::Split<'r, 'h>,
}

impl<'r, 'h> Iterator for Split<'r, 'h> {
    type Item = &'h str;

    #[inline]
    fn next(&mut self) -> Option<&'h str> {
        self.it.next().map(|span| &self.haystack[span])
    }
}

impl<'r, 'h> core::iter::FusedIterator for Split<'r, 'h> {}

/// An iterator over at most `N` substrings delimited by a regex match.
///
/// The last substring yielded by this iterator will be whatever remains after
/// `N-1` splits.
///
/// `'r` is the lifetime of the compiled regular expression and `'h` is the
/// lifetime of the byte string being split.
///
/// This iterator is created by [`Regex::splitn`].
///
/// # Time complexity
///
/// Note that since an iterator runs potentially many searches on the haystack
/// and since each search has worst case `O(m * n)` time complexity, the
/// overall worst case time complexity for iteration is `O(m * n^2)`.
///
/// Although note that the worst case time here has an upper bound given
/// by the `limit` parameter to [`Regex::splitn`].
#[derive(Debug)]
pub struct SplitN<'r, 'h> {
    haystack: &'h str,
    it: meta::SplitN<'r, 'h>,
}

impl<'r, 'h> Iterator for SplitN<'r, 'h> {
    type Item = &'h str;

    #[inline]
    fn next(&mut self) -> Option<&'h str> {
        self.it.next().map(|span| &self.haystack[span])
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl<'r, 'h> core::iter::FusedIterator for SplitN<'r, 'h> {}

/// An iterator over the names of all capture groups in a regex.
///
/// This iterator yields values of type `Option<&str>` in order of the opening
/// capture group parenthesis in the regex pattern. `None` is yielded for
/// groups with no name. The first element always corresponds to the implicit
/// and unnamed group for the overall match.
///
/// `'r` is the lifetime of the compiled regular expression.
///
/// This iterator is created by [`Regex::capture_names`].
#[derive(Clone, Debug)]
pub struct CaptureNames<'r>(captures::GroupInfoPatternNames<'r>);

impl<'r> Iterator for CaptureNames<'r> {
    type Item = Option<&'r str>;

    #[inline]
    fn next(&mut self) -> Option<Option<&'r str>> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.0.count()
    }
}

impl<'r> ExactSizeIterator for CaptureNames<'r> {}

impl<'r> core::iter::FusedIterator for CaptureNames<'r> {}

/// An iterator over all group matches in a [`Captures`] value.
///
/// This iterator yields values of type `Option<Match<'h>>`, where `'h` is the
/// lifetime of the haystack that the matches are for. The order of elements
/// yielded corresponds to the order of the opening parenthesis for the group
/// in the regex pattern. `None` is yielded for groups that did not participate
/// in the match.
///
/// The first element always corresponds to the implicit group for the overall
/// match. Since this iterator is created by a [`Captures`] value, and a
/// `Captures` value is only created when a match occurs, it follows that the
/// first element yielded by this iterator is guaranteed to be non-`None`.
///
/// The lifetime `'c` corresponds to the lifetime of the `Captures` value that
/// created this iterator, and the lifetime `'h` corresponds to the originally
/// matched haystack.
#[derive(Clone, Debug)]
pub struct SubCaptureMatches<'c, 'h> {
    haystack: &'h str,
    it: captures::CapturesPatternIter<'c>,
}

impl<'c, 'h> Iterator for SubCaptureMatches<'c, 'h> {
    type Item = Option<Match<'h>>;

    #[inline]
    fn next(&mut self) -> Option<Option<Match<'h>>> {
        self.it.next().map(|group| {
            group.map(|sp| Match::new(self.haystack, sp.start, sp.end))
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.it.count()
    }
}

impl<'c, 'h> ExactSizeIterator for SubCaptureMatches<'c, 'h> {}

impl<'c, 'h> core::iter::FusedIterator for SubCaptureMatches<'c, 'h> {}

/// A trait for types that can be used to replace matches in a haystack.
///
/// In general, users of this crate shouldn't need to implement this trait,
/// since implementations are already provided for `&str` along with other
/// variants of string types, as well as `FnMut(&Captures) -> String` (or any
/// `FnMut(&Captures) -> T` where `T: AsRef<str>`). Those cover most use cases,
/// but callers can implement this trait directly if necessary.
///
/// # Example
///
/// This example shows a basic implementation of  the `Replacer` trait. This
/// can be done much more simply using the replacement string interpolation
/// support (e.g., `$first $last`), but this approach avoids needing to parse
/// the replacement string at all.
///
/// ```
/// use regex::{Captures, Regex, Replacer};
///
/// struct NameSwapper;
///
/// impl Replacer for NameSwapper {
///     fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
///         dst.push_str(&caps["first"]);
///         dst.push_str(" ");
///         dst.push_str(&caps["last"]);
///     }
/// }
///
/// let re = Regex::new(r"(?<last>[^,\s]+),\s+(?<first>\S+)").unwrap();
/// let result = re.replace("Springsteen, Bruce", NameSwapper);
/// assert_eq!(result, "Bruce Springsteen");
/// ```
pub trait Replacer {
    /// Appends possibly empty data to `dst` to replace the current match.
    ///
    /// The current match is represented by `caps`, which is guaranteed to
    /// have a match at capture group `0`.
    ///
    /// For example, a no-op replacement would be `dst.push_str(&caps[0])`.
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String);

    /// Return a fixed unchanging replacement string.
    ///
    /// When doing replacements, if access to [`Captures`] is not needed (e.g.,
    /// the replacement string does not need `$` expansion), then it can be
    /// beneficial to avoid finding sub-captures.
    ///
    /// In general, this is called once for every call to a replacement routine
    /// such as [`Regex::replace_all`].
    fn no_expansion<'r>(&'r mut self) -> Option<Cow<'r, str>> {
        None
    }

    /// Returns a type that implements `Replacer`, but that borrows and wraps
    /// this `Replacer`.
    ///
    /// This is useful when you want to take a generic `Replacer` (which might
    /// not be cloneable) and use it without consuming it, so it can be used
    /// more than once.
    ///
    /// # Example
    ///
    /// ```
    /// use regex::{Regex, Replacer};
    ///
    /// fn replace_all_twice<R: Replacer>(
    ///     re: Regex,
    ///     src: &str,
    ///     mut rep: R,
    /// ) -> String {
    ///     let dst = re.replace_all(src, rep.by_ref());
    ///     let dst = re.replace_all(&dst, rep.by_ref());
    ///     dst.into_owned()
    /// }
    /// ```
    fn by_ref<'r>(&'r mut self) -> ReplacerRef<'r, Self> {
        ReplacerRef(self)
    }
}

impl<'a> Replacer for &'a str {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        caps.expand(*self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

impl<'a> Replacer for &'a String {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.as_str().replace_append(caps, dst)
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

impl Replacer for String {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.as_str().replace_append(caps, dst)
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

impl<'a> Replacer for Cow<'a, str> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.as_ref().replace_append(caps, dst)
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

impl<'a> Replacer for &'a Cow<'a, str> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.as_ref().replace_append(caps, dst)
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

impl<F, T> Replacer for F
where
    F: FnMut(&Captures<'_>) -> T,
    T: AsRef<str>,
{
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        dst.push_str((*self)(caps).as_ref());
    }
}

/// A by-reference adaptor for a [`Replacer`].
///
/// This permits reusing the same `Replacer` value in multiple calls to a
/// replacement routine like [`Regex::replace_all`].
///
/// This type is created by [`Replacer::by_ref`].
#[derive(Debug)]
pub struct ReplacerRef<'a, R: ?Sized>(&'a mut R);

impl<'a, R: Replacer + ?Sized + 'a> Replacer for ReplacerRef<'a, R> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.0.replace_append(caps, dst)
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        self.0.no_expansion()
    }
}

/// A helper type for forcing literal string replacement.
///
/// It can be used with routines like [`Regex::replace`] and
/// [`Regex::replace_all`] to do a literal string replacement without expanding
/// `$name` to their corresponding capture groups. This can be both convenient
/// (to avoid escaping `$`, for example) and faster (since capture groups
/// don't need to be found).
///
/// `'s` is the lifetime of the literal string to use.
///
/// # Example
///
/// ```
/// use regex::{NoExpand, Regex};
///
/// let re = Regex::new(r"(?<last>[^,\s]+),\s+(\S+)").unwrap();
/// let result = re.replace("Springsteen, Bruce", NoExpand("$2 $last"));
/// assert_eq!(result, "$2 $last");
/// ```
#[derive(Clone, Debug)]
pub struct NoExpand<'s>(pub &'s str);

impl<'s> Replacer for NoExpand<'s> {
    fn replace_append(&mut self, _: &Captures<'_>, dst: &mut String) {
        dst.push_str(self.0);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(self.0))
    }
}

/// Quickly checks the given replacement string for whether interpolation
/// should be done on it. It returns `None` if a `$` was found anywhere in the
/// given string, which suggests interpolation needs to be done. But if there's
/// no `$` anywhere, then interpolation definitely does not need to be done. In
/// that case, the given string is returned as a borrowed `Cow`.
///
/// This is meant to be used to implement the `Replacer::no_expandsion` method
/// in its various trait impls.
fn no_expansion<T: AsRef<str>>(replacement: &T) -> Option<Cow<'_, str>> {
    let replacement = replacement.as_ref();
    match crate::find_byte::find_byte(b'$', replacement.as_bytes()) {
        Some(_) => None,
        None => Some(Cow::Borrowed(replacement)),
    }
}
