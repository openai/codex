use core::{borrow::Borrow, cell::RefCell};

use alloc::{sync::Arc, vec, vec::Vec};

use regex_syntax::{
    hir::{self, Hir},
    utf8::{Utf8Range, Utf8Sequences},
    ParserBuilder,
};

use crate::{
    nfa::thompson::{
        builder::Builder,
        error::BuildError,
        literal_trie::LiteralTrie,
        map::{Utf8BoundedMap, Utf8SuffixKey, Utf8SuffixMap},
        nfa::{Transition, NFA},
        range_trie::RangeTrie,
    },
    util::{
        look::{Look, LookMatcher},
        primitives::{PatternID, StateID},
    },
};

/// The configuration used for a Thompson NFA compiler.
#[derive(Clone, Debug, Default)]
pub struct Config {
    utf8: Option<bool>,
    reverse: Option<bool>,
    nfa_size_limit: Option<Option<usize>>,
    shrink: Option<bool>,
    which_captures: Option<WhichCaptures>,
    look_matcher: Option<LookMatcher>,
    #[cfg(test)]
    unanchored_prefix: Option<bool>,
}

impl Config {
    /// Return a new default Thompson NFA compiler configuration.
    pub fn new() -> Config {
        Config::default()
    }

    /// Whether to enable UTF-8 mode during search or not.
    ///
    /// A regex engine is said to be in UTF-8 mode when it guarantees that
    /// all matches returned by it have spans consisting of only valid UTF-8.
    /// That is, it is impossible for a match span to be returned that
    /// contains any invalid UTF-8.
    ///
    /// UTF-8 mode generally consists of two things:
    ///
    /// 1. Whether the NFA's states are constructed such that all paths to a
    /// match state that consume at least one byte always correspond to valid
    /// UTF-8.
    /// 2. Whether all paths to a match state that do _not_ consume any bytes
    /// should always correspond to valid UTF-8 boundaries.
    ///
    /// (1) is a guarantee made by whoever constructs the NFA.
    /// If you're parsing a regex from its concrete syntax, then
    /// [`syntax::Config::utf8`](crate::util::syntax::Config::utf8) can make
    /// this guarantee for you. It does it by returning an error if the regex
    /// pattern could every report a non-empty match span that contains invalid
    /// UTF-8. So long as `syntax::Config::utf8` mode is enabled and your regex
    /// successfully parses, then you're guaranteed that the corresponding NFA
    /// will only ever report non-empty match spans containing valid UTF-8.
    ///
    /// (2) is a trickier guarantee because it cannot be enforced by the NFA
    /// state graph itself. Consider, for example, the regex `a*`. It matches
    /// the empty strings in `☃` at positions `0`, `1`, `2` and `3`, where
    /// positions `1` and `2` occur within the UTF-8 encoding of a codepoint,
    /// and thus correspond to invalid UTF-8 boundaries. Therefore, this
    /// guarantee must be made at a higher level than the NFA state graph
    /// itself. This crate deals with this case in each regex engine. Namely,
    /// when a zero-width match that splits a codepoint is found and UTF-8
    /// mode enabled, then it is ignored and the engine moves on looking for
    /// the next match.
    ///
    /// Thus, UTF-8 mode is both a promise that the NFA built only reports
    /// non-empty matches that are valid UTF-8, and an *instruction* to regex
    /// engines that empty matches that split codepoints should be banned.
    ///
    /// Because UTF-8 mode is fundamentally about avoiding invalid UTF-8 spans,
    /// it only makes sense to enable this option when you *know* your haystack
    /// is valid UTF-8. (For example, a `&str`.) Enabling UTF-8 mode and
    /// searching a haystack that contains invalid UTF-8 leads to **unspecified
    /// behavior**.
    ///
    /// Therefore, it may make sense to enable `syntax::Config::utf8` while
    /// simultaneously *disabling* this option. That would ensure all non-empty
    /// match spans are valid UTF-8, but that empty match spans may still split
    /// a codepoint or match at other places that aren't valid UTF-8.
    ///
    /// In general, this mode is only relevant if your regex can match the
    /// empty string. Most regexes don't.
    ///
    /// This is enabled by default.
    ///
    /// # Example
    ///
    /// This example shows how UTF-8 mode can impact the match spans that may
    /// be reported in certain cases.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::{self, pikevm::PikeVM},
    ///     Match, Input,
    /// };
    ///
    /// let re = PikeVM::new("")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// // UTF-8 mode is enabled by default.
    /// let mut input = Input::new("☃");
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 0..0)), caps.get_match());
    ///
    /// // Even though an empty regex matches at 1..1, our next match is
    /// // 3..3 because 1..1 and 2..2 split the snowman codepoint (which is
    /// // three bytes long).
    /// input.set_start(1);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 3..3)), caps.get_match());
    ///
    /// // But if we disable UTF-8, then we'll get matches at 1..1 and 2..2:
    /// let re = PikeVM::builder()
    ///     .thompson(thompson::Config::new().utf8(false))
    ///     .build("")?;
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 1..1)), caps.get_match());
    ///
    /// input.set_start(2);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 2..2)), caps.get_match());
    ///
    /// input.set_start(3);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 3..3)), caps.get_match());
    ///
    /// input.set_start(4);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(None, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn utf8(mut self, yes: bool) -> Config {
        self.utf8 = Some(yes);
        self
    }

    /// Reverse the NFA.
    ///
    /// A NFA reversal is performed by reversing all of the concatenated
    /// sub-expressions in the original pattern, recursively. (Look around
    /// operators are also inverted.) The resulting NFA can be used to match
    /// the pattern starting from the end of a string instead of the beginning
    /// of a string.
    ///
    /// Reversing the NFA is useful for building a reverse DFA, which is most
    /// useful for finding the start of a match after its ending position has
    /// been found. NFA execution engines typically do not work on reverse
    /// NFAs. For example, currently, the Pike VM reports the starting location
    /// of matches without a reverse NFA.
    ///
    /// Currently, enabling this setting requires disabling the
    /// [`captures`](Config::captures) setting. If both are enabled, then the
    /// compiler will return an error. It is expected that this limitation will
    /// be lifted in the future.
    ///
    /// This is disabled by default.
    ///
    /// # Example
    ///
    /// This example shows how to build a DFA from a reverse NFA, and then use
    /// the DFA to search backwards.
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{self, Automaton},
    ///     nfa::thompson::{NFA, WhichCaptures},
    ///     HalfMatch, Input,
    /// };
    ///
    /// let dfa = dfa::dense::Builder::new()
    ///     .thompson(NFA::config()
    ///         .which_captures(WhichCaptures::None)
    ///         .reverse(true)
    ///     )
    ///     .build("baz[0-9]+")?;
    /// let expected = Some(HalfMatch::must(0, 3));
    /// assert_eq!(
    ///     expected,
    ///     dfa.try_search_rev(&Input::new("foobaz12345bar"))?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn reverse(mut self, yes: bool) -> Config {
        self.reverse = Some(yes);
        self
    }

    /// Sets an approximate size limit on the total heap used by the NFA being
    /// compiled.
    ///
    /// This permits imposing constraints on the size of a compiled NFA. This
    /// may be useful in contexts where the regex pattern is untrusted and one
    /// wants to avoid using too much memory.
    ///
    /// This size limit does not apply to auxiliary heap used during
    /// compilation that is not part of the built NFA.
    ///
    /// Note that this size limit is applied during compilation in order for
    /// the limit to prevent too much heap from being used. However, the
    /// implementation may use an intermediate NFA representation that is
    /// otherwise slightly bigger than the final public form. Since the size
    /// limit may be applied to an intermediate representation, there is not
    /// necessarily a precise correspondence between the configured size limit
    /// and the heap usage of the final NFA.
    ///
    /// There is no size limit by default.
    ///
    /// # Example
    ///
    /// This example demonstrates how Unicode mode can greatly increase the
    /// size of the NFA.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::nfa::thompson::NFA;
    ///
    /// // 400KB isn't enough!
    /// NFA::compiler()
    ///     .configure(NFA::config().nfa_size_limit(Some(400_000)))
    ///     .build(r"\w{20}")
    ///     .unwrap_err();
    ///
    /// // ... but 500KB probably is.
    /// let nfa = NFA::compiler()
    ///     .configure(NFA::config().nfa_size_limit(Some(500_000)))
    ///     .build(r"\w{20}")?;
    ///
    /// assert_eq!(nfa.pattern_len(), 1);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn nfa_size_limit(mut self, bytes: Option<usize>) -> Config {
        self.nfa_size_limit = Some(bytes);
        self
    }

    /// Apply best effort heuristics to shrink the NFA at the expense of more
    /// time/memory.
    ///
    /// Generally speaking, if one is using an NFA to compile a DFA, then the
    /// extra time used to shrink the NFA will be more than made up for during
    /// DFA construction (potentially by a lot). In other words, enabling this
    /// can substantially decrease the overall amount of time it takes to build
    /// a DFA.
    ///
    /// A reason to keep this disabled is if you want to compile an NFA and
    /// start using it as quickly as possible without needing to build a DFA,
    /// and you don't mind using a bit of extra memory for the NFA. e.g., for
    /// an NFA simulation or for a lazy DFA.
    ///
    /// NFA shrinking is currently most useful when compiling a reverse
    /// NFA with large Unicode character classes. In particular, it trades
    /// additional CPU time during NFA compilation in favor of generating fewer
    /// NFA states.
    ///
    /// This is disabled by default because it can increase compile times
    /// quite a bit if you aren't building a full DFA.
    ///
    /// # Example
    ///
    /// This example shows that NFA shrinking can lead to substantial space
    /// savings in some cases. Notice that, as noted above, we build a reverse
    /// DFA and use a pattern with a large Unicode character class.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::nfa::thompson::{NFA, WhichCaptures};
    ///
    /// // Currently we have to disable captures when enabling reverse NFA.
    /// let config = NFA::config()
    ///     .which_captures(WhichCaptures::None)
    ///     .reverse(true);
    /// let not_shrunk = NFA::compiler()
    ///     .configure(config.clone().shrink(false))
    ///     .build(r"\w")?;
    /// let shrunk = NFA::compiler()
    ///     .configure(config.clone().shrink(true))
    ///     .build(r"\w")?;
    ///
    /// // While a specific shrink factor is not guaranteed, the savings can be
    /// // considerable in some cases.
    /// assert!(shrunk.states().len() * 2 < not_shrunk.states().len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn shrink(mut self, yes: bool) -> Config {
        self.shrink = Some(yes);
        self
    }

    /// Whether to include 'Capture' states in the NFA.
    ///
    /// Currently, enabling this setting requires disabling the
    /// [`reverse`](Config::reverse) setting. If both are enabled, then the
    /// compiler will return an error. It is expected that this limitation will
    /// be lifted in the future.
    ///
    /// This is enabled by default.
    ///
    /// # Example
    ///
    /// This example demonstrates that some regex engines, like the Pike VM,
    /// require capturing states to be present in the NFA to report match
    /// offsets.
    ///
    /// (Note that since this method is deprecated, the example below uses
    /// [`Config::which_captures`] to disable capture states.)
    ///
    /// ```
    /// use regex_automata::nfa::thompson::{
    ///     pikevm::PikeVM,
    ///     NFA,
    ///     WhichCaptures,
    /// };
    ///
    /// let re = PikeVM::builder()
    ///     .thompson(NFA::config().which_captures(WhichCaptures::None))
    ///     .build(r"[a-z]+")?;
    /// let mut cache = re.create_cache();
    ///
    /// assert!(re.is_match(&mut cache, "abc"));
    /// assert_eq!(None, re.find(&mut cache, "abc"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[deprecated(since = "0.3.5", note = "use which_captures instead")]
    pub fn captures(self, yes: bool) -> Config {
        self.which_captures(if yes {
            WhichCaptures::All
        } else {
            WhichCaptures::None
        })
    }

    /// Configures what kinds of capture groups are compiled into
    /// [`State::Capture`](crate::nfa::thompson::State::Capture) states in a
    /// Thompson NFA.
    ///
    /// Currently, using any option except for [`WhichCaptures::None`] requires
    /// disabling the [`reverse`](Config::reverse) setting. If both are
    /// enabled, then the compiler will return an error. It is expected that
    /// this limitation will be lifted in the future.
    ///
    /// This is set to [`WhichCaptures::All`] by default. Callers may wish to
    /// use [`WhichCaptures::Implicit`] in cases where one wants avoid the
    /// overhead of capture states for explicit groups. Usually this occurs
    /// when one wants to use the `PikeVM` only for determining the overall
    /// match. Otherwise, the `PikeVM` could use much more memory than is
    /// necessary.
    ///
    /// # Example
    ///
    /// This example demonstrates that some regex engines, like the Pike VM,
    /// require capturing states to be present in the NFA to report match
    /// offsets.
    ///
    /// ```
    /// use regex_automata::nfa::thompson::{
    ///     pikevm::PikeVM,
    ///     NFA,
    ///     WhichCaptures,
    /// };
    ///
    /// let re = PikeVM::builder()
    ///     .thompson(NFA::config().which_captures(WhichCaptures::None))
    ///     .build(r"[a-z]+")?;
    /// let mut cache = re.create_cache();
    ///
    /// assert!(re.is_match(&mut cache, "abc"));
    /// assert_eq!(None, re.find(&mut cache, "abc"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// The same applies to the bounded backtracker:
    ///
    /// ```
    /// use regex_automata::nfa::thompson::{
    ///     backtrack::BoundedBacktracker,
    ///     NFA,
    ///     WhichCaptures,
    /// };
    ///
    /// let re = BoundedBacktracker::builder()
    ///     .thompson(NFA::config().which_captures(WhichCaptures::None))
    ///     .build(r"[a-z]+")?;
    /// let mut cache = re.create_cache();
    ///
    /// assert!(re.try_is_match(&mut cache, "abc")?);
    /// assert_eq!(None, re.try_find(&mut cache, "abc")?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn which_captures(mut self, which_captures: WhichCaptures) -> Config {
        self.which_captures = Some(which_captures);
        self
    }

    /// Sets the look-around matcher that should be used with this NFA.
    ///
    /// A look-around matcher determines how to match look-around assertions.
    /// In particular, some assertions are configurable. For example, the
    /// `(?m:^)` and `(?m:$)` assertions can have their line terminator changed
    /// from the default of `\n` to any other byte.
    ///
    /// # Example
    ///
    /// This shows how to change the line terminator for multi-line assertions.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::{self, pikevm::PikeVM},
    ///     util::look::LookMatcher,
    ///     Match, Input,
    /// };
    ///
    /// let mut lookm = LookMatcher::new();
    /// lookm.set_line_terminator(b'\x00');
    ///
    /// let re = PikeVM::builder()
    ///     .thompson(thompson::Config::new().look_matcher(lookm))
    ///     .build(r"(?m)^[a-z]+$")?;
    /// let mut cache = re.create_cache();
    ///
    /// // Multi-line assertions now use NUL as a terminator.
    /// assert_eq!(
    ///     Some(Match::must(0, 1..4)),
    ///     re.find(&mut cache, b"\x00abc\x00"),
    /// );
    /// // ... and \n is no longer recognized as a terminator.
    /// assert_eq!(
    ///     None,
    ///     re.find(&mut cache, b"\nabc\n"),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn look_matcher(mut self, m: LookMatcher) -> Config {
        self.look_matcher = Some(m);
        self
    }

    /// Whether to compile an unanchored prefix into this NFA.
    ///
    /// This is enabled by default. It is made available for tests only to make
    /// it easier to unit test the output of the compiler.
    #[cfg(test)]
    fn unanchored_prefix(mut self, yes: bool) -> Config {
        self.unanchored_prefix = Some(yes);
        self
    }

    /// Returns whether this configuration has enabled UTF-8 mode.
    pub fn get_utf8(&self) -> bool {
        self.utf8.unwrap_or(true)
    }

    /// Returns whether this configuration has enabled reverse NFA compilation.
    pub fn get_reverse(&self) -> bool {
        self.reverse.unwrap_or(false)
    }

    /// Return the configured NFA size limit, if it exists, in the number of
    /// bytes of heap used.
    pub fn get_nfa_size_limit(&self) -> Option<usize> {
        self.nfa_size_limit.unwrap_or(None)
    }

    /// Return whether NFA shrinking is enabled.
    pub fn get_shrink(&self) -> bool {
        self.shrink.unwrap_or(false)
    }

    /// Return whether NFA compilation is configured to produce capture states.
    #[deprecated(since = "0.3.5", note = "use get_which_captures instead")]
    pub fn get_captures(&self) -> bool {
        self.get_which_captures().is_any()
    }

    /// Return what kinds of capture states will be compiled into an NFA.
    pub fn get_which_captures(&self) -> WhichCaptures {
        self.which_captures.unwrap_or(WhichCaptures::All)
    }

    /// Return the look-around matcher for this NFA.
    pub fn get_look_matcher(&self) -> LookMatcher {
        self.look_matcher.clone().unwrap_or(LookMatcher::default())
    }

    /// Return whether NFA compilation is configured to include an unanchored
    /// prefix.
    ///
    /// This is always false when not in test mode.
    fn get_unanchored_prefix(&self) -> bool {
        #[cfg(test)]
        {
            self.unanchored_prefix.unwrap_or(true)
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    /// Overwrite the default configuration such that the options in `o` are
    /// always used. If an option in `o` is not set, then the corresponding
    /// option in `self` is used. If it's not set in `self` either, then it
    /// remains not set.
    pub(crate) fn overwrite(&self, o: Config) -> Config {
        Config {
            utf8: o.utf8.or(self.utf8),
            reverse: o.reverse.or(self.reverse),
            nfa_size_limit: o.nfa_size_limit.or(self.nfa_size_limit),
            shrink: o.shrink.or(self.shrink),
            which_captures: o.which_captures.or(self.which_captures),
            look_matcher: o.look_matcher.or_else(|| self.look_matcher.clone()),
            #[cfg(test)]
            unanchored_prefix: o.unanchored_prefix.or(self.unanchored_prefix),
        }
    }
}

/// A configuration indicating which kinds of
/// [`State::Capture`](crate::nfa::thompson::State::Capture) states to include.
///
/// This configuration can be used with [`Config::which_captures`] to control
/// which capture states are compiled into a Thompson NFA.
///
/// The default configuration is [`WhichCaptures::All`].
#[derive(Clone, Copy, Debug)]
pub enum WhichCaptures {
    /// All capture states, including those corresponding to both implicit and
    /// explicit capture groups, are included in the Thompson NFA.
    All,
    /// Only capture states corresponding to implicit capture groups are
    /// included. Implicit capture groups appear in every pattern implicitly
    /// and correspond to the overall match of a pattern.
    ///
    /// This is useful when one only cares about the overall match of a
    /// pattern. By excluding capture states from explicit capture groups,
    /// one might be able to reduce the memory usage of a multi-pattern regex
    /// substantially if it was otherwise written to have many explicit capture
    /// groups.
    Implicit,
    /// No capture states are compiled into the Thompson NFA.
    ///
    /// This is useful when capture states are either not needed (for example,
    /// if one is only trying to build a DFA) or if they aren't supported (for
    /// example, a reverse NFA).
    None,
}

impl Default for WhichCaptures {
    fn default() -> WhichCaptures {
        WhichCaptures::All
    }
}

impl WhichCaptures {
    /// Returns true if this configuration indicates that no capture states
    /// should be produced in an NFA.
    pub fn is_none(&self) -> bool {
        matches!(*self, WhichCaptures::None)
    }

    /// Returns true if this configuration indicates that some capture states
    /// should be added to an NFA. Note that this might only include capture
    /// states for implicit capture groups.
    pub fn is_any(&self) -> bool {
        !self.is_none()
    }
}

/*
This compiler below uses Thompson's construction algorithm. The compiler takes
a regex-syntax::Hir as input and emits an NFA graph as output. The NFA graph
is structured in a way that permits it to be executed by a virtual machine and
also used to efficiently build a DFA.

The compiler deals with a slightly expanded set of NFA states than what is
in a final NFA (as exhibited by builder::State and nfa::State). Notably a
compiler state includes an empty node that has exactly one unconditional
epsilon transition to the next state. In other words, it's a "goto" instruction
if one views Thompson's NFA as a set of bytecode instructions. These goto
instructions are removed in a subsequent phase before returning the NFA to the
caller. The purpose of these empty nodes is that they make the construction
algorithm substantially simpler to implement. We remove them before returning
to the caller because they can represent substantial overhead when traversing
the NFA graph (either while searching using the NFA directly or while building
a DFA).

In the future, it would be nice to provide a Glushkov compiler as well, as it
would work well as a bit-parallel NFA for smaller regexes. But the Thompson
construction is one I'm more familiar with and seems more straight-forward to
deal with when it comes to large Unicode character classes.

Internally, the compiler uses interior mutability to improve composition in the
face of the borrow checker. In particular, we'd really like to be able to write
things like this:

    self.c_concat(exprs.iter().map(|e| self.c(e)))

Which elegantly uses iterators to build up a sequence of compiled regex
sub-expressions and then hands it off to the concatenating compiler routine.
Without interior mutability, the borrow checker won't let us borrow `self`
mutably both inside and outside the closure at the same time.
*/

/// A builder for compiling an NFA from a regex's high-level intermediate
/// representation (HIR).
///
/// This compiler provides a way to translate a parsed regex pattern into an
/// NFA state graph. The NFA state graph can either be used directly to execute
/// a search (e.g., with a Pike VM), or it can be further used to build a DFA.
///
/// This compiler provides APIs both for compiling regex patterns directly from
/// their concrete syntax, or via a [`regex_syntax::hir::Hir`].
///
/// This compiler has various options that may be configured via
/// [`thompson::Config`](Config).
///
/// Note that a compiler is not the same as a [`thompson::Builder`](Builder).
/// A `Builder` provides a lower level API that is uncoupled from a regex
/// pattern's concrete syntax or even its HIR. Instead, it permits stitching
/// together an NFA by hand. See its docs for examples.
///
/// # Example: compilation from concrete syntax
///
/// This shows how to compile an NFA from a pattern string while setting a size
/// limit on how big the NFA is allowed to be (in terms of bytes of heap used).
///
/// ```
/// use regex_automata::{
///     nfa::thompson::{NFA, pikevm::PikeVM},
///     Match,
/// };
///
/// let config = NFA::config().nfa_size_limit(Some(1_000));
/// let nfa = NFA::compiler().configure(config).build(r"(?-u)\w")?;
///
/// let re = PikeVM::new_from_nfa(nfa)?;
/// let mut cache = re.create_cache();
/// let mut caps = re.create_captures();
/// let expected = Some(Match::must(0, 3..4));
/// re.captures(&mut cache, "!@#A#@!", &mut caps);
/// assert_eq!(expected, caps.get_match());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: compilation from HIR
///
/// This shows how to hand assemble a regular expression via its HIR, and then
/// compile an NFA directly from it.
///
/// ```
/// use regex_automata::{nfa::thompson::{NFA, pikevm::PikeVM}, Match};
/// use regex_syntax::hir::{Hir, Class, ClassBytes, ClassBytesRange};
///
/// let hir = Hir::class(Class::Bytes(ClassBytes::new(vec![
///     ClassBytesRange::new(b'0', b'9'),
///     ClassBytesRange::new(b'A', b'Z'),
///     ClassBytesRange::new(b'_', b'_'),
///     ClassBytesRange::new(b'a', b'z'),
/// ])));
///
/// let config = NFA::config().nfa_size_limit(Some(1_000));
/// let nfa = NFA::compiler().configure(config).build_from_hir(&hir)?;
///
/// let re = PikeVM::new_from_nfa(nfa)?;
/// let mut cache = re.create_cache();
/// let mut caps = re.create_captures();
/// let expected = Some(Match::must(0, 3..4));
/// re.captures(&mut cache, "!@#A#@!", &mut caps);
/// assert_eq!(expected, caps.get_match());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct Compiler {
    /// A regex parser, used when compiling an NFA directly from a pattern
    /// string.
    parser: ParserBuilder,
    /// The compiler configuration.
    config: Config,
    /// The builder for actually constructing an NFA. This provides a
    /// convenient abstraction for writing a compiler.
    builder: RefCell<Builder>,
    /// State used for compiling character classes to UTF-8 byte automata.
    /// State is not retained between character class compilations. This just
    /// serves to amortize allocation to the extent possible.
    utf8_state: RefCell<Utf8State>,
    /// State used for arranging character classes in reverse into a trie.
    trie_state: RefCell<RangeTrie>,
    /// State used for caching common suffixes when compiling reverse UTF-8
    /// automata (for Unicode character classes).
    utf8_suffix: RefCell<Utf8SuffixMap>,
}

