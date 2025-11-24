/*!
Provides types for dealing with capturing groups.

Capturing groups refer to sub-patterns of regexes that some regex engines can
report matching offsets for. For example, matching `[a-z]([0-9]+)` against
`a789` would give `a789` as the overall match (for the implicit capturing group
at index `0`) and `789` as the match for the capturing group `([0-9]+)` (an
explicit capturing group at index `1`).

Not all regex engines can report match offsets for capturing groups. Indeed,
to a first approximation, regex engines that can report capturing group offsets
tend to be quite a bit slower than regex engines that can't. This is because
tracking capturing groups at search time usually requires more "power" that
in turn adds overhead.

Other regex implementations might call capturing groups "submatches."

# Overview

The main types in this module are:

* [`Captures`] records the capturing group offsets found during a search. It
provides convenience routines for looking up capturing group offsets by either
index or name.
* [`GroupInfo`] records the mapping between capturing groups and "slots,"
where the latter are how capturing groups are recorded during a regex search.
This also keeps a mapping from capturing group name to index, and capture
group index to name. A `GroupInfo` is used by `Captures` internally to
provide a convenient API. It is unlikely that you'll use a `GroupInfo`
directly, but for example, if you've compiled an Thompson NFA, then you can use
[`thompson::NFA::group_info`](crate::nfa::thompson::NFA::group_info) to get its
underlying `GroupInfo`.
*/

use alloc::{string::String, sync::Arc, vec, vec::Vec};

use crate::util::{
    interpolate,
    primitives::{
        NonMaxUsize, PatternID, PatternIDError, PatternIDIter, SmallIndex,
    },
    search::{Match, Span},
};

/// The span offsets of capturing groups after a match has been found.
///
/// This type represents the output of regex engines that can report the
/// offsets at which capturing groups matches or "submatches" occur. For
/// example, the [`PikeVM`](crate::nfa::thompson::pikevm::PikeVM). When a match
/// occurs, it will at minimum contain the [`PatternID`] of the pattern that
/// matched. Depending upon how it was constructed, it may also contain the
/// start/end offsets of the entire match of the pattern and the start/end
/// offsets of each capturing group that participated in the match.
///
/// Values of this type are always created for a specific [`GroupInfo`]. It is
/// unspecified behavior to use a `Captures` value in a search with any regex
/// engine that has a different `GroupInfo` than the one the `Captures` were
/// created with.
///
/// # Constructors
///
/// There are three constructors for this type that control what kind of
/// information is available upon a match:
///
/// * [`Captures::all`]: Will store overall pattern match offsets in addition
/// to the offsets of capturing groups that participated in the match.
/// * [`Captures::matches`]: Will store only the overall pattern
/// match offsets. The offsets of capturing groups (even ones that participated
/// in the match) are not available.
/// * [`Captures::empty`]: Will only store the pattern ID that matched. No
/// match offsets are available at all.
///
/// If you aren't sure which to choose, then pick the first one. The first one
/// is what convenience routines like,
/// [`PikeVM::create_captures`](crate::nfa::thompson::pikevm::PikeVM::create_captures),
/// will use automatically.
///
/// The main difference between these choices is performance. Namely, if you
/// ask for _less_ information, then the execution of regex search may be able
/// to run more quickly.
///
/// # Notes
///
/// It is worth pointing out that this type is not coupled to any one specific
/// regex engine. Instead, its coupling is with [`GroupInfo`], which is the
/// thing that is responsible for mapping capturing groups to "slot" offsets.
/// Slot offsets are indices into a single sequence of memory at which matching
/// haystack offsets for the corresponding group are written by regex engines.
///
/// # Example
///
/// This example shows how to parse a simple date and extract the components of
/// the date via capturing groups:
///
/// ```
/// use regex_automata::{nfa::thompson::pikevm::PikeVM, Span};
///
/// let re = PikeVM::new(r"^([0-9]{4})-([0-9]{2})-([0-9]{2})$")?;
/// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
///
/// re.captures(&mut cache, "2010-03-14", &mut caps);
/// assert!(caps.is_match());
/// assert_eq!(Some(Span::from(0..4)), caps.get_group(1));
/// assert_eq!(Some(Span::from(5..7)), caps.get_group(2));
/// assert_eq!(Some(Span::from(8..10)), caps.get_group(3));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: named capturing groups
///
/// This example is like the one above, but leverages the ability to name
/// capturing groups in order to make the code a bit clearer:
///
/// ```
/// use regex_automata::{nfa::thompson::pikevm::PikeVM, Span};
///
/// let re = PikeVM::new(r"^(?P<y>[0-9]{4})-(?P<m>[0-9]{2})-(?P<d>[0-9]{2})$")?;
/// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
///
/// re.captures(&mut cache, "2010-03-14", &mut caps);
/// assert!(caps.is_match());
/// assert_eq!(Some(Span::from(0..4)), caps.get_group_by_name("y"));
/// assert_eq!(Some(Span::from(5..7)), caps.get_group_by_name("m"));
/// assert_eq!(Some(Span::from(8..10)), caps.get_group_by_name("d"));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct Captures {
    /// The group info that these capture groups are coupled to. This is what
    /// gives the "convenience" of the `Captures` API. Namely, it provides the
    /// slot mapping and the name|-->index mapping for capture lookups by name.
    group_info: GroupInfo,
    /// The ID of the pattern that matched. Regex engines must set this to
    /// None when no match occurs.
    pid: Option<PatternID>,
    /// The slot values, i.e., submatch offsets.
    ///
    /// In theory, the smallest sequence of slots would be something like
    /// `max(groups(pattern) for pattern in regex) * 2`, but instead, we use
    /// `sum(groups(pattern) for pattern in regex) * 2`. Why?
    ///
    /// Well, the former could be used in theory, because we don't generally
    /// have any overlapping APIs that involve capturing groups. Therefore,
    /// there's technically never any need to have slots set for multiple
    /// patterns. However, this might change some day, in which case, we would
    /// need to have slots available.
    ///
    /// The other reason is that during the execution of some regex engines,
    /// there exists a point in time where multiple slots for different
    /// patterns may be written to before knowing which pattern has matched.
    /// Therefore, the regex engines themselves, in order to support multiple
    /// patterns correctly, must have all slots available. If `Captures`
    /// doesn't have all slots available, then regex engines can't write
    /// directly into the caller provided `Captures` and must instead write
    /// into some other storage and then copy the slots involved in the match
    /// at the end of the search.
    ///
    /// So overall, at least as of the time of writing, it seems like the path
    /// of least resistance is to just require allocating all possible slots
    /// instead of the conceptual minimum. Another way to justify this is that
    /// the most common case is a single pattern, in which case, there is no
    /// inefficiency here since the 'max' and 'sum' calculations above are
    /// equivalent in that case.
    ///
    /// N.B. The mapping from group index to slot is maintained by `GroupInfo`
    /// and is considered an API guarantee. See `GroupInfo` for more details on
    /// that mapping.
    ///
    /// N.B. `Option<NonMaxUsize>` has the same size as a `usize`.
    slots: Vec<Option<NonMaxUsize>>,
}

