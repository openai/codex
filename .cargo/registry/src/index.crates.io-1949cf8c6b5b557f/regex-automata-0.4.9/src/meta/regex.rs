use core::{
    borrow::Borrow,
    panic::{RefUnwindSafe, UnwindSafe},
};

use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};

use regex_syntax::{
    ast,
    hir::{self, Hir},
};

use crate::{
    meta::{
        error::BuildError,
        strategy::{self, Strategy},
        wrappers,
    },
    nfa::thompson::WhichCaptures,
    util::{
        captures::{Captures, GroupInfo},
        iter,
        pool::{Pool, PoolGuard},
        prefilter::Prefilter,
        primitives::{NonMaxUsize, PatternID},
        search::{HalfMatch, Input, Match, MatchKind, PatternSet, Span},
    },
};

/// A type alias for our pool of meta::Cache that fixes the type parameters to
/// what we use for the meta regex below.
type CachePool = Pool<Cache, CachePoolFn>;

/// Same as above, but for the guard returned by a pool.
type CachePoolGuard<'a> = PoolGuard<'a, Cache, CachePoolFn>;

/// The type of the closure we use to create new caches. We need to spell out
/// all of the marker traits or else we risk leaking !MARKER impls.
type CachePoolFn =
    Box<dyn Fn() -> Cache + Send + Sync + UnwindSafe + RefUnwindSafe>;

/// A regex matcher that works by composing several other regex matchers
/// automatically.
///
/// In effect, a meta regex papers over a lot of the quirks or performance
/// problems in each of the regex engines in this crate. Its goal is to provide
/// an infallible and simple API that "just does the right thing" in the common
/// case.
///
/// A meta regex is the implementation of a `Regex` in the `regex` crate.
/// Indeed, the `regex` crate API is essentially just a light wrapper over
/// this type. This includes the `regex` crate's `RegexSet` API!
///
/// # Composition
///
/// This is called a "meta" matcher precisely because it uses other regex
/// matchers to provide a convenient high level regex API. Here are some
/// examples of how other regex matchers are composed:
///
/// * When calling [`Regex::captures`], instead of immediately
/// running a slower but more capable regex engine like the
/// [`PikeVM`](crate::nfa::thompson::pikevm::PikeVM), the meta regex engine
/// will usually first look for the bounds of a match with a higher throughput
/// regex engine like a [lazy DFA](crate::hybrid). Only when a match is found
/// is a slower engine like `PikeVM` used to find the matching span for each
/// capture group.
/// * While higher throughout engines like the lazy DFA cannot handle
/// Unicode word boundaries in general, they can still be used on pure ASCII
/// haystacks by pretending that Unicode word boundaries are just plain ASCII
/// word boundaries. However, if a haystack is not ASCII, the meta regex engine
/// will automatically switch to a (possibly slower) regex engine that supports
/// Unicode word boundaries in general.
/// * In some cases where a regex pattern is just a simple literal or a small
/// set of literals, an actual regex engine won't be used at all. Instead,
/// substring or multi-substring search algorithms will be employed.
///
/// There are many other forms of composition happening too, but the above
/// should give a general idea. In particular, it may perhaps be surprising
/// that *multiple* regex engines might get executed for a single search. That
/// is, the decision of what regex engine to use is not _just_ based on the
/// pattern, but also based on the dynamic execution of the search itself.
///
/// The primary reason for this composition is performance. The fundamental
/// tension is that the faster engines tend to be less capable, and the more
/// capable engines tend to be slower.
///
/// Note that the forms of composition that are allowed are determined by
/// compile time crate features and configuration. For example, if the `hybrid`
/// feature isn't enabled, or if [`Config::hybrid`] has been disabled, then the
/// meta regex engine will never use a lazy DFA.
///
/// # Synchronization and cloning
///
/// Most of the regex engines in this crate require some kind of mutable
/// "scratch" space to read and write from while performing a search. Since
/// a meta regex composes these regex engines, a meta regex also requires
/// mutable scratch space. This scratch space is called a [`Cache`].
///
/// Most regex engines _also_ usually have a read-only component, typically
/// a [Thompson `NFA`](crate::nfa::thompson::NFA).
///
/// In order to make the `Regex` API convenient, most of the routines hide
/// the fact that a `Cache` is needed at all. To achieve this, a [memory
/// pool](crate::util::pool::Pool) is used internally to retrieve `Cache`
/// values in a thread safe way that also permits reuse. This in turn implies
/// that every such search call requires some form of synchronization. Usually
/// this synchronization is fast enough to not notice, but in some cases, it
/// can be a bottleneck. This typically occurs when all of the following are
/// true:
///
/// * The same `Regex` is shared across multiple threads simultaneously,
/// usually via a [`util::lazy::Lazy`](crate::util::lazy::Lazy) or something
/// similar from the `once_cell` or `lazy_static` crates.
/// * The primary unit of work in each thread is a regex search.
/// * Searches are run on very short haystacks.
///
/// This particular case can lead to high contention on the pool used by a
/// `Regex` internally, which can in turn increase latency to a noticeable
/// effect. This cost can be mitigated in one of the following ways:
///
/// * Use a distinct copy of a `Regex` in each thread, usually by cloning it.
/// Cloning a `Regex` _does not_ do a deep copy of its read-only component.
/// But it does lead to each `Regex` having its own memory pool, which in
/// turn eliminates the problem of contention. In general, this technique should
/// not result in any additional memory usage when compared to sharing the same
/// `Regex` across multiple threads simultaneously.
/// * Use lower level APIs, like [`Regex::search_with`], which permit passing
/// a `Cache` explicitly. In this case, it is up to you to determine how best
/// to provide a `Cache`. For example, you might put a `Cache` in thread-local
/// storage if your use case allows for it.
///
/// Overall, this is an issue that happens rarely in practice, but it can
/// happen.
///
/// # Warning: spin-locks may be used in alloc-only mode
///
/// When this crate is built without the `std` feature and the high level APIs
/// on a `Regex` are used, then a spin-lock will be used to synchronize access
/// to an internal pool of `Cache` values. This may be undesirable because
/// a spin-lock is [effectively impossible to implement correctly in user
/// space][spinlocks-are-bad]. That is, more concretely, the spin-lock could
/// result in a deadlock.
///
/// [spinlocks-are-bad]: https://matklad.github.io/2020/01/02/spinlocks-considered-harmful.html
///
/// If one wants to avoid the use of spin-locks when the `std` feature is
/// disabled, then you must use APIs that accept a `Cache` value explicitly.
/// For example, [`Regex::search_with`].
///
/// # Example
///
/// ```
/// use regex_automata::meta::Regex;
///
/// let re = Regex::new(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$")?;
/// assert!(re.is_match("2010-03-14"));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: anchored search
///
/// This example shows how to use [`Input::anchored`] to run an anchored
/// search, even when the regex pattern itself isn't anchored. An anchored
/// search guarantees that if a match is found, then the start offset of the
/// match corresponds to the offset at which the search was started.
///
/// ```
/// use regex_automata::{meta::Regex, Anchored, Input, Match};
///
/// let re = Regex::new(r"\bfoo\b")?;
/// let input = Input::new("xx foo xx").range(3..).anchored(Anchored::Yes);
/// // The offsets are in terms of the original haystack.
/// assert_eq!(Some(Match::must(0, 3..6)), re.find(input));
///
/// // Notice that no match occurs here, because \b still takes the
/// // surrounding context into account, even if it means looking back
/// // before the start of your search.
/// let hay = "xxfoo xx";
/// let input = Input::new(hay).range(2..).anchored(Anchored::Yes);
/// assert_eq!(None, re.find(input));
/// // Indeed, you cannot achieve the above by simply slicing the
/// // haystack itself, since the regex engine can't see the
/// // surrounding context. This is why 'Input' permits setting
/// // the bounds of a search!
/// let input = Input::new(&hay[2..]).anchored(Anchored::Yes);
/// // WRONG!
/// assert_eq!(Some(Match::must(0, 0..3)), re.find(input));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: earliest search
///
/// This example shows how to use [`Input::earliest`] to run a search that
/// might stop before finding the typical leftmost match.
///
/// ```
/// use regex_automata::{meta::Regex, Anchored, Input, Match};
///
/// let re = Regex::new(r"[a-z]{3}|b")?;
/// let input = Input::new("abc").earliest(true);
/// assert_eq!(Some(Match::must(0, 1..2)), re.find(input));
///
/// // Note that "earliest" isn't really a match semantic unto itself.
/// // Instead, it is merely an instruction to whatever regex engine
/// // gets used internally to quit as soon as it can. For example,
/// // this regex uses a different search technique, and winds up
/// // producing a different (but valid) match!
/// let re = Regex::new(r"abc|b")?;
/// let input = Input::new("abc").earliest(true);
/// assert_eq!(Some(Match::must(0, 0..3)), re.find(input));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: change the line terminator
///
/// This example shows how to enable multi-line mode by default and change
/// the line terminator to the NUL byte:
///
/// ```
/// use regex_automata::{meta::Regex, util::syntax, Match};
///
/// let re = Regex::builder()
///     .syntax(syntax::Config::new().multi_line(true))
///     .configure(Regex::config().line_terminator(b'\x00'))
///     .build(r"^foo$")?;
/// let hay = "\x00foo\x00";
/// assert_eq!(Some(Match::must(0, 1..4)), re.find(hay));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Regex {
    /// The actual regex implementation.
    imp: Arc<RegexI>,
    /// A thread safe pool of caches.
    ///
    /// For the higher level search APIs, a `Cache` is automatically plucked
    /// from this pool before running a search. The lower level `with` methods
    /// permit the caller to provide their own cache, thereby bypassing
    /// accesses to this pool.
    ///
    /// Note that we put this outside the `Arc` so that cloning a `Regex`
    /// results in creating a fresh `CachePool`. This in turn permits callers
    /// to clone regexes into separate threads where each such regex gets
    /// the pool's "thread owner" optimization. Otherwise, if one shares the
    /// `Regex` directly, then the pool will go through a slower mutex path for
    /// all threads except for the "owner."
    pool: CachePool,
}

/// The internal implementation of `Regex`, split out so that it can be wrapped
/// in an `Arc`.
#[derive(Debug)]
struct RegexI {
    /// The core matching engine.
    ///
    /// Why is this reference counted when RegexI is already wrapped in an Arc?
    /// Well, we need to capture this in a closure to our `Pool` below in order
    /// to create new `Cache` values when needed. So since it needs to be in
    /// two places, we make it reference counted.
    ///
    /// We make `RegexI` itself reference counted too so that `Regex` itself
    /// stays extremely small and very cheap to clone.
    strat: Arc<dyn Strategy>,
    /// Metadata about the regexes driving the strategy. The metadata is also
    /// usually stored inside the strategy too, but we put it here as well
    /// so that we can get quick access to it (without virtual calls) before
    /// executing the regex engine. For example, we use this metadata to
    /// detect a subset of cases where we know a match is impossible, and can
    /// thus avoid calling into the strategy at all.
    ///
    /// Since `RegexInfo` is stored in multiple places, it is also reference
    /// counted.
    info: RegexInfo,
}

/// Convenience constructors for a `Regex` using the default configuration.
impl Regex {
    /// Builds a `Regex` from a single pattern string using the default
    /// configuration.
    ///
    /// If there was a problem parsing the pattern or a problem turning it into
    /// a regex matcher, then an error is returned.
    ///
    /// If you want to change the configuration of a `Regex`, use a [`Builder`]
    /// with a [`Config`].
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Match};
    ///
    /// let re = Regex::new(r"(?Rm)^foo$")?;
    /// let hay = "\r\nfoo\r\n";
    /// assert_eq!(Some(Match::must(0, 2..5)), re.find(hay));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(pattern: &str) -> Result<Regex, BuildError> {
        Regex::builder().build(pattern)
    }

    /// Builds a `Regex` from many pattern strings using the default
    /// configuration.
    ///
    /// If there was a problem parsing any of the patterns or a problem turning
    /// them into a regex matcher, then an error is returned.
    ///
    /// If you want to change the configuration of a `Regex`, use a [`Builder`]
    /// with a [`Config`].
    ///
    /// # Example: simple lexer
    ///
    /// This simplistic example leverages the multi-pattern support to build a
    /// simple little lexer. The pattern ID in the match tells you which regex
    /// matched, which in turn might be used to map back to the "type" of the
    /// token returned by the lexer.
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Match};
    ///
    /// let re = Regex::new_many(&[
    ///     r"[[:space:]]",
    ///     r"[A-Za-z0-9][A-Za-z0-9_]+",
    ///     r"->",
    ///     r".",
    /// ])?;
    /// let haystack = "fn is_boss(bruce: i32, springsteen: String) -> bool;";
    /// let matches: Vec<Match> = re.find_iter(haystack).collect();
    /// assert_eq!(matches, vec![
    ///     Match::must(1, 0..2),   // 'fn'
    ///     Match::must(0, 2..3),   // ' '
    ///     Match::must(1, 3..10),  // 'is_boss'
    ///     Match::must(3, 10..11), // '('
    ///     Match::must(1, 11..16), // 'bruce'
    ///     Match::must(3, 16..17), // ':'
    ///     Match::must(0, 17..18), // ' '
    ///     Match::must(1, 18..21), // 'i32'
    ///     Match::must(3, 21..22), // ','
    ///     Match::must(0, 22..23), // ' '
    ///     Match::must(1, 23..34), // 'springsteen'
    ///     Match::must(3, 34..35), // ':'
    ///     Match::must(0, 35..36), // ' '
    ///     Match::must(1, 36..42), // 'String'
    ///     Match::must(3, 42..43), // ')'
    ///     Match::must(0, 43..44), // ' '
    ///     Match::must(2, 44..46), // '->'
    ///     Match::must(0, 46..47), // ' '
    ///     Match::must(1, 47..51), // 'bool'
    ///     Match::must(3, 51..52), // ';'
    /// ]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// One can write a lexer like the above using a regex like
    /// `(?P<space>[[:space:]])|(?P<ident>[A-Za-z0-9][A-Za-z0-9_]+)|...`,
    /// but then you need to ask whether capture group matched to determine
    /// which branch in the regex matched, and thus, which token the match
    /// corresponds to. In contrast, the above example includes the pattern ID
    /// in the match. There's no need to use capture groups at all.
    ///
    /// # Example: finding the pattern that caused an error
    ///
    /// When a syntax error occurs, it is possible to ask which pattern
    /// caused the syntax error.
    ///
    /// ```
    /// use regex_automata::{meta::Regex, PatternID};
    ///
    /// let err = Regex::new_many(&["a", "b", r"\p{Foo}", "c"]).unwrap_err();
    /// assert_eq!(Some(PatternID::must(2)), err.pattern());
    /// ```
    ///
    /// # Example: zero patterns is valid
    ///
    /// Building a regex with zero patterns results in a regex that never
    /// matches anything. Because this routine is generic, passing an empty
    /// slice usually requires a turbo-fish (or something else to help type
    /// inference).
    ///
    /// ```
    /// use regex_automata::{meta::Regex, util::syntax, Match};
    ///
    /// let re = Regex::new_many::<&str>(&[])?;
    /// assert_eq!(None, re.find(""));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new_many<P: AsRef<str>>(
        patterns: &[P],
    ) -> Result<Regex, BuildError> {
        Regex::builder().build_many(patterns)
    }

    /// Return a default configuration for a `Regex`.
    ///
    /// This is a convenience routine to avoid needing to import the [`Config`]
    /// type when customizing the construction of a `Regex`.
    ///
    /// # Example: lower the NFA size limit
    ///
    /// In some cases, the default size limit might be too big. The size limit
    /// can be lowered, which will prevent large regex patterns from compiling.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::meta::Regex;
    ///
    /// let result = Regex::builder()
    ///     .configure(Regex::config().nfa_size_limit(Some(20 * (1<<10))))
    ///     // Not even 20KB is enough to build a single large Unicode class!
    ///     .build(r"\pL");
    /// assert!(result.is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn config() -> Config {
        Config::new()
    }

    /// Return a builder for configuring the construction of a `Regex`.
    ///
    /// This is a convenience routine to avoid needing to import the
    /// [`Builder`] type in common cases.
    ///
    /// # Example: change the line terminator
    ///
    /// This example shows how to enable multi-line mode by default and change
    /// the line terminator to the NUL byte:
    ///
    /// ```
    /// use regex_automata::{meta::Regex, util::syntax, Match};
    ///
    /// let re = Regex::builder()
    ///     .syntax(syntax::Config::new().multi_line(true))
    ///     .configure(Regex::config().line_terminator(b'\x00'))
    ///     .build(r"^foo$")?;
    /// let hay = "\x00foo\x00";
    /// assert_eq!(Some(Match::must(0, 1..4)), re.find(hay));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder() -> Builder {
        Builder::new()
    }
}