impl Compiler {
    /// Create a new NFA builder with its default configuration.
    pub fn new() -> Compiler {
        Compiler {
            parser: ParserBuilder::new(),
            config: Config::default(),
            builder: RefCell::new(Builder::new()),
            utf8_state: RefCell::new(Utf8State::new()),
            trie_state: RefCell::new(RangeTrie::new()),
            utf8_suffix: RefCell::new(Utf8SuffixMap::new(1000)),
        }
    }

    /// Compile the given regular expression pattern into an NFA.
    ///
    /// If there was a problem parsing the regex, then that error is returned.
    ///
    /// Otherwise, if there was a problem building the NFA, then an error is
    /// returned. The only error that can occur is if the compiled regex would
    /// exceed the size limits configured on this builder, or if any part of
    /// the NFA would exceed the integer representations used. (For example,
    /// too many states might plausibly occur on a 16-bit target.)
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::{NFA, pikevm::PikeVM}, Match};
    ///
    /// let config = NFA::config().nfa_size_limit(Some(1_000));
    /// let nfa = NFA::compiler().configure(config).build(r"(?-u)\w")?;
    ///
    /// let re = PikeVM::new_from_nfa(nfa)?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    /// let expected = Some(Match::must(0, 3..4));
    /// re.captures(&mut cache, "!@#A#@!", &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build(&self, pattern: &str) -> Result<NFA, BuildError> {
        self.build_many(&[pattern])
    }

    /// Compile the given regular expression patterns into a single NFA.
    ///
    /// When matches are returned, the pattern ID corresponds to the index of
    /// the pattern in the slice given.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::{NFA, pikevm::PikeVM}, Match};
    ///
    /// let config = NFA::config().nfa_size_limit(Some(1_000));
    /// let nfa = NFA::compiler().configure(config).build_many(&[
    ///     r"(?-u)\s",
    ///     r"(?-u)\w",
    /// ])?;
    ///
    /// let re = PikeVM::new_from_nfa(nfa)?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    /// let expected = Some(Match::must(1, 1..2));
    /// re.captures(&mut cache, "!A! !A!", &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_many<P: AsRef<str>>(
        &self,
        patterns: &[P],
    ) -> Result<NFA, BuildError> {
        let mut hirs = vec![];
        for p in patterns {
            hirs.push(
                self.parser
                    .build()
                    .parse(p.as_ref())
                    .map_err(BuildError::syntax)?,
            );
            debug!("parsed: {:?}", p.as_ref());
        }
        self.build_many_from_hir(&hirs)
    }

    /// Compile the given high level intermediate representation of a regular
    /// expression into an NFA.
    ///
    /// If there was a problem building the NFA, then an error is returned. The
    /// only error that can occur is if the compiled regex would exceed the
    /// size limits configured on this builder, or if any part of the NFA would
    /// exceed the integer representations used. (For example, too many states
    /// might plausibly occur on a 16-bit target.)
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::{NFA, pikevm::PikeVM}, Match};
    /// use regex_syntax::hir::{Hir, Class, ClassBytes, ClassBytesRange};
    ///
    /// let hir = Hir::class(Class::Bytes(ClassBytes::new(vec![
    ///     ClassBytesRange::new(b'0', b'9'),
    ///     ClassBytesRange::new(b'A', b'Z'),
    ///     ClassBytesRange::new(b'_', b'_'),
    ///     ClassBytesRange::new(b'a', b'z'),
    /// ])));
    ///
    /// let config = NFA::config().nfa_size_limit(Some(1_000));
    /// let nfa = NFA::compiler().configure(config).build_from_hir(&hir)?;
    ///
    /// let re = PikeVM::new_from_nfa(nfa)?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    /// let expected = Some(Match::must(0, 3..4));
    /// re.captures(&mut cache, "!@#A#@!", &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_from_hir(&self, expr: &Hir) -> Result<NFA, BuildError> {
        self.build_many_from_hir(&[expr])
    }

    /// Compile the given high level intermediate representations of regular
    /// expressions into a single NFA.
    ///
    /// When matches are returned, the pattern ID corresponds to the index of
    /// the pattern in the slice given.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::{NFA, pikevm::PikeVM}, Match};
    /// use regex_syntax::hir::{Hir, Class, ClassBytes, ClassBytesRange};
    ///
    /// let hirs = &[
    ///     Hir::class(Class::Bytes(ClassBytes::new(vec![
    ///         ClassBytesRange::new(b'\t', b'\r'),
    ///         ClassBytesRange::new(b' ', b' '),
    ///     ]))),
    ///     Hir::class(Class::Bytes(ClassBytes::new(vec![
    ///         ClassBytesRange::new(b'0', b'9'),
    ///         ClassBytesRange::new(b'A', b'Z'),
    ///         ClassBytesRange::new(b'_', b'_'),
    ///         ClassBytesRange::new(b'a', b'z'),
    ///     ]))),
    /// ];
    ///
    /// let config = NFA::config().nfa_size_limit(Some(1_000));
    /// let nfa = NFA::compiler().configure(config).build_many_from_hir(hirs)?;
    ///
    /// let re = PikeVM::new_from_nfa(nfa)?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    /// let expected = Some(Match::must(1, 1..2));
    /// re.captures(&mut cache, "!A! !A!", &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_many_from_hir<H: Borrow<Hir>>(
        &self,
        exprs: &[H],
    ) -> Result<NFA, BuildError> {
        self.compile(exprs)
    }

    /// Apply the given NFA configuration options to this builder.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::nfa::thompson::NFA;
    ///
    /// let config = NFA::config().nfa_size_limit(Some(1_000));
    /// let nfa = NFA::compiler().configure(config).build(r"(?-u)\w")?;
    /// assert_eq!(nfa.pattern_len(), 1);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn configure(&mut self, config: Config) -> &mut Compiler {
        self.config = self.config.overwrite(config);
        self
    }

    /// Set the syntax configuration for this builder using
    /// [`syntax::Config`](crate::util::syntax::Config).
    ///
    /// This permits setting things like case insensitivity, Unicode and multi
    /// line mode.
    ///
    /// This syntax configuration only applies when an NFA is built directly
    /// from a pattern string. If an NFA is built from an HIR, then all syntax
    /// settings are ignored.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, util::syntax};
    ///
    /// let syntax_config = syntax::Config::new().unicode(false);
    /// let nfa = NFA::compiler().syntax(syntax_config).build(r"\w")?;
    /// // If Unicode were enabled, the number of states would be much bigger.
    /// assert!(nfa.states().len() < 15);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn syntax(
        &mut self,
        config: crate::util::syntax::Config,
    ) -> &mut Compiler {
        config.apply(&mut self.parser);
        self
    }
}