impl Captures {
    /// Create new storage for the offsets of all matching capturing groups.
    ///
    /// This routine provides the most information for matches---namely, the
    /// spans of matching capturing groups---but also requires the regex search
    /// routines to do the most work.
    ///
    /// It is unspecified behavior to use the returned `Captures` value in a
    /// search with a `GroupInfo` other than the one that is provided to this
    /// constructor.
    ///
    /// # Example
    ///
    /// This example shows that all capturing groups---but only ones that
    /// participated in a match---are available to query after a match has
    /// been found:
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     util::captures::Captures,
    ///     Span, Match,
    /// };
    ///
    /// let re = PikeVM::new(
    ///     r"^(?:(?P<lower>[a-z]+)|(?P<upper>[A-Z]+))(?P<digits>[0-9]+)$",
    /// )?;
    /// let mut cache = re.create_cache();
    /// let mut caps = Captures::all(re.get_nfa().group_info().clone());
    ///
    /// re.captures(&mut cache, "ABC123", &mut caps);
    /// assert!(caps.is_match());
    /// assert_eq!(Some(Match::must(0, 0..6)), caps.get_match());
    /// // The 'lower' group didn't match, so it won't have any offsets.
    /// assert_eq!(None, caps.get_group_by_name("lower"));
    /// assert_eq!(Some(Span::from(0..3)), caps.get_group_by_name("upper"));
    /// assert_eq!(Some(Span::from(3..6)), caps.get_group_by_name("digits"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn all(group_info: GroupInfo) -> Captures {
        let slots = group_info.slot_len();
        Captures { group_info, pid: None, slots: vec![None; slots] }
    }

    /// Create new storage for only the full match spans of a pattern. This
    /// does not include any capturing group offsets.
    ///
    /// It is unspecified behavior to use the returned `Captures` value in a
    /// search with a `GroupInfo` other than the one that is provided to this
    /// constructor.
    ///
    /// # Example
    ///
    /// This example shows that only overall match offsets are reported when
    /// this constructor is used. Accessing any capturing groups other than
    /// the 0th will always return `None`.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     util::captures::Captures,
    ///     Match,
    /// };
    ///
    /// let re = PikeVM::new(
    ///     r"^(?:(?P<lower>[a-z]+)|(?P<upper>[A-Z]+))(?P<digits>[0-9]+)$",
    /// )?;
    /// let mut cache = re.create_cache();
    /// let mut caps = Captures::matches(re.get_nfa().group_info().clone());
    ///
    /// re.captures(&mut cache, "ABC123", &mut caps);
    /// assert!(caps.is_match());
    /// assert_eq!(Some(Match::must(0, 0..6)), caps.get_match());
    /// // We didn't ask for capturing group offsets, so they aren't available.
    /// assert_eq!(None, caps.get_group_by_name("lower"));
    /// assert_eq!(None, caps.get_group_by_name("upper"));
    /// assert_eq!(None, caps.get_group_by_name("digits"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn matches(group_info: GroupInfo) -> Captures {
        // This is OK because we know there are at least this many slots,
        // and GroupInfo construction guarantees that the number of slots fits
        // into a usize.
        let slots = group_info.pattern_len().checked_mul(2).unwrap();
        Captures { group_info, pid: None, slots: vec![None; slots] }
    }

    /// Create new storage for only tracking which pattern matched. No offsets
    /// are stored at all.
    ///
    /// It is unspecified behavior to use the returned `Captures` value in a
    /// search with a `GroupInfo` other than the one that is provided to this
    /// constructor.
    ///
    /// # Example
    ///
    /// This example shows that only the pattern that matched can be accessed
    /// from a `Captures` value created via this constructor.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     util::captures::Captures,
    ///     PatternID,
    /// };
    ///
    /// let re = PikeVM::new_many(&[r"[a-z]+", r"[A-Z]+"])?;
    /// let mut cache = re.create_cache();
    /// let mut caps = Captures::empty(re.get_nfa().group_info().clone());
    ///
    /// re.captures(&mut cache, "aABCz", &mut caps);
    /// assert!(caps.is_match());
    /// assert_eq!(Some(PatternID::must(0)), caps.pattern());
    /// // We didn't ask for any offsets, so they aren't available.
    /// assert_eq!(None, caps.get_match());
    ///
    /// re.captures(&mut cache, &"aABCz"[1..], &mut caps);
    /// assert!(caps.is_match());
    /// assert_eq!(Some(PatternID::must(1)), caps.pattern());
    /// // We didn't ask for any offsets, so they aren't available.
    /// assert_eq!(None, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn empty(group_info: GroupInfo) -> Captures {
        Captures { group_info, pid: None, slots: vec![] }
    }

    /// Returns true if and only if this capturing group represents a match.
    ///
    /// This is a convenience routine for `caps.pattern().is_some()`.
    ///
    /// # Example
    ///
    /// When using the PikeVM (for example), the lightest weight way of
    /// detecting whether a match exists is to create capturing groups that
    /// only track the ID of the pattern that match (if any):
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     util::captures::Captures,
    /// };
    ///
    /// let re = PikeVM::new(r"[a-z]+")?;
    /// let mut cache = re.create_cache();
    /// let mut caps = Captures::empty(re.get_nfa().group_info().clone());
    ///
    /// re.captures(&mut cache, "aABCz", &mut caps);
    /// assert!(caps.is_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_match(&self) -> bool {
        self.pid.is_some()
    }

    /// Returns the identifier of the pattern that matched when this
    /// capturing group represents a match. If no match was found, then this
    /// always returns `None`.
    ///
    /// This returns a pattern ID in precisely the cases in which `is_match`
    /// returns `true`. Similarly, the pattern ID returned is always the
    /// same pattern ID found in the `Match` returned by `get_match`.
    ///
    /// # Example
    ///
    /// When using the PikeVM (for example), the lightest weight way of
    /// detecting which pattern matched is to create capturing groups that only
    /// track the ID of the pattern that match (if any):
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     util::captures::Captures,
    ///     PatternID,
    /// };
    ///
    /// let re = PikeVM::new_many(&[r"[a-z]+", r"[A-Z]+"])?;
    /// let mut cache = re.create_cache();
    /// let mut caps = Captures::empty(re.get_nfa().group_info().clone());
    ///
    /// re.captures(&mut cache, "ABC", &mut caps);
    /// assert_eq!(Some(PatternID::must(1)), caps.pattern());
    /// // Recall that offsets are only available when using a non-empty
    /// // Captures value. So even though a match occurred, this returns None!
    /// assert_eq!(None, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn pattern(&self) -> Option<PatternID> {
        self.pid
    }

    /// Returns the pattern ID and the span of the match, if one occurred.
    ///
    /// This always returns `None` when `Captures` was created with
    /// [`Captures::empty`], even if a match was found.
    ///
    /// If this routine returns a non-`None` value, then `is_match` is
    /// guaranteed to return `true` and `pattern` is also guaranteed to return
    /// a non-`None` value.
    ///
    /// # Example
    ///
    /// This example shows how to get the full match from a search:
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, Match};
    ///
    /// let re = PikeVM::new_many(&[r"[a-z]+", r"[A-Z]+"])?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, "ABC", &mut caps);
    /// assert_eq!(Some(Match::must(1, 0..3)), caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn get_match(&self) -> Option<Match> {
        Some(Match::new(self.pattern()?, self.get_group(0)?))
    }

    /// Returns the span of a capturing group match corresponding to the group
    /// index given, only if both the overall pattern matched and the capturing
    /// group participated in that match.
    ///
    /// This returns `None` if `index` is invalid. `index` is valid if and only
    /// if it's less than [`Captures::group_len`] for the matching pattern.
    ///
    /// This always returns `None` when `Captures` was created with
    /// [`Captures::empty`], even if a match was found. This also always
    /// returns `None` for any `index > 0` when `Captures` was created with
    /// [`Captures::matches`].
    ///
    /// If this routine returns a non-`None` value, then `is_match` is
    /// guaranteed to return `true`, `pattern` is guaranteed to return a
    /// non-`None` value and `get_match` is guaranteed to return a non-`None`
    /// value.
    ///
    /// By convention, the 0th capture group will always return the same
    /// span as the span returned by `get_match`. This is because the 0th
    /// capture group always corresponds to the entirety of the pattern's
    /// match. (It is similarly always unnamed because it is implicit.) This
    /// isn't necessarily true of all regex engines. For example, one can
    /// hand-compile a [`thompson::NFA`](crate::nfa::thompson::NFA) via a
    /// [`thompson::Builder`](crate::nfa::thompson::Builder), which isn't
    /// technically forced to make the 0th capturing group always correspond to
    /// the entire match.
    ///
    /// # Example
    ///
    /// This example shows how to get the capturing groups, by index, from a
    /// match:
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, Span, Match};
    ///
    /// let re = PikeVM::new(r"^(?P<first>\pL+)\s+(?P<last>\pL+)$")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, "Bruce Springsteen", &mut caps);
    /// assert_eq!(Some(Match::must(0, 0..17)), caps.get_match());
    /// assert_eq!(Some(Span::from(0..5)), caps.get_group(1));
    /// assert_eq!(Some(Span::from(6..17)), caps.get_group(2));
    /// // Looking for a non-existent capturing group will return None:
    /// assert_eq!(None, caps.get_group(3));
    /// # // literals are too big for 32-bit usize: #1039
    /// # #[cfg(target_pointer_width = "64")]
    /// assert_eq!(None, caps.get_group(9944060567225171988));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn get_group(&self, index: usize) -> Option<Span> {
        let pid = self.pattern()?;
        // There's a little bit of work needed to map captures to slots in the
        // fully general case. But in the overwhelming common case of a single
        // pattern, we can just do some simple arithmetic.
        let (slot_start, slot_end) = if self.group_info().pattern_len() == 1 {
            (index.checked_mul(2)?, index.checked_mul(2)?.checked_add(1)?)
        } else {
            self.group_info().slots(pid, index)?
        };
        let start = self.slots.get(slot_start).copied()??;
        let end = self.slots.get(slot_end).copied()??;
        Some(Span { start: start.get(), end: end.get() })
    }

    /// Returns the span of a capturing group match corresponding to the group
    /// name given, only if both the overall pattern matched and the capturing
    /// group participated in that match.
    ///
    /// This returns `None` if `name` does not correspond to a valid capturing
    /// group for the pattern that matched.
    ///
    /// This always returns `None` when `Captures` was created with
    /// [`Captures::empty`], even if a match was found. This also always
    /// returns `None` for any `index > 0` when `Captures` was created with
    /// [`Captures::matches`].
    ///
    /// If this routine returns a non-`None` value, then `is_match` is
    /// guaranteed to return `true`, `pattern` is guaranteed to return a
    /// non-`None` value and `get_match` is guaranteed to return a non-`None`
    /// value.
    ///
    /// # Example
    ///
    /// This example shows how to get the capturing groups, by name, from a
    /// match:
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, Span, Match};
    ///
    /// let re = PikeVM::new(r"^(?P<first>\pL+)\s+(?P<last>\pL+)$")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, "Bruce Springsteen", &mut caps);
    /// assert_eq!(Some(Match::must(0, 0..17)), caps.get_match());
    /// assert_eq!(Some(Span::from(0..5)), caps.get_group_by_name("first"));
    /// assert_eq!(Some(Span::from(6..17)), caps.get_group_by_name("last"));
    /// // Looking for a non-existent capturing group will return None:
    /// assert_eq!(None, caps.get_group_by_name("middle"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_group_by_name(&self, name: &str) -> Option<Span> {
        let index = self.group_info().to_index(self.pattern()?, name)?;
        self.get_group(index)
    }

    /// Returns an iterator of possible spans for every capturing group in the
    /// matching pattern.
    ///
    /// If this `Captures` value does not correspond to a match, then the
    /// iterator returned yields no elements.
    ///
    /// Note that the iterator returned yields elements of type `Option<Span>`.
    /// A span is present if and only if it corresponds to a capturing group
    /// that participated in a match.
    ///
    /// # Example
    ///
    /// This example shows how to collect all capturing groups:
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, Span};
    ///
    /// let re = PikeVM::new(
    ///     // Matches first/last names, with an optional middle name.
    ///     r"^(?P<first>\pL+)\s+(?:(?P<middle>\pL+)\s+)?(?P<last>\pL+)$",
    /// )?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, "Harry James Potter", &mut caps);
    /// assert!(caps.is_match());
    /// let groups: Vec<Option<Span>> = caps.iter().collect();
    /// assert_eq!(groups, vec![
    ///     Some(Span::from(0..18)),
    ///     Some(Span::from(0..5)),
    ///     Some(Span::from(6..11)),
    ///     Some(Span::from(12..18)),
    /// ]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This example uses the same regex as the previous example, but with a
    /// haystack that omits the middle name. This results in a capturing group
    /// that is present in the elements yielded by the iterator but without a
    /// match:
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, Span};
    ///
    /// let re = PikeVM::new(
    ///     // Matches first/last names, with an optional middle name.
    ///     r"^(?P<first>\pL+)\s+(?:(?P<middle>\pL+)\s+)?(?P<last>\pL+)$",
    /// )?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, "Harry Potter", &mut caps);
    /// assert!(caps.is_match());
    /// let groups: Vec<Option<Span>> = caps.iter().collect();
    /// assert_eq!(groups, vec![
    ///     Some(Span::from(0..12)),
    ///     Some(Span::from(0..5)),
    ///     None,
    ///     Some(Span::from(6..12)),
    /// ]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn iter(&self) -> CapturesPatternIter<'_> {
        let names = self
            .pattern()
            .map_or(GroupInfoPatternNames::empty().enumerate(), |pid| {
                self.group_info().pattern_names(pid).enumerate()
            });
        CapturesPatternIter { caps: self, names }
    }

    /// Return the total number of capturing groups for the matching pattern.
    ///
    /// If this `Captures` value does not correspond to a match, then this
    /// always returns `0`.
    ///
    /// This always returns the same number of elements yielded by
    /// [`Captures::iter`]. That is, the number includes capturing groups even
    /// if they don't participate in the match.
    ///
    /// # Example
    ///
    /// This example shows how to count the total number of capturing groups
    /// associated with a pattern. Notice that it includes groups that did not
    /// participate in a match (just like `Captures::iter` does).
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::nfa::thompson::pikevm::PikeVM;
    ///
    /// let re = PikeVM::new(
    ///     // Matches first/last names, with an optional middle name.
    ///     r"^(?P<first>\pL+)\s+(?:(?P<middle>\pL+)\s+)?(?P<last>\pL+)$",
    /// )?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, "Harry Potter", &mut caps);
    /// assert_eq!(4, caps.group_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn group_len(&self) -> usize {
        let pid = match self.pattern() {
            None => return 0,
            Some(pid) => pid,
        };
        self.group_info().group_len(pid)
    }

    /// Returns a reference to the underlying group info on which these
    /// captures are based.
    ///
    /// The difference between `GroupInfo` and `Captures` is that the former
    /// defines the structure of capturing groups where as the latter is what
    /// stores the actual match information. So where as `Captures` only gives
    /// you access to the current match, `GroupInfo` lets you query any
    /// information about all capturing groups, even ones for patterns that
    /// weren't involved in a match.
    ///
    /// Note that a `GroupInfo` uses reference counting internally, so it may
    /// be cloned cheaply.
    ///
    /// # Example
    ///
    /// This example shows how to get all capturing group names from the
    /// underlying `GroupInfo`. Notice that we don't even need to run a
    /// search.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, PatternID};
    ///
    /// let re = PikeVM::new_many(&[
    ///     r"(?P<foo>a)",
    ///     r"(a)(b)",
    ///     r"ab",
    ///     r"(?P<bar>a)(?P<quux>a)",
    ///     r"(?P<foo>z)",
    /// ])?;
    /// let caps = re.create_captures();
    ///
    /// let expected = vec![
    ///     (PatternID::must(0), 0, None),
    ///     (PatternID::must(0), 1, Some("foo")),
    ///     (PatternID::must(1), 0, None),
    ///     (PatternID::must(1), 1, None),
    ///     (PatternID::must(1), 2, None),
    ///     (PatternID::must(2), 0, None),
    ///     (PatternID::must(3), 0, None),
    ///     (PatternID::must(3), 1, Some("bar")),
    ///     (PatternID::must(3), 2, Some("quux")),
    ///     (PatternID::must(4), 0, None),
    ///     (PatternID::must(4), 1, Some("foo")),
    /// ];
    /// // We could also just use 're.get_nfa().group_info()'.
    /// let got: Vec<(PatternID, usize, Option<&str>)> =
    ///     caps.group_info().all_names().collect();
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn group_info(&self) -> &GroupInfo {
        &self.group_info
    }

    /// Interpolates the capture references in `replacement` with the
    /// corresponding substrings in `haystack` matched by each reference. The
    /// interpolated string is returned.
    ///
    /// See the [`interpolate` module](interpolate) for documentation on the
    /// format of the replacement string.
    ///
    /// # Example
    ///
    /// This example shows how to use interpolation, and also shows how it
    /// can work with multi-pattern regexes.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, PatternID};
    ///
    /// let re = PikeVM::new_many(&[
    ///     r"(?<day>[0-9]{2})-(?<month>[0-9]{2})-(?<year>[0-9]{4})",
    ///     r"(?<year>[0-9]{4})-(?<month>[0-9]{2})-(?<day>[0-9]{2})",
    /// ])?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    ///
    /// let replacement = "year=$year, month=$month, day=$day";
    ///
    /// // This matches the first pattern.
    /// let hay = "On 14-03-2010, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// let result = caps.interpolate_string(hay, replacement);
    /// assert_eq!("year=2010, month=03, day=14", result);
    ///
    /// // And this matches the second pattern.
    /// let hay = "On 2010-03-14, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// let result = caps.interpolate_string(hay, replacement);
    /// assert_eq!("year=2010, month=03, day=14", result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn interpolate_string(
        &self,
        haystack: &str,
        replacement: &str,
    ) -> String {
        let mut dst = String::new();
        self.interpolate_string_into(haystack, replacement, &mut dst);
        dst
    }

    /// Interpolates the capture references in `replacement` with the
    /// corresponding substrings in `haystack` matched by each reference. The
    /// interpolated string is written to `dst`.
    ///
    /// See the [`interpolate` module](interpolate) for documentation on the
    /// format of the replacement string.
    ///
    /// # Example
    ///
    /// This example shows how to use interpolation, and also shows how it
    /// can work with multi-pattern regexes.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, PatternID};
    ///
    /// let re = PikeVM::new_many(&[
    ///     r"(?<day>[0-9]{2})-(?<month>[0-9]{2})-(?<year>[0-9]{4})",
    ///     r"(?<year>[0-9]{4})-(?<month>[0-9]{2})-(?<day>[0-9]{2})",
    /// ])?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    ///
    /// let replacement = "year=$year, month=$month, day=$day";
    ///
    /// // This matches the first pattern.
    /// let hay = "On 14-03-2010, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// let mut dst = String::new();
    /// caps.interpolate_string_into(hay, replacement, &mut dst);
    /// assert_eq!("year=2010, month=03, day=14", dst);
    ///
    /// // And this matches the second pattern.
    /// let hay = "On 2010-03-14, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// let mut dst = String::new();
    /// caps.interpolate_string_into(hay, replacement, &mut dst);
    /// assert_eq!("year=2010, month=03, day=14", dst);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn interpolate_string_into(
        &self,
        haystack: &str,
        replacement: &str,
        dst: &mut String,
    ) {
        interpolate::string(
            replacement,
            |index, dst| {
                let span = match self.get_group(index) {
                    None => return,
                    Some(span) => span,
                };
                dst.push_str(&haystack[span]);
            },
            |name| self.group_info().to_index(self.pattern()?, name),
            dst,
        );
    }

    /// Interpolates the capture references in `replacement` with the
    /// corresponding substrings in `haystack` matched by each reference. The
    /// interpolated byte string is returned.
    ///
    /// See the [`interpolate` module](interpolate) for documentation on the
    /// format of the replacement string.
    ///
    /// # Example
    ///
    /// This example shows how to use interpolation, and also shows how it
    /// can work with multi-pattern regexes.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, PatternID};
    ///
    /// let re = PikeVM::new_many(&[
    ///     r"(?<day>[0-9]{2})-(?<month>[0-9]{2})-(?<year>[0-9]{4})",
    ///     r"(?<year>[0-9]{4})-(?<month>[0-9]{2})-(?<day>[0-9]{2})",
    /// ])?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    ///
    /// let replacement = b"year=$year, month=$month, day=$day";
    ///
    /// // This matches the first pattern.
    /// let hay = b"On 14-03-2010, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// let result = caps.interpolate_bytes(hay, replacement);
    /// assert_eq!(&b"year=2010, month=03, day=14"[..], result);
    ///
    /// // And this matches the second pattern.
    /// let hay = b"On 2010-03-14, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// let result = caps.interpolate_bytes(hay, replacement);
    /// assert_eq!(&b"year=2010, month=03, day=14"[..], result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn interpolate_bytes(
        &self,
        haystack: &[u8],
        replacement: &[u8],
    ) -> Vec<u8> {
        let mut dst = vec![];
        self.interpolate_bytes_into(haystack, replacement, &mut dst);
        dst
    }

    /// Interpolates the capture references in `replacement` with the
    /// corresponding substrings in `haystack` matched by each reference. The
    /// interpolated byte string is written to `dst`.
    ///
    /// See the [`interpolate` module](interpolate) for documentation on the
    /// format of the replacement string.
    ///
    /// # Example
    ///
    /// This example shows how to use interpolation, and also shows how it
    /// can work with multi-pattern regexes.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, PatternID};
    ///
    /// let re = PikeVM::new_many(&[
    ///     r"(?<day>[0-9]{2})-(?<month>[0-9]{2})-(?<year>[0-9]{4})",
    ///     r"(?<year>[0-9]{4})-(?<month>[0-9]{2})-(?<day>[0-9]{2})",
    /// ])?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    ///
    /// let replacement = b"year=$year, month=$month, day=$day";
    ///
    /// // This matches the first pattern.
    /// let hay = b"On 14-03-2010, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// let mut dst = vec![];
    /// caps.interpolate_bytes_into(hay, replacement, &mut dst);
    /// assert_eq!(&b"year=2010, month=03, day=14"[..], dst);
    ///
    /// // And this matches the second pattern.
    /// let hay = b"On 2010-03-14, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// let mut dst = vec![];
    /// caps.interpolate_bytes_into(hay, replacement, &mut dst);
    /// assert_eq!(&b"year=2010, month=03, day=14"[..], dst);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn interpolate_bytes_into(
        &self,
        haystack: &[u8],
        replacement: &[u8],
        dst: &mut Vec<u8>,
    ) {
        interpolate::bytes(
            replacement,
            |index, dst| {
                let span = match self.get_group(index) {
                    None => return,
                    Some(span) => span,
                };
                dst.extend_from_slice(&haystack[span]);
            },
            |name| self.group_info().to_index(self.pattern()?, name),
            dst,
        );
    }

    /// This is a convenience routine for extracting the substrings
    /// corresponding to matching capture groups in the given `haystack`. The
    /// `haystack` should be the same substring used to find the match spans in
    /// this `Captures` value.
    ///
    /// This is identical to [`Captures::extract_bytes`], except it works with
    /// `&str` instead of `&[u8]`.
    ///
    /// # Panics
    ///
    /// This panics if the number of explicit matching groups in this
    /// `Captures` value is less than `N`. This also panics if this `Captures`
    /// value does not correspond to a match.
    ///
    /// Note that this does *not* panic if the number of explicit matching
    /// groups is bigger than `N`. In that case, only the first `N` matching
    /// groups are extracted.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::nfa::thompson::pikevm::PikeVM;
    ///
    /// let re = PikeVM::new(r"([0-9]{4})-([0-9]{2})-([0-9]{2})")?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    ///
    /// let hay = "On 2010-03-14, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// assert!(caps.is_match());
    /// let (full, [year, month, day]) = caps.extract(hay);
    /// assert_eq!("2010-03-14", full);
    /// assert_eq!("2010", year);
    /// assert_eq!("03", month);
    /// assert_eq!("14", day);
    ///
    /// // We can also ask for fewer than all capture groups.
    /// let (full, [year]) = caps.extract(hay);
    /// assert_eq!("2010-03-14", full);
    /// assert_eq!("2010", year);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn extract<'h, const N: usize>(
        &self,
        haystack: &'h str,
    ) -> (&'h str, [&'h str; N]) {
        let mut matched = self.iter().flatten();
        let whole_match = &haystack[matched.next().expect("a match")];
        let group_matches = [0; N].map(|_| {
            let sp = matched.next().expect("too few matching groups");
            &haystack[sp]
        });
        (whole_match, group_matches)
    }

    /// This is a convenience routine for extracting the substrings
    /// corresponding to matching capture groups in the given `haystack`. The
    /// `haystack` should be the same substring used to find the match spans in
    /// this `Captures` value.
    ///
    /// This is identical to [`Captures::extract`], except it works with
    /// `&[u8]` instead of `&str`.
    ///
    /// # Panics
    ///
    /// This panics if the number of explicit matching groups in this
    /// `Captures` value is less than `N`. This also panics if this `Captures`
    /// value does not correspond to a match.
    ///
    /// Note that this does *not* panic if the number of explicit matching
    /// groups is bigger than `N`. In that case, only the first `N` matching
    /// groups are extracted.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::nfa::thompson::pikevm::PikeVM;
    ///
    /// let re = PikeVM::new(r"([0-9]{4})-([0-9]{2})-([0-9]{2})")?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    ///
    /// let hay = b"On 2010-03-14, I became a Tenneessee lamb.";
    /// re.captures(&mut cache, hay, &mut caps);
    /// assert!(caps.is_match());
    /// let (full, [year, month, day]) = caps.extract_bytes(hay);
    /// assert_eq!(b"2010-03-14", full);
    /// assert_eq!(b"2010", year);
    /// assert_eq!(b"03", month);
    /// assert_eq!(b"14", day);
    ///
    /// // We can also ask for fewer than all capture groups.
    /// let (full, [year]) = caps.extract_bytes(hay);
    /// assert_eq!(b"2010-03-14", full);
    /// assert_eq!(b"2010", year);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn extract_bytes<'h, const N: usize>(
        &self,
        haystack: &'h [u8],
    ) -> (&'h [u8], [&'h [u8]; N]) {
        let mut matched = self.iter().flatten();
        let whole_match = &haystack[matched.next().expect("a match")];
        let group_matches = [0; N].map(|_| {
            let sp = matched.next().expect("too few matching groups");
            &haystack[sp]
        });
        (whole_match, group_matches)
    }
}