/// High level convenience routines for using a regex to search a haystack.
impl Regex {
    /// Returns true if and only if this regex matches the given haystack.
    ///
    /// This routine may short circuit if it knows that scanning future input
    /// will never lead to a different result. (Consider how this might make
    /// a difference given the regex `a+` on the haystack `aaaaaaaaaaaaaaa`.
    /// This routine _may_ stop after it sees the first `a`, but routines like
    /// `find` need to continue searching because `+` is greedy by default.)
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new("foo[0-9]+bar")?;
    ///
    /// assert!(re.is_match("foo12345bar"));
    /// assert!(!re.is_match("foobar"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: consistency with search APIs
    ///
    /// `is_match` is guaranteed to return `true` whenever `find` returns a
    /// match. This includes searches that are executed entirely within a
    /// codepoint:
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Input};
    ///
    /// let re = Regex::new("a*")?;
    ///
    /// // This doesn't match because the default configuration bans empty
    /// // matches from splitting a codepoint.
    /// assert!(!re.is_match(Input::new("☃").span(1..2)));
    /// assert_eq!(None, re.find(Input::new("☃").span(1..2)));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Notice that when UTF-8 mode is disabled, then the above reports a
    /// match because the restriction against zero-width matches that split a
    /// codepoint has been lifted:
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Input, Match};
    ///
    /// let re = Regex::builder()
    ///     .configure(Regex::config().utf8_empty(false))
    ///     .build("a*")?;
    ///
    /// assert!(re.is_match(Input::new("☃").span(1..2)));
    /// assert_eq!(
    ///     Some(Match::must(0, 1..1)),
    ///     re.find(Input::new("☃").span(1..2)),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// A similar idea applies when using line anchors with CRLF mode enabled,
    /// which prevents them from matching between a `\r` and a `\n`.
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Input, Match};
    ///
    /// let re = Regex::new(r"(?Rm:$)")?;
    /// assert!(!re.is_match(Input::new("\r\n").span(1..1)));
    /// // A regular line anchor, which only considers \n as a
    /// // line terminator, will match.
    /// let re = Regex::new(r"(?m:$)")?;
    /// assert!(re.is_match(Input::new("\r\n").span(1..1)));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_match<'h, I: Into<Input<'h>>>(&self, input: I) -> bool {
        let input = input.into().earliest(true);
        if self.imp.info.is_impossible(&input) {
            return false;
        }
        let mut guard = self.pool.get();
        let result = self.imp.strat.is_match(&mut guard, &input);
        // See 'Regex::search' for why we put the guard back explicitly.
        PoolGuard::put(guard);
        result
    }

    /// Executes a leftmost search and returns the first match that is found,
    /// if one exists.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Match};
    ///
    /// let re = Regex::new("foo[0-9]+")?;
    /// assert_eq!(Some(Match::must(0, 0..8)), re.find("foo12345"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn find<'h, I: Into<Input<'h>>>(&self, input: I) -> Option<Match> {
        self.search(&input.into())
    }

    /// Executes a leftmost forward search and writes the spans of capturing
    /// groups that participated in a match into the provided [`Captures`]
    /// value. If no match was found, then [`Captures::is_match`] is guaranteed
    /// to return `false`.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Span};
    ///
    /// let re = Regex::new(r"^([0-9]{4})-([0-9]{2})-([0-9]{2})$")?;
    /// let mut caps = re.create_captures();
    ///
    /// re.captures("2010-03-14", &mut caps);
    /// assert!(caps.is_match());
    /// assert_eq!(Some(Span::from(0..4)), caps.get_group(1));
    /// assert_eq!(Some(Span::from(5..7)), caps.get_group(2));
    /// assert_eq!(Some(Span::from(8..10)), caps.get_group(3));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn captures<'h, I: Into<Input<'h>>>(
        &self,
        input: I,
        caps: &mut Captures,
    ) {
        self.search_captures(&input.into(), caps)
    }

    /// Returns an iterator over all non-overlapping leftmost matches in
    /// the given haystack. If no match exists, then the iterator yields no
    /// elements.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Match};
    ///
    /// let re = Regex::new("foo[0-9]+")?;
    /// let haystack = "foo1 foo12 foo123";
    /// let matches: Vec<Match> = re.find_iter(haystack).collect();
    /// assert_eq!(matches, vec![
    ///     Match::must(0, 0..4),
    ///     Match::must(0, 5..10),
    ///     Match::must(0, 11..17),
    /// ]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn find_iter<'r, 'h, I: Into<Input<'h>>>(
        &'r self,
        input: I,
    ) -> FindMatches<'r, 'h> {
        let cache = self.pool.get();
        let it = iter::Searcher::new(input.into());
        FindMatches { re: self, cache, it }
    }

    /// Returns an iterator over all non-overlapping `Captures` values. If no
    /// match exists, then the iterator yields no elements.
    ///
    /// This yields the same matches as [`Regex::find_iter`], but it includes
    /// the spans of all capturing groups that participate in each match.
    ///
    /// **Tip:** See [`util::iter::Searcher`](crate::util::iter::Searcher) for
    /// how to correctly iterate over all matches in a haystack while avoiding
    /// the creation of a new `Captures` value for every match. (Which you are
    /// forced to do with an `Iterator`.)
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Span};
    ///
    /// let re = Regex::new("foo(?P<numbers>[0-9]+)")?;
    ///
    /// let haystack = "foo1 foo12 foo123";
    /// let matches: Vec<Span> = re
    ///     .captures_iter(haystack)
    ///     // The unwrap is OK since 'numbers' matches if the pattern matches.
    ///     .map(|caps| caps.get_group_by_name("numbers").unwrap())
    ///     .collect();
    /// assert_eq!(matches, vec![
    ///     Span::from(3..4),
    ///     Span::from(8..10),
    ///     Span::from(14..17),
    /// ]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn captures_iter<'r, 'h, I: Into<Input<'h>>>(
        &'r self,
        input: I,
    ) -> CapturesMatches<'r, 'h> {
        let cache = self.pool.get();
        let caps = self.create_captures();
        let it = iter::Searcher::new(input.into());
        CapturesMatches { re: self, cache, caps, it }
    }

    /// Returns an iterator of spans of the haystack given, delimited by a
    /// match of the regex. Namely, each element of the iterator corresponds to
    /// a part of the haystack that *isn't* matched by the regular expression.
    ///
    /// # Example
    ///
    /// To split a string delimited by arbitrary amounts of spaces or tabs:
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r"[ \t]+")?;
    /// let hay = "a b \t  c\td    e";
    /// let fields: Vec<&str> = re.split(hay).map(|span| &hay[span]).collect();
    /// assert_eq!(fields, vec!["a", "b", "c", "d", "e"]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: more cases
    ///
    /// Basic usage:
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r" ")?;
    /// let hay = "Mary had a little lamb";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["Mary", "had", "a", "little", "lamb"]);
    ///
    /// let re = Regex::new(r"X")?;
    /// let hay = "";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec![""]);
    ///
    /// let re = Regex::new(r"X")?;
    /// let hay = "lionXXtigerXleopard";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["lion", "", "tiger", "leopard"]);
    ///
    /// let re = Regex::new(r"::")?;
    /// let hay = "lion::tiger::leopard";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["lion", "tiger", "leopard"]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// If a haystack contains multiple contiguous matches, you will end up
    /// with empty spans yielded by the iterator:
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r"X")?;
    /// let hay = "XXXXaXXbXc";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["", "", "", "", "a", "", "b", "c"]);
    ///
    /// let re = Regex::new(r"/")?;
    /// let hay = "(///)";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["(", "", "", ")"]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Separators at the start or end of a haystack are neighbored by empty
    /// spans.
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r"0")?;
    /// let hay = "010";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["", "1", ""]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// When the empty string is used as a regex, it splits at every valid
    /// UTF-8 boundary by default (which includes the beginning and end of the
    /// haystack):
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r"")?;
    /// let hay = "rust";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["", "r", "u", "s", "t", ""]);
    ///
    /// // Splitting by an empty string is UTF-8 aware by default!
    /// let re = Regex::new(r"")?;
    /// let hay = "☃";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["", "☃", ""]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// But note that UTF-8 mode for empty strings can be disabled, which will
    /// then result in a match at every byte offset in the haystack,
    /// including between every UTF-8 code unit.
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::builder()
    ///     .configure(Regex::config().utf8_empty(false))
    ///     .build(r"")?;
    /// let hay = "☃".as_bytes();
    /// let got: Vec<&[u8]> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec![
    ///     // Writing byte string slices is just brutal. The problem is that
    ///     // b"foo" has type &[u8; 3] instead of &[u8].
    ///     &[][..], &[b'\xE2'][..], &[b'\x98'][..], &[b'\x83'][..], &[][..],
    /// ]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Contiguous separators (commonly shows up with whitespace), can lead to
    /// possibly surprising behavior. For example, this code is correct:
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r" ")?;
    /// let hay = "    a  b c";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["", "", "", "", "a", "", "b", "c"]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// It does *not* give you `["a", "b", "c"]`. For that behavior, you'd want
    /// to match contiguous space characters:
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r" +")?;
    /// let hay = "    a  b c";
    /// let got: Vec<&str> = re.split(hay).map(|sp| &hay[sp]).collect();
    /// // N.B. This does still include a leading empty span because ' +'
    /// // matches at the beginning of the haystack.
    /// assert_eq!(got, vec!["", "a", "b", "c"]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn split<'r, 'h, I: Into<Input<'h>>>(
        &'r self,
        input: I,
    ) -> Split<'r, 'h> {
        Split { finder: self.find_iter(input), last: 0 }
    }

    /// Returns an iterator of at most `limit` spans of the haystack given,
    /// delimited by a match of the regex. (A `limit` of `0` will return no
    /// spans.) Namely, each element of the iterator corresponds to a part
    /// of the haystack that *isn't* matched by the regular expression. The
    /// remainder of the haystack that is not split will be the last element in
    /// the iterator.
    ///
    /// # Example
    ///
    /// Get the first two words in some haystack:
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r"\W+").unwrap();
    /// let hay = "Hey! How are you?";
    /// let fields: Vec<&str> =
    ///     re.splitn(hay, 3).map(|span| &hay[span]).collect();
    /// assert_eq!(fields, vec!["Hey", "How", "are you?"]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Examples: more cases
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r" ")?;
    /// let hay = "Mary had a little lamb";
    /// let got: Vec<&str> = re.splitn(hay, 3).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["Mary", "had", "a little lamb"]);
    ///
    /// let re = Regex::new(r"X")?;
    /// let hay = "";
    /// let got: Vec<&str> = re.splitn(hay, 3).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec![""]);
    ///
    /// let re = Regex::new(r"X")?;
    /// let hay = "lionXXtigerXleopard";
    /// let got: Vec<&str> = re.splitn(hay, 3).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["lion", "", "tigerXleopard"]);
    ///
    /// let re = Regex::new(r"::")?;
    /// let hay = "lion::tiger::leopard";
    /// let got: Vec<&str> = re.splitn(hay, 2).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["lion", "tiger::leopard"]);
    ///
    /// let re = Regex::new(r"X")?;
    /// let hay = "abcXdef";
    /// let got: Vec<&str> = re.splitn(hay, 1).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["abcXdef"]);
    ///
    /// let re = Regex::new(r"X")?;
    /// let hay = "abcdef";
    /// let got: Vec<&str> = re.splitn(hay, 2).map(|sp| &hay[sp]).collect();
    /// assert_eq!(got, vec!["abcdef"]);
    ///
    /// let re = Regex::new(r"X")?;
    /// let hay = "abcXdef";
    /// let got: Vec<&str> = re.splitn(hay, 0).map(|sp| &hay[sp]).collect();
    /// assert!(got.is_empty());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn splitn<'r, 'h, I: Into<Input<'h>>>(
        &'r self,
        input: I,
        limit: usize,
    ) -> SplitN<'r, 'h> {
        SplitN { splits: self.split(input), limit }
    }
}