impl Compiler {
    /// Compile the sequence of HIR expressions given. Pattern IDs are
    /// allocated starting from 0, in correspondence with the slice given.
    ///
    /// It is legal to provide an empty slice. In that case, the NFA returned
    /// has no patterns and will never match anything.
    fn compile<H: Borrow<Hir>>(&self, exprs: &[H]) -> Result<NFA, BuildError> {
        if exprs.len() > PatternID::LIMIT {
            return Err(BuildError::too_many_patterns(exprs.len()));
        }
        if self.config.get_reverse()
            && self.config.get_which_captures().is_any()
        {
            return Err(BuildError::unsupported_captures());
        }

        self.builder.borrow_mut().clear();
        self.builder.borrow_mut().set_utf8(self.config.get_utf8());
        self.builder.borrow_mut().set_reverse(self.config.get_reverse());
        self.builder
            .borrow_mut()
            .set_look_matcher(self.config.get_look_matcher());
        self.builder
            .borrow_mut()
            .set_size_limit(self.config.get_nfa_size_limit())?;

        // We always add an unanchored prefix unless we were specifically told
        // not to (for tests only), or if we know that the regex is anchored
        // for all matches. When an unanchored prefix is not added, then the
        // NFA's anchored and unanchored start states are equivalent.
        let all_anchored = exprs.iter().all(|e| {
            let props = e.borrow().properties();
            if self.config.get_reverse() {
                props.look_set_suffix().contains(hir::Look::End)
            } else {
                props.look_set_prefix().contains(hir::Look::Start)
            }
        });
        let anchored = !self.config.get_unanchored_prefix() || all_anchored;
        let unanchored_prefix = if anchored {
            self.c_empty()?
        } else {
            self.c_at_least(&Hir::dot(hir::Dot::AnyByte), false, 0)?
        };

        let compiled = self.c_alt_iter(exprs.iter().map(|e| {
            let _ = self.start_pattern()?;
            let one = self.c_cap(0, None, e.borrow())?;
            let match_state_id = self.add_match()?;
            self.patch(one.end, match_state_id)?;
            let _ = self.finish_pattern(one.start)?;
            Ok(ThompsonRef { start: one.start, end: match_state_id })
        }))?;
        self.patch(unanchored_prefix.end, compiled.start)?;
        let nfa = self
            .builder
            .borrow_mut()
            .build(compiled.start, unanchored_prefix.start)?;

        debug!("HIR-to-NFA compilation complete, config: {:?}", self.config);
        Ok(nfa)
    }