/// Lower level "slot" oriented APIs. One does not typically need to use these
/// when executing a search. They are instead mostly intended for folks that
/// are writing their own regex engine while reusing this `Captures` type.
impl Captures {
    /// Clear this `Captures` value.
    ///
    /// After clearing, all slots inside this `Captures` value will be set to
    /// `None`. Similarly, any pattern ID that it was previously associated
    /// with (for a match) is erased.
    ///
    /// It is not usually necessary to call this routine. Namely, a `Captures`
    /// value only provides high level access to the capturing groups of the
    /// pattern that matched, and only low level access to individual slots.
    /// Thus, even if slots corresponding to groups that aren't associated
    /// with the matching pattern are set, then it won't impact the higher
    /// level APIs. Namely, higher level APIs like [`Captures::get_group`] will
    /// return `None` if no pattern ID is present, even if there are spans set
    /// in the underlying slots.
    ///
    /// Thus, to "clear" a `Captures` value of a match, it is usually only
    /// necessary to call [`Captures::set_pattern`] with `None`.
    ///
    /// # Example
    ///
    /// This example shows what happens when a `Captures` value is cleared.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::nfa::thompson::pikevm::PikeVM;
    ///
    /// let re = PikeVM::new(r"^(?P<first>\pL+)\s+(?P<last>\pL+)$")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, "Bruce Springsteen", &mut caps);
    /// assert!(caps.is_match());
    /// let slots: Vec<Option<usize>> =
    ///     caps.slots().iter().map(|s| s.map(|x| x.get())).collect();
    /// // Note that the following ordering is considered an API guarantee.
    /// assert_eq!(slots, vec![
    ///     Some(0),
    ///     Some(17),
    ///     Some(0),
    ///     Some(5),
    ///     Some(6),
    ///     Some(17),
    /// ]);
    ///
    /// // Now clear the slots. Everything is gone and it is no longer a match.
    /// caps.clear();
    /// assert!(!caps.is_match());
    /// let slots: Vec<Option<usize>> =
    ///     caps.slots().iter().map(|s| s.map(|x| x.get())).collect();
    /// assert_eq!(slots, vec![
    ///     None,
    ///     None,
    ///     None,
    ///     None,
    ///     None,
    ///     None,
    /// ]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.pid = None;
        for slot in self.slots.iter_mut() {
            *slot = None;
        }
    }

    /// Set the pattern on this `Captures` value.
    ///
    /// When the pattern ID is `None`, then this `Captures` value does not
    /// correspond to a match (`is_match` will return `false`). Otherwise, it
    /// corresponds to a match.
    ///
    /// This is useful in search implementations where you might want to
    /// initially call `set_pattern(None)` in order to avoid the cost of
    /// calling `clear()` if it turns out to not be necessary.
    ///
    /// # Example
    ///
    /// This example shows that `set_pattern` merely overwrites the pattern ID.
    /// It does not actually change the underlying slot values.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::nfa::thompson::pikevm::PikeVM;
    ///
    /// let re = PikeVM::new(r"^(?P<first>\pL+)\s+(?P<last>\pL+)$")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, "Bruce Springsteen", &mut caps);
    /// assert!(caps.is_match());
    /// assert!(caps.pattern().is_some());
    /// let slots: Vec<Option<usize>> =
    ///     caps.slots().iter().map(|s| s.map(|x| x.get())).collect();
    /// // Note that the following ordering is considered an API guarantee.
    /// assert_eq!(slots, vec![
    ///     Some(0),
    ///     Some(17),
    ///     Some(0),
    ///     Some(5),
    ///     Some(6),
    ///     Some(17),
    /// ]);
    ///
    /// // Now set the pattern to None. Note that the slot values remain.
    /// caps.set_pattern(None);
    /// assert!(!caps.is_match());
    /// assert!(!caps.pattern().is_some());
    /// let slots: Vec<Option<usize>> =
    ///     caps.slots().iter().map(|s| s.map(|x| x.get())).collect();
    /// // Note that the following ordering is considered an API guarantee.
    /// assert_eq!(slots, vec![
    ///     Some(0),
    ///     Some(17),
    ///     Some(0),
    ///     Some(5),
    ///     Some(6),
    ///     Some(17),
    /// ]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn set_pattern(&mut self, pid: Option<PatternID>) {
        self.pid = pid;
    }

    /// Returns the underlying slots, where each slot stores a single offset.
    ///
    /// Every matching capturing group generally corresponds to two slots: one
    /// slot for the starting position and another for the ending position.
    /// Typically, either both are present or neither are. (The weasel word
    /// "typically" is used here because it really depends on the regex engine
    /// implementation. Every sensible regex engine likely adheres to this
    /// invariant, and every regex engine in this crate is sensible.)
    ///
    /// Generally speaking, callers should prefer to use higher level routines
    /// like [`Captures::get_match`] or [`Captures::get_group`].
    ///
    /// An important note here is that a regex engine may not reset all of the
    /// slots to `None` values when no match occurs, or even when a match of
    /// a different pattern occurs. But this depends on how the regex engine
    /// implementation deals with slots.
    ///
    /// # Example
    ///
    /// This example shows how to get the underlying slots from a regex match.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     util::primitives::{PatternID, NonMaxUsize},
    /// };
    ///
    /// let re = PikeVM::new_many(&[
    ///     r"[a-z]+",
    ///     r"[0-9]+",
    /// ])?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, "123", &mut caps);
    /// assert_eq!(Some(PatternID::must(1)), caps.pattern());
    /// // Note that the only guarantee we have here is that slots 2 and 3
    /// // are set to correct values. The contents of the first two slots are
    /// // unspecified since the 0th pattern did not match.
    /// let expected = &[
    ///     None,
    ///     None,
    ///     NonMaxUsize::new(0),
    ///     NonMaxUsize::new(3),
    /// ];
    /// assert_eq!(expected, caps.slots());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn slots(&self) -> &[Option<NonMaxUsize>] {
        &self.slots
    }

    /// Returns the underlying slots as a mutable slice, where each slot stores
    /// a single offset.
    ///
    /// This tends to be most useful for regex engine implementations for
    /// writing offsets for matching capturing groups to slots.
    ///
    /// See [`Captures::slots`] for more information about slots.
    #[inline]
    pub fn slots_mut(&mut self) -> &mut [Option<NonMaxUsize>] {
        &mut self.slots
    }
}

