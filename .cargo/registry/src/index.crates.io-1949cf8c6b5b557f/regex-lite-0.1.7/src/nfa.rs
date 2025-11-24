use core::{cell::RefCell, mem::size_of};

use alloc::{string::String, sync::Arc, vec, vec::Vec};

use crate::{
    error::Error,
    hir::{self, Hir, HirKind},
    int::U32,
};

pub(crate) type StateID = u32;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Config {
    pub(crate) size_limit: Option<usize>,
}

impl Default for Config {
    fn default() -> Config {
        Config { size_limit: Some(10 * (1 << 20)) }
    }
}

#[derive(Clone)]
pub(crate) struct NFA {
    /// The pattern string this NFA was generated from.
    ///
    /// We put it here for lack of a better place to put it. ¯\_(ツ)_/¯
    pattern: String,
    /// The states that make up this NFA.
    states: Vec<State>,
    /// The ID of the start state.
    start: StateID,
    /// Whether this NFA can only match at the beginning of a haystack.
    is_start_anchored: bool,
    /// Whether this NFA can match the empty string.
    is_match_empty: bool,
    /// If every match has the same number of matching capture groups, then
    /// this corresponds to the number of groups.
    static_explicit_captures_len: Option<usize>,
    /// A map from capture group name to its corresponding index.
    cap_name_to_index: CaptureNameMap,
    /// A map from capture group index to the corresponding name, if one
    /// exists.
    cap_index_to_name: Vec<Option<Arc<str>>>,
    /// Heap memory used indirectly by NFA states and other things (like the
    /// various capturing group representations above). Since each state
    /// might use a different amount of heap, we need to keep track of this
    /// incrementally.
    memory_extra: usize,
}

impl NFA {
    /// Creates a new NFA from the given configuration and HIR.
    pub(crate) fn new(
        config: Config,
        pattern: String,
        hir: &Hir,
    ) -> Result<NFA, Error> {
        Compiler::new(config, pattern).compile(hir)
    }

    /// Returns the pattern string used to construct this NFA.
    pub(crate) fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Returns the state corresponding to the given ID.
    ///
    /// # Panics
    ///
    /// If the ID does not refer to a valid state, then this panics.
    pub(crate) fn state(&self, id: StateID) -> &State {
        &self.states[id.as_usize()]
    }

    /// Returns the total number of states in this NFA.
    pub(crate) fn len(&self) -> usize {
        self.states.len()
    }

    /// Returns the ID of the starting state for this NFA.
    pub(crate) fn start(&self) -> StateID {
        self.start
    }

    /// Returns the capture group index for the corresponding named group.
    /// If no such group with the given name exists, then `None` is returned.
    pub(crate) fn to_index(&self, name: &str) -> Option<usize> {
        self.cap_name_to_index.get(name).cloned().map(|i| i.as_usize())
    }

    /*
    /// Returns the capture group name for the corresponding index.
    /// If no such group with the given index, then `None` is returned.
    pub(crate) fn to_name(&self, index: usize) -> Option<&str> {
        self.cap_index_to_name.get(index)?.as_deref()
    }
    */

    /// Returns an iterator over all of the capture groups, along with their
    /// names if they exist, in this NFA.
    pub(crate) fn capture_names(&self) -> CaptureNames<'_> {
        CaptureNames { it: self.cap_index_to_name.iter() }
    }

    /// Returns the total number of capture groups, including the first and
    /// implicit group, in this NFA.
    pub(crate) fn group_len(&self) -> usize {
        self.cap_index_to_name.len()
    }

    /// Returns true if and only if this NFA can only match at the beginning of
    /// a haystack.
    pub(crate) fn is_start_anchored(&self) -> bool {
        self.is_start_anchored
    }

    /// If the pattern always reports the same number of matching capture groups
    /// for every match, then this returns the number of those groups. This
    /// doesn't include the implicit group found in every pattern.
    pub(crate) fn static_explicit_captures_len(&self) -> Option<usize> {
        self.static_explicit_captures_len
    }

    /// Returns the heap memory usage, in bytes, used by this NFA.
    fn memory_usage(&self) -> usize {
        (self.states.len() * size_of::<State>())
            + (self.cap_index_to_name.len() * size_of::<Option<Arc<str>>>())
            + self.memory_extra
    }
}