    /// Compile an arbitrary HIR expression.
    fn c(&self, expr: &Hir) -> Result<ThompsonRef, BuildError> {
        use regex_syntax::hir::{Class, HirKind::*};

        match *expr.kind() {
            Empty => self.c_empty(),
            Literal(hir::Literal(ref bytes)) => self.c_literal(bytes),
            Class(Class::Bytes(ref c)) => self.c_byte_class(c),
            Class(Class::Unicode(ref c)) => self.c_unicode_class(c),
            Look(ref look) => self.c_look(look),
            Repetition(ref rep) => self.c_repetition(rep),
            Capture(ref c) => self.c_cap(c.index, c.name.as_deref(), &c.sub),
            Concat(ref es) => self.c_concat(es.iter().map(|e| self.c(e))),
            Alternation(ref es) => self.c_alt_slice(es),
        }
    }

    /// Compile a concatenation of the sub-expressions yielded by the given
    /// iterator. If the iterator yields no elements, then this compiles down
    /// to an "empty" state that always matches.
    ///
    /// If the compiler is in reverse mode, then the expressions given are
    /// automatically compiled in reverse.
    fn c_concat<I>(&self, mut it: I) -> Result<ThompsonRef, BuildError>
    where
        I: DoubleEndedIterator<Item = Result<ThompsonRef, BuildError>>,
    {
        let first = if self.is_reverse() { it.next_back() } else { it.next() };
        let ThompsonRef { start, mut end } = match first {
            Some(result) => result?,
            None => return self.c_empty(),
        };
        loop {
            let next =
                if self.is_reverse() { it.next_back() } else { it.next() };
            let compiled = match next {
                Some(result) => result?,
                None => break,
            };
            self.patch(end, compiled.start)?;
            end = compiled.end;
        }
        Ok(ThompsonRef { start, end })
    }

    /// Compile an alternation of the given HIR values.
    ///
    /// This is like 'c_alt_iter', but it accepts a slice of HIR values instead
    /// of an iterator of compiled NFA subgraphs. The point of accepting a
    /// slice here is that it opens up some optimization opportunities. For
    /// example, if all of the HIR values are literals, then this routine might
    /// re-shuffle them to make NFA epsilon closures substantially faster.
    fn c_alt_slice(&self, exprs: &[Hir]) -> Result<ThompsonRef, BuildError> {
        // self.c_alt_iter(exprs.iter().map(|e| self.c(e)))
        let literal_count = exprs
            .iter()
            .filter(|e| {
                matches!(*e.kind(), hir::HirKind::Literal(hir::Literal(_)))
            })
            .count();
        if literal_count <= 1 || literal_count < exprs.len() {
            return self.c_alt_iter(exprs.iter().map(|e| self.c(e)));
        }

        let mut trie = if self.is_reverse() {
            LiteralTrie::reverse()
        } else {
            LiteralTrie::forward()
        };
        for expr in exprs.iter() {
            let literal = match *expr.kind() {
                hir::HirKind::Literal(hir::Literal(ref bytes)) => bytes,
                _ => unreachable!(),
            };
            trie.add(literal)?;
        }
        trie.compile(&mut self.builder.borrow_mut())
    }

    /// Compile an alternation, where each element yielded by the given
    /// iterator represents an item in the alternation. If the iterator yields
    /// no elements, then this compiles down to a "fail" state.
    ///
    /// In an alternation, expressions appearing earlier are "preferred" at
    /// match time over expressions appearing later. At least, this is true
    /// when using "leftmost first" match semantics. (If "leftmost longest" are
    /// ever added in the future, then this preference order of priority would
    /// not apply in that mode.)
    fn c_alt_iter<I>(&self, mut it: I) -> Result<ThompsonRef, BuildError>
    where
        I: Iterator<Item = Result<ThompsonRef, BuildError>>,
    {
        let first = match it.next() {
            None => return self.c_fail(),
            Some(result) => result?,
        };
        let second = match it.next() {
            None => return Ok(first),
            Some(result) => result?,
        };

        let union = self.add_union()?;
        let end = self.add_empty()?;
        self.patch(union, first.start)?;
        self.patch(first.end, end)?;
        self.patch(union, second.start)?;
        self.patch(second.end, end)?;
        for result in it {
            let compiled = result?;
            self.patch(union, compiled.start)?;
            self.patch(compiled.end, end)?;
        }
        Ok(ThompsonRef { start: union, end })
    }