impl core::fmt::Debug for Captures {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut dstruct = f.debug_struct("Captures");
        dstruct.field("pid", &self.pid);
        if let Some(pid) = self.pid {
            dstruct.field("spans", &CapturesDebugMap { pid, caps: self });
        }
        dstruct.finish()
    }
}

/// A little helper type to provide a nice map-like debug representation for
/// our capturing group spans.
struct CapturesDebugMap<'a> {
    pid: PatternID,
    caps: &'a Captures,
}

impl<'a> core::fmt::Debug for CapturesDebugMap<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
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

        let mut map = f.debug_map();
        let names = self.caps.group_info().pattern_names(self.pid);
        for (group_index, maybe_name) in names.enumerate() {
            let key = Key(group_index, maybe_name);
            match self.caps.get_group(group_index) {
                None => map.entry(&key, &None::<()>),
                Some(span) => map.entry(&key, &span),
            };
        }
        map.finish()
    }
}

/// An iterator over all capturing groups in a `Captures` value.
///
/// This iterator includes capturing groups that did not participate in a
/// match. See the [`Captures::iter`] method documentation for more details
/// and examples.
///
/// The lifetime parameter `'a` refers to the lifetime of the underlying
/// `Captures` value.
#[derive(Clone, Debug)]
pub struct CapturesPatternIter<'a> {
    caps: &'a Captures,
    names: core::iter::Enumerate<GroupInfoPatternNames<'a>>,
}

impl<'a> Iterator for CapturesPatternIter<'a> {
    type Item = Option<Span>;