/// Lower level search routines that give more control.
impl Regex {
    /// Returns the start and end offset of the leftmost match. If no match
    /// exists, then `None` is returned.
    ///
    /// This is like [`Regex::find`] but, but it accepts a concrete `&Input`
    /// instead of an `Into<Input>`.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Input, Match};
    ///
    /// let re = Regex::new(r"Samwise|Sam")?;
    /// let input = Input::new(
    ///     "one of the chief characters, Samwise the Brave",
    /// );
    /// assert_eq!(Some(Match::must(0, 29..36)), re.search(&input));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn search(&self, input: &Input<'_>) -> Option<Match> {
        if self.imp.info.is_impossible(input) {
            return None;
        }
        let mut guard = self.pool.get();
        let result = self.imp.strat.search(&mut guard, input);
        // We do this dance with the guard and explicitly put it back in the
        // pool because it seems to result in better codegen. If we let the
        // guard's Drop impl put it back in the pool, then functions like
        // ptr::drop_in_place get called and they *don't* get inlined. This
        // isn't usually a big deal, but in latency sensitive benchmarks the
        // extra function call can matter.
        //
        // I used `rebar measure -f '^grep/every-line$' -e meta` to measure
        // the effects here.
        //
        // Note that this doesn't eliminate the latency effects of using the
        // pool. There is still some (minor) cost for the "thread owner" of the
        // pool. (i.e., The thread that first calls a regex search routine.)
        // However, for other threads using the regex, the pool access can be
        // quite expensive as it goes through a mutex. Callers can avoid this
        // by either cloning the Regex (which creates a distinct copy of the
        // pool), or callers can use the lower level APIs that accept a 'Cache'
        // directly and do their own handling.
        PoolGuard::put(guard);
        result
    }

    /// Returns the end offset of the leftmost match. If no match exists, then
    /// `None` is returned.
    ///
    /// This is distinct from [`Regex::search`] in that it only returns the end
    /// of a match and not the start of the match. Depending on a variety of
    /// implementation details, this _may_ permit the regex engine to do less
    /// overall work. For example, if a DFA is being used to execute a search,
    /// then the start of a match usually requires running a separate DFA in
    /// reverse to the find the start of a match. If one only needs the end of
    /// a match, then the separate reverse scan to find the start of a match
    /// can be skipped. (Note that the reverse scan is avoided even when using
    /// `Regex::search` when possible, for example, in the case of an anchored
    /// search.)
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Input, HalfMatch};
    ///
    /// let re = Regex::new(r"Samwise|Sam")?;
    /// let input = Input::new(
    ///     "one of the chief characters, Samwise the Brave",
    /// );
    /// assert_eq!(Some(HalfMatch::must(0, 36)), re.search_half(&input));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn search_half(&self, input: &Input<'_>) -> Option<HalfMatch> {
        if self.imp.info.is_impossible(input) {
            return None;
        }
        let mut guard = self.pool.get();
        let result = self.imp.strat.search_half(&mut guard, input);
        // See 'Regex::search' for why we put the guard back explicitly.
        PoolGuard::put(guard);
        result
    }

    /// Executes a leftmost forward search and writes the spans of capturing
    /// groups that participated in a match into the provided [`Captures`]
    /// value. If no match was found, then [`Captures::is_match`] is guaranteed
    /// to return `false`.
    ///
    /// This is like [`Regex::captures`], but it accepts a concrete `&Input`
    /// instead of an `Into<Input>`.
    ///
    /// # Example: specific pattern search
    ///
    /// This example shows how to build a multi-pattern `Regex` that permits
    /// searching for specific patterns.
    ///
    /// ```
    /// use regex_automata::{
    ///     meta::Regex,
    ///     Anchored, Match, PatternID, Input,
    /// };
    ///
    /// let re = Regex::new_many(&["[a-z0-9]{6}", "[a-z][a-z0-9]{5}"])?;
    /// let mut caps = re.create_captures();
    /// let haystack = "foo123";
    ///
    /// // Since we are using the default leftmost-first match and both
    /// // patterns match at the same starting position, only the first pattern
    /// // will be returned in this case when doing a search for any of the
    /// // patterns.
    /// let expected = Some(Match::must(0, 0..6));
    /// re.search_captures(&Input::new(haystack), &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// // But if we want to check whether some other pattern matches, then we
    /// // can provide its pattern ID.
    /// let expected = Some(Match::must(1, 0..6));
    /// let input = Input::new(haystack)
    ///     .anchored(Anchored::Pattern(PatternID::must(1)));
    /// re.search_captures(&input, &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: specifying the bounds of a search
    ///
    /// This example shows how providing the bounds of a search can produce
    /// different results than simply sub-slicing the haystack.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{meta::Regex, Match, Input};
    ///
    /// let re = Regex::new(r"\b[0-9]{3}\b")?;
    /// let mut caps = re.create_captures();
    /// let haystack = "foo123bar";
    ///
    /// // Since we sub-slice the haystack, the search doesn't know about
    /// // the larger context and assumes that `123` is surrounded by word
    /// // boundaries. And of course, the match position is reported relative
    /// // to the sub-slice as well, which means we get `0..3` instead of
    /// // `3..6`.
    /// let expected = Some(Match::must(0, 0..3));
    /// let input = Input::new(&haystack[3..6]);
    /// re.search_captures(&input, &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// // But if we provide the bounds of the search within the context of the
    /// // entire haystack, then the search can take the surrounding context
    /// // into account. (And if we did find a match, it would be reported
    /// // as a valid offset into `haystack` instead of its sub-slice.)
    /// let expected = None;
    /// let input = Input::new(haystack).range(3..6);
    /// re.search_captures(&input, &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn search_captures(&self, input: &Input<'_>, caps: &mut Captures) {
        caps.set_pattern(None);
        let pid = self.search_slots(input, caps.slots_mut());
        caps.set_pattern(pid);
    }

    /// Executes a leftmost forward search and writes the spans of capturing
    /// groups that participated in a match into the provided `slots`, and
    /// returns the matching pattern ID. The contents of the slots for patterns
    /// other than the matching pattern are unspecified. If no match was found,
    /// then `None` is returned and the contents of `slots` is unspecified.
    ///
    /// This is like [`Regex::search`], but it accepts a raw slots slice
    /// instead of a `Captures` value. This is useful in contexts where you
    /// don't want or need to allocate a `Captures`.
    ///
    /// It is legal to pass _any_ number of slots to this routine. If the regex
    /// engine would otherwise write a slot offset that doesn't fit in the
    /// provided slice, then it is simply skipped. In general though, there are
    /// usually three slice lengths you might want to use:
    ///
    /// * An empty slice, if you only care about which pattern matched.
    /// * A slice with [`pattern_len() * 2`](Regex::pattern_len) slots, if you
    /// only care about the overall match spans for each matching pattern.
    /// * A slice with
    /// [`slot_len()`](crate::util::captures::GroupInfo::slot_len) slots, which
    /// permits recording match offsets for every capturing group in every
    /// pattern.
    ///
    /// # Example
    ///
    /// This example shows how to find the overall match offsets in a
    /// multi-pattern search without allocating a `Captures` value. Indeed, we
    /// can put our slots right on the stack.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{meta::Regex, PatternID, Input};
    ///
    /// let re = Regex::new_many(&[
    ///     r"\pL+",
    ///     r"\d+",
    /// ])?;
    /// let input = Input::new("!@#123");
    ///
    /// // We only care about the overall match offsets here, so we just
    /// // allocate two slots for each pattern. Each slot records the start
    /// // and end of the match.
    /// let mut slots = [None; 4];
    /// let pid = re.search_slots(&input, &mut slots);
    /// assert_eq!(Some(PatternID::must(1)), pid);
    ///
    /// // The overall match offsets are always at 'pid * 2' and 'pid * 2 + 1'.
    /// // See 'GroupInfo' for more details on the mapping between groups and
    /// // slot indices.
    /// let slot_start = pid.unwrap().as_usize() * 2;
    /// let slot_end = slot_start + 1;
    /// assert_eq!(Some(3), slots[slot_start].map(|s| s.get()));
    /// assert_eq!(Some(6), slots[slot_end].map(|s| s.get()));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn search_slots(
        &self,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        if self.imp.info.is_impossible(input) {
            return None;
        }
        let mut guard = self.pool.get();
        let result = self.imp.strat.search_slots(&mut guard, input, slots);
        // See 'Regex::search' for why we put the guard back explicitly.
        PoolGuard::put(guard);
        result
    }

    /// Writes the set of patterns that match anywhere in the given search
    /// configuration to `patset`. If multiple patterns match at the same
    /// position and this `Regex` was configured with [`MatchKind::All`]
    /// semantics, then all matching patterns are written to the given set.
    ///
    /// Unless all of the patterns in this `Regex` are anchored, then generally
    /// speaking, this will scan the entire haystack.
    ///
    /// This search routine *does not* clear the pattern set. This gives some
    /// flexibility to the caller (e.g., running multiple searches with the
    /// same pattern set), but does make the API bug-prone if you're reusing
    /// the same pattern set for multiple searches but intended them to be
    /// independent.
    ///
    /// If a pattern ID matched but the given `PatternSet` does not have
    /// sufficient capacity to store it, then it is not inserted and silently
    /// dropped.
    ///
    /// # Example
    ///
    /// This example shows how to find all matching patterns in a haystack,
    /// even when some patterns match at the same position as other patterns.
    /// It is important that we configure the `Regex` with [`MatchKind::All`]
    /// semantics here, or else overlapping matches will not be reported.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{meta::Regex, Input, MatchKind, PatternSet};
    ///
    /// let patterns = &[
    ///     r"\w+", r"\d+", r"\pL+", r"foo", r"bar", r"barfoo", r"foobar",
    /// ];
    /// let re = Regex::builder()
    ///     .configure(Regex::config().match_kind(MatchKind::All))
    ///     .build_many(patterns)?;
    ///
    /// let input = Input::new("foobar");
    /// let mut patset = PatternSet::new(re.pattern_len());
    /// re.which_overlapping_matches(&input, &mut patset);
    /// let expected = vec![0, 2, 3, 4, 6];
    /// let got: Vec<usize> = patset.iter().map(|p| p.as_usize()).collect();
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn which_overlapping_matches(
        &self,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) {
        if self.imp.info.is_impossible(input) {
            return;
        }
        let mut guard = self.pool.get();
        let result = self
            .imp
            .strat
            .which_overlapping_matches(&mut guard, input, patset);
        // See 'Regex::search' for why we put the guard back explicitly.
        PoolGuard::put(guard);
        result
    }
}

/// Lower level search routines that give more control, and require the caller
/// to provide an explicit [`Cache`] parameter.
impl Regex {
    /// This is like [`Regex::search`], but requires the caller to
    /// explicitly pass a [`Cache`].
    ///
    /// # Why pass a `Cache` explicitly?
    ///
    /// Passing a `Cache` explicitly will bypass the use of an internal memory
    /// pool used by `Regex` to get a `Cache` for a search. The use of this
    /// pool can be slower in some cases when a `Regex` is used from multiple
    /// threads simultaneously. Typically, performance only becomes an issue
    /// when there is heavy contention, which in turn usually only occurs
    /// when each thread's primary unit of work is a regex search on a small
    /// haystack.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Input, Match};
    ///
    /// let re = Regex::new(r"Samwise|Sam")?;
    /// let mut cache = re.create_cache();
    /// let input = Input::new(
    ///     "one of the chief characters, Samwise the Brave",
    /// );
    /// assert_eq!(
    ///     Some(Match::must(0, 29..36)),
    ///     re.search_with(&mut cache, &input),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn search_with(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Option<Match> {
        if self.imp.info.is_impossible(input) {
            return None;
        }
        self.imp.strat.search(cache, input)
    }

    /// This is like [`Regex::search_half`], but requires the caller to
    /// explicitly pass a [`Cache`].
    ///
    /// # Why pass a `Cache` explicitly?
    ///
    /// Passing a `Cache` explicitly will bypass the use of an internal memory
    /// pool used by `Regex` to get a `Cache` for a search. The use of this
    /// pool can be slower in some cases when a `Regex` is used from multiple
    /// threads simultaneously. Typically, performance only becomes an issue
    /// when there is heavy contention, which in turn usually only occurs
    /// when each thread's primary unit of work is a regex search on a small
    /// haystack.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Input, HalfMatch};
    ///
    /// let re = Regex::new(r"Samwise|Sam")?;
    /// let mut cache = re.create_cache();
    /// let input = Input::new(
    ///     "one of the chief characters, Samwise the Brave",
    /// );
    /// assert_eq!(
    ///     Some(HalfMatch::must(0, 36)),
    ///     re.search_half_with(&mut cache, &input),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn search_half_with(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Option<HalfMatch> {
        if self.imp.info.is_impossible(input) {
            return None;
        }
        self.imp.strat.search_half(cache, input)
    }

    /// This is like [`Regex::search_captures`], but requires the caller to
    /// explicitly pass a [`Cache`].
    ///
    /// # Why pass a `Cache` explicitly?
    ///
    /// Passing a `Cache` explicitly will bypass the use of an internal memory
    /// pool used by `Regex` to get a `Cache` for a search. The use of this
    /// pool can be slower in some cases when a `Regex` is used from multiple
    /// threads simultaneously. Typically, performance only becomes an issue
    /// when there is heavy contention, which in turn usually only occurs
    /// when each thread's primary unit of work is a regex search on a small
    /// haystack.
    ///
    /// # Example: specific pattern search
    ///
    /// This example shows how to build a multi-pattern `Regex` that permits
    /// searching for specific patterns.
    ///
    /// ```
    /// use regex_automata::{
    ///     meta::Regex,
    ///     Anchored, Match, PatternID, Input,
    /// };
    ///
    /// let re = Regex::new_many(&["[a-z0-9]{6}", "[a-z][a-z0-9]{5}"])?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    /// let haystack = "foo123";
    ///
    /// // Since we are using the default leftmost-first match and both
    /// // patterns match at the same starting position, only the first pattern
    /// // will be returned in this case when doing a search for any of the
    /// // patterns.
    /// let expected = Some(Match::must(0, 0..6));
    /// re.search_captures_with(&mut cache, &Input::new(haystack), &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// // But if we want to check whether some other pattern matches, then we
    /// // can provide its pattern ID.
    /// let expected = Some(Match::must(1, 0..6));
    /// let input = Input::new(haystack)
    ///     .anchored(Anchored::Pattern(PatternID::must(1)));
    /// re.search_captures_with(&mut cache, &input, &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: specifying the bounds of a search
    ///
    /// This example shows how providing the bounds of a search can produce
    /// different results than simply sub-slicing the haystack.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{meta::Regex, Match, Input};
    ///
    /// let re = Regex::new(r"\b[0-9]{3}\b")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    /// let haystack = "foo123bar";
    ///
    /// // Since we sub-slice the haystack, the search doesn't know about
    /// // the larger context and assumes that `123` is surrounded by word
    /// // boundaries. And of course, the match position is reported relative
    /// // to the sub-slice as well, which means we get `0..3` instead of
    /// // `3..6`.
    /// let expected = Some(Match::must(0, 0..3));
    /// let input = Input::new(&haystack[3..6]);
    /// re.search_captures_with(&mut cache, &input, &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// // But if we provide the bounds of the search within the context of the
    /// // entire haystack, then the search can take the surrounding context
    /// // into account. (And if we did find a match, it would be reported
    /// // as a valid offset into `haystack` instead of its sub-slice.)
    /// let expected = None;
    /// let input = Input::new(haystack).range(3..6);
    /// re.search_captures_with(&mut cache, &input, &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn search_captures_with(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        caps: &mut Captures,
    ) {
        caps.set_pattern(None);
        let pid = self.search_slots_with(cache, input, caps.slots_mut());
        caps.set_pattern(pid);
    }

    /// This is like [`Regex::search_slots`], but requires the caller to
    /// explicitly pass a [`Cache`].
    ///
    /// # Why pass a `Cache` explicitly?
    ///
    /// Passing a `Cache` explicitly will bypass the use of an internal memory
    /// pool used by `Regex` to get a `Cache` for a search. The use of this
    /// pool can be slower in some cases when a `Regex` is used from multiple
    /// threads simultaneously. Typically, performance only becomes an issue
    /// when there is heavy contention, which in turn usually only occurs
    /// when each thread's primary unit of work is a regex search on a small
    /// haystack.
    ///
    /// # Example
    ///
    /// This example shows how to find the overall match offsets in a
    /// multi-pattern search without allocating a `Captures` value. Indeed, we
    /// can put our slots right on the stack.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{meta::Regex, PatternID, Input};
    ///
    /// let re = Regex::new_many(&[
    ///     r"\pL+",
    ///     r"\d+",
    /// ])?;
    /// let mut cache = re.create_cache();
    /// let input = Input::new("!@#123");
    ///
    /// // We only care about the overall match offsets here, so we just
    /// // allocate two slots for each pattern. Each slot records the start
    /// // and end of the match.
    /// let mut slots = [None; 4];
    /// let pid = re.search_slots_with(&mut cache, &input, &mut slots);
    /// assert_eq!(Some(PatternID::must(1)), pid);
    ///
    /// // The overall match offsets are always at 'pid * 2' and 'pid * 2 + 1'.
    /// // See 'GroupInfo' for more details on the mapping between groups and
    /// // slot indices.
    /// let slot_start = pid.unwrap().as_usize() * 2;
    /// let slot_end = slot_start + 1;
    /// assert_eq!(Some(3), slots[slot_start].map(|s| s.get()));
    /// assert_eq!(Some(6), slots[slot_end].map(|s| s.get()));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn search_slots_with(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        if self.imp.info.is_impossible(input) {
            return None;
        }
        self.imp.strat.search_slots(cache, input, slots)
    }

    /// This is like [`Regex::which_overlapping_matches`], but requires the
    /// caller to explicitly pass a [`Cache`].
    ///
    /// Passing a `Cache` explicitly will bypass the use of an internal memory
    /// pool used by `Regex` to get a `Cache` for a search. The use of this
    /// pool can be slower in some cases when a `Regex` is used from multiple
    /// threads simultaneously. Typically, performance only becomes an issue
    /// when there is heavy contention, which in turn usually only occurs
    /// when each thread's primary unit of work is a regex search on a small
    /// haystack.
    ///
    /// # Why pass a `Cache` explicitly?
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{meta::Regex, Input, MatchKind, PatternSet};
    ///
    /// let patterns = &[
    ///     r"\w+", r"\d+", r"\pL+", r"foo", r"bar", r"barfoo", r"foobar",
    /// ];
    /// let re = Regex::builder()
    ///     .configure(Regex::config().match_kind(MatchKind::All))
    ///     .build_many(patterns)?;
    /// let mut cache = re.create_cache();
    ///
    /// let input = Input::new("foobar");
    /// let mut patset = PatternSet::new(re.pattern_len());
    /// re.which_overlapping_matches_with(&mut cache, &input, &mut patset);
    /// let expected = vec![0, 2, 3, 4, 6];
    /// let got: Vec<usize> = patset.iter().map(|p| p.as_usize()).collect();
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn which_overlapping_matches_with(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) {
        if self.imp.info.is_impossible(input) {
            return;
        }
        self.imp.strat.which_overlapping_matches(cache, input, patset)
    }
}