    /// Compile the given capture sub-expression. `expr` should be the
    /// sub-expression contained inside the capture. If "capture" states are
    /// enabled, then they are added as appropriate.
    ///
    /// This accepts the pieces of a capture instead of a `hir::Capture` so
    /// that it's easy to manufacture a "fake" group when necessary, e.g., for
    /// adding the entire pattern as if it were a group in order to create
    /// appropriate "capture" states in the NFA.
    fn c_cap(
        &self,
        index: u32,
        name: Option<&str>,
        expr: &Hir,
    ) -> Result<ThompsonRef, BuildError> {
        match self.config.get_which_captures() {
            // No capture states means we always skip them.
            WhichCaptures::None => return self.c(expr),
            // Implicit captures states means we only add when index==0 since
            // index==0 implies the group is implicit.
            WhichCaptures::Implicit if index > 0 => return self.c(expr),
            _ => {}
        }

        let start = self.add_capture_start(index, name)?;
        let inner = self.c(expr)?;
        let end = self.add_capture_end(index)?;
        self.patch(start, inner.start)?;
        self.patch(inner.end, end)?;
        Ok(ThompsonRef { start, end })
    }

    /// Compile the given repetition expression. This handles all types of
    /// repetitions and greediness.
    fn c_repetition(
        &self,
        rep: &hir::Repetition,
    ) -> Result<ThompsonRef, BuildError> {
        match (rep.min, rep.max) {
            (0, Some(1)) => self.c_zero_or_one(&rep.sub, rep.greedy),
            (min, None) => self.c_at_least(&rep.sub, rep.greedy, min),
            (min, Some(max)) if min == max => self.c_exactly(&rep.sub, min),
            (min, Some(max)) => self.c_bounded(&rep.sub, rep.greedy, min, max),
        }
    }

    /// Compile the given expression such that it matches at least `min` times,
    /// but no more than `max` times.
    ///
    /// When `greedy` is true, then the preference is for the expression to
    /// match as much as possible. Otherwise, it will match as little as
    /// possible.
    fn c_bounded(
        &self,
        expr: &Hir,
        greedy: bool,
        min: u32,
        max: u32,
    ) -> Result<ThompsonRef, BuildError> {
        let prefix = self.c_exactly(expr, min)?;
        if min == max {
            return Ok(prefix);
        }

        // It is tempting here to compile the rest here as a concatenation
        // of zero-or-one matches. i.e., for `a{2,5}`, compile it as if it
        // were `aaa?a?a?`. The problem here is that it leads to this program:
        //
        //     >000000: 61 => 01
        //      000001: 61 => 02
        //      000002: union(03, 04)
        //      000003: 61 => 04
        //      000004: union(05, 06)
        //      000005: 61 => 06
        //      000006: union(07, 08)
        //      000007: 61 => 08
        //      000008: MATCH
        //
        // And effectively, once you hit state 2, the epsilon closure will
        // include states 3, 5, 6, 7 and 8, which is quite a bit. It is better
        // to instead compile it like so:
        //
        //     >000000: 61 => 01
        //      000001: 61 => 02
        //      000002: union(03, 08)
        //      000003: 61 => 04
        //      000004: union(05, 08)
        //      000005: 61 => 06
        //      000006: union(07, 08)
        //      000007: 61 => 08
        //      000008: MATCH
        //
        // So that the epsilon closure of state 2 is now just 3 and 8.
        let empty = self.add_empty()?;
        let mut prev_end = prefix.end;
        for _ in min..max {
            let union = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            let compiled = self.c(expr)?;
            self.patch(prev_end, union)?;
            self.patch(union, compiled.start)?;
            self.patch(union, empty)?;
            prev_end = compiled.end;
        }
        self.patch(prev_end, empty)?;
        Ok(ThompsonRef { start: prefix.start, end: empty })
    }

    /// Compile the given expression such that it may be matched `n` or more
    /// times, where `n` can be any integer. (Although a particularly large
    /// integer is likely to run afoul of any configured size limits.)
    ///
    /// When `greedy` is true, then the preference is for the expression to
    /// match as much as possible. Otherwise, it will match as little as
    /// possible.
    fn c_at_least(
        &self,
        expr: &Hir,
        greedy: bool,
        n: u32,
    ) -> Result<ThompsonRef, BuildError> {
        if n == 0 {
            // When the expression cannot match the empty string, then we
            // can get away with something much simpler: just one 'alt'
            // instruction that optionally repeats itself. But if the expr
            // can match the empty string... see below.
            if expr.properties().minimum_len().map_or(false, |len| len > 0) {
                let union = if greedy {
                    self.add_union()
                } else {
                    self.add_union_reverse()
                }?;
                let compiled = self.c(expr)?;
                self.patch(union, compiled.start)?;
                self.patch(compiled.end, union)?;
                return Ok(ThompsonRef { start: union, end: union });
            }

            // What's going on here? Shouldn't x* be simpler than this? It
            // turns out that when implementing leftmost-first (Perl-like)
            // match semantics, x* results in an incorrect preference order
            // when computing the transitive closure of states if and only if
            // 'x' can match the empty string. So instead, we compile x* as
            // (x+)?, which preserves the correct preference order.
            //
            // See: https://github.com/rust-lang/regex/issues/779
            let compiled = self.c(expr)?;
            let plus = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            self.patch(compiled.end, plus)?;
            self.patch(plus, compiled.start)?;

            let question = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            let empty = self.add_empty()?;
            self.patch(question, compiled.start)?;
            self.patch(question, empty)?;
            self.patch(plus, empty)?;
            Ok(ThompsonRef { start: question, end: empty })
        } else if n == 1 {
            let compiled = self.c(expr)?;
            let union = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            self.patch(compiled.end, union)?;
            self.patch(union, compiled.start)?;
            Ok(ThompsonRef { start: compiled.start, end: union })
        } else {
            let prefix = self.c_exactly(expr, n - 1)?;
            let last = self.c(expr)?;
            let union = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            self.patch(prefix.end, last.start)?;
            self.patch(last.end, union)?;
            self.patch(union, last.start)?;
            Ok(ThompsonRef { start: prefix.start, end: union })
        }
    }

    /// Compile the given expression such that it may be matched zero or one
    /// times.
    ///
    /// When `greedy` is true, then the preference is for the expression to
    /// match as much as possible. Otherwise, it will match as little as
    /// possible.
    fn c_zero_or_one(
        &self,
        expr: &Hir,
        greedy: bool,
    ) -> Result<ThompsonRef, BuildError> {
        let union =
            if greedy { self.add_union() } else { self.add_union_reverse() }?;
        let compiled = self.c(expr)?;
        let empty = self.add_empty()?;
        self.patch(union, compiled.start)?;
        self.patch(union, empty)?;
        self.patch(compiled.end, empty)?;
        Ok(ThompsonRef { start: union, end: empty })
    }

    /// Compile the given HIR expression exactly `n` times.
    fn c_exactly(
        &self,
        expr: &Hir,
        n: u32,
    ) -> Result<ThompsonRef, BuildError> {
        let it = (0..n).map(|_| self.c(expr));
        self.c_concat(it)
    }

    /// Compile the given byte oriented character class.
    ///
    /// This uses "sparse" states to represent an alternation between ranges in
    /// this character class. We can use "sparse" states instead of stitching
    /// together a "union" state because all ranges in a character class have
    /// equal priority *and* are non-overlapping (thus, only one can match, so
    /// there's never a question of priority in the first place). This saves a
    /// fair bit of overhead when traversing an NFA.
    ///
    /// This routine compiles an empty character class into a "fail" state.
    fn c_byte_class(
        &self,
        cls: &hir::ClassBytes,
    ) -> Result<ThompsonRef, BuildError> {
        let end = self.add_empty()?;
        let mut trans = Vec::with_capacity(cls.ranges().len());
        for r in cls.iter() {
            trans.push(Transition {
                start: r.start(),
                end: r.end(),
                next: end,
            });
        }
        Ok(ThompsonRef { start: self.add_sparse(trans)?, end })
    }