impl core::fmt::Debug for NFA {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "NFA(")?;
        writeln!(f, "pattern: {}", self.pattern)?;
        for (sid, state) in self.states.iter().enumerate() {
            writeln!(f, "{:07?}: {:?}", sid, state)?;
        }
        writeln!(f, ")")?;
        Ok(())
    }
}

/// An iterator over all capture groups in an NFA.
///
/// If a particular group has a name, then it is yielded. Otherwise, `None`
/// is yielded.
#[derive(Clone, Debug)]
pub(crate) struct CaptureNames<'a> {
    it: core::slice::Iter<'a, Option<Arc<str>>>,
}

impl<'a> Iterator for CaptureNames<'a> {
    type Item = Option<&'a str>;

    fn next(&mut self) -> Option<Option<&'a str>> {
        self.it.next().map(|n| n.as_deref())
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) enum State {
    Char { target: StateID, ch: char },
    Ranges { target: StateID, ranges: Vec<(char, char)> },
    Splits { targets: Vec<StateID>, reverse: bool },
    Goto { target: StateID, look: Option<hir::Look> },
    Capture { target: StateID, slot: u32 },
    Fail,
    Match,
}

impl State {
    /// Returns the heap memory usage of this NFA state in bytes.
    fn memory_usage(&self) -> usize {
        match *self {
            State::Char { .. }
            | State::Goto { .. }
            | State::Capture { .. }
            | State::Fail { .. }
            | State::Match => 0,
            State::Splits { ref targets, .. } => {
                targets.len() * size_of::<StateID>()
            }
            State::Ranges { ref ranges, .. } => {
                ranges.len() * size_of::<(char, char)>()
            }
        }
    }

    /// Returns an iterator over the given split targets. The order of the
    /// iterator yields elements in reverse when `reverse` is true.
    pub(crate) fn iter_splits<'a>(
        splits: &'a [StateID],
        reverse: bool,
    ) -> impl Iterator<Item = StateID> + 'a {
        let mut it = splits.iter();
        core::iter::from_fn(move || {
            if reverse { it.next_back() } else { it.next() }.copied()
        })
    }
}

impl core::fmt::Debug for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            State::Char { target, ch } => {
                write!(f, "{:?} => {:?}", ch, target)
            }
            State::Ranges { target, ref ranges } => {
                for (i, &(start, end)) in ranges.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}-{:?} => {:?}", start, end, target)?;
                }
                Ok(())
            }
            State::Splits { ref targets, reverse } => {
                write!(f, "splits(")?;
                for (i, sid) in
                    State::iter_splits(targets, reverse).enumerate()
                {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", sid)?;
                }
                write!(f, ")")
            }
            State::Goto { target, look: None } => {
                write!(f, "goto({:?})", target)
            }
            State::Goto { target, look: Some(look) } => {
                write!(f, "{:?} => {:?}", look, target)
            }
            State::Capture { target, slot } => {
                write!(f, "capture(slot={:?}) => {:?}", slot, target,)
            }
            State::Fail => write!(f, "FAIL"),
            State::Match => {
                write!(f, "MATCH")
            }
        }
    }
}

/// A map from capture group name to its corresponding capture group index.
///
/// We define a type alias here so that we can transparently use a `HashMap`
/// whenever it's available. We do so presumably because it's faster, although
/// there are no benchmarks verifying this.
#[cfg(feature = "std")]
type CaptureNameMap = std::collections::HashMap<Arc<str>, u32>;
#[cfg(not(feature = "std"))]
type CaptureNameMap = alloc::collections::BTreeMap<Arc<str>, u32>;

#[derive(Debug)]
struct Compiler {
    config: Config,
    nfa: RefCell<NFA>,
}

impl Compiler {
    fn new(config: Config, pattern: String) -> Compiler {
        let nfa = RefCell::new(NFA {
            pattern,
            states: vec![],
            start: 0,
            is_start_anchored: false,
            is_match_empty: false,
            static_explicit_captures_len: None,
            cap_name_to_index: CaptureNameMap::default(),
            cap_index_to_name: vec![],
            memory_extra: 0,
        });
        Compiler { config, nfa }
    }

    fn compile(self, hir: &Hir) -> Result<NFA, Error> {
        self.nfa.borrow_mut().is_start_anchored = hir.is_start_anchored();
        self.nfa.borrow_mut().is_match_empty = hir.is_match_empty();
        self.nfa.borrow_mut().static_explicit_captures_len =
            hir.static_explicit_captures_len();
        let compiled = self.c_capture(0, None, hir)?;
        let mat = self.add(State::Match)?;
        self.patch(compiled.end, mat)?;
        self.nfa.borrow_mut().start = compiled.start;
        Ok(self.nfa.into_inner())
    }