    fn next(&mut self) -> Option<Option<Span>> {
        let (group_index, _) = self.names.next()?;
        Some(self.caps.get_group(group_index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.names.size_hint()
    }

    fn count(self) -> usize {
        self.names.count()
    }
}

impl<'a> ExactSizeIterator for CapturesPatternIter<'a> {}
impl<'a> core::iter::FusedIterator for CapturesPatternIter<'a> {}

/// Represents information about capturing groups in a compiled regex.
///
/// The information encapsulated by this type consists of the following. For
/// each pattern:
///
/// * A map from every capture group name to its corresponding capture group
/// index.
/// * A map from every capture group index to its corresponding capture group
/// name.
/// * A map from capture group index to its corresponding slot index. A slot
/// refers to one half of a capturing group. That is, a capture slot is either
/// the start or end of a capturing group. A slot is usually the mechanism
/// by which a regex engine records offsets for each capturing group during a
/// search.
///
/// A `GroupInfo` uses reference counting internally and is thus cheap to
/// clone.
///
/// # Mapping from capture groups to slots
///
/// One of the main responsibilities of a `GroupInfo` is to build a mapping
/// from `(PatternID, u32)` (where the `u32` is a capture index) to something
/// called a "slot." As mentioned above, a slot refers to one half of a
/// capturing group. Both combined provide the start and end offsets of
/// a capturing group that participated in a match.
///
/// **The mapping between group indices and slots is an API guarantee.** That
/// is, the mapping won't change within a semver compatible release.
///
/// Slots exist primarily because this is a convenient mechanism by which
/// regex engines report group offsets at search time. For example, the
/// [`nfa::thompson::State::Capture`](crate::nfa::thompson::State::Capture)
/// NFA state includes the slot index. When a regex engine transitions through
/// this state, it will likely use the slot index to write the current haystack
/// offset to some region of memory. When a match is found, those slots are
/// then reported to the caller, typically via a convenient abstraction like a
/// [`Captures`] value.
///
/// Because this crate provides first class support for multi-pattern regexes,
/// and because of some performance related reasons, the mapping between
/// capturing groups and slots is a little complex. However, in the case of a
/// single pattern, the mapping can be described very simply: for all capture
/// group indices `i`, its corresponding slots are at `i * 2` and `i * 2 + 1`.
/// Notice that the pattern ID isn't involved at all here, because it only
/// applies to a single-pattern regex, it is therefore always `0`.
///
/// In the multi-pattern case, the mapping is a bit more complicated. To talk
/// about it, we must define what we mean by "implicit" vs "explicit"
/// capturing groups:
///
/// * An **implicit** capturing group refers to the capturing group that is
/// present for every pattern automatically, and corresponds to the overall
/// match of a pattern. Every pattern has precisely one implicit capturing
/// group. It is always unnamed and it always corresponds to the capture group
/// index `0`.
/// * An **explicit** capturing group refers to any capturing group that
/// appears in the concrete syntax of the pattern. (Or, if an NFA was hand
/// built without any concrete syntax, it refers to any capturing group with an
/// index greater than `0`.)
///
/// Some examples:
///
/// * `\w+` has one implicit capturing group and zero explicit capturing
/// groups.
/// * `(\w+)` has one implicit group and one explicit group.
/// * `foo(\d+)(?:\pL+)(\d+)` has one implicit group and two explicit groups.
///
/// Turning back to the slot mapping, we can now state it as follows:
///
/// * Given a pattern ID `pid`, the slots for its implicit group are always
/// at `pid * 2` and `pid * 2 + 1`.
/// * Given a pattern ID `0`, the slots for its explicit groups start
/// at `group_info.pattern_len() * 2`.
/// * Given a pattern ID `pid > 0`, the slots for its explicit groups start
/// immediately following where the slots for the explicit groups of `pid - 1`
/// end.
///
/// In particular, while there is a concrete formula one can use to determine
/// where the slots for the implicit group of any pattern are, there is no
/// general formula for determining where the slots for explicit capturing
/// groups are. This is because each pattern can contain a different number
/// of groups.
///
/// The intended way of getting the slots for a particular capturing group
/// (whether implicit or explicit) is via the [`GroupInfo::slot`] or
/// [`GroupInfo::slots`] method.
///
/// See below for a concrete example of how capturing groups get mapped to
/// slots.
///
/// # Example
///
/// This example shows how to build a new `GroupInfo` and query it for
/// information.
///
/// ```
/// use regex_automata::util::{captures::GroupInfo, primitives::PatternID};
///
/// let info = GroupInfo::new(vec![
///     vec![None, Some("foo")],
///     vec![None],
///     vec![None, None, None, Some("bar"), None],
///     vec![None, None, Some("foo")],
/// ])?;
/// // The number of patterns being tracked.
/// assert_eq!(4, info.pattern_len());
/// // We can query the number of groups for any pattern.
/// assert_eq!(2, info.group_len(PatternID::must(0)));
/// assert_eq!(1, info.group_len(PatternID::must(1)));
/// assert_eq!(5, info.group_len(PatternID::must(2)));
/// assert_eq!(3, info.group_len(PatternID::must(3)));
/// // An invalid pattern always has zero groups.
/// assert_eq!(0, info.group_len(PatternID::must(999)));
/// // 2 slots per group
/// assert_eq!(22, info.slot_len());
///
/// // We can map a group index for a particular pattern to its name, if
/// // one exists.
/// assert_eq!(Some("foo"), info.to_name(PatternID::must(3), 2));
/// assert_eq!(None, info.to_name(PatternID::must(2), 4));
/// // Or map a name to its group index.
/// assert_eq!(Some(1), info.to_index(PatternID::must(0), "foo"));
/// assert_eq!(Some(2), info.to_index(PatternID::must(3), "foo"));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: mapping from capture groups to slots
///
/// This example shows the specific mapping from capture group indices for
/// each pattern to their corresponding slots. The slot values shown in this
/// example are considered an API guarantee.
///
/// ```
/// use regex_automata::util::{captures::GroupInfo, primitives::PatternID};
///
/// let info = GroupInfo::new(vec![
///     vec![None, Some("foo")],
///     vec![None],
///     vec![None, None, None, Some("bar"), None],
///     vec![None, None, Some("foo")],
/// ])?;
///
/// // We first show the slots for each pattern's implicit group.
/// assert_eq!(Some((0, 1)), info.slots(PatternID::must(0), 0));
/// assert_eq!(Some((2, 3)), info.slots(PatternID::must(1), 0));
/// assert_eq!(Some((4, 5)), info.slots(PatternID::must(2), 0));
/// assert_eq!(Some((6, 7)), info.slots(PatternID::must(3), 0));
///
/// // And now we show the slots for each pattern's explicit group.
/// assert_eq!(Some((8, 9)), info.slots(PatternID::must(0), 1));
/// assert_eq!(Some((10, 11)), info.slots(PatternID::must(2), 1));
/// assert_eq!(Some((12, 13)), info.slots(PatternID::must(2), 2));
/// assert_eq!(Some((14, 15)), info.slots(PatternID::must(2), 3));
/// assert_eq!(Some((16, 17)), info.slots(PatternID::must(2), 4));
/// assert_eq!(Some((18, 19)), info.slots(PatternID::must(3), 1));
/// assert_eq!(Some((20, 21)), info.slots(PatternID::must(3), 2));
///
/// // Asking for the slots for an invalid pattern ID or even for an invalid
/// // group index for a specific pattern will return None. So for example,
/// // you're guaranteed to not get the slots for a different pattern than the
/// // one requested.
/// assert_eq!(None, info.slots(PatternID::must(5), 0));
/// assert_eq!(None, info.slots(PatternID::must(1), 1));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct GroupInfo(Arc<GroupInfoInner>);

impl GroupInfo {
    /// Creates a new group info from a sequence of patterns, where each
    /// sequence of patterns yields a sequence of possible group names. The
    /// index of each pattern in the sequence corresponds to its `PatternID`,
    /// and the index of each group in each pattern's sequence corresponds to
    /// its corresponding group index.
    ///
    /// While this constructor is very generic and therefore perhaps hard to
    /// chew on, an example of a valid concrete type that can be passed to
    /// this constructor is `Vec<Vec<Option<String>>>`. The outer `Vec`
    /// corresponds to the patterns, i.e., one `Vec<Option<String>>` per
    /// pattern. The inner `Vec` corresponds to the capturing groups for
    /// each pattern. The `Option<String>` corresponds to the name of the
    /// capturing group, if present.
    ///
    /// It is legal to pass an empty iterator to this constructor. It will
    /// return an empty group info with zero slots. An empty group info is
    /// useful for cases where you have no patterns or for cases where slots
    /// aren't being used at all (e.g., for most DFAs in this crate).
    ///
    /// # Errors
    ///
    /// This constructor returns an error if the given capturing groups are
    /// invalid in some way. Those reasons include, but are not necessarily
    /// limited to:
    ///
    /// * Too many patterns (i.e., `PatternID` would overflow).
    /// * Too many capturing groups (e.g., `u32` would overflow).
    /// * A pattern is given that has no capturing groups. (All patterns must
    /// have at least an implicit capturing group at index `0`.)
    /// * The capturing group at index `0` has a name. It must be unnamed.
    /// * There are duplicate capturing group names within the same pattern.
    /// (Multiple capturing groups with the same name may exist, but they
    /// must be in different patterns.)
    ///
    /// An example below shows how to trigger some of the above error
    /// conditions.
    ///
    /// # Example
    ///
    /// This example shows how to build a new `GroupInfo` and query it for
    /// information.
    ///
    /// ```
    /// use regex_automata::util::captures::GroupInfo;
    ///
    /// let info = GroupInfo::new(vec![
    ///     vec![None, Some("foo")],
    ///     vec![None],
    ///     vec![None, None, None, Some("bar"), None],
    ///     vec![None, None, Some("foo")],
    /// ])?;
    /// // The number of patterns being tracked.
    /// assert_eq!(4, info.pattern_len());
    /// // 2 slots per group
    /// assert_eq!(22, info.slot_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: empty `GroupInfo`
    ///
    /// This example shows how to build a new `GroupInfo` and query it for
    /// information.
    ///
    /// ```
    /// use regex_automata::util::captures::GroupInfo;
    ///
    /// let info = GroupInfo::empty();
    /// // Everything is zero.
    /// assert_eq!(0, info.pattern_len());
    /// assert_eq!(0, info.slot_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error conditions
    ///
    /// This example shows how to provoke some of the ways in which building
    /// a `GroupInfo` can fail.
    ///
    /// ```
    /// use regex_automata::util::captures::GroupInfo;
    ///
    /// // Either the group info is empty, or all patterns must have at least
    /// // one capturing group.
    /// assert!(GroupInfo::new(vec![
    ///     vec![None, Some("a")], // ok
    ///     vec![None], // ok
    ///     vec![], // not ok
    /// ]).is_err());
    /// // Note that building an empty group info is OK.
    /// assert!(GroupInfo::new(Vec::<Vec<Option<String>>>::new()).is_ok());
    ///
    /// // The first group in each pattern must correspond to an implicit
    /// // anonymous group. i.e., One that is not named. By convention, this
    /// // group corresponds to the overall match of a regex. Every other group
    /// // in a pattern is explicit and optional.
    /// assert!(GroupInfo::new(vec![vec![Some("foo")]]).is_err());
    ///
    /// // There must not be duplicate group names within the same pattern.
    /// assert!(GroupInfo::new(vec![
    ///     vec![None, Some("foo"), Some("foo")],
    /// ]).is_err());
    /// // But duplicate names across distinct patterns is OK.
    /// assert!(GroupInfo::new(vec![
    ///     vec![None, Some("foo")],
    ///     vec![None, Some("foo")],
    /// ]).is_ok());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// There are other ways for building a `GroupInfo` to fail but are
    /// difficult to show. For example, if the number of patterns given would
    /// overflow `PatternID`.
    pub fn new<P, G, N>(pattern_groups: P) -> Result<GroupInfo, GroupInfoError>
    where
        P: IntoIterator<Item = G>,
        G: IntoIterator<Item = Option<N>>,
        N: AsRef<str>,
    {
        let mut group_info = GroupInfoInner {
            slot_ranges: vec![],
            name_to_index: vec![],
            index_to_name: vec![],
            memory_extra: 0,
        };
        for (pattern_index, groups) in pattern_groups.into_iter().enumerate() {
            // If we can't convert the pattern index to an ID, then the caller
            // tried to build capture info for too many patterns.
            let pid = PatternID::new(pattern_index)
                .map_err(GroupInfoError::too_many_patterns)?;

            let mut groups_iter = groups.into_iter().enumerate();
            match groups_iter.next() {
                None => return Err(GroupInfoError::missing_groups(pid)),
                Some((_, Some(_))) => {
                    return Err(GroupInfoError::first_must_be_unnamed(pid))
                }
                Some((_, None)) => {}
            }
            group_info.add_first_group(pid);
            // Now iterate over the rest, which correspond to all of the
            // (conventionally) explicit capture groups in a regex pattern.
            for (group_index, maybe_name) in groups_iter {
                // Just like for patterns, if the group index can't be
                // converted to a "small" index, then the caller has given too
                // many groups for a particular pattern.
                let group = SmallIndex::new(group_index).map_err(|_| {
                    GroupInfoError::too_many_groups(pid, group_index)
                })?;
                group_info.add_explicit_group(pid, group, maybe_name)?;
            }
        }
        group_info.fixup_slot_ranges()?;
        Ok(GroupInfo(Arc::new(group_info)))
    }

    /// This creates an empty `GroupInfo`.
    ///
    /// This is a convenience routine for calling `GroupInfo::new` with an
    /// iterator that yields no elements.
    ///
    /// # Example
    ///
    /// This example shows how to build a new empty `GroupInfo` and query it
    /// for information.
    ///
    /// ```
    /// use regex_automata::util::captures::GroupInfo;
    ///
    /// let info = GroupInfo::empty();
    /// // Everything is zero.
    /// assert_eq!(0, info.pattern_len());
    /// assert_eq!(0, info.all_group_len());
    /// assert_eq!(0, info.slot_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn empty() -> GroupInfo {
        GroupInfo::new(core::iter::empty::<[Option<&str>; 0]>())
            .expect("empty group info is always valid")
    }

    /// Return the capture group index corresponding to the given name in the
    /// given pattern. If no such capture group name exists in the given
    /// pattern, then this returns `None`.
    ///
    /// If the given pattern ID is invalid, then this returns `None`.
    ///
    /// This also returns `None` for all inputs if these captures are empty
    /// (e.g., built from an empty [`GroupInfo`]). To check whether captures
    /// are present for a specific pattern, use [`GroupInfo::group_len`].
    ///
    /// # Example
    ///
    /// This example shows how to find the capture index for the given pattern
    /// and group name.
    ///
    /// Remember that capture indices are relative to the pattern, such that
    /// the same capture index value may refer to different capturing groups
    /// for distinct patterns.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{nfa::thompson::NFA, PatternID};
    ///
    /// let (pid0, pid1) = (PatternID::must(0), PatternID::must(1));
    ///
    /// let nfa = NFA::new_many(&[
    ///     r"a(?P<quux>\w+)z(?P<foo>\s+)",
    ///     r"a(?P<foo>\d+)z",
    /// ])?;
    /// let groups = nfa.group_info();
    /// assert_eq!(Some(2), groups.to_index(pid0, "foo"));
    /// // Recall that capture index 0 is always unnamed and refers to the
    /// // entire pattern. So the first capturing group present in the pattern
    /// // itself always starts at index 1.
    /// assert_eq!(Some(1), groups.to_index(pid1, "foo"));
    ///
    /// // And if a name does not exist for a particular pattern, None is
    /// // returned.
    /// assert!(groups.to_index(pid0, "quux").is_some());
    /// assert!(groups.to_index(pid1, "quux").is_none());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_index(&self, pid: PatternID, name: &str) -> Option<usize> {
        let indices = self.0.name_to_index.get(pid.as_usize())?;
        indices.get(name).cloned().map(|i| i.as_usize())
    }