    /// Compile the given Unicode character class.
    ///
    /// This routine specifically tries to use various types of compression,
    /// since UTF-8 automata of large classes can get quite large. The specific
    /// type of compression used depends on forward vs reverse compilation, and
    /// whether NFA shrinking is enabled or not.
    ///
    /// Aside from repetitions causing lots of repeat group, this is like the
    /// single most expensive part of regex compilation. Therefore, a large part
    /// of the expense of compilation may be reduce by disabling Unicode in the
    /// pattern.
    ///
    /// This routine compiles an empty character class into a "fail" state.
    fn c_unicode_class(
        &self,
        cls: &hir::ClassUnicode,
    ) -> Result<ThompsonRef, BuildError> {
        // If all we have are ASCII ranges wrapped in a Unicode package, then
        // there is zero reason to bring out the big guns. We can fit all ASCII
        // ranges within a single sparse state.
        if cls.is_ascii() {
            let end = self.add_empty()?;
            let mut trans = Vec::with_capacity(cls.ranges().len());
            for r in cls.iter() {
                // The unwraps below are OK because we've verified that this
                // class only contains ASCII codepoints.
                trans.push(Transition {
                    // FIXME(1.59): use the 'TryFrom<char> for u8' impl.
                    start: u8::try_from(u32::from(r.start())).unwrap(),
                    end: u8::try_from(u32::from(r.end())).unwrap(),
                    next: end,
                });
            }
            Ok(ThompsonRef { start: self.add_sparse(trans)?, end })
        } else if self.is_reverse() {
            if !self.config.get_shrink() {
                // When we don't want to spend the extra time shrinking, we
                // compile the UTF-8 automaton in reverse using something like
                // the "naive" approach, but will attempt to re-use common
                // suffixes.
                self.c_unicode_class_reverse_with_suffix(cls)
            } else {
                // When we want to shrink our NFA for reverse UTF-8 automata,
                // we cannot feed UTF-8 sequences directly to the UTF-8
                // compiler, since the UTF-8 compiler requires all sequences
                // to be lexicographically sorted. Instead, we organize our
                // sequences into a range trie, which can then output our
                // sequences in the correct order. Unfortunately, building the
                // range trie is fairly expensive (but not nearly as expensive
                // as building a DFA). Hence the reason why the 'shrink' option
                // exists, so that this path can be toggled off. For example,
                // we might want to turn this off if we know we won't be
                // compiling a DFA.
                let mut trie = self.trie_state.borrow_mut();
                trie.clear();

                for rng in cls.iter() {
                    for mut seq in Utf8Sequences::new(rng.start(), rng.end()) {
                        seq.reverse();
                        trie.insert(seq.as_slice());
                    }
                }
                let mut builder = self.builder.borrow_mut();
                let mut utf8_state = self.utf8_state.borrow_mut();
                let mut utf8c =
                    Utf8Compiler::new(&mut *builder, &mut *utf8_state)?;
                trie.iter(|seq| {
                    utf8c.add(&seq)?;
                    Ok(())
                })?;
                utf8c.finish()
            }
        } else {
            // In the forward direction, we always shrink our UTF-8 automata
            // because we can stream it right into the UTF-8 compiler. There
            // is almost no downside (in either memory or time) to using this
            // approach.
            let mut builder = self.builder.borrow_mut();
            let mut utf8_state = self.utf8_state.borrow_mut();
            let mut utf8c =
                Utf8Compiler::new(&mut *builder, &mut *utf8_state)?;
            for rng in cls.iter() {
                for seq in Utf8Sequences::new(rng.start(), rng.end()) {
                    utf8c.add(seq.as_slice())?;
                }
            }
            utf8c.finish()
        }

        // For reference, the code below is the "naive" version of compiling a
        // UTF-8 automaton. It is deliciously simple (and works for both the
        // forward and reverse cases), but will unfortunately produce very
        // large NFAs. When compiling a forward automaton, the size difference
        // can sometimes be an order of magnitude. For example, the '\w' regex
        // will generate about ~3000 NFA states using the naive approach below,
        // but only 283 states when using the approach above. This is because
        // the approach above actually compiles a *minimal* (or near minimal,
        // because of the bounded hashmap for reusing equivalent states) UTF-8
        // automaton.
        //
        // The code below is kept as a reference point in order to make it
        // easier to understand the higher level goal here. Although, it will
        // almost certainly bit-rot, so keep that in mind. Also, if you try to
        // use it, some of the tests in this module will fail because they look
        // for terser byte code produce by the more optimized handling above.
        // But the integration test suite should still pass.
        //
        // One good example of the substantial difference this can make is to
        // compare and contrast performance of the Pike VM when the code below
        // is active vs the code above. Here's an example to try:
        //
        //   regex-cli find match pikevm -b -p '(?m)^\w{20}' non-ascii-file
        //
        // With Unicode classes generated below, this search takes about 45s on
        // my machine. But with the compressed version above, the search takes
        // only around 1.4s. The NFA is also 20% smaller. This is in part due
        // to the compression, but also because of the utilization of 'sparse'
        // NFA states. They lead to much less state shuffling during the NFA
        // search.
        /*
        let it = cls
            .iter()
            .flat_map(|rng| Utf8Sequences::new(rng.start(), rng.end()))
            .map(|seq| {
                let it = seq
                    .as_slice()
                    .iter()
                    .map(|rng| self.c_range(rng.start, rng.end));
                self.c_concat(it)
            });
        self.c_alt_iter(it)
        */
    }

    /// Compile the given Unicode character class in reverse with suffix
    /// caching.
    ///
    /// This is a "quick" way to compile large Unicode classes into reverse
    /// UTF-8 automata while doing a small amount of compression on that
    /// automata by reusing common suffixes.
    ///
    /// A more comprehensive compression scheme can be accomplished by using
    /// a range trie to efficiently sort a reverse sequence of UTF-8 byte
    /// rqanges, and then use Daciuk's algorithm via `Utf8Compiler`.
    ///
    /// This is the technique used when "NFA shrinking" is disabled.
    ///
    /// (This also tries to use "sparse" states where possible, just like
    /// `c_byte_class` does.)
    fn c_unicode_class_reverse_with_suffix(
        &self,
        cls: &hir::ClassUnicode,
    ) -> Result<ThompsonRef, BuildError> {
        // N.B. It would likely be better to cache common *prefixes* in the
        // reverse direction, but it's not quite clear how to do that. The
        // advantage of caching suffixes is that it does give us a win, and
        // has a very small additional overhead.
        let mut cache = self.utf8_suffix.borrow_mut();
        cache.clear();

        let union = self.add_union()?;
        let alt_end = self.add_empty()?;
        for urng in cls.iter() {
            for seq in Utf8Sequences::new(urng.start(), urng.end()) {
                let mut end = alt_end;
                for brng in seq.as_slice() {
                    let key = Utf8SuffixKey {
                        from: end,
                        start: brng.start,
                        end: brng.end,
                    };
                    let hash = cache.hash(&key);
                    if let Some(id) = cache.get(&key, hash) {
                        end = id;
                        continue;
                    }

                    let compiled = self.c_range(brng.start, brng.end)?;
                    self.patch(compiled.end, end)?;
                    end = compiled.start;
                    cache.set(key, hash, end);
                }
                self.patch(union, end)?;
            }
        }
        Ok(ThompsonRef { start: union, end: alt_end })
    }

    /// Compile the given HIR look-around assertion to an NFA look-around
    /// assertion.
    fn c_look(&self, anchor: &hir::Look) -> Result<ThompsonRef, BuildError> {
        let look = match *anchor {
            hir::Look::Start => Look::Start,
            hir::Look::End => Look::End,
            hir::Look::StartLF => Look::StartLF,
            hir::Look::EndLF => Look::EndLF,
            hir::Look::StartCRLF => Look::StartCRLF,
            hir::Look::EndCRLF => Look::EndCRLF,
            hir::Look::WordAscii => Look::WordAscii,
            hir::Look::WordAsciiNegate => Look::WordAsciiNegate,
            hir::Look::WordUnicode => Look::WordUnicode,
            hir::Look::WordUnicodeNegate => Look::WordUnicodeNegate,
            hir::Look::WordStartAscii => Look::WordStartAscii,
            hir::Look::WordEndAscii => Look::WordEndAscii,
            hir::Look::WordStartUnicode => Look::WordStartUnicode,
            hir::Look::WordEndUnicode => Look::WordEndUnicode,
            hir::Look::WordStartHalfAscii => Look::WordStartHalfAscii,
            hir::Look::WordEndHalfAscii => Look::WordEndHalfAscii,
            hir::Look::WordStartHalfUnicode => Look::WordStartHalfUnicode,
            hir::Look::WordEndHalfUnicode => Look::WordEndHalfUnicode,
        };
        let id = self.add_look(look)?;
        Ok(ThompsonRef { start: id, end: id })
    }

    /// Compile the given byte string to a concatenation of bytes.
    fn c_literal(&self, bytes: &[u8]) -> Result<ThompsonRef, BuildError> {
        self.c_concat(bytes.iter().copied().map(|b| self.c_range(b, b)))
    }

    /// Compile a "range" state with one transition that may only be followed
    /// if the input byte is in the (inclusive) range given.
    ///
    /// Both the `start` and `end` locations point to the state created.
    /// Callers will likely want to keep the `start`, but patch the `end` to
    /// point to some other state.
    fn c_range(&self, start: u8, end: u8) -> Result<ThompsonRef, BuildError> {
        let id = self.add_range(start, end)?;
        Ok(ThompsonRef { start: id, end: id })
    }

    /// Compile an "empty" state with one unconditional epsilon transition.
    ///
    /// Both the `start` and `end` locations point to the state created.
    /// Callers will likely want to keep the `start`, but patch the `end` to
    /// point to some other state.
    fn c_empty(&self) -> Result<ThompsonRef, BuildError> {
        let id = self.add_empty()?;
        Ok(ThompsonRef { start: id, end: id })
    }

    /// Compile a "fail" state that can never have any outgoing transitions.
    fn c_fail(&self) -> Result<ThompsonRef, BuildError> {
        let id = self.add_fail()?;
        Ok(ThompsonRef { start: id, end: id })
    }

    // The below helpers are meant to be simple wrappers around the
    // corresponding Builder methods. For the most part, they let us write
    // 'self.add_foo()' instead of 'self.builder.borrow_mut().add_foo()', where
    // the latter is a mouthful. Some of the methods do inject a little bit
    // of extra logic. e.g., Flipping look-around operators when compiling in
    // reverse mode.

    fn patch(&self, from: StateID, to: StateID) -> Result<(), BuildError> {
        self.builder.borrow_mut().patch(from, to)
    }

    fn start_pattern(&self) -> Result<PatternID, BuildError> {
        self.builder.borrow_mut().start_pattern()
    }

    fn finish_pattern(
        &self,
        start_id: StateID,
    ) -> Result<PatternID, BuildError> {
        self.builder.borrow_mut().finish_pattern(start_id)
    }

    fn add_empty(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_empty()
    }

    fn add_range(&self, start: u8, end: u8) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_range(Transition {
            start,
            end,
            next: StateID::ZERO,
        })
    }

    fn add_sparse(
        &self,
        ranges: Vec<Transition>,
    ) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_sparse(ranges)
    }

    fn add_look(&self, mut look: Look) -> Result<StateID, BuildError> {
        if self.is_reverse() {
            look = look.reversed();
        }
        self.builder.borrow_mut().add_look(StateID::ZERO, look)
    }

    fn add_union(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_union(vec![])
    }

    fn add_union_reverse(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_union_reverse(vec![])
    }

    fn add_capture_start(
        &self,
        capture_index: u32,
        name: Option<&str>,
    ) -> Result<StateID, BuildError> {
        let name = name.map(|n| Arc::from(n));
        self.builder.borrow_mut().add_capture_start(
            StateID::ZERO,
            capture_index,
            name,
        )
    }

    fn add_capture_end(
        &self,
        capture_index: u32,
    ) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_capture_end(StateID::ZERO, capture_index)
    }

    fn add_fail(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_fail()
    }

    fn add_match(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_match()
    }

    fn is_reverse(&self) -> bool {
        self.config.get_reverse()
    }
}