/// Various non-search routines for querying properties of a `Regex` and
/// convenience routines for creating [`Captures`] and [`Cache`] values.
impl Regex {
    /// Creates a new object for recording capture group offsets. This is used
    /// in search APIs like [`Regex::captures`] and [`Regex::search_captures`].
    ///
    /// This is a convenience routine for
    /// `Captures::all(re.group_info().clone())`. Callers may build other types
    /// of `Captures` values that record less information (and thus require
    /// less work from the regex engine) using [`Captures::matches`] and
    /// [`Captures::empty`].
    ///
    /// # Example
    ///
    /// This shows some alternatives to [`Regex::create_captures`]:
    ///
    /// ```
    /// use regex_automata::{
    ///     meta::Regex,
    ///     util::captures::Captures,
    ///     Match, PatternID, Span,
    /// };
    ///
    /// let re = Regex::new(r"(?<first>[A-Z][a-z]+) (?<last>[A-Z][a-z]+)")?;
    ///
    /// // This is equivalent to Regex::create_captures. It stores matching
    /// // offsets for all groups in the regex.
    /// let mut all = Captures::all(re.group_info().clone());
    /// re.captures("Bruce Springsteen", &mut all);
    /// assert_eq!(Some(Match::must(0, 0..17)), all.get_match());
    /// assert_eq!(Some(Span::from(0..5)), all.get_group_by_name("first"));
    /// assert_eq!(Some(Span::from(6..17)), all.get_group_by_name("last"));
    ///
    /// // In this version, we only care about the implicit groups, which
    /// // means offsets for the explicit groups will be unavailable. It can
    /// // sometimes be faster to ask for fewer groups, since the underlying
    /// // regex engine needs to do less work to keep track of them.
    /// let mut matches = Captures::matches(re.group_info().clone());
    /// re.captures("Bruce Springsteen", &mut matches);
    /// // We still get the overall match info.
    /// assert_eq!(Some(Match::must(0, 0..17)), matches.get_match());
    /// // But now the explicit groups are unavailable.
    /// assert_eq!(None, matches.get_group_by_name("first"));
    /// assert_eq!(None, matches.get_group_by_name("last"));
    ///
    /// // Finally, in this version, we don't ask to keep track of offsets for
    /// // *any* groups. All we get back is whether a match occurred, and if
    /// // so, the ID of the pattern that matched.
    /// let mut empty = Captures::empty(re.group_info().clone());
    /// re.captures("Bruce Springsteen", &mut empty);
    /// // it's a match!
    /// assert!(empty.is_match());
    /// // for pattern ID 0
    /// assert_eq!(Some(PatternID::ZERO), empty.pattern());
    /// // Match offsets are unavailable.
    /// assert_eq!(None, empty.get_match());
    /// // And of course, explicit groups are unavailable too.
    /// assert_eq!(None, empty.get_group_by_name("first"));
    /// assert_eq!(None, empty.get_group_by_name("last"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn create_captures(&self) -> Captures {
        Captures::all(self.group_info().clone())
    }

    /// Creates a new cache for use with lower level search APIs like
    /// [`Regex::search_with`].
    ///
    /// The cache returned should only be used for searches for this `Regex`.
    /// If you want to reuse the cache for another `Regex`, then you must call
    /// [`Cache::reset`] with that `Regex`.
    ///
    /// This is a convenience routine for [`Cache::new`].
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Input, Match};
    ///
    /// let re = Regex::new(r"(?-u)m\w+\s+m\w+")?;
    /// let mut cache = re.create_cache();
    /// let input = Input::new("crazy janey and her mission man");
    /// assert_eq!(
    ///     Some(Match::must(0, 20..31)),
    ///     re.search_with(&mut cache, &input),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn create_cache(&self) -> Cache {
        self.imp.strat.create_cache()
    }

    /// Returns the total number of patterns in this regex.
    ///
    /// The standard [`Regex::new`] constructor always results in a `Regex`
    /// with a single pattern, but [`Regex::new_many`] permits building a
    /// multi-pattern regex.
    ///
    /// A `Regex` guarantees that the maximum possible `PatternID` returned in
    /// any match is `Regex::pattern_len() - 1`. In the case where the number
    /// of patterns is `0`, a match is impossible.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let re = Regex::new(r"(?m)^[a-z]$")?;
    /// assert_eq!(1, re.pattern_len());
    ///
    /// let re = Regex::new_many::<&str>(&[])?;
    /// assert_eq!(0, re.pattern_len());
    ///
    /// let re = Regex::new_many(&["a", "b", "c"])?;
    /// assert_eq!(3, re.pattern_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn pattern_len(&self) -> usize {
        self.imp.info.pattern_len()
    }

    /// Returns the total number of capturing groups.
    ///
    /// This includes the implicit capturing group corresponding to the
    /// entire match. Therefore, the minimum value returned is `1`.
    ///
    /// # Example
    ///
    /// This shows a few patterns and how many capture groups they have.
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let len = |pattern| {
    ///     Regex::new(pattern).map(|re| re.captures_len())
    /// };
    ///
    /// assert_eq!(1, len("a")?);
    /// assert_eq!(2, len("(a)")?);
    /// assert_eq!(3, len("(a)|(b)")?);
    /// assert_eq!(5, len("(a)(b)|(c)(d)")?);
    /// assert_eq!(2, len("(a)|b")?);
    /// assert_eq!(2, len("a|(b)")?);
    /// assert_eq!(2, len("(b)*")?);
    /// assert_eq!(2, len("(b)+")?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: multiple patterns
    ///
    /// This routine also works for multiple patterns. The total number is
    /// the sum of the capture groups of each pattern.
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let len = |patterns| {
    ///     Regex::new_many(patterns).map(|re| re.captures_len())
    /// };
    ///
    /// assert_eq!(2, len(&["a", "b"])?);
    /// assert_eq!(4, len(&["(a)", "(b)"])?);
    /// assert_eq!(6, len(&["(a)|(b)", "(c)|(d)"])?);
    /// assert_eq!(8, len(&["(a)(b)|(c)(d)", "(x)(y)"])?);
    /// assert_eq!(3, len(&["(a)", "b"])?);
    /// assert_eq!(3, len(&["a", "(b)"])?);
    /// assert_eq!(4, len(&["(a)", "(b)*"])?);
    /// assert_eq!(4, len(&["(a)+", "(b)+"])?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn captures_len(&self) -> usize {
        self.imp
            .info
            .props_union()
            .explicit_captures_len()
            .saturating_add(self.pattern_len())
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
    /// use regex_automata::meta::Regex;
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
    ///
    /// # Example: multiple patterns
    ///
    /// This property extends to regexes with multiple patterns as well. In
    /// order for their to be a static number of capture groups in this case,
    /// every pattern must have the same static number.
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// let len = |patterns| {
    ///     Regex::new_many(patterns).map(|re| re.static_captures_len())
    /// };
    ///
    /// assert_eq!(Some(1), len(&["a", "b"])?);
    /// assert_eq!(Some(2), len(&["(a)", "(b)"])?);
    /// assert_eq!(Some(2), len(&["(a)|(b)", "(c)|(d)"])?);
    /// assert_eq!(Some(3), len(&["(a)(b)|(c)(d)", "(x)(y)"])?);
    /// assert_eq!(None, len(&["(a)", "b"])?);
    /// assert_eq!(None, len(&["a", "(b)"])?);
    /// assert_eq!(None, len(&["(a)", "(b)*"])?);
    /// assert_eq!(Some(2), len(&["(a)+", "(b)+"])?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn static_captures_len(&self) -> Option<usize> {
        self.imp
            .info
            .props_union()
            .static_explicit_captures_len()
            .map(|len| len.saturating_add(1))
    }

    /// Return information about the capture groups in this `Regex`.
    ///
    /// A `GroupInfo` is an immutable object that can be cheaply cloned. It
    /// is responsible for maintaining a mapping between the capture groups
    /// in the concrete syntax of zero or more regex patterns and their
    /// internal representation used by some of the regex matchers. It is also
    /// responsible for maintaining a mapping between the name of each group
    /// (if one exists) and its corresponding group index.
    ///
    /// A `GroupInfo` is ultimately what is used to build a [`Captures`] value,
    /// which is some mutable space where group offsets are stored as a result
    /// of a search.
    ///
    /// # Example
    ///
    /// This shows some alternatives to [`Regex::create_captures`]:
    ///
    /// ```
    /// use regex_automata::{
    ///     meta::Regex,
    ///     util::captures::Captures,
    ///     Match, PatternID, Span,
    /// };
    ///
    /// let re = Regex::new(r"(?<first>[A-Z][a-z]+) (?<last>[A-Z][a-z]+)")?;
    ///
    /// // This is equivalent to Regex::create_captures. It stores matching
    /// // offsets for all groups in the regex.
    /// let mut all = Captures::all(re.group_info().clone());
    /// re.captures("Bruce Springsteen", &mut all);
    /// assert_eq!(Some(Match::must(0, 0..17)), all.get_match());
    /// assert_eq!(Some(Span::from(0..5)), all.get_group_by_name("first"));
    /// assert_eq!(Some(Span::from(6..17)), all.get_group_by_name("last"));
    ///
    /// // In this version, we only care about the implicit groups, which
    /// // means offsets for the explicit groups will be unavailable. It can
    /// // sometimes be faster to ask for fewer groups, since the underlying
    /// // regex engine needs to do less work to keep track of them.
    /// let mut matches = Captures::matches(re.group_info().clone());
    /// re.captures("Bruce Springsteen", &mut matches);
    /// // We still get the overall match info.
    /// assert_eq!(Some(Match::must(0, 0..17)), matches.get_match());
    /// // But now the explicit groups are unavailable.
    /// assert_eq!(None, matches.get_group_by_name("first"));
    /// assert_eq!(None, matches.get_group_by_name("last"));
    ///
    /// // Finally, in this version, we don't ask to keep track of offsets for
    /// // *any* groups. All we get back is whether a match occurred, and if
    /// // so, the ID of the pattern that matched.
    /// let mut empty = Captures::empty(re.group_info().clone());
    /// re.captures("Bruce Springsteen", &mut empty);
    /// // it's a match!
    /// assert!(empty.is_match());
    /// // for pattern ID 0
    /// assert_eq!(Some(PatternID::ZERO), empty.pattern());
    /// // Match offsets are unavailable.
    /// assert_eq!(None, empty.get_match());
    /// // And of course, explicit groups are unavailable too.
    /// assert_eq!(None, empty.get_group_by_name("first"));
    /// assert_eq!(None, empty.get_group_by_name("last"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn group_info(&self) -> &GroupInfo {
        self.imp.strat.group_info()
    }

    /// Returns the configuration object used to build this `Regex`.
    ///
    /// If no configuration object was explicitly passed, then the
    /// configuration returned represents the default.
    #[inline]
    pub fn get_config(&self) -> &Config {
        self.imp.info.config()
    }

    /// Returns true if this regex has a high chance of being "accelerated."
    ///
    /// The precise meaning of "accelerated" is specifically left unspecified,
    /// but the general meaning is that the search is a high likelihood of
    /// running faster than a character-at-a-time loop inside a standard
    /// regex engine.
    ///
    /// When a regex is accelerated, it is only a *probabilistic* claim. That
    /// is, just because the regex is believed to be accelerated, that doesn't
    /// mean it will definitely execute searches very fast. Similarly, if a
    /// regex is *not* accelerated, that is also a probabilistic claim. That
    /// is, a regex for which `is_accelerated` returns `false` could still run
    /// searches more quickly than a regex for which `is_accelerated` returns
    /// `true`.
    ///
    /// Whether a regex is marked as accelerated or not is dependent on
    /// implementations details that may change in a semver compatible release.
    /// That is, a regex that is accelerated in a `x.y.1` release might not be
    /// accelerated in a `x.y.2` release.
    ///
    /// Basically, the value of acceleration boils down to a hedge: a hodge
    /// podge of internal heuristics combine to make a probabilistic guess
    /// that this regex search may run "fast." The value in knowing this from
    /// a caller's perspective is that it may act as a signal that no further
    /// work should be done to accelerate a search. For example, a grep-like
    /// tool might try to do some extra work extracting literals from a regex
    /// to create its own heuristic acceleration strategies. But it might
    /// choose to defer to this crate's acceleration strategy if one exists.
    /// This routine permits querying whether such a strategy is active for a
    /// particular regex.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::meta::Regex;
    ///
    /// // A simple literal is very likely to be accelerated.
    /// let re = Regex::new(r"foo")?;
    /// assert!(re.is_accelerated());
    ///
    /// // A regex with no literals is likely to not be accelerated.
    /// let re = Regex::new(r"\w")?;
    /// assert!(!re.is_accelerated());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_accelerated(&self) -> bool {
        self.imp.strat.is_accelerated()
    }

    /// Return the total approximate heap memory, in bytes, used by this `Regex`.
    ///
    /// Note that currently, there is no high level configuration for setting
    /// a limit on the specific value returned by this routine. Instead, the
    /// following routines can be used to control heap memory at a bit of a
    /// lower level:
    ///
    /// * [`Config::nfa_size_limit`] controls how big _any_ of the NFAs are
    /// allowed to be.
    /// * [`Config::onepass_size_limit`] controls how big the one-pass DFA is
    /// allowed to be.
    /// * [`Config::hybrid_cache_capacity`] controls how much memory the lazy
    /// DFA is permitted to allocate to store its transition table.
    /// * [`Config::dfa_size_limit`] controls how big a fully compiled DFA is
    /// allowed to be.
    /// * [`Config::dfa_state_limit`] controls the conditions under which the
    /// meta regex engine will even attempt to build a fully compiled DFA.
    #[inline]
    pub fn memory_usage(&self) -> usize {
        self.imp.strat.memory_usage()
    }
}