    /// Return the capture name for the given index and given pattern. If the
    /// corresponding group does not have a name, then this returns `None`.
    ///
    /// If the pattern ID is invalid, then this returns `None`.
    ///
    /// If the group index is invalid for the given pattern, then this returns
    /// `None`. A group `index` is valid for a pattern `pid` in an `nfa` if and
    /// only if `index < nfa.pattern_capture_len(pid)`.
    ///
    /// This also returns `None` for all inputs if these captures are empty
    /// (e.g., built from an empty [`GroupInfo`]). To check whether captures
    /// are present for a specific pattern, use [`GroupInfo::group_len`].
    ///
    /// # Example
    ///
    /// This example shows how to find the capture group name for the given
    /// pattern and group index.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{nfa::thompson::NFA, PatternID};
    ///
    /// let (pid0, pid1) = (PatternID::must(0), PatternID::must(1));
    ///
    /// let nfa = NFA::new_many(&[
    ///     r"a(?P<foo>\w+)z(\s+)x(\d+)",
    ///     r"a(\d+)z(?P<foo>\s+)",
    /// ])?;
    /// let groups = nfa.group_info();
    /// assert_eq!(None, groups.to_name(pid0, 0));
    /// assert_eq!(Some("foo"), groups.to_name(pid0, 1));
    /// assert_eq!(None, groups.to_name(pid0, 2));
    /// assert_eq!(None, groups.to_name(pid0, 3));
    ///
    /// assert_eq!(None, groups.to_name(pid1, 0));
    /// assert_eq!(None, groups.to_name(pid1, 1));
    /// assert_eq!(Some("foo"), groups.to_name(pid1, 2));
    /// // '3' is not a valid capture index for the second pattern.
    /// assert_eq!(None, groups.to_name(pid1, 3));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_name(&self, pid: PatternID, group_index: usize) -> Option<&str> {
        let pattern_names = self.0.index_to_name.get(pid.as_usize())?;
        pattern_names.get(group_index)?.as_deref()
    }

    /// Return an iterator of all capture groups and their names (if present)
    /// for a particular pattern.
    ///
    /// If the given pattern ID is invalid or if this `GroupInfo` is empty,
    /// then the iterator yields no elements.
    ///
    /// The number of elements yielded by this iterator is always equal to
    /// the result of calling [`GroupInfo::group_len`] with the same
    /// `PatternID`.
    ///
    /// # Example
    ///
    /// This example shows how to get a list of all capture group names for
    /// a particular pattern.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, PatternID};
    ///
    /// let nfa = NFA::new(r"(a)(?P<foo>b)(c)(d)(?P<bar>e)")?;
    /// // The first is the implicit group that is always unnammed. The next
    /// // 5 groups are the explicit groups found in the concrete syntax above.
    /// let expected = vec![None, None, Some("foo"), None, None, Some("bar")];
    /// let got: Vec<Option<&str>> =
    ///     nfa.group_info().pattern_names(PatternID::ZERO).collect();
    /// assert_eq!(expected, got);
    ///
    /// // Using an invalid pattern ID will result in nothing yielded.
    /// let got = nfa.group_info().pattern_names(PatternID::must(999)).count();
    /// assert_eq!(0, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn pattern_names(&self, pid: PatternID) -> GroupInfoPatternNames<'_> {
        GroupInfoPatternNames {
            it: self
                .0
                .index_to_name
                .get(pid.as_usize())
                .map(|indices| indices.iter())
                .unwrap_or([].iter()),
        }
    }

    /// Return an iterator of all capture groups for all patterns supported by
    /// this `GroupInfo`. Each item yielded is a triple of the group's pattern
    /// ID, index in the pattern and the group's name, if present.
    ///
    /// # Example
    ///
    /// This example shows how to get a list of all capture groups found in
    /// one NFA, potentially spanning multiple patterns.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, PatternID};
    ///
    /// let nfa = NFA::new_many(&[
    ///     r"(?P<foo>a)",
    ///     r"a",
    ///     r"(a)",
    /// ])?;
    /// let expected = vec![
    ///     (PatternID::must(0), 0, None),
    ///     (PatternID::must(0), 1, Some("foo")),
    ///     (PatternID::must(1), 0, None),
    ///     (PatternID::must(2), 0, None),
    ///     (PatternID::must(2), 1, None),
    /// ];
    /// let got: Vec<(PatternID, usize, Option<&str>)> =
    ///     nfa.group_info().all_names().collect();
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Unlike other capturing group related routines, this routine doesn't
    /// panic even if captures aren't enabled on this NFA:
    ///
    /// ```
    /// use regex_automata::nfa::thompson::{NFA, WhichCaptures};
    ///
    /// let nfa = NFA::compiler()
    ///     .configure(NFA::config().which_captures(WhichCaptures::None))
    ///     .build_many(&[
    ///         r"(?P<foo>a)",
    ///         r"a",
    ///         r"(a)",
    ///     ])?;
    /// // When captures aren't enabled, there's nothing to return.
    /// assert_eq!(0, nfa.group_info().all_names().count());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn all_names(&self) -> GroupInfoAllNames<'_> {
        GroupInfoAllNames {
            group_info: self,
            pids: PatternID::iter(self.pattern_len()),
            current_pid: None,
            names: None,
        }
    }

    /// Returns the starting and ending slot corresponding to the given
    /// capturing group for the given pattern. The ending slot is always one
    /// more than the starting slot returned.
    ///
    /// Note that this is like [`GroupInfo::slot`], except that it also returns
    /// the ending slot value for convenience.
    ///
    /// If either the pattern ID or the capture index is invalid, then this
    /// returns None.
    ///
    /// # Example
    ///
    /// This example shows that the starting slots for the first capturing
    /// group of each pattern are distinct.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, PatternID};
    ///
    /// let nfa = NFA::new_many(&["a", "b"])?;
    /// assert_ne!(
    ///     nfa.group_info().slots(PatternID::must(0), 0),
    ///     nfa.group_info().slots(PatternID::must(1), 0),
    /// );
    ///
    /// // Also, the start and end slot values are never equivalent.
    /// let (start, end) = nfa.group_info().slots(PatternID::ZERO, 0).unwrap();
    /// assert_ne!(start, end);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn slots(
        &self,
        pid: PatternID,
        group_index: usize,
    ) -> Option<(usize, usize)> {
        // Since 'slot' only even returns valid starting slots, we know that
        // there must also be an end slot and that end slot is always one more
        // than the start slot.
        self.slot(pid, group_index).map(|start| (start, start + 1))
    }

    /// Returns the starting slot corresponding to the given capturing group
    /// for the given pattern. The ending slot is always one more than the
    /// value returned.
    ///
    /// If either the pattern ID or the capture index is invalid, then this
    /// returns None.
    ///
    /// # Example
    ///
    /// This example shows that the starting slots for the first capturing
    /// group of each pattern are distinct.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, PatternID};
    ///
    /// let nfa = NFA::new_many(&["a", "b"])?;
    /// assert_ne!(
    ///     nfa.group_info().slot(PatternID::must(0), 0),
    ///     nfa.group_info().slot(PatternID::must(1), 0),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn slot(&self, pid: PatternID, group_index: usize) -> Option<usize> {
        if group_index >= self.group_len(pid) {
            return None;
        }
        // At this point, we know that 'pid' refers to a real pattern and that
        // 'group_index' refers to a real group. We therefore also know that
        // the pattern and group can be combined to return a correct slot.
        // That's why we don't need to use checked arithmetic below.
        if group_index == 0 {
            Some(pid.as_usize() * 2)
        } else {
            // As above, we don't need to check that our slot is less than the
            // end of our range since we already know the group index is a
            // valid index for the given pattern.
            let (start, _) = self.0.slot_ranges[pid];
            Some(start.as_usize() + ((group_index - 1) * 2))
        }
    }

    /// Returns the total number of patterns in this `GroupInfo`.
    ///
    /// This may return zero if the `GroupInfo` was constructed with no
    /// patterns.
    ///
    /// This is guaranteed to be no bigger than [`PatternID::LIMIT`] because
    /// `GroupInfo` construction will fail if too many patterns are added.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::nfa::thompson::NFA;
    ///
    /// let nfa = NFA::new_many(&["[0-9]+", "[a-z]+", "[A-Z]+"])?;
    /// assert_eq!(3, nfa.group_info().pattern_len());
    ///
    /// let nfa = NFA::never_match();
    /// assert_eq!(0, nfa.group_info().pattern_len());
    ///
    /// let nfa = NFA::always_match();
    /// assert_eq!(1, nfa.group_info().pattern_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn pattern_len(&self) -> usize {
        self.0.pattern_len()
    }

    /// Return the number of capture groups in a pattern.
    ///
    /// If the pattern ID is invalid, then this returns `0`.
    ///
    /// # Example
    ///
    /// This example shows how the values returned by this routine may vary
    /// for different patterns and NFA configurations.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::{NFA, WhichCaptures}, PatternID};
    ///
    /// let nfa = NFA::new(r"(a)(b)(c)")?;
    /// // There are 3 explicit groups in the pattern's concrete syntax and
    /// // 1 unnamed and implicit group spanning the entire pattern.
    /// assert_eq!(4, nfa.group_info().group_len(PatternID::ZERO));
    ///
    /// let nfa = NFA::new(r"abc")?;
    /// // There is just the unnamed implicit group.
    /// assert_eq!(1, nfa.group_info().group_len(PatternID::ZERO));
    ///
    /// let nfa = NFA::compiler()
    ///     .configure(NFA::config().which_captures(WhichCaptures::None))
    ///     .build(r"abc")?;
    /// // We disabled capturing groups, so there are none.
    /// assert_eq!(0, nfa.group_info().group_len(PatternID::ZERO));
    ///
    /// let nfa = NFA::compiler()
    ///     .configure(NFA::config().which_captures(WhichCaptures::None))
    ///     .build(r"(a)(b)(c)")?;
    /// // We disabled capturing groups, so there are none, even if there are
    /// // explicit groups in the concrete syntax.
    /// assert_eq!(0, nfa.group_info().group_len(PatternID::ZERO));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn group_len(&self, pid: PatternID) -> usize {
        self.0.group_len(pid)
    }

    /// Return the total number of capture groups across all patterns.
    ///
    /// This includes implicit groups that represent the entire match of a
    /// pattern.
    ///
    /// # Example
    ///
    /// This example shows how the values returned by this routine may vary
    /// for different patterns and NFA configurations.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::{NFA, WhichCaptures}, PatternID};
    ///
    /// let nfa = NFA::new(r"(a)(b)(c)")?;
    /// // There are 3 explicit groups in the pattern's concrete syntax and
    /// // 1 unnamed and implicit group spanning the entire pattern.
    /// assert_eq!(4, nfa.group_info().all_group_len());
    ///
    /// let nfa = NFA::new(r"abc")?;
    /// // There is just the unnamed implicit group.
    /// assert_eq!(1, nfa.group_info().all_group_len());
    ///
    /// let nfa = NFA::new_many(&["(a)", "b", "(c)"])?;
    /// // Each pattern has one implicit groups, and two
    /// // patterns have one explicit group each.
    /// assert_eq!(5, nfa.group_info().all_group_len());
    ///
    /// let nfa = NFA::compiler()
    ///     .configure(NFA::config().which_captures(WhichCaptures::None))
    ///     .build(r"abc")?;
    /// // We disabled capturing groups, so there are none.
    /// assert_eq!(0, nfa.group_info().all_group_len());
    ///
    /// let nfa = NFA::compiler()
    ///     .configure(NFA::config().which_captures(WhichCaptures::None))
    ///     .build(r"(a)(b)(c)")?;
    /// // We disabled capturing groups, so there are none, even if there are
    /// // explicit groups in the concrete syntax.
    /// assert_eq!(0, nfa.group_info().group_len(PatternID::ZERO));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn all_group_len(&self) -> usize {
        self.slot_len() / 2
    }

    /// Returns the total number of slots in this `GroupInfo` across all
    /// patterns.
    ///
    /// The total number of slots is always twice the total number of capturing
    /// groups, including both implicit and explicit groups.
    ///
    /// # Example
    ///
    /// This example shows the relationship between the number of capturing
    /// groups and slots.
    ///
    /// ```
    /// use regex_automata::util::captures::GroupInfo;
    ///
    /// // There are 11 total groups here.
    /// let info = GroupInfo::new(vec![
    ///     vec![None, Some("foo")],
    ///     vec![None],
    ///     vec![None, None, None, Some("bar"), None],
    ///     vec![None, None, Some("foo")],
    /// ])?;
    /// // 2 slots per group gives us 11*2=22 slots.
    /// assert_eq!(22, info.slot_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn slot_len(&self) -> usize {
        self.0.small_slot_len().as_usize()
    }

    /// Returns the total number of slots for implicit capturing groups.
    ///
    /// This is like [`GroupInfo::slot_len`], except it doesn't include the
    /// explicit slots for each pattern. Since there are always exactly 2
    /// implicit slots for each pattern, the number of implicit slots is always
    /// equal to twice the number of patterns.
    ///
    /// # Example
    ///
    /// This example shows the relationship between the number of capturing
    /// groups, implicit slots and explicit slots.
    ///
    /// ```
    /// use regex_automata::util::captures::GroupInfo;
    ///
    /// // There are 11 total groups here.
    /// let info = GroupInfo::new(vec![vec![None, Some("foo"), Some("bar")]])?;
    /// // 2 slots per group gives us 11*2=22 slots.
    /// assert_eq!(6, info.slot_len());
    /// // 2 implicit slots per pattern gives us 2 implicit slots since there
    /// // is 1 pattern.
    /// assert_eq!(2, info.implicit_slot_len());
    /// // 2 explicit capturing groups gives us 2*2=4 explicit slots.
    /// assert_eq!(4, info.explicit_slot_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn implicit_slot_len(&self) -> usize {
        self.pattern_len() * 2
    }

    /// Returns the total number of slots for explicit capturing groups.
    ///
    /// This is like [`GroupInfo::slot_len`], except it doesn't include the
    /// implicit slots for each pattern. (There are always 2 implicit slots for
    /// each pattern.)
    ///
    /// For a non-empty `GroupInfo`, it is always the case that `slot_len` is
    /// strictly greater than `explicit_slot_len`. For an empty `GroupInfo`,
    /// both the total number of slots and the number of explicit slots is
    /// `0`.
    ///
    /// # Example
    ///
    /// This example shows the relationship between the number of capturing
    /// groups, implicit slots and explicit slots.
    ///
    /// ```
    /// use regex_automata::util::captures::GroupInfo;
    ///
    /// // There are 11 total groups here.
    /// let info = GroupInfo::new(vec![vec![None, Some("foo"), Some("bar")]])?;
    /// // 2 slots per group gives us 11*2=22 slots.
    /// assert_eq!(6, info.slot_len());
    /// // 2 implicit slots per pattern gives us 2 implicit slots since there
    /// // is 1 pattern.
    /// assert_eq!(2, info.implicit_slot_len());
    /// // 2 explicit capturing groups gives us 2*2=4 explicit slots.
    /// assert_eq!(4, info.explicit_slot_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn explicit_slot_len(&self) -> usize {
        self.slot_len().saturating_sub(self.implicit_slot_len())
    }

    /// Returns the memory usage, in bytes, of this `GroupInfo`.
    ///
    /// This does **not** include the stack size used up by this `GroupInfo`.
    /// To compute that, use `std::mem::size_of::<GroupInfo>()`.
    #[inline]
    pub fn memory_usage(&self) -> usize {
        use core::mem::size_of as s;

        s::<GroupInfoInner>()
            + self.0.slot_ranges.len() * s::<(SmallIndex, SmallIndex)>()
            + self.0.name_to_index.len() * s::<CaptureNameMap>()
            + self.0.index_to_name.len() * s::<Vec<Option<Arc<str>>>>()
            + self.0.memory_extra
    }
}