/// A value that represents the result of compiling a sub-expression of a
/// regex's HIR. Specifically, this represents a sub-graph of the NFA that
/// has an initial state at `start` and a final state at `end`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ThompsonRef {
    pub(crate) start: StateID,
    pub(crate) end: StateID,
}

/// A UTF-8 compiler based on Daciuk's algorithm for compilining minimal DFAs
/// from a lexicographically sorted sequence of strings in linear time.
///
/// The trick here is that any Unicode codepoint range can be converted to
/// a sequence of byte ranges that form a UTF-8 automaton. Connecting them
/// together via an alternation is trivial, and indeed, it works. However,
/// there is a lot of redundant structure in many UTF-8 automatons. Since our
/// UTF-8 ranges are in lexicographic order, we can use Daciuk's algorithm
/// to build nearly minimal DFAs in linear time. (They are guaranteed to be
/// minimal because we use a bounded cache of previously build DFA states.)
///
/// The drawback is that this sadly doesn't work for reverse automata, since
/// the ranges are no longer in lexicographic order. For that, we invented the
/// range trie (which gets its own module). Once a range trie is built, we then
/// use this same Utf8Compiler to build a reverse UTF-8 automaton.
///
/// The high level idea is described here:
/// https://blog.burntsushi.net/transducers/#finite-state-machines-as-data-structures
///
/// There is also another implementation of this in the `fst` crate.
#[derive(Debug)]
struct Utf8Compiler<'a> {
    builder: &'a mut Builder,
    state: &'a mut Utf8State,
    target: StateID,
}

#[derive(Clone, Debug)]
struct Utf8State {
    compiled: Utf8BoundedMap,
    uncompiled: Vec<Utf8Node>,
}

#[derive(Clone, Debug)]
struct Utf8Node {
    trans: Vec<Transition>,
    last: Option<Utf8LastTransition>,
}

#[derive(Clone, Debug)]
struct Utf8LastTransition {
    start: u8,
    end: u8,
}

impl Utf8State {
    fn new() -> Utf8State {
        Utf8State { compiled: Utf8BoundedMap::new(10_000), uncompiled: vec![] }
    }

    fn clear(&mut self) {
        self.compiled.clear();
        self.uncompiled.clear();
    }
}

impl<'a> Utf8Compiler<'a> {
    fn new(
        builder: &'a mut Builder,
        state: &'a mut Utf8State,
    ) -> Result<Utf8Compiler<'a>, BuildError> {
        let target = builder.add_empty()?;
        state.clear();
        let mut utf8c = Utf8Compiler { builder, state, target };
        utf8c.add_empty();
        Ok(utf8c)
    }

    fn finish(&mut self) -> Result<ThompsonRef, BuildError> {
        self.compile_from(0)?;
        let node = self.pop_root();
        let start = self.compile(node)?;
        Ok(ThompsonRef { start, end: self.target })
    }

    fn add(&mut self, ranges: &[Utf8Range]) -> Result<(), BuildError> {
        let prefix_len = ranges
            .iter()
            .zip(&self.state.uncompiled)
            .take_while(|&(range, node)| {
                node.last.as_ref().map_or(false, |t| {
                    (t.start, t.end) == (range.start, range.end)
                })
            })
            .count();
        assert!(prefix_len < ranges.len());
        self.compile_from(prefix_len)?;
        self.add_suffix(&ranges[prefix_len..]);
        Ok(())
    }

    fn compile_from(&mut self, from: usize) -> Result<(), BuildError> {
        let mut next = self.target;
        while from + 1 < self.state.uncompiled.len() {
            let node = self.pop_freeze(next);
            next = self.compile(node)?;
        }
        self.top_last_freeze(next);
        Ok(())
    }

    fn compile(
        &mut self,
        node: Vec<Transition>,
    ) -> Result<StateID, BuildError> {
        let hash = self.state.compiled.hash(&node);
        if let Some(id) = self.state.compiled.get(&node, hash) {
            return Ok(id);
        }
        let id = self.builder.add_sparse(node.clone())?;
        self.state.compiled.set(node, hash, id);
        Ok(id)
    }

    fn add_suffix(&mut self, ranges: &[Utf8Range]) {
        assert!(!ranges.is_empty());
        let last = self
            .state
            .uncompiled
            .len()
            .checked_sub(1)
            .expect("non-empty nodes");
        assert!(self.state.uncompiled[last].last.is_none());
        self.state.uncompiled[last].last = Some(Utf8LastTransition {
            start: ranges[0].start,
            end: ranges[0].end,
        });
        for r in &ranges[1..] {
            self.state.uncompiled.push(Utf8Node {
                trans: vec![],
                last: Some(Utf8LastTransition { start: r.start, end: r.end }),
            });
        }
    }

    fn add_empty(&mut self) {
        self.state.uncompiled.push(Utf8Node { trans: vec![], last: None });
    }

    fn pop_freeze(&mut self, next: StateID) -> Vec<Transition> {
        let mut uncompiled = self.state.uncompiled.pop().unwrap();
        uncompiled.set_last_transition(next);
        uncompiled.trans
    }

    fn pop_root(&mut self) -> Vec<Transition> {
        assert_eq!(self.state.uncompiled.len(), 1);
        assert!(self.state.uncompiled[0].last.is_none());
        self.state.uncompiled.pop().expect("non-empty nodes").trans
    }

    fn top_last_freeze(&mut self, next: StateID) {
        let last = self
            .state
            .uncompiled
            .len()
            .checked_sub(1)
            .expect("non-empty nodes");
        self.state.uncompiled[last].set_last_transition(next);
    }
}

impl Utf8Node {
    fn set_last_transition(&mut self, next: StateID) {
        if let Some(last) = self.last.take() {
            self.trans.push(Transition {
                start: last.start,
                end: last.end,
                next,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use crate::{
        nfa::thompson::{SparseTransitions, State},
        util::primitives::SmallIndex,
    };

    use super::*;

    fn build(pattern: &str) -> NFA {
        NFA::compiler()
            .configure(
                NFA::config()
                    .which_captures(WhichCaptures::None)
                    .unanchored_prefix(false),
            )
            .build(pattern)
            .unwrap()
    }

    fn pid(id: usize) -> PatternID {
        PatternID::new(id).unwrap()
    }

    fn sid(id: usize) -> StateID {
        StateID::new(id).unwrap()
    }

    fn s_byte(byte: u8, next: usize) -> State {
        let next = sid(next);
        let trans = Transition { start: byte, end: byte, next };
        State::ByteRange { trans }
    }

    fn s_range(start: u8, end: u8, next: usize) -> State {
        let next = sid(next);
        let trans = Transition { start, end, next };
        State::ByteRange { trans }
    }

    fn s_sparse(transitions: &[(u8, u8, usize)]) -> State {
        let transitions = transitions
            .iter()
            .map(|&(start, end, next)| Transition {
                start,
                end,
                next: sid(next),
            })
            .collect();
        State::Sparse(SparseTransitions { transitions })
    }

    fn s_look(look: Look, next: usize) -> State {
        let next = sid(next);
        State::Look { look, next }
    }

    fn s_bin_union(alt1: usize, alt2: usize) -> State {
        State::BinaryUnion { alt1: sid(alt1), alt2: sid(alt2) }
    }

    fn s_union(alts: &[usize]) -> State {
        State::Union {
            alternates: alts
                .iter()
                .map(|&id| sid(id))
                .collect::<Vec<StateID>>()
                .into_boxed_slice(),
        }
    }

    fn s_cap(next: usize, pattern: usize, index: usize, slot: usize) -> State {
        State::Capture {
            next: sid(next),
            pattern_id: pid(pattern),
            group_index: SmallIndex::new(index).unwrap(),
            slot: SmallIndex::new(slot).unwrap(),
        }
    }

    fn s_fail() -> State {
        State::Fail
    }

    fn s_match(id: usize) -> State {
        State::Match { pattern_id: pid(id) }
    }

    // Test that building an unanchored NFA has an appropriate `(?s:.)*?`
    // prefix.
    #[test]
    fn compile_unanchored_prefix() {
        let nfa = NFA::compiler()
            .configure(NFA::config().which_captures(WhichCaptures::None))
            .build(r"a")
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[
                s_bin_union(2, 1),
                s_range(0, 255, 0),
                s_byte(b'a', 3),
                s_match(0),
            ]
        );
    }

    #[test]
    fn compile_no_unanchored_prefix_with_start_anchor() {
        let nfa = NFA::compiler()
            .configure(NFA::config().which_captures(WhichCaptures::None))
            .build(r"^a")
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[s_look(Look::Start, 1), s_byte(b'a', 2), s_match(0)]
        );
    }

    #[test]
    fn compile_yes_unanchored_prefix_with_end_anchor() {
        let nfa = NFA::compiler()
            .configure(NFA::config().which_captures(WhichCaptures::None))
            .build(r"a$")
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[
                s_bin_union(2, 1),
                s_range(0, 255, 0),
                s_byte(b'a', 3),
                s_look(Look::End, 4),
                s_match(0),
            ]
        );
    }

    #[test]
    fn compile_yes_reverse_unanchored_prefix_with_start_anchor() {
        let nfa = NFA::compiler()
            .configure(
                NFA::config()
                    .reverse(true)
                    .which_captures(WhichCaptures::None),
            )
            .build(r"^a")
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[
                s_bin_union(2, 1),
                s_range(0, 255, 0),
                s_byte(b'a', 3),
                // Anchors get flipped in a reverse automaton.
                s_look(Look::End, 4),
                s_match(0),
            ],
        );
    }