impl Clone for Regex {
    fn clone(&self) -> Regex {
        let imp = Arc::clone(&self.imp);
        let pool = {
            let strat = Arc::clone(&imp.strat);
            let create: CachePoolFn = Box::new(move || strat.create_cache());
            Pool::new(create)
        };
        Regex { imp, pool }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RegexInfo(Arc<RegexInfoI>);

#[derive(Clone, Debug)]
struct RegexInfoI {
    config: Config,
    props: Vec<hir::Properties>,
    props_union: hir::Properties,
}

impl RegexInfo {
    fn new(config: Config, hirs: &[&Hir]) -> RegexInfo {
        // Collect all of the properties from each of the HIRs, and also
        // union them into one big set of properties representing all HIRs
        // as if they were in one big alternation.
        let mut props = vec![];
        for hir in hirs.iter() {
            props.push(hir.properties().clone());
        }
        let props_union = hir::Properties::union(&props);

        RegexInfo(Arc::new(RegexInfoI { config, props, props_union }))
    }

    pub(crate) fn config(&self) -> &Config {
        &self.0.config
    }

    pub(crate) fn props(&self) -> &[hir::Properties] {
        &self.0.props
    }

    pub(crate) fn props_union(&self) -> &hir::Properties {
        &self.0.props_union
    }

    pub(crate) fn pattern_len(&self) -> usize {
        self.props().len()
    }

    pub(crate) fn memory_usage(&self) -> usize {
        self.props().iter().map(|p| p.memory_usage()).sum::<usize>()
            + self.props_union().memory_usage()
    }

    /// Returns true when the search is guaranteed to be anchored. That is,
    /// when a match is reported, its offset is guaranteed to correspond to
    /// the start of the search.
    ///
    /// This includes returning true when `input` _isn't_ anchored but the
    /// underlying regex is.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn is_anchored_start(&self, input: &Input<'_>) -> bool {
        input.get_anchored().is_anchored() || self.is_always_anchored_start()
    }

    /// Returns true when this regex is always anchored to the start of a
    /// search. And in particular, that regardless of an `Input` configuration,
    /// if any match is reported it must start at `0`.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn is_always_anchored_start(&self) -> bool {
        use regex_syntax::hir::Look;
        self.props_union().look_set_prefix().contains(Look::Start)
    }

    /// Returns true when this regex is always anchored to the end of a
    /// search. And in particular, that regardless of an `Input` configuration,
    /// if any match is reported it must end at the end of the haystack.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn is_always_anchored_end(&self) -> bool {
        use regex_syntax::hir::Look;
        self.props_union().look_set_suffix().contains(Look::End)
    }

    /// Returns true if and only if it is known that a match is impossible
    /// for the given input. This is useful for short-circuiting and avoiding
    /// running the regex engine if it's known no match can be reported.
    ///
    /// Note that this doesn't necessarily detect every possible case. For
    /// example, when `pattern_len() == 0`, a match is impossible, but that
    /// case is so rare that it's fine to be handled by the regex engine
    /// itself. That is, it's not worth the cost of adding it here in order to
    /// make it a little faster. The reason is that this is called for every
    /// search. so there is some cost to adding checks here. Arguably, some of
    /// the checks that are here already probably shouldn't be here...
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_impossible(&self, input: &Input<'_>) -> bool {
        // The underlying regex is anchored, so if we don't start the search
        // at position 0, a match is impossible, because the anchor can only
        // match at position 0.
        if input.start() > 0 && self.is_always_anchored_start() {
            return true;
        }
        // Same idea, but for the end anchor.
        if input.end() < input.haystack().len()
            && self.is_always_anchored_end()
        {
            return true;
        }
        // If the haystack is smaller than the minimum length required, then
        // we know there can be no match.
        let minlen = match self.props_union().minimum_len() {
            None => return false,
            Some(minlen) => minlen,
        };
        if input.get_span().len() < minlen {
            return true;
        }
        // Same idea as minimum, but for maximum. This is trickier. We can
        // only apply the maximum when we know the entire span that we're
        // searching *has* to match according to the regex (and possibly the
        // input configuration). If we know there is too much for the regex
        // to match, we can bail early.
        //
        // I don't think we can apply the maximum otherwise unfortunately.
        if self.is_anchored_start(input) && self.is_always_anchored_end() {
            let maxlen = match self.props_union().maximum_len() {
                None => return false,
                Some(maxlen) => maxlen,
            };
            if input.get_span().len() > maxlen {
                return true;
            }
        }
        false
    }
}

/// An iterator over all non-overlapping matches.
///
/// The iterator yields a [`Match`] value until no more matches could be found.
///
/// The lifetime parameters are as follows:
///
/// * `'r` represents the lifetime of the `Regex` that produced this iterator.
/// * `'h` represents the lifetime of the haystack being searched.
///
/// This iterator can be created with the [`Regex::find_iter`] method.
#[derive(Debug)]
pub struct FindMatches<'r, 'h> {
    re: &'r Regex,
    cache: CachePoolGuard<'r>,
    it: iter::Searcher<'h>,
}

impl<'r, 'h> FindMatches<'r, 'h> {
    /// Returns the `Regex` value that created this iterator.
    #[inline]
    pub fn regex(&self) -> &'r Regex {
        self.re
    }

    /// Returns the current `Input` associated with this iterator.
    ///
    /// The `start` position on the given `Input` may change during iteration,
    /// but all other values are guaranteed to remain invariant.
    #[inline]
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        self.it.input()
    }
}

impl<'r, 'h> Iterator for FindMatches<'r, 'h> {
    type Item = Match;

    #[inline]
    fn next(&mut self) -> Option<Match> {
        let FindMatches { re, ref mut cache, ref mut it } = *self;
        it.advance(|input| Ok(re.search_with(cache, input)))
    }

    #[inline]
    fn count(self) -> usize {
        // If all we care about is a count of matches, then we only need to
        // find the end position of each match. This can give us a 2x perf
        // boost in some cases, because it avoids needing to do a reverse scan
        // to find the start of a match.
        let FindMatches { re, mut cache, it } = self;
        // This does the deref for PoolGuard once instead of every iter.
        let cache = &mut *cache;
        it.into_half_matches_iter(
            |input| Ok(re.search_half_with(cache, input)),
        )
        .count()
    }
}

impl<'r, 'h> core::iter::FusedIterator for FindMatches<'r, 'h> {}

/// An iterator over all non-overlapping leftmost matches with their capturing
/// groups.
///
/// The iterator yields a [`Captures`] value until no more matches could be
/// found.
///
/// The lifetime parameters are as follows:
///
/// * `'r` represents the lifetime of the `Regex` that produced this iterator.
/// * `'h` represents the lifetime of the haystack being searched.
///
/// This iterator can be created with the [`Regex::captures_iter`] method.
#[derive(Debug)]
pub struct CapturesMatches<'r, 'h> {
    re: &'r Regex,
    cache: CachePoolGuard<'r>,
    caps: Captures,
    it: iter::Searcher<'h>,
}

impl<'r, 'h> CapturesMatches<'r, 'h> {
    /// Returns the `Regex` value that created this iterator.
    #[inline]
    pub fn regex(&self) -> &'r Regex {
        self.re
    }

    /// Returns the current `Input` associated with this iterator.
    ///
    /// The `start` position on the given `Input` may change during iteration,
    /// but all other values are guaranteed to remain invariant.
    #[inline]
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        self.it.input()
    }
}

impl<'r, 'h> Iterator for CapturesMatches<'r, 'h> {
    type Item = Captures;

    #[inline]
    fn next(&mut self) -> Option<Captures> {
        // Splitting 'self' apart seems necessary to appease borrowck.
        let CapturesMatches { re, ref mut cache, ref mut caps, ref mut it } =
            *self;
        let _ = it.advance(|input| {
            re.search_captures_with(cache, input, caps);
            Ok(caps.get_match())
        });
        if caps.is_match() {
            Some(caps.clone())
        } else {
            None
        }
    }

    #[inline]
    fn count(self) -> usize {
        let CapturesMatches { re, mut cache, it, .. } = self;
        // This does the deref for PoolGuard once instead of every iter.
        let cache = &mut *cache;
        it.into_half_matches_iter(
            |input| Ok(re.search_half_with(cache, input)),
        )
        .count()
    }
}

impl<'r, 'h> core::iter::FusedIterator for CapturesMatches<'r, 'h> {}

/// Yields all substrings delimited by a regular expression match.
///
/// The spans correspond to the offsets between matches.
///
/// The lifetime parameters are as follows:
///
/// * `'r` represents the lifetime of the `Regex` that produced this iterator.
/// * `'h` represents the lifetime of the haystack being searched.
///
/// This iterator can be created with the [`Regex::split`] method.
#[derive(Debug)]
pub struct Split<'r, 'h> {
    finder: FindMatches<'r, 'h>,
    last: usize,
}

impl<'r, 'h> Split<'r, 'h> {
    /// Returns the current `Input` associated with this iterator.
    ///
    /// The `start` position on the given `Input` may change during iteration,
    /// but all other values are guaranteed to remain invariant.
    #[inline]
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        self.finder.input()
    }
}

impl<'r, 'h> Iterator for Split<'r, 'h> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        match self.finder.next() {
            None => {
                let len = self.finder.it.input().haystack().len();
                if self.last > len {
                    None
                } else {
                    let span = Span::from(self.last..len);
                    self.last = len + 1; // Next call will return None
                    Some(span)
                }
            }
            Some(m) => {
                let span = Span::from(self.last..m.start());
                self.last = m.end();
                Some(span)
            }
        }
    }
}

impl<'r, 'h> core::iter::FusedIterator for Split<'r, 'h> {}

/// Yields at most `N` spans delimited by a regular expression match.
///
/// The spans correspond to the offsets between matches. The last span will be
/// whatever remains after splitting.
///
/// The lifetime parameters are as follows:
///
/// * `'r` represents the lifetime of the `Regex` that produced this iterator.
/// * `'h` represents the lifetime of the haystack being searched.
///
/// This iterator can be created with the [`Regex::splitn`] method.
#[derive(Debug)]
pub struct SplitN<'r, 'h> {
    splits: Split<'r, 'h>,
    limit: usize,
}

impl<'r, 'h> SplitN<'r, 'h> {
    /// Returns the current `Input` associated with this iterator.
    ///
    /// The `start` position on the given `Input` may change during iteration,
    /// but all other values are guaranteed to remain invariant.
    #[inline]
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        self.splits.input()
    }
}

impl<'r, 'h> Iterator for SplitN<'r, 'h> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        if self.limit == 0 {
            return None;
        }

        self.limit -= 1;
        if self.limit > 0 {
            return self.splits.next();
        }

        let len = self.splits.finder.it.input().haystack().len();
        if self.splits.last > len {
            // We've already returned all substrings.
            None
        } else {
            // self.n == 0, so future calls will return None immediately
            Some(Span::from(self.splits.last..len))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.limit))
    }
}

impl<'r, 'h> core::iter::FusedIterator for SplitN<'r, 'h> {}

/// Represents mutable scratch space used by regex engines during a search.
///
/// Most of the regex engines in this crate require some kind of
/// mutable state in order to execute a search. This mutable state is
/// explicitly separated from the core regex object (such as a
/// [`thompson::NFA`](crate::nfa::thompson::NFA)) so that the read-only regex
/// object can be shared across multiple threads simultaneously without any
/// synchronization. Conversely, a `Cache` must either be duplicated if using
/// the same `Regex` from multiple threads, or else there must be some kind of
/// synchronization that guarantees exclusive access while it's in use by one
/// thread.
///
/// A `Regex` attempts to do this synchronization for you by using a thread
/// pool internally. Its size scales roughly with the number of simultaneous
/// regex searches.
///
/// For cases where one does not want to rely on a `Regex`'s internal thread
/// pool, lower level routines such as [`Regex::search_with`] are provided
/// that permit callers to pass a `Cache` into the search routine explicitly.
///
/// General advice is that the thread pool is often more than good enough.
/// However, it may be possible to observe the effects of its latency,
/// especially when searching many small haystacks from many threads
/// simultaneously.
///
/// Caches can be created from their corresponding `Regex` via
/// [`Regex::create_cache`]. A cache can only be used with either the `Regex`
/// that created it, or the `Regex` that was most recently used to reset it
/// with [`Cache::reset`]. Using a cache with any other `Regex` may result in
/// panics or incorrect results.
///
/// # Example
///
/// ```
/// use regex_automata::{meta::Regex, Input, Match};
///
/// let re = Regex::new(r"(?-u)m\w+\s+m\w+")?;
/// let mut cache = re.create_cache();
/// let input = Input::new("crazy janey and her mission man");
/// assert_eq!(
///     Some(Match::must(0, 20..31)),
///     re.search_with(&mut cache, &input),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct Cache {
    pub(crate) capmatches: Captures,
    pub(crate) pikevm: wrappers::PikeVMCache,
    pub(crate) backtrack: wrappers::BoundedBacktrackerCache,
    pub(crate) onepass: wrappers::OnePassCache,
    pub(crate) hybrid: wrappers::HybridCache,
    pub(crate) revhybrid: wrappers::ReverseHybridCache,
}

impl Cache {
    /// Creates a new `Cache` for use with this regex.
    ///
    /// The cache returned should only be used for searches for the given
    /// `Regex`. If you want to reuse the cache for another `Regex`, then you
    /// must call [`Cache::reset`] with that `Regex`.
    pub fn new(re: &Regex) -> Cache {
        re.create_cache()
    }