    fn c(&self, hir: &Hir) -> Result<ThompsonRef, Error> {
        match *hir.kind() {
            HirKind::Empty => self.c_empty(),
            HirKind::Char(ch) => self.c_char(ch),
            HirKind::Class(ref class) => self.c_class(class),
            HirKind::Look(ref look) => self.c_look(look),
            HirKind::Repetition(ref rep) => self.c_repetition(rep),
            HirKind::Capture(ref cap) => {
                self.c_capture(cap.index, cap.name.as_deref(), &cap.sub)
            }
            HirKind::Concat(ref subs) => {
                self.c_concat(subs.iter().map(|s| self.c(s)))
            }
            HirKind::Alternation(ref subs) => {
                self.c_alternation(subs.iter().map(|s| self.c(s)))
            }
        }
    }

    /// Compile a "fail" state that can never be transitioned out of.
    fn c_fail(&self) -> Result<ThompsonRef, Error> {
        let id = self.add(State::Fail)?;
        Ok(ThompsonRef { start: id, end: id })
    }

    /// Compile an "empty" state with one unconditional epsilon transition.
    ///
    /// Both the `start` and `end` locations point to the state created.
    /// Callers will likely want to keep the `start`, but patch the `end` to
    /// point to some other state.
    fn c_empty(&self) -> Result<ThompsonRef, Error> {
        let id = self.add_empty()?;
        Ok(ThompsonRef { start: id, end: id })
    }

    /// Compile the given literal char to an NFA.
    fn c_char(&self, ch: char) -> Result<ThompsonRef, Error> {
        let id = self.add(State::Char { target: 0, ch })?;
        Ok(ThompsonRef { start: id, end: id })
    }

    /// Compile the given character class into an NFA.
    ///
    /// If the class is empty, then this compiles to a `Fail` state.
    fn c_class(&self, class: &hir::Class) -> Result<ThompsonRef, Error> {
        let id = if class.ranges.is_empty() {
            // Technically using an explicit fail state probably isn't
            // necessary. Because if you try to match against an empty Ranges,
            // then it should turn up with nothing regardless of input, and
            // thus "acts" like a Fail state. But it's better to be more
            // explicit, and there's no real cost to doing so.
            self.add(State::Fail)
        } else {
            let ranges =
                class.ranges.iter().map(|r| (r.start, r.end)).collect();
            self.add(State::Ranges { target: 0, ranges })
        }?;
        Ok(ThompsonRef { start: id, end: id })
    }

    /// Compile the given HIR look-around assertion to an NFA look-around
    /// assertion.
    fn c_look(&self, look: &hir::Look) -> Result<ThompsonRef, Error> {
        let id = self.add(State::Goto { target: 0, look: Some(*look) })?;
        Ok(ThompsonRef { start: id, end: id })
    }