/// A map from capture group name to its corresponding capture group index.
///
/// This type is actually wrapped inside a Vec indexed by pattern ID on a
/// `GroupInfo`, since multiple patterns may have the same capture group name.
/// That is, each pattern gets its own namespace of capture group names.
///
/// Perhaps a more memory efficient representation would be
/// HashMap<(PatternID, Arc<str>), usize>, but this makes it difficult to look
/// up a capture index by name without producing a `Arc<str>`, which requires
/// an allocation. To fix this, I think we'd need to define our own unsized
/// type or something? Anyway, I didn't give this much thought since it
/// probably doesn't matter much in the grand scheme of things. But it did
/// stand out to me as mildly wasteful.
#[cfg(feature = "std")]
type CaptureNameMap = std::collections::HashMap<Arc<str>, SmallIndex>;
#[cfg(not(feature = "std"))]
type CaptureNameMap = alloc::collections::BTreeMap<Arc<str>, SmallIndex>;

/// The inner guts of `GroupInfo`. This type only exists so that it can
/// be wrapped in an `Arc` to make `GroupInfo` reference counted.
#[derive(Debug, Default)]
struct GroupInfoInner {
    slot_ranges: Vec<(SmallIndex, SmallIndex)>,
    name_to_index: Vec<CaptureNameMap>,
    index_to_name: Vec<Vec<Option<Arc<str>>>>,
    memory_extra: usize,
}

impl GroupInfoInner {
    /// This adds the first unnamed group for the given pattern ID. The given
    /// pattern ID must be zero if this is the first time this method is
    /// called, or must be exactly one more than the pattern ID supplied to the
    /// previous call to this method. (This method panics if this rule is
    /// violated.)
    ///
    /// This can be thought of as initializing the GroupInfo state for the
    /// given pattern and closing off the state for any previous pattern.
    fn add_first_group(&mut self, pid: PatternID) {
        assert_eq!(pid.as_usize(), self.slot_ranges.len());
        assert_eq!(pid.as_usize(), self.name_to_index.len());
        assert_eq!(pid.as_usize(), self.index_to_name.len());
        // This is the start of our slots for the explicit capturing groups.
        // Note that since the slots for the 0th group for every pattern appear
        // before any slots for the nth group (where n > 0) in any pattern, we
        // will have to fix up the slot ranges once we know how many patterns
        // we've added capture groups for.
        let slot_start = self.small_slot_len();
        self.slot_ranges.push((slot_start, slot_start));
        self.name_to_index.push(CaptureNameMap::new());
        self.index_to_name.push(vec![None]);
        self.memory_extra += core::mem::size_of::<Option<Arc<str>>>();
    }