    /// Reset this cache such that it can be used for searching with the given
    /// `Regex` (and only that `Regex`).
    ///
    /// A cache reset permits potentially reusing memory already allocated in
    /// this cache with a different `Regex`.
    ///
    /// # Example
    ///
    /// This shows how to re-purpose a cache for use with a different `Regex`.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{meta::Regex, Match, Input};
    ///
    /// let re1 = Regex::new(r"\w")?;
    /// let re2 = Regex::new(r"\W")?;
    ///
    /// let mut cache = re1.create_cache();
    /// assert_eq!(
    ///     Some(Match::must(0, 0..2)),
    ///     re1.search_with(&mut cache, &Input::new("Δ")),
    /// );
    ///
    /// // Using 'cache' with re2 is not allowed. It may result in panics or
    /// // incorrect results. In order to re-purpose the cache, we must reset
    /// // it with the Regex we'd like to use it with.
    /// //
    /// // Similarly, after this reset, using the cache with 're1' is also not
    /// // allowed.
    /// cache.reset(&re2);
    /// assert_eq!(
    ///     Some(Match::must(0, 0..3)),
    ///     re2.search_with(&mut cache, &Input::new("☃")),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn reset(&mut self, re: &Regex) {
        re.imp.strat.reset_cache(self)
    }

    /// Returns the heap memory usage, in bytes, of this cache.
    ///
    /// This does **not** include the stack size used up by this cache. To
    /// compute that, use `std::mem::size_of::<Cache>()`.
    pub fn memory_usage(&self) -> usize {
        let mut bytes = 0;
        bytes += self.pikevm.memory_usage();
        bytes += self.backtrack.memory_usage();
        bytes += self.onepass.memory_usage();
        bytes += self.hybrid.memory_usage();
        bytes += self.revhybrid.memory_usage();
        bytes
    }
}

/// An object describing the configuration of a `Regex`.
///
/// This configuration only includes options for the
/// non-syntax behavior of a `Regex`, and can be applied via the
/// [`Builder::configure`] method. For configuring the syntax options, see
/// [`util::syntax::Config`](crate::util::syntax::Config).
///
/// # Example: lower the NFA size limit
///
/// In some cases, the default size limit might be too big. The size limit can
/// be lowered, which will prevent large regex patterns from compiling.
///
/// ```
/// # if cfg!(miri) { return Ok(()); } // miri takes too long
/// use regex_automata::meta::Regex;
///
/// let result = Regex::builder()
///     .configure(Regex::config().nfa_size_limit(Some(20 * (1<<10))))
///     // Not even 20KB is enough to build a single large Unicode class!
///     .build(r"\pL");
/// assert!(result.is_err());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct Config {
    // As with other configuration types in this crate, we put all our knobs
    // in options so that we can distinguish between "default" and "not set."
    // This makes it possible to easily combine multiple configurations
    // without default values overwriting explicitly specified values. See the
    // 'overwrite' method.
    //
    // For docs on the fields below, see the corresponding method setters.
    match_kind: Option<MatchKind>,
    utf8_empty: Option<bool>,
    autopre: Option<bool>,
    pre: Option<Option<Prefilter>>,
    which_captures: Option<WhichCaptures>,
    nfa_size_limit: Option<Option<usize>>,
    onepass_size_limit: Option<Option<usize>>,
    hybrid_cache_capacity: Option<usize>,
    hybrid: Option<bool>,
    dfa: Option<bool>,
    dfa_size_limit: Option<Option<usize>>,
    dfa_state_limit: Option<Option<usize>>,
    onepass: Option<bool>,
    backtrack: Option<bool>,
    byte_classes: Option<bool>,
    line_terminator: Option<u8>,
}

impl Config {
    /// Create a new configuration object for a `Regex`.
    pub fn new() -> Config {
        Config::default()
    }