    /// Compile the given repetition expression. This handles all types of
    /// repetitions and greediness.
    fn c_repetition(
        &self,
        rep: &hir::Repetition,
    ) -> Result<ThompsonRef, Error> {
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
        hir: &Hir,
        greedy: bool,
        min: u32,
        max: u32,
    ) -> Result<ThompsonRef, Error> {
        let prefix = self.c_exactly(hir, min)?;
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
            let splits =
                self.add(State::Splits { targets: vec![], reverse: !greedy })?;
            let compiled = self.c(hir)?;
            self.patch(prev_end, splits)?;
            self.patch(splits, compiled.start)?;
            self.patch(splits, empty)?;
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
        hir: &Hir,
        greedy: bool,
        n: u32,
    ) -> Result<ThompsonRef, Error> {
        if n == 0 {
            // When the expression cannot match the empty string, then we
            // can get away with something much simpler: just one 'alt'
            // instruction that optionally repeats itself. But if the expr
            // can match the empty string... see below.
            if !hir.is_match_empty() {
                let splits = self.add(State::Splits {
                    targets: vec![],
                    reverse: !greedy,
                })?;
                let compiled = self.c(hir)?;
                self.patch(splits, compiled.start)?;
                self.patch(compiled.end, splits)?;
                return Ok(ThompsonRef { start: splits, end: splits });
            }

            // What's going on here? Shouldn't x* be simpler than this? It
            // turns out that when implementing leftmost-first (Perl-like)
            // match semantics, x* results in an incorrect preference order
            // when computing the transitive closure of states if and only if
            // 'x' can match the empty string. So instead, we compile x* as
            // (x+)?, which preserves the correct preference order.
            //
            // See: https://github.com/rust-lang/regex/issues/779
            let compiled = self.c(hir)?;
            let plus =
                self.add(State::Splits { targets: vec![], reverse: !greedy })?;
            self.patch(compiled.end, plus)?;
            self.patch(plus, compiled.start)?;

            let question =
                self.add(State::Splits { targets: vec![], reverse: !greedy })?;
            let empty = self.add_empty()?;
            self.patch(question, compiled.start)?;
            self.patch(question, empty)?;
            self.patch(plus, empty)?;
            Ok(ThompsonRef { start: question, end: empty })
        } else if n == 1 {
            let compiled = self.c(hir)?;
            let splits =
                self.add(State::Splits { targets: vec![], reverse: !greedy })?;
            self.patch(compiled.end, splits)?;
            self.patch(splits, compiled.start)?;
            Ok(ThompsonRef { start: compiled.start, end: splits })
        } else {
            let prefix = self.c_exactly(hir, n - 1)?;
            let last = self.c(hir)?;
            let splits =
                self.add(State::Splits { targets: vec![], reverse: !greedy })?;
            self.patch(prefix.end, last.start)?;
            self.patch(last.end, splits)?;
            self.patch(splits, last.start)?;
            Ok(ThompsonRef { start: prefix.start, end: splits })
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
        hir: &Hir,
        greedy: bool,
    ) -> Result<ThompsonRef, Error> {
        let splits =
            self.add(State::Splits { targets: vec![], reverse: !greedy })?;
        let compiled = self.c(hir)?;
        let empty = self.add_empty()?;
        self.patch(splits, compiled.start)?;
        self.patch(splits, empty)?;
        self.patch(compiled.end, empty)?;
        Ok(ThompsonRef { start: splits, end: empty })
    }

    /// Compile the given HIR expression exactly `n` times.
    fn c_exactly(&self, hir: &Hir, n: u32) -> Result<ThompsonRef, Error> {
        self.c_concat((0..n).map(|_| self.c(hir)))
    }

    /// Compile the given expression and insert capturing states at the
    /// beginning and end of it. The slot for the capture states is computed
    /// from the index.
    fn c_capture(
        &self,
        index: u32,
        name: Option<&str>,
        hir: &Hir,
    ) -> Result<ThompsonRef, Error> {
        // For discontiguous indices, push placeholders for earlier capture
        // groups that weren't explicitly added. This can happen, for example,
        // with patterns like '(a){0}(a)' where '(a){0}' is completely removed
        // from the pattern.
        let existing_groups_len = self.nfa.borrow().cap_index_to_name.len();
        for _ in 0..(index.as_usize().saturating_sub(existing_groups_len)) {
            self.nfa.borrow_mut().cap_index_to_name.push(None);
        }
        if index.as_usize() >= existing_groups_len {
            if let Some(name) = name {
                let name = Arc::from(name);
                let mut nfa = self.nfa.borrow_mut();
                nfa.cap_name_to_index.insert(Arc::clone(&name), index);
                nfa.cap_index_to_name.push(Some(Arc::clone(&name)));
                // This is an approximation.
                nfa.memory_extra += name.len() + size_of::<u32>();
            } else {
                self.nfa.borrow_mut().cap_index_to_name.push(None);
            }
        }

        let Some(slot) = index.checked_mul(2) else {
            return Err(Error::new("capture group slots exhausted"));
        };
        let start = self.add(State::Capture { target: 0, slot })?;
        let inner = self.c(hir)?;
        let Some(slot) = slot.checked_add(1) else {
            return Err(Error::new("capture group slots exhausted"));
        };
        let end = self.add(State::Capture { target: 0, slot })?;
        self.patch(start, inner.start)?;
        self.patch(inner.end, end)?;

        Ok(ThompsonRef { start, end })
    }

    /// Compile a concatenation of the sub-expressions yielded by the given
    /// iterator. If the iterator yields no elements, then this compiles down
    /// to an "empty" state that always matches.
    fn c_concat<I>(&self, mut it: I) -> Result<ThompsonRef, Error>
    where
        I: Iterator<Item = Result<ThompsonRef, Error>>,
    {
        let ThompsonRef { start, mut end } = match it.next() {
            Some(result) => result?,
            None => return self.c_empty(),
        };
        for result in it {
            let compiled = result?;
            self.patch(end, compiled.start)?;
            end = compiled.end;
        }
        Ok(ThompsonRef { start, end })
    }

    /// Compile an alternation, where each element yielded by the given
    /// iterator represents an item in the alternation. If the iterator yields
    /// no elements, then this compiles down to a "fail" state.
    ///
    /// In an alternation, expressions appearing earlier are "preferred" at
    /// match time over expressions appearing later. (This is currently always
    /// true, as this crate only supports leftmost-first semantics.)
    fn c_alternation<I>(&self, mut it: I) -> Result<ThompsonRef, Error>
    where
        I: Iterator<Item = Result<ThompsonRef, Error>>,
    {
        let first = match it.next() {
            None => return self.c_fail(),
            Some(result) => result?,
        };
        let second = match it.next() {
            None => return Ok(first),
            Some(result) => result?,
        };

        let splits =
            self.add(State::Splits { targets: vec![], reverse: false })?;
        let end = self.add_empty()?;
        self.patch(splits, first.start)?;
        self.patch(first.end, end)?;
        self.patch(splits, second.start)?;
        self.patch(second.end, end)?;
        for result in it {
            let compiled = result?;
            self.patch(splits, compiled.start)?;
            self.patch(compiled.end, end)?;
        }
        Ok(ThompsonRef { start: splits, end })
    }

    /// A convenience routine for adding an empty state, also known as an
    /// unconditional epsilon transition. These are quite useful for making
    /// NFA construction simpler.
    ///
    /// (In the regex crate, we do a second pass to remove these, but don't
    /// bother with that here.)
    fn add_empty(&self) -> Result<StateID, Error> {
        self.add(State::Goto { target: 0, look: None })
    }

    /// The common implementation of "add a state." It handles the common
    /// error cases of state ID exhausting (by owning state ID allocation) and
    /// whether the size limit has been exceeded.
    fn add(&self, state: State) -> Result<StateID, Error> {
        let id = u32::try_from(self.nfa.borrow().states.len())
            .map_err(|_| Error::new("exhausted state IDs, too many states"))?;
        self.nfa.borrow_mut().memory_extra += state.memory_usage();
        self.nfa.borrow_mut().states.push(state);
        self.check_size_limit()?;
        Ok(id)
    }

    /// Add a transition from one state to another.
    ///
    /// This routine is called "patch" since it is very common to add the
    /// states you want, typically with "dummy" state ID transitions, and then
    /// "patch" in the real state IDs later. This is because you don't always
    /// know all of the necessary state IDs to add because they might not
    /// exist yet.
    ///
    /// # Errors
    ///
    /// This may error if patching leads to an increase in heap usage beyond
    /// the configured size limit. Heap usage only grows when patching adds a
    /// new transition (as in the case of a "splits" state).
    fn patch(&self, from: StateID, to: StateID) -> Result<(), Error> {
        let mut new_memory_extra = self.nfa.borrow().memory_extra;
        match self.nfa.borrow_mut().states[from.as_usize()] {
            State::Char { ref mut target, .. } => {
                *target = to;
            }
            State::Ranges { ref mut target, .. } => {
                *target = to;
            }
            State::Splits { ref mut targets, .. } => {
                targets.push(to);
                new_memory_extra += size_of::<StateID>();
            }
            State::Goto { ref mut target, .. } => {
                *target = to;
            }
            State::Capture { ref mut target, .. } => {
                *target = to;
            }
            State::Fail | State::Match => {}
        }
        if new_memory_extra != self.nfa.borrow().memory_extra {
            self.nfa.borrow_mut().memory_extra = new_memory_extra;
            self.check_size_limit()?;
        }
        Ok(())
    }

    /// Checks that the current heap memory usage of the NFA being compiled
    /// doesn't exceed the configured size limit. If it does, an error is
    /// returned.
    fn check_size_limit(&self) -> Result<(), Error> {
        if let Some(limit) = self.config.size_limit {
            if self.nfa.borrow().memory_usage() > limit {
                return Err(Error::new("compiled regex exceeded size limit"));
            }
        }
        Ok(())
    }
}

/// A value that represents the result of compiling a sub-expression of a
/// regex's HIR. Specifically, this represents a sub-graph of the NFA that
/// has an initial state at `start` and a final state at `end`.
#[derive(Clone, Copy, Debug)]
struct ThompsonRef {
    start: StateID,
    end: StateID,
}