    #[test]
    fn compile_no_reverse_unanchored_prefix_with_end_anchor() {
        let nfa = NFA::compiler()
            .configure(
                NFA::config()
                    .reverse(true)
                    .which_captures(WhichCaptures::None),
            )
            .build(r"a$")
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[
                // Anchors get flipped in a reverse automaton.
                s_look(Look::Start, 1),
                s_byte(b'a', 2),
                s_match(0),
            ],
        );
    }

    #[test]
    fn compile_empty() {
        assert_eq!(build("").states(), &[s_match(0),]);
    }

    #[test]
    fn compile_literal() {
        assert_eq!(build("a").states(), &[s_byte(b'a', 1), s_match(0),]);
        assert_eq!(
            build("ab").states(),
            &[s_byte(b'a', 1), s_byte(b'b', 2), s_match(0),]
        );
        assert_eq!(
            build("☃").states(),
            &[s_byte(0xE2, 1), s_byte(0x98, 2), s_byte(0x83, 3), s_match(0)]
        );

        // Check that non-UTF-8 literals work.
        let nfa = NFA::compiler()
            .configure(
                NFA::config()
                    .which_captures(WhichCaptures::None)
                    .unanchored_prefix(false),
            )
            .syntax(crate::util::syntax::Config::new().utf8(false))
            .build(r"(?-u)\xFF")
            .unwrap();
        assert_eq!(nfa.states(), &[s_byte(b'\xFF', 1), s_match(0),]);
    }

    #[test]
    fn compile_class_ascii() {
        assert_eq!(
            build(r"[a-z]").states(),
            &[s_range(b'a', b'z', 1), s_match(0),]
        );
        assert_eq!(
            build(r"[x-za-c]").states(),
            &[s_sparse(&[(b'a', b'c', 1), (b'x', b'z', 1)]), s_match(0)]
        );
    }

    #[test]
    #[cfg(not(miri))]
    fn compile_class_unicode() {
        assert_eq!(
            build(r"[\u03B1-\u03B4]").states(),
            &[s_range(0xB1, 0xB4, 2), s_byte(0xCE, 0), s_match(0)]
        );
        assert_eq!(
            build(r"[\u03B1-\u03B4\u{1F919}-\u{1F91E}]").states(),
            &[
                s_range(0xB1, 0xB4, 5),
                s_range(0x99, 0x9E, 5),
                s_byte(0xA4, 1),
                s_byte(0x9F, 2),
                s_sparse(&[(0xCE, 0xCE, 0), (0xF0, 0xF0, 3)]),
                s_match(0),
            ]
        );
        assert_eq!(
            build(r"[a-z☃]").states(),
            &[
                s_byte(0x83, 3),
                s_byte(0x98, 0),
                s_sparse(&[(b'a', b'z', 3), (0xE2, 0xE2, 1)]),
                s_match(0),
            ]
        );
    }

    #[test]
    fn compile_repetition() {
        assert_eq!(
            build(r"a?").states(),
            &[s_bin_union(1, 2), s_byte(b'a', 2), s_match(0),]
        );
        assert_eq!(
            build(r"a??").states(),
            &[s_bin_union(2, 1), s_byte(b'a', 2), s_match(0),]
        );
    }

    #[test]
    fn compile_group() {
        assert_eq!(
            build(r"ab+").states(),
            &[s_byte(b'a', 1), s_byte(b'b', 2), s_bin_union(1, 3), s_match(0)]
        );
        assert_eq!(
            build(r"(ab)").states(),
            &[s_byte(b'a', 1), s_byte(b'b', 2), s_match(0)]
        );
        assert_eq!(
            build(r"(ab)+").states(),
            &[s_byte(b'a', 1), s_byte(b'b', 2), s_bin_union(0, 3), s_match(0)]
        );
    }

    #[test]
    fn compile_alternation() {
        assert_eq!(
            build(r"a|b").states(),
            &[s_range(b'a', b'b', 1), s_match(0)]
        );
        assert_eq!(
            build(r"ab|cd").states(),
            &[
                s_byte(b'b', 3),
                s_byte(b'd', 3),
                s_sparse(&[(b'a', b'a', 0), (b'c', b'c', 1)]),
                s_match(0)
            ],
        );
        assert_eq!(
            build(r"|b").states(),
            &[s_byte(b'b', 2), s_bin_union(2, 0), s_match(0)]
        );
        assert_eq!(
            build(r"a|").states(),
            &[s_byte(b'a', 2), s_bin_union(0, 2), s_match(0)]
        );
    }

    // This tests the use of a non-binary union, i.e., a state with more than
    // 2 unconditional epsilon transitions. The only place they tend to appear
    // is in reverse NFAs when shrinking is disabled. Otherwise, 'binary-union'
    // and 'sparse' tend to cover all other cases of alternation.
    #[test]
    fn compile_non_binary_union() {
        let nfa = NFA::compiler()
            .configure(
                NFA::config()
                    .which_captures(WhichCaptures::None)
                    .reverse(true)
                    .shrink(false)
                    .unanchored_prefix(false),
            )
            .build(r"[\u1000\u2000\u3000]")
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[
                s_union(&[3, 6, 9]),
                s_byte(0xE1, 10),
                s_byte(0x80, 1),
                s_byte(0x80, 2),
                s_byte(0xE2, 10),
                s_byte(0x80, 4),
                s_byte(0x80, 5),
                s_byte(0xE3, 10),
                s_byte(0x80, 7),
                s_byte(0x80, 8),
                s_match(0),
            ]
        );
    }

    #[test]
    fn compile_many_start_pattern() {
        let nfa = NFA::compiler()
            .configure(
                NFA::config()
                    .which_captures(WhichCaptures::None)
                    .unanchored_prefix(false),
            )
            .build_many(&["a", "b"])
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[
                s_byte(b'a', 1),
                s_match(0),
                s_byte(b'b', 3),
                s_match(1),
                s_bin_union(0, 2),
            ]
        );
        assert_eq!(nfa.start_anchored().as_usize(), 4);
        assert_eq!(nfa.start_unanchored().as_usize(), 4);
        // Test that the start states for each individual pattern are correct.
        assert_eq!(nfa.start_pattern(pid(0)).unwrap(), sid(0));
        assert_eq!(nfa.start_pattern(pid(1)).unwrap(), sid(2));
    }

    // This tests that our compiler can handle an empty character class. At the
    // time of writing, the regex parser forbids it, so the only way to test it
    // is to provide a hand written HIR.
    #[test]
    fn empty_class_bytes() {
        use regex_syntax::hir::{Class, ClassBytes, Hir};

        let hir = Hir::class(Class::Bytes(ClassBytes::new(vec![])));
        let config = NFA::config()
            .which_captures(WhichCaptures::None)
            .unanchored_prefix(false);
        let nfa =
            NFA::compiler().configure(config).build_from_hir(&hir).unwrap();
        assert_eq!(nfa.states(), &[s_fail(), s_match(0)]);
    }

    // Like empty_class_bytes, but for a Unicode class.
    #[test]
    fn empty_class_unicode() {
        use regex_syntax::hir::{Class, ClassUnicode, Hir};

        let hir = Hir::class(Class::Unicode(ClassUnicode::new(vec![])));
        let config = NFA::config()
            .which_captures(WhichCaptures::None)
            .unanchored_prefix(false);
        let nfa =
            NFA::compiler().configure(config).build_from_hir(&hir).unwrap();
        assert_eq!(nfa.states(), &[s_fail(), s_match(0)]);
    }

    #[test]
    fn compile_captures_all() {
        let nfa = NFA::compiler()
            .configure(
                NFA::config()
                    .unanchored_prefix(false)
                    .which_captures(WhichCaptures::All),
            )
            .build("a(b)c")
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[
                s_cap(1, 0, 0, 0),
                s_byte(b'a', 2),
                s_cap(3, 0, 1, 2),
                s_byte(b'b', 4),
                s_cap(5, 0, 1, 3),
                s_byte(b'c', 6),
                s_cap(7, 0, 0, 1),
                s_match(0)
            ]
        );
        let ginfo = nfa.group_info();
        assert_eq!(2, ginfo.all_group_len());
    }

    #[test]
    fn compile_captures_implicit() {
        let nfa = NFA::compiler()
            .configure(
                NFA::config()
                    .unanchored_prefix(false)
                    .which_captures(WhichCaptures::Implicit),
            )
            .build("a(b)c")
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[
                s_cap(1, 0, 0, 0),
                s_byte(b'a', 2),
                s_byte(b'b', 3),
                s_byte(b'c', 4),
                s_cap(5, 0, 0, 1),
                s_match(0)
            ]
        );
        let ginfo = nfa.group_info();
        assert_eq!(1, ginfo.all_group_len());
    }

    #[test]
    fn compile_captures_none() {
        let nfa = NFA::compiler()
            .configure(
                NFA::config()
                    .unanchored_prefix(false)
                    .which_captures(WhichCaptures::None),
            )
            .build("a(b)c")
            .unwrap();
        assert_eq!(
            nfa.states(),
            &[s_byte(b'a', 1), s_byte(b'b', 2), s_byte(b'c', 3), s_match(0)]
        );
        let ginfo = nfa.group_info();
        assert_eq!(0, ginfo.all_group_len());
    }
}