    /// Set the match semantics for a `Regex`.
    ///
    /// The default value is [`MatchKind::LeftmostFirst`].
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Match, MatchKind};
    ///
    /// // By default, leftmost-first semantics are used, which
    /// // disambiguates matches at the same position by selecting
    /// // the one that corresponds earlier in the pattern.
    /// let re = Regex::new("sam|samwise")?;
    /// assert_eq!(Some(Match::must(0, 0..3)), re.find("samwise"));
    ///
    /// // But with 'all' semantics, match priority is ignored
    /// // and all match states are included. When coupled with
    /// // a leftmost search, the search will report the last
    /// // possible match.
    /// let re = Regex::builder()
    ///     .configure(Regex::config().match_kind(MatchKind::All))
    ///     .build("sam|samwise")?;
    /// assert_eq!(Some(Match::must(0, 0..7)), re.find("samwise"));
    /// // Beware that this can lead to skipping matches!
    /// // Usually 'all' is used for anchored reverse searches
    /// // only, or for overlapping searches.
    /// assert_eq!(Some(Match::must(0, 4..11)), re.find("sam samwise"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn match_kind(self, kind: MatchKind) -> Config {
        Config { match_kind: Some(kind), ..self }
    }

    /// Toggles whether empty matches are permitted to occur between the code
    /// units of a UTF-8 encoded codepoint.
    ///
    /// This should generally be enabled when search a `&str` or anything that
    /// you otherwise know is valid UTF-8. It should be disabled in all other
    /// cases. Namely, if the haystack is not valid UTF-8 and this is enabled,
    /// then behavior is unspecified.
    ///
    /// By default, this is enabled.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Match};
    ///
    /// let re = Regex::new("")?;
    /// let got: Vec<Match> = re.find_iter("☃").collect();
    /// // Matches only occur at the beginning and end of the snowman.
    /// assert_eq!(got, vec![
    ///     Match::must(0, 0..0),
    ///     Match::must(0, 3..3),
    /// ]);
    ///
    /// let re = Regex::builder()
    ///     .configure(Regex::config().utf8_empty(false))
    ///     .build("")?;
    /// let got: Vec<Match> = re.find_iter("☃").collect();
    /// // Matches now occur at every position!
    /// assert_eq!(got, vec![
    ///     Match::must(0, 0..0),
    ///     Match::must(0, 1..1),
    ///     Match::must(0, 2..2),
    ///     Match::must(0, 3..3),
    /// ]);
    ///
    /// Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn utf8_empty(self, yes: bool) -> Config {
        Config { utf8_empty: Some(yes), ..self }
    }

    /// Toggles whether automatic prefilter support is enabled.
    ///
    /// If this is disabled and [`Config::prefilter`] is not set, then the
    /// meta regex engine will not use any prefilters. This can sometimes
    /// be beneficial in cases where you know (or have measured) that the
    /// prefilter leads to overall worse search performance.
    ///
    /// By default, this is enabled.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{meta::Regex, Match};
    ///
    /// let re = Regex::builder()
    ///     .configure(Regex::config().auto_prefilter(false))
    ///     .build(r"Bruce \w+")?;
    /// let hay = "Hello Bruce Springsteen!";
    /// assert_eq!(Some(Match::must(0, 6..23)), re.find(hay));
    ///
    /// Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn auto_prefilter(self, yes: bool) -> Config {
        Config { autopre: Some(yes), ..self }
    }

    /// Overrides and sets the prefilter to use inside a `Regex`.
    ///
    /// This permits one to forcefully set a prefilter in cases where the
    /// caller knows better than whatever the automatic prefilter logic is
    /// capable of.
    ///
    /// By default, this is set to `None` and an automatic prefilter will be
    /// used if one could be built. (Assuming [`Config::auto_prefilter`] is
    /// enabled, which it is by default.)
    ///
    /// # Example
    ///
    /// This example shows how to set your own prefilter. In the case of a
    /// pattern like `Bruce \w+`, the automatic prefilter is likely to be
    /// constructed in a way that it will look for occurrences of `Bruce `.
    /// In most cases, this is the best choice. But in some cases, it may be
    /// the case that running `memchr` on `B` is the best choice. One can
    /// achieve that behavior by overriding the automatic prefilter logic
    /// and providing a prefilter that just matches `B`.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     meta::Regex,
    ///     util::prefilter::Prefilter,
    ///     Match, MatchKind,
    /// };
    ///
    /// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["B"])
    ///     .expect("a prefilter");
    /// let re = Regex::builder()
    ///     .configure(Regex::config().prefilter(Some(pre)))
    ///     .build(r"Bruce \w+")?;
    /// let hay = "Hello Bruce Springsteen!";
    /// assert_eq!(Some(Match::must(0, 6..23)), re.find(hay));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: incorrect prefilters can lead to incorrect results!
    ///
    /// Be warned that setting an incorrect prefilter can lead to missed
    /// matches. So if you use this option, ensure your prefilter can _never_
    /// report false negatives. (A false positive is, on the other hand, quite
    /// okay and generally unavoidable.)
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     meta::Regex,
    ///     util::prefilter::Prefilter,
    ///     Match, MatchKind,
    /// };
    ///
    /// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["Z"])
    ///     .expect("a prefilter");
    /// let re = Regex::builder()
    ///     .configure(Regex::config().prefilter(Some(pre)))
    ///     .build(r"Bruce \w+")?;
    /// let hay = "Hello Bruce Springsteen!";
    /// // Oops! No match found, but there should be one!
    /// assert_eq!(None, re.find(hay));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn prefilter(self, pre: Option<Prefilter>) -> Config {
        Config { pre: Some(pre), ..self }
    }

    /// Configures what kinds of groups are compiled as "capturing" in the
    /// underlying regex engine.
    ///
    /// This is set to [`WhichCaptures::All`] by default. Callers may wish to
    /// use [`WhichCaptures::Implicit`] in cases where one wants avoid the
    /// overhead of capture states for explicit groups.
    ///
    /// Note that another approach to avoiding the overhead of capture groups
    /// is by using non-capturing groups in the regex pattern. That is,
    /// `(?:a)` instead of `(a)`. This option is useful when you can't control
    /// the concrete syntax but know that you don't need the underlying capture
    /// states. For example, using `WhichCaptures::Implicit` will behave as if
    /// all explicit capturing groups in the pattern were non-capturing.
    ///
    /// Setting this to `WhichCaptures::None` is usually not the right thing to
    /// do. When no capture states are compiled, some regex engines (such as
    /// the `PikeVM`) won't be able to report match offsets. This will manifest
    /// as no match being found.
    ///
    /// # Example
    ///
    /// This example demonstrates how the results of capture groups can change
    /// based on this option. First we show the default (all capture groups in
    /// the pattern are capturing):
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Match, Span};
    ///
    /// let re = Regex::new(r"foo([0-9]+)bar")?;
    /// let hay = "foo123bar";
    ///
    /// let mut caps = re.create_captures();
    /// re.captures(hay, &mut caps);
    /// assert_eq!(Some(Span::from(0..9)), caps.get_group(0));
    /// assert_eq!(Some(Span::from(3..6)), caps.get_group(1));
    ///
    /// Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And now we show the behavior when we only include implicit capture
    /// groups. In this case, we can only find the overall match span, but the
    /// spans of any other explicit group don't exist because they are treated
    /// as non-capturing. (In effect, when `WhichCaptures::Implicit` is used,
    /// there is no real point in using [`Regex::captures`] since it will never
    /// be able to report more information than [`Regex::find`].)
    ///
    /// ```
    /// use regex_automata::{
    ///     meta::Regex,
    ///     nfa::thompson::WhichCaptures,
    ///     Match,
    ///     Span,
    /// };
    ///
    /// let re = Regex::builder()
    ///     .configure(Regex::config().which_captures(WhichCaptures::Implicit))
    ///     .build(r"foo([0-9]+)bar")?;
    /// let hay = "foo123bar";
    ///
    /// let mut caps = re.create_captures();
    /// re.captures(hay, &mut caps);
    /// assert_eq!(Some(Span::from(0..9)), caps.get_group(0));
    /// assert_eq!(None, caps.get_group(1));
    ///
    /// Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn which_captures(mut self, which_captures: WhichCaptures) -> Config {
        self.which_captures = Some(which_captures);
        self
    }

    /// Sets the size limit, in bytes, to enforce on the construction of every
    /// NFA build by the meta regex engine.
    ///
    /// Setting it to `None` disables the limit. This is not recommended if
    /// you're compiling untrusted patterns.
    ///
    /// Note that this limit is applied to _each_ NFA built, and if any of
    /// them exceed the limit, then construction will fail. This limit does
    /// _not_ correspond to the total memory used by all NFAs in the meta regex
    /// engine.
    ///
    /// This defaults to some reasonable number that permits most reasonable
    /// patterns.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::meta::Regex;
    ///
    /// let result = Regex::builder()
    ///     .configure(Regex::config().nfa_size_limit(Some(20 * (1<<10))))
    ///     // Not even 20KB is enough to build a single large Unicode class!
    ///     .build(r"\pL");
    /// assert!(result.is_err());
    ///
    /// // But notice that building such a regex with the exact same limit
    /// // can succeed depending on other aspects of the configuration. For
    /// // example, a single *forward* NFA will (at time of writing) fit into
    /// // the 20KB limit, but a *reverse* NFA of the same pattern will not.
    /// // So if one configures a meta regex such that a reverse NFA is never
    /// // needed and thus never built, then the 20KB limit will be enough for
    /// // a pattern like \pL!
    /// let result = Regex::builder()
    ///     .configure(Regex::config()
    ///         .nfa_size_limit(Some(20 * (1<<10)))
    ///         // The DFAs are the only thing that (currently) need a reverse
    ///         // NFA. So if both are disabled, the meta regex engine will
    ///         // skip building the reverse NFA. Note that this isn't an API
    ///         // guarantee. A future semver compatible version may introduce
    ///         // new use cases for a reverse NFA.
    ///         .hybrid(false)
    ///         .dfa(false)
    ///     )
    ///     // Not even 20KB is enough to build a single large Unicode class!
    ///     .build(r"\pL");
    /// assert!(result.is_ok());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn nfa_size_limit(self, limit: Option<usize>) -> Config {
        Config { nfa_size_limit: Some(limit), ..self }
    }

    /// Sets the size limit, in bytes, for the one-pass DFA.
    ///
    /// Setting it to `None` disables the limit. Disabling the limit is
    /// strongly discouraged when compiling untrusted patterns. Even if the
    /// patterns are trusted, it still may not be a good idea, since a one-pass
    /// DFA can use a lot of memory. With that said, as the size of a regex
    /// increases, the likelihood of it being one-pass likely decreases.
    ///
    /// This defaults to some reasonable number that permits most reasonable
    /// one-pass patterns.
    ///
    /// # Example
    ///
    /// This shows how to set the one-pass DFA size limit. Note that since
    /// a one-pass DFA is an optional component of the meta regex engine,
    /// this size limit only impacts what is built internally and will never
    /// determine whether a `Regex` itself fails to build.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::meta::Regex;
    ///
    /// let result = Regex::builder()
    ///     .configure(Regex::config().onepass_size_limit(Some(2 * (1<<20))))
    ///     .build(r"\pL{5}");
    /// assert!(result.is_ok());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn onepass_size_limit(self, limit: Option<usize>) -> Config {
        Config { onepass_size_limit: Some(limit), ..self }
    }

    /// Set the cache capacity, in bytes, for the lazy DFA.
    ///
    /// The cache capacity of the lazy DFA determines approximately how much
    /// heap memory it is allowed to use to store its state transitions. The
    /// state transitions are computed at search time, and if the cache fills
    /// up it, it is cleared. At this point, any previously generated state
    /// transitions are lost and are re-generated if they're needed again.
    ///
    /// This sort of cache filling and clearing works quite well _so long as
    /// cache clearing happens infrequently_. If it happens too often, then the
    /// meta regex engine will stop using the lazy DFA and switch over to a
    /// different regex engine.
    ///
    /// In cases where the cache is cleared too often, it may be possible to
    /// give the cache more space and reduce (or eliminate) how often it is
    /// cleared. Similarly, sometimes a regex is so big that the lazy DFA isn't
    /// used at all if its cache capacity isn't big enough.
    ///
    /// The capacity set here is a _limit_ on how much memory is used. The
    /// actual memory used is only allocated as it's needed.
    ///
    /// Determining the right value for this is a little tricky and will likely
    /// required some profiling. Enabling the `logging` feature and setting the
    /// log level to `trace` will also tell you how often the cache is being
    /// cleared.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::meta::Regex;
    ///
    /// let result = Regex::builder()
    ///     .configure(Regex::config().hybrid_cache_capacity(20 * (1<<20)))
    ///     .build(r"\pL{5}");
    /// assert!(result.is_ok());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn hybrid_cache_capacity(self, limit: usize) -> Config {
        Config { hybrid_cache_capacity: Some(limit), ..self }
    }

    /// Sets the size limit, in bytes, for heap memory used for a fully
    /// compiled DFA.
    ///
    /// **NOTE:** If you increase this, you'll likely also need to increase
    /// [`Config::dfa_state_limit`].
    ///
    /// In contrast to the lazy DFA, building a full DFA requires computing
    /// all of its state transitions up front. This can be a very expensive
    /// process, and runs in worst case `2^n` time and space (where `n` is
    /// proportional to the size of the regex). However, a full DFA unlocks
    /// some additional optimization opportunities.
    ///
    /// Because full DFAs can be so expensive, the default limits for them are
    /// incredibly small. Generally speaking, if your regex is moderately big
    /// or if you're using Unicode features (`\w` is Unicode-aware by default
    /// for example), then you can expect that the meta regex engine won't even
    /// attempt to build a DFA for it.
    ///
    /// If this and [`Config::dfa_state_limit`] are set to `None`, then the
    /// meta regex will not use any sort of limits when deciding whether to
    /// build a DFA. This in turn makes construction of a `Regex` take
    /// worst case exponential time and space. Even short patterns can result
    /// in huge space blow ups. So it is strongly recommended to keep some kind
    /// of limit set!
    ///
    /// The default is set to a small number that permits some simple regexes
    /// to get compiled into DFAs in reasonable time.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::meta::Regex;
    ///
    /// let result = Regex::builder()
    ///     // 100MB is much bigger than the default.
    ///     .configure(Regex::config()
    ///         .dfa_size_limit(Some(100 * (1<<20)))
    ///         // We don't care about size too much here, so just
    ///         // remove the NFA state limit altogether.
    ///         .dfa_state_limit(None))
    ///     .build(r"\pL{5}");
    /// assert!(result.is_ok());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn dfa_size_limit(self, limit: Option<usize>) -> Config {
        Config { dfa_size_limit: Some(limit), ..self }
    }

    /// Sets a limit on the total number of NFA states, beyond which, a full
    /// DFA is not attempted to be compiled.
    ///
    /// This limit works in concert with [`Config::dfa_size_limit`]. Namely,
    /// where as `Config::dfa_size_limit` is applied by attempting to construct
    /// a DFA, this limit is used to avoid the attempt in the first place. This
    /// is useful to avoid hefty initialization costs associated with building
    /// a DFA for cases where it is obvious the DFA will ultimately be too big.
    ///
    /// By default, this is set to a very small number.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::meta::Regex;
    ///
    /// let result = Regex::builder()
    ///     .configure(Regex::config()
    ///         // Sometimes the default state limit rejects DFAs even
    ///         // if they would fit in the size limit. Here, we disable
    ///         // the check on the number of NFA states and just rely on
    ///         // the size limit.
    ///         .dfa_state_limit(None))
    ///     .build(r"(?-u)\w{30}");
    /// assert!(result.is_ok());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn dfa_state_limit(self, limit: Option<usize>) -> Config {
        Config { dfa_state_limit: Some(limit), ..self }
    }

    /// Whether to attempt to shrink the size of the alphabet for the regex
    /// pattern or not. When enabled, the alphabet is shrunk into a set of
    /// equivalence classes, where every byte in the same equivalence class
    /// cannot discriminate between a match or non-match.
    ///
    /// **WARNING:** This is only useful for debugging DFAs. Disabling this
    /// does not yield any speed advantages. Indeed, disabling it can result
    /// in much higher memory usage. Disabling byte classes is useful for
    /// debugging the actual generated transitions because it lets one see the
    /// transitions defined on actual bytes instead of the equivalence classes.
    ///
    /// This option is enabled by default and should never be disabled unless
    /// one is debugging the meta regex engine's internals.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Match};
    ///
    /// let re = Regex::builder()
    ///     .configure(Regex::config().byte_classes(false))
    ///     .build(r"[a-z]+")?;
    /// let hay = "!!quux!!";
    /// assert_eq!(Some(Match::must(0, 2..6)), re.find(hay));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn byte_classes(self, yes: bool) -> Config {
        Config { byte_classes: Some(yes), ..self }
    }

    /// Set the line terminator to be used by the `^` and `$` anchors in
    /// multi-line mode.
    ///
    /// This option has no effect when CRLF mode is enabled. That is,
    /// regardless of this setting, `(?Rm:^)` and `(?Rm:$)` will always treat
    /// `\r` and `\n` as line terminators (and will never match between a `\r`
    /// and a `\n`).
    ///
    /// By default, `\n` is the line terminator.
    ///
    /// **Warning**: This does not change the behavior of `.`. To do that,
    /// you'll need to configure the syntax option
    /// [`syntax::Config::line_terminator`](crate::util::syntax::Config::line_terminator)
    /// in addition to this. Otherwise, `.` will continue to match any
    /// character other than `\n`.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, util::syntax, Match};
    ///
    /// let re = Regex::builder()
    ///     .syntax(syntax::Config::new().multi_line(true))
    ///     .configure(Regex::config().line_terminator(b'\x00'))
    ///     .build(r"^foo$")?;
    /// let hay = "\x00foo\x00";
    /// assert_eq!(Some(Match::must(0, 1..4)), re.find(hay));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn line_terminator(self, byte: u8) -> Config {
        Config { line_terminator: Some(byte), ..self }
    }

    /// Toggle whether the hybrid NFA/DFA (also known as the "lazy DFA") should
    /// be available for use by the meta regex engine.
    ///
    /// Enabling this does not necessarily mean that the lazy DFA will
    /// definitely be used. It just means that it will be _available_ for use
    /// if the meta regex engine thinks it will be useful.
    ///
    /// When the `hybrid` crate feature is enabled, then this is enabled by
    /// default. Otherwise, if the crate feature is disabled, then this is
    /// always disabled, regardless of its setting by the caller.
    pub fn hybrid(self, yes: bool) -> Config {
        Config { hybrid: Some(yes), ..self }
    }

    /// Toggle whether a fully compiled DFA should be available for use by the
    /// meta regex engine.
    ///
    /// Enabling this does not necessarily mean that a DFA will definitely be
    /// used. It just means that it will be _available_ for use if the meta
    /// regex engine thinks it will be useful.
    ///
    /// When the `dfa-build` crate feature is enabled, then this is enabled by
    /// default. Otherwise, if the crate feature is disabled, then this is
    /// always disabled, regardless of its setting by the caller.
    pub fn dfa(self, yes: bool) -> Config {
        Config { dfa: Some(yes), ..self }
    }

    /// Toggle whether a one-pass DFA should be available for use by the meta
    /// regex engine.
    ///
    /// Enabling this does not necessarily mean that a one-pass DFA will
    /// definitely be used. It just means that it will be _available_ for
    /// use if the meta regex engine thinks it will be useful. (Indeed, a
    /// one-pass DFA can only be used when the regex is one-pass. See the
    /// [`dfa::onepass`](crate::dfa::onepass) module for more details.)
    ///
    /// When the `dfa-onepass` crate feature is enabled, then this is enabled
    /// by default. Otherwise, if the crate feature is disabled, then this is
    /// always disabled, regardless of its setting by the caller.
    pub fn onepass(self, yes: bool) -> Config {
        Config { onepass: Some(yes), ..self }
    }

    /// Toggle whether a bounded backtracking regex engine should be available
    /// for use by the meta regex engine.
    ///
    /// Enabling this does not necessarily mean that a bounded backtracker will
    /// definitely be used. It just means that it will be _available_ for use
    /// if the meta regex engine thinks it will be useful.
    ///
    /// When the `nfa-backtrack` crate feature is enabled, then this is enabled
    /// by default. Otherwise, if the crate feature is disabled, then this is
    /// always disabled, regardless of its setting by the caller.
    pub fn backtrack(self, yes: bool) -> Config {
        Config { backtrack: Some(yes), ..self }
    }

    /// Returns the match kind on this configuration, as set by
    /// [`Config::match_kind`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_match_kind(&self) -> MatchKind {
        self.match_kind.unwrap_or(MatchKind::LeftmostFirst)
    }

    /// Returns whether empty matches must fall on valid UTF-8 boundaries, as
    /// set by [`Config::utf8_empty`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_utf8_empty(&self) -> bool {
        self.utf8_empty.unwrap_or(true)
    }

    /// Returns whether automatic prefilters are enabled, as set by
    /// [`Config::auto_prefilter`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_auto_prefilter(&self) -> bool {
        self.autopre.unwrap_or(true)
    }

    /// Returns a manually set prefilter, if one was set by
    /// [`Config::prefilter`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref().unwrap_or(&None).as_ref()
    }

    /// Returns the capture configuration, as set by
    /// [`Config::which_captures`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_which_captures(&self) -> WhichCaptures {
        self.which_captures.unwrap_or(WhichCaptures::All)
    }

    /// Returns NFA size limit, as set by [`Config::nfa_size_limit`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_nfa_size_limit(&self) -> Option<usize> {
        self.nfa_size_limit.unwrap_or(Some(10 * (1 << 20)))
    }

    /// Returns one-pass DFA size limit, as set by
    /// [`Config::onepass_size_limit`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_onepass_size_limit(&self) -> Option<usize> {
        self.onepass_size_limit.unwrap_or(Some(1 * (1 << 20)))
    }

    /// Returns hybrid NFA/DFA cache capacity, as set by
    /// [`Config::hybrid_cache_capacity`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_hybrid_cache_capacity(&self) -> usize {
        self.hybrid_cache_capacity.unwrap_or(2 * (1 << 20))
    }

    /// Returns DFA size limit, as set by [`Config::dfa_size_limit`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_dfa_size_limit(&self) -> Option<usize> {
        // The default for this is VERY small because building a full DFA is
        // ridiculously costly. But for regexes that are very small, it can be
        // beneficial to use a full DFA. In particular, a full DFA can enable
        // additional optimizations via something called "accelerated" states.
        // Namely, when there's a state with only a few outgoing transitions,
        // we can temporary suspend walking the transition table and use memchr
        // for just those outgoing transitions to skip ahead very quickly.
        //
        // Generally speaking, if Unicode is enabled in your regex and you're
        // using some kind of Unicode feature, then it's going to blow this
        // size limit. Moreover, Unicode tends to defeat the "accelerated"
        // state optimization too, so it's a double whammy.
        //
        // We also use a limit on the number of NFA states to avoid even
        // starting the DFA construction process. Namely, DFA construction
        // itself could make lots of initial allocs proportional to the size
        // of the NFA, and if the NFA is large, it doesn't make sense to pay
        // that cost if we know it's likely to be blown by a large margin.
        self.dfa_size_limit.unwrap_or(Some(40 * (1 << 10)))
    }

    /// Returns DFA size limit in terms of the number of states in the NFA, as
    /// set by [`Config::dfa_state_limit`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_dfa_state_limit(&self) -> Option<usize> {
        // Again, as with the size limit, we keep this very small.
        self.dfa_state_limit.unwrap_or(Some(30))
    }

    /// Returns whether byte classes are enabled, as set by
    /// [`Config::byte_classes`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_byte_classes(&self) -> bool {
        self.byte_classes.unwrap_or(true)
    }

    /// Returns the line terminator for this configuration, as set by
    /// [`Config::line_terminator`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_line_terminator(&self) -> u8 {
        self.line_terminator.unwrap_or(b'\n')
    }

    /// Returns whether the hybrid NFA/DFA regex engine may be used, as set by
    /// [`Config::hybrid`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_hybrid(&self) -> bool {
        #[cfg(feature = "hybrid")]
        {
            self.hybrid.unwrap_or(true)
        }
        #[cfg(not(feature = "hybrid"))]
        {
            false
        }
    }

    /// Returns whether the DFA regex engine may be used, as set by
    /// [`Config::dfa`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_dfa(&self) -> bool {
        #[cfg(feature = "dfa-build")]
        {
            self.dfa.unwrap_or(true)
        }
        #[cfg(not(feature = "dfa-build"))]
        {
            false
        }
    }

    /// Returns whether the one-pass DFA regex engine may be used, as set by
    /// [`Config::onepass`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_onepass(&self) -> bool {
        #[cfg(feature = "dfa-onepass")]
        {
            self.onepass.unwrap_or(true)
        }
        #[cfg(not(feature = "dfa-onepass"))]
        {
            false
        }
    }

    /// Returns whether the bounded backtracking regex engine may be used, as
    /// set by [`Config::backtrack`].
    ///
    /// If it was not explicitly set, then a default value is returned.
    pub fn get_backtrack(&self) -> bool {
        #[cfg(feature = "nfa-backtrack")]
        {
            self.backtrack.unwrap_or(true)
        }
        #[cfg(not(feature = "nfa-backtrack"))]
        {
            false
        }
    }

    /// Overwrite the default configuration such that the options in `o` are
    /// always used. If an option in `o` is not set, then the corresponding
    /// option in `self` is used. If it's not set in `self` either, then it
    /// remains not set.
    pub(crate) fn overwrite(&self, o: Config) -> Config {
        Config {
            match_kind: o.match_kind.or(self.match_kind),
            utf8_empty: o.utf8_empty.or(self.utf8_empty),
            autopre: o.autopre.or(self.autopre),
            pre: o.pre.or_else(|| self.pre.clone()),
            which_captures: o.which_captures.or(self.which_captures),
            nfa_size_limit: o.nfa_size_limit.or(self.nfa_size_limit),
            onepass_size_limit: o
                .onepass_size_limit
                .or(self.onepass_size_limit),
            hybrid_cache_capacity: o
                .hybrid_cache_capacity
                .or(self.hybrid_cache_capacity),
            hybrid: o.hybrid.or(self.hybrid),
            dfa: o.dfa.or(self.dfa),
            dfa_size_limit: o.dfa_size_limit.or(self.dfa_size_limit),
            dfa_state_limit: o.dfa_state_limit.or(self.dfa_state_limit),
            onepass: o.onepass.or(self.onepass),
            backtrack: o.backtrack.or(self.backtrack),
            byte_classes: o.byte_classes.or(self.byte_classes),
            line_terminator: o.line_terminator.or(self.line_terminator),
        }
    }
}