    /// Add an explicit capturing group for the given pattern with the given
    /// index. If the group has a name, then that must be given as well.
    ///
    /// Note that every capturing group except for the first or zeroth group is
    /// explicit.
    ///
    /// This returns an error if adding this group would result in overflowing
    /// slot indices or if a capturing group with the same name for this
    /// pattern has already been added.
    fn add_explicit_group<N: AsRef<str>>(
        &mut self,
        pid: PatternID,
        group: SmallIndex,
        maybe_name: Option<N>,
    ) -> Result<(), GroupInfoError> {
        // We also need to check that the slot index generated for
        // this group is also valid. Although, this is a little weird
        // because we offset these indices below, at which point, we'll
        // have to recheck them. Gosh this is annoying. Note that
        // the '+2' below is OK because 'end' is guaranteed to be less
        // than isize::MAX.
        let end = &mut self.slot_ranges[pid].1;
        *end = SmallIndex::new(end.as_usize() + 2).map_err(|_| {
            GroupInfoError::too_many_groups(pid, group.as_usize())
        })?;
        if let Some(name) = maybe_name {
            let name = Arc::<str>::from(name.as_ref());
            if self.name_to_index[pid].contains_key(&*name) {
                return Err(GroupInfoError::duplicate(pid, &name));
            }
            let len = name.len();
            self.name_to_index[pid].insert(Arc::clone(&name), group);
            self.index_to_name[pid].push(Some(name));
            // Adds the memory used by the Arc<str> in both maps.
            self.memory_extra +=
                2 * (len + core::mem::size_of::<Option<Arc<str>>>());
            // And also the value entry for the 'name_to_index' map.
            // This is probably an underestimate for 'name_to_index' since
            // hashmaps/btrees likely have some non-zero overhead, but we
            // assume here that they have zero overhead.
            self.memory_extra += core::mem::size_of::<SmallIndex>();
        } else {
            self.index_to_name[pid].push(None);
            self.memory_extra += core::mem::size_of::<Option<Arc<str>>>();
        }
        // This is a sanity assert that checks that our group index
        // is in line with the number of groups added so far for this
        // pattern.
        assert_eq!(group.one_more(), self.group_len(pid));
        // And is also in line with the 'index_to_name' map.
        assert_eq!(group.one_more(), self.index_to_name[pid].len());
        Ok(())
    }

    /// This corrects the slot ranges to account for the slots corresponding
    /// to the zeroth group of each pattern. That is, every slot range is
    /// offset by 'pattern_len() * 2', since each pattern uses two slots to
    /// represent the zeroth group.
    fn fixup_slot_ranges(&mut self) -> Result<(), GroupInfoError> {
        use crate::util::primitives::IteratorIndexExt;
        // Since we know number of patterns fits in PatternID and
        // PatternID::MAX < isize::MAX, it follows that multiplying by 2 will
        // never overflow usize.
        let offset = self.pattern_len().checked_mul(2).unwrap();
        for (pid, &mut (ref mut start, ref mut end)) in
            self.slot_ranges.iter_mut().with_pattern_ids()
        {
            let group_len = 1 + ((end.as_usize() - start.as_usize()) / 2);
            let new_end = match end.as_usize().checked_add(offset) {
                Some(new_end) => new_end,
                None => {
                    return Err(GroupInfoError::too_many_groups(
                        pid, group_len,
                    ))
                }
            };
            *end = SmallIndex::new(new_end).map_err(|_| {
                GroupInfoError::too_many_groups(pid, group_len)
            })?;
            // Since start <= end, if end is valid then start must be too.
            *start = SmallIndex::new(start.as_usize() + offset).unwrap();
        }
        Ok(())
    }

    /// Return the total number of patterns represented by this capture slot
    /// info.
    fn pattern_len(&self) -> usize {
        self.slot_ranges.len()
    }

    /// Return the total number of capturing groups for the given pattern. If
    /// the given pattern isn't valid for this capture slot info, then 0 is
    /// returned.
    fn group_len(&self, pid: PatternID) -> usize {
        let (start, end) = match self.slot_ranges.get(pid.as_usize()) {
            None => return 0,
            Some(range) => range,
        };
        // The difference between any two SmallIndex values always fits in a
        // usize since we know that SmallIndex::MAX <= isize::MAX-1. We also
        // know that start<=end by construction and that the number of groups
        // never exceeds SmallIndex and thus never overflows usize.
        1 + ((end.as_usize() - start.as_usize()) / 2)
    }

    /// Return the total number of slots in this capture slot info as a
    /// "small index."
    fn small_slot_len(&self) -> SmallIndex {
        // Since slots are allocated in order of pattern (starting at 0) and
        // then in order of capture group, it follows that the number of slots
        // is the end of the range of slots for the last pattern. This is
        // true even when the last pattern has no capturing groups, since
        // 'slot_ranges' will still represent it explicitly with an empty
        // range.
        self.slot_ranges.last().map_or(SmallIndex::ZERO, |&(_, end)| end)
    }
}

/// An error that may occur when building a `GroupInfo`.
///
/// Building a `GroupInfo` does a variety of checks to make sure the
/// capturing groups satisfy a number of invariants. This includes, but is not
/// limited to, ensuring that the first capturing group is unnamed and that
/// there are no duplicate capture groups for a specific pattern.
#[derive(Clone, Debug)]
pub struct GroupInfoError {
    kind: GroupInfoErrorKind,
}

/// The kind of error that occurs when building a `GroupInfo` fails.
///
/// We keep this un-exported because it's not clear how useful it is to
/// export it.
#[derive(Clone, Debug)]
enum GroupInfoErrorKind {
    /// This occurs when too many patterns have been added. i.e., It would
    /// otherwise overflow a `PatternID`.
    TooManyPatterns { err: PatternIDError },
    /// This occurs when too many capturing groups have been added for a
    /// particular pattern.
    TooManyGroups {
        /// The ID of the pattern that had too many groups.
        pattern: PatternID,
        /// The minimum number of groups that the caller has tried to add for
        /// a pattern.
        minimum: usize,
    },
    /// An error that occurs when a pattern has no capture groups. Either the
    /// group info must be empty, or all patterns must have at least one group
    /// (corresponding to the unnamed group for the entire pattern).
    MissingGroups {
        /// The ID of the pattern that had no capturing groups.
        pattern: PatternID,
    },
    /// An error that occurs when one tries to provide a name for the capture
    /// group at index 0. This capturing group must currently always be
    /// unnamed.
    FirstMustBeUnnamed {
        /// The ID of the pattern that was found to have a named first
        /// capturing group.
        pattern: PatternID,
    },
    /// An error that occurs when duplicate capture group names for the same
    /// pattern are added.
    ///
    /// NOTE: At time of writing, this error can never occur if you're using
    /// regex-syntax, since the parser itself will reject patterns with
    /// duplicate capture group names. This error can only occur when the
    /// builder is used to hand construct NFAs.
    Duplicate {
        /// The pattern in which the duplicate capture group name was found.
        pattern: PatternID,
        /// The duplicate name.
        name: String,
    },
}

impl GroupInfoError {
    fn too_many_patterns(err: PatternIDError) -> GroupInfoError {
        GroupInfoError { kind: GroupInfoErrorKind::TooManyPatterns { err } }
    }

    fn too_many_groups(pattern: PatternID, minimum: usize) -> GroupInfoError {
        GroupInfoError {
            kind: GroupInfoErrorKind::TooManyGroups { pattern, minimum },
        }
    }

    fn missing_groups(pattern: PatternID) -> GroupInfoError {
        GroupInfoError { kind: GroupInfoErrorKind::MissingGroups { pattern } }
    }

    fn first_must_be_unnamed(pattern: PatternID) -> GroupInfoError {
        GroupInfoError {
            kind: GroupInfoErrorKind::FirstMustBeUnnamed { pattern },
        }
    }

    fn duplicate(pattern: PatternID, name: &str) -> GroupInfoError {
        GroupInfoError {
            kind: GroupInfoErrorKind::Duplicate {
                pattern,
                name: String::from(name),
            },
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GroupInfoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            GroupInfoErrorKind::TooManyPatterns { .. }
            | GroupInfoErrorKind::TooManyGroups { .. }
            | GroupInfoErrorKind::MissingGroups { .. }
            | GroupInfoErrorKind::FirstMustBeUnnamed { .. }
            | GroupInfoErrorKind::Duplicate { .. } => None,
        }
    }
}

impl core::fmt::Display for GroupInfoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use self::GroupInfoErrorKind::*;

        match self.kind {
            TooManyPatterns { ref err } => {
                write!(f, "too many patterns to build capture info: {}", err)
            }
            TooManyGroups { pattern, minimum } => {
                write!(
                    f,
                    "too many capture groups (at least {}) were \
                     found for pattern {}",
                    minimum,
                    pattern.as_usize()
                )
            }
            MissingGroups { pattern } => write!(
                f,
                "no capturing groups found for pattern {} \
                 (either all patterns have zero groups or all patterns have \
                  at least one group)",
                pattern.as_usize(),
            ),
            FirstMustBeUnnamed { pattern } => write!(
                f,
                "first capture group (at index 0) for pattern {} has a name \
                 (it must be unnamed)",
                pattern.as_usize(),
            ),
            Duplicate { pattern, ref name } => write!(
                f,
                "duplicate capture group name '{}' found for pattern {}",
                name,
                pattern.as_usize(),
            ),
        }
    }
}

/// An iterator over capturing groups and their names for a specific pattern.
///
/// This iterator is created by [`GroupInfo::pattern_names`].
///
/// The lifetime parameter `'a` refers to the lifetime of the `GroupInfo`
/// from which this iterator was created.
#[derive(Clone, Debug)]
pub struct GroupInfoPatternNames<'a> {
    it: core::slice::Iter<'a, Option<Arc<str>>>,
}

impl GroupInfoPatternNames<'static> {
    fn empty() -> GroupInfoPatternNames<'static> {
        GroupInfoPatternNames { it: [].iter() }
    }
}

impl<'a> Iterator for GroupInfoPatternNames<'a> {
    type Item = Option<&'a str>;

    fn next(&mut self) -> Option<Option<&'a str>> {
        self.it.next().map(|x| x.as_deref())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    fn count(self) -> usize {
        self.it.count()
    }
}

impl<'a> ExactSizeIterator for GroupInfoPatternNames<'a> {}
impl<'a> core::iter::FusedIterator for GroupInfoPatternNames<'a> {}

/// An iterator over capturing groups and their names for a `GroupInfo`.
///
/// This iterator is created by [`GroupInfo::all_names`].
///
/// The lifetime parameter `'a` refers to the lifetime of the `GroupInfo`
/// from which this iterator was created.
#[derive(Debug)]
pub struct GroupInfoAllNames<'a> {
    group_info: &'a GroupInfo,
    pids: PatternIDIter,
    current_pid: Option<PatternID>,
    names: Option<core::iter::Enumerate<GroupInfoPatternNames<'a>>>,
}

impl<'a> Iterator for GroupInfoAllNames<'a> {
    type Item = (PatternID, usize, Option<&'a str>);

    fn next(&mut self) -> Option<(PatternID, usize, Option<&'a str>)> {
        // If the group info has no captures, then we never have anything
        // to yield. We need to consider this case explicitly (at time of
        // writing) because 'pattern_capture_names' will panic if captures
        // aren't enabled.
        if self.group_info.0.index_to_name.is_empty() {
            return None;
        }
        if self.current_pid.is_none() {
            self.current_pid = Some(self.pids.next()?);
        }
        let pid = self.current_pid.unwrap();
        if self.names.is_none() {
            self.names = Some(self.group_info.pattern_names(pid).enumerate());
        }
        let (group_index, name) = match self.names.as_mut().unwrap().next() {
            Some((group_index, name)) => (group_index, name),
            None => {
                self.current_pid = None;
                self.names = None;
                return self.next();
            }
        };
        Some((pid, group_index, name))
    }
}