/// A builder for configuring and constructing a `Regex`.
///
/// The builder permits configuring two different aspects of a `Regex`:
///
/// * [`Builder::configure`] will set high-level configuration options as
/// described by a [`Config`].
/// * [`Builder::syntax`] will set the syntax level configuration options
/// as described by a [`util::syntax::Config`](crate::util::syntax::Config).
/// This only applies when building a `Regex` from pattern strings.
///
/// Once configured, the builder can then be used to construct a `Regex` from
/// one of 4 different inputs:
///
/// * [`Builder::build`] creates a regex from a single pattern string.
/// * [`Builder::build_many`] creates a regex from many pattern strings.
/// * [`Builder::build_from_hir`] creates a regex from a
/// [`regex-syntax::Hir`](Hir) expression.
/// * [`Builder::build_many_from_hir`] creates a regex from many
/// [`regex-syntax::Hir`](Hir) expressions.
///
/// The latter two methods in particular provide a way to construct a fully
/// feature regular expression matcher directly from an `Hir` expression
/// without having to first convert it to a string. (This is in contrast to the
/// top-level `regex` crate which intentionally provides no such API in order
/// to avoid making `regex-syntax` a public dependency.)
///
/// As a convenience, this builder may be created via [`Regex::builder`], which
/// may help avoid an extra import.
///
/// # Example: change the line terminator
///
/// This example shows how to enable multi-line mode by default and change the
/// line terminator to the NUL byte:
///
/// ```
/// use regex_automata::{meta::Regex, util::syntax, Match};
///
/// let re = Regex::builder()
///     .syntax(syntax::Config::new().multi_line(true))
///     .configure(Regex::config().line_terminator(b'\x00'))
///     .build(r"^foo$")?;
/// let hay = "\x00foo\x00";
/// assert_eq!(Some(Match::must(0, 1..4)), re.find(hay));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: disable UTF-8 requirement
///
/// By default, regex patterns are required to match UTF-8. This includes
/// regex patterns that can produce matches of length zero. In the case of an
/// empty match, by default, matches will not appear between the code units of
/// a UTF-8 encoded codepoint.
///
/// However, it can be useful to disable this requirement, particularly if
/// you're searching things like `&[u8]` that are not known to be valid UTF-8.
///
/// ```
/// use regex_automata::{meta::Regex, util::syntax, Match};
///
/// let mut builder = Regex::builder();
/// // Disables the requirement that non-empty matches match UTF-8.
/// builder.syntax(syntax::Config::new().utf8(false));
/// // Disables the requirement that empty matches match UTF-8 boundaries.
/// builder.configure(Regex::config().utf8_empty(false));
///
/// // We can match raw bytes via \xZZ syntax, but we need to disable
/// // Unicode mode to do that. We could disable it everywhere, or just
/// // selectively, as shown here.
/// let re = builder.build(r"(?-u:\xFF)foo(?-u:\xFF)")?;
/// let hay = b"\xFFfoo\xFF";
/// assert_eq!(Some(Match::must(0, 0..5)), re.find(hay));
///
/// // We can also match between code units.
/// let re = builder.build(r"")?;
/// let hay = "☃";
/// assert_eq!(re.find_iter(hay).collect::<Vec<Match>>(), vec![
///     Match::must(0, 0..0),
///     Match::must(0, 1..1),
///     Match::must(0, 2..2),
///     Match::must(0, 3..3),
/// ]);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct Builder {
    config: Config,
    ast: ast::parse::ParserBuilder,
    hir: hir::translate::TranslatorBuilder,
}

impl Builder {
    /// Creates a new builder for configuring and constructing a [`Regex`].
    pub fn new() -> Builder {
        Builder {
            config: Config::default(),
            ast: ast::parse::ParserBuilder::new(),
            hir: hir::translate::TranslatorBuilder::new(),
        }
    }

    /// Builds a `Regex` from a single pattern string.
    ///
    /// If there was a problem parsing the pattern or a problem turning it into
    /// a regex matcher, then an error is returned.
    ///
    /// # Example
    ///
    /// This example shows how to configure syntax options.
    ///
    /// ```
    /// use regex_automata::{meta::Regex, util::syntax, Match};
    ///
    /// let re = Regex::builder()
    ///     .syntax(syntax::Config::new().crlf(true).multi_line(true))
    ///     .build(r"^foo$")?;
    /// let hay = "\r\nfoo\r\n";
    /// assert_eq!(Some(Match::must(0, 2..5)), re.find(hay));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build(&self, pattern: &str) -> Result<Regex, BuildError> {
        self.build_many(&[pattern])
    }

    /// Builds a `Regex` from many pattern strings.
    ///
    /// If there was a problem parsing any of the patterns or a problem turning
    /// them into a regex matcher, then an error is returned.
    ///
    /// # Example: finding the pattern that caused an error
    ///
    /// When a syntax error occurs, it is possible to ask which pattern
    /// caused the syntax error.
    ///
    /// ```
    /// use regex_automata::{meta::Regex, PatternID};
    ///
    /// let err = Regex::builder()
    ///     .build_many(&["a", "b", r"\p{Foo}", "c"])
    ///     .unwrap_err();
    /// assert_eq!(Some(PatternID::must(2)), err.pattern());
    /// ```
    ///
    /// # Example: zero patterns is valid
    ///
    /// Building a regex with zero patterns results in a regex that never
    /// matches anything. Because this routine is generic, passing an empty
    /// slice usually requires a turbo-fish (or something else to help type
    /// inference).
    ///
    /// ```
    /// use regex_automata::{meta::Regex, util::syntax, Match};
    ///
    /// let re = Regex::builder()
    ///     .build_many::<&str>(&[])?;
    /// assert_eq!(None, re.find(""));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_many<P: AsRef<str>>(
        &self,
        patterns: &[P],
    ) -> Result<Regex, BuildError> {
        use crate::util::primitives::IteratorIndexExt;
        log! {
            debug!("building meta regex with {} patterns:", patterns.len());
            for (pid, p) in patterns.iter().with_pattern_ids() {
                let p = p.as_ref();
                // We might split a grapheme with this truncation logic, but
                // that's fine. We at least avoid splitting a codepoint.
                let maxoff = p
                    .char_indices()
                    .map(|(i, ch)| i + ch.len_utf8())
                    .take(1000)
                    .last()
                    .unwrap_or(0);
                if maxoff < p.len() {
                    debug!("{:?}: {}[... snip ...]", pid, &p[..maxoff]);
                } else {
                    debug!("{:?}: {}", pid, p);
                }
            }
        }
        let (mut asts, mut hirs) = (vec![], vec![]);
        for (pid, p) in patterns.iter().with_pattern_ids() {
            let ast = self
                .ast
                .build()
                .parse(p.as_ref())
                .map_err(|err| BuildError::ast(pid, err))?;
            asts.push(ast);
        }
        for ((pid, p), ast) in
            patterns.iter().with_pattern_ids().zip(asts.iter())
        {
            let hir = self
                .hir
                .build()
                .translate(p.as_ref(), ast)
                .map_err(|err| BuildError::hir(pid, err))?;
            hirs.push(hir);
        }
        self.build_many_from_hir(&hirs)
    }

    /// Builds a `Regex` directly from an `Hir` expression.
    ///
    /// This is useful if you needed to parse a pattern string into an `Hir`
    /// for other reasons (such as analysis or transformations). This routine
    /// permits building a `Regex` directly from the `Hir` expression instead
    /// of first converting the `Hir` back to a pattern string.
    ///
    /// When using this method, any options set via [`Builder::syntax`] are
    /// ignored. Namely, the syntax options only apply when parsing a pattern
    /// string, which isn't relevant here.
    ///
    /// If there was a problem building the underlying regex matcher for the
    /// given `Hir`, then an error is returned.
    ///
    /// # Example
    ///
    /// This example shows how one can hand-construct an `Hir` expression and
    /// build a regex from it without doing any parsing at all.
    ///
    /// ```
    /// use {
    ///     regex_automata::{meta::Regex, Match},
    ///     regex_syntax::hir::{Hir, Look},
    /// };
    ///
    /// // (?Rm)^foo$
    /// let hir = Hir::concat(vec![
    ///     Hir::look(Look::StartCRLF),
    ///     Hir::literal("foo".as_bytes()),
    ///     Hir::look(Look::EndCRLF),
    /// ]);
    /// let re = Regex::builder()
    ///     .build_from_hir(&hir)?;
    /// let hay = "\r\nfoo\r\n";
    /// assert_eq!(Some(Match::must(0, 2..5)), re.find(hay));
    ///
    /// Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_from_hir(&self, hir: &Hir) -> Result<Regex, BuildError> {
        self.build_many_from_hir(&[hir])
    }

    /// Builds a `Regex` directly from many `Hir` expressions.
    ///
    /// This is useful if you needed to parse pattern strings into `Hir`
    /// expressions for other reasons (such as analysis or transformations).
    /// This routine permits building a `Regex` directly from the `Hir`
    /// expressions instead of first converting the `Hir` expressions back to
    /// pattern strings.
    ///
    /// When using this method, any options set via [`Builder::syntax`] are
    /// ignored. Namely, the syntax options only apply when parsing a pattern
    /// string, which isn't relevant here.
    ///
    /// If there was a problem building the underlying regex matcher for the
    /// given `Hir` expressions, then an error is returned.
    ///
    /// Note that unlike [`Builder::build_many`], this can only fail as a
    /// result of building the underlying matcher. In that case, there is
    /// no single `Hir` expression that can be isolated as a reason for the
    /// failure. So if this routine fails, it's not possible to determine which
    /// `Hir` expression caused the failure.
    ///
    /// # Example
    ///
    /// This example shows how one can hand-construct multiple `Hir`
    /// expressions and build a single regex from them without doing any
    /// parsing at all.
    ///
    /// ```
    /// use {
    ///     regex_automata::{meta::Regex, Match},
    ///     regex_syntax::hir::{Hir, Look},
    /// };
    ///
    /// // (?Rm)^foo$
    /// let hir1 = Hir::concat(vec![
    ///     Hir::look(Look::StartCRLF),
    ///     Hir::literal("foo".as_bytes()),
    ///     Hir::look(Look::EndCRLF),
    /// ]);
    /// // (?Rm)^bar$
    /// let hir2 = Hir::concat(vec![
    ///     Hir::look(Look::StartCRLF),
    ///     Hir::literal("bar".as_bytes()),
    ///     Hir::look(Look::EndCRLF),
    /// ]);
    /// let re = Regex::builder()
    ///     .build_many_from_hir(&[&hir1, &hir2])?;
    /// let hay = "\r\nfoo\r\nbar";
    /// let got: Vec<Match> = re.find_iter(hay).collect();
    /// let expected = vec![
    ///     Match::must(0, 2..5),
    ///     Match::must(1, 7..10),
    /// ];
    /// assert_eq!(expected, got);
    ///
    /// Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_many_from_hir<H: Borrow<Hir>>(
        &self,
        hirs: &[H],
    ) -> Result<Regex, BuildError> {
        let config = self.config.clone();
        // We collect the HIRs into a vec so we can write internal routines
        // with '&[&Hir]'. i.e., Don't use generics everywhere to keep code
        // bloat down..
        let hirs: Vec<&Hir> = hirs.iter().map(|hir| hir.borrow()).collect();
        let info = RegexInfo::new(config, &hirs);
        let strat = strategy::new(&info, &hirs)?;
        let pool = {
            let strat = Arc::clone(&strat);
            let create: CachePoolFn = Box::new(move || strat.create_cache());
            Pool::new(create)
        };
        Ok(Regex { imp: Arc::new(RegexI { strat, info }), pool })
    }

    /// Configure the behavior of a `Regex`.
    ///
    /// This configuration controls non-syntax options related to the behavior
    /// of a `Regex`. This includes things like whether empty matches can split
    /// a codepoint, prefilters, line terminators and a long list of options
    /// for configuring which regex engines the meta regex engine will be able
    /// to use internally.
    ///
    /// # Example
    ///
    /// This example shows how to disable UTF-8 empty mode. This will permit
    /// empty matches to occur between the UTF-8 encoding of a codepoint.
    ///
    /// ```
    /// use regex_automata::{meta::Regex, Match};
    ///
    /// let re = Regex::new("")?;
    /// let got: Vec<Match> = re.find_iter("☃").collect();
    /// // Matches only occur at the beginning and end of the snowman.
    /// assert_eq!(got, vec![
    ///     Match::must(0, 0..0),
    ///     Match::must(0, 3..3),
    /// ]);
    ///
    /// let re = Regex::builder()
    ///     .configure(Regex::config().utf8_empty(false))
    ///     .build("")?;
    /// let got: Vec<Match> = re.find_iter("☃").collect();
    /// // Matches now occur at every position!
    /// assert_eq!(got, vec![
    ///     Match::must(0, 0..0),
    ///     Match::must(0, 1..1),
    ///     Match::must(0, 2..2),
    ///     Match::must(0, 3..3),
    /// ]);
    ///
    /// Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn configure(&mut self, config: Config) -> &mut Builder {
        self.config = self.config.overwrite(config);
        self
    }

    /// Configure the syntax options when parsing a pattern string while
    /// building a `Regex`.
    ///
    /// These options _only_ apply when [`Builder::build`] or [`Builder::build_many`]
    /// are used. The other build methods accept `Hir` values, which have
    /// already been parsed.
    ///
    /// # Example
    ///
    /// This example shows how to enable case insensitive mode.
    ///
    /// ```
    /// use regex_automata::{meta::Regex, util::syntax, Match};
    ///
    /// let re = Regex::builder()
    ///     .syntax(syntax::Config::new().case_insensitive(true))
    ///     .build(r"δ")?;
    /// assert_eq!(Some(Match::must(0, 0..2)), re.find(r"Δ"));
    ///
    /// Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn syntax(
        &mut self,
        config: crate::util::syntax::Config,
    ) -> &mut Builder {
        config.apply_ast(&mut self.ast);
        config.apply_hir(&mut self.hir);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // I found this in the course of building out the benchmark suite for
    // rebar.
    #[test]
    fn regression_suffix_literal_count() {
        let _ = env_logger::try_init();

        let re = Regex::new(r"[a-zA-Z]+ing").unwrap();
        assert_eq!(1, re.find_iter("tingling").count());
    }
}
