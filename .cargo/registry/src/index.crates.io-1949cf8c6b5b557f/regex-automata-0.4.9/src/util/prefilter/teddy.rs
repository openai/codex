use crate::util::{
    prefilter::PrefilterI,
    search::{MatchKind, Span},
};

#[derive(Clone, Debug)]
pub(crate) struct Teddy {
    #[cfg(not(feature = "perf-literal-multisubstring"))]
    _unused: (),
    /// The actual Teddy searcher.
    ///
    /// Technically, it's possible that Teddy doesn't actually get used, since
    /// Teddy does require its haystack to at least be of a certain size
    /// (usually around the size of whatever vector is being used, so ~16
    /// or ~32 bytes). For haystacks shorter than that, the implementation
    /// currently uses Rabin-Karp.
    #[cfg(feature = "perf-literal-multisubstring")]
    searcher: aho_corasick::packed::Searcher,
    /// When running an anchored search, the packed searcher can't handle it so
    /// we defer to Aho-Corasick itself. Kind of sad, but changing the packed
    /// searchers to support anchored search would be difficult at worst and
    /// annoying at best. Since packed searchers only apply to small numbers of
    /// literals, we content ourselves that this is not much of an added cost.
    /// (That packed searchers only work with a small number of literals is
    /// also why we use a DFA here. Otherwise, the memory usage of a DFA would
    /// likely be unacceptable.)
    #[cfg(feature = "perf-literal-multisubstring")]
    anchored_ac: aho_corasick::dfa::DFA,
    /// The length of the smallest literal we look for.
    ///
    /// We use this as a heuristic to figure out whether this will be "fast" or
    /// not. Generally, the longer the better, because longer needles are more
    /// discriminating and thus reduce false positive rate.
    #[cfg(feature = "perf-literal-multisubstring")]
    minimum_len: usize,
}

impl Teddy {
    pub(crate) fn new<B: AsRef<[u8]>>(
        kind: MatchKind,
        needles: &[B],
    ) -> Option<Teddy> {
        #[cfg(not(feature = "perf-literal-multisubstring"))]
        {
            None
        }
        #[cfg(feature = "perf-literal-multisubstring")]
        {
            // We only really support leftmost-first semantics. In
            // theory we could at least support leftmost-longest, as the
            // aho-corasick crate does, but regex-automata doesn't know about
            // leftmost-longest currently.
            //
            // And like the aho-corasick prefilter, if we're using `All`
            // semantics, then we can still use leftmost semantics for a
            // prefilter. (This might be a suspicious choice for the literal
            // engine, which uses a prefilter as a regex engine directly, but
            // that only happens when using leftmost-first semantics.)
            let (packed_match_kind, ac_match_kind) = match kind {
                MatchKind::LeftmostFirst | MatchKind::All => (
                    aho_corasick::packed::MatchKind::LeftmostFirst,
                    aho_corasick::MatchKind::LeftmostFirst,
                ),
            };
            let minimum_len =
                needles.iter().map(|n| n.as_ref().len()).min().unwrap_or(0);
            let packed = aho_corasick::packed::Config::new()
                .match_kind(packed_match_kind)
                .builder()
                .extend(needles)
                .build()?;
            let anchored_ac = aho_corasick::dfa::DFA::builder()
                .match_kind(ac_match_kind)
                .start_kind(aho_corasick::StartKind::Anchored)
                .prefilter(false)
                .build(needles)
                .ok()?;
            Some(Teddy { searcher: packed, anchored_ac, minimum_len })
        }
    }
}

impl PrefilterI for Teddy {
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(feature = "perf-literal-multisubstring"))]
        {
            unreachable!()
        }
        #[cfg(feature = "perf-literal-multisubstring")]
        {
            let ac_span =
                aho_corasick::Span { start: span.start, end: span.end };
            self.searcher
                .find_in(haystack, ac_span)
                .map(|m| Span { start: m.start(), end: m.end() })
        }
    }

    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(feature = "perf-literal-multisubstring"))]
        {
            unreachable!()
        }
        #[cfg(feature = "perf-literal-multisubstring")]
        {
            use aho_corasick::automaton::Automaton;
            let input = aho_corasick::Input::new(haystack)
                .anchored(aho_corasick::Anchored::Yes)
                .span(span.start..span.end);
            self.anchored_ac
                .try_find(&input)
                // OK because we build the DFA with anchored support.
                .expect("aho-corasick DFA should never fail")
                .map(|m| Span { start: m.start(), end: m.end() })
        }
    }

    fn memory_usage(&self) -> usize {
        #[cfg(not(feature = "perf-literal-multisubstring"))]
        {
            unreachable!()
        }
        #[cfg(feature = "perf-literal-multisubstring")]
        {
            use aho_corasick::automaton::Automaton;
            self.searcher.memory_usage() + self.anchored_ac.memory_usage()
        }
    }

    fn is_fast(&self) -> bool {
        #[cfg(not(feature = "perf-literal-multisubstring"))]
        {
            unreachable!()
        }
        #[cfg(feature = "perf-literal-multisubstring")]
        {
            // Teddy is usually quite fast, but I have seen some cases where
            // a large number of literals can overwhelm it and make it not so
            // fast. We make an educated but conservative guess at a limit, at
            // which point, we're not so comfortable thinking Teddy is "fast."
            //
            // Well... this used to incorporate a "limit" on the *number*
            // of literals, but I have since changed it to a minimum on the
            // *smallest* literal. Namely, when there is a very small literal
            // (1 or 2 bytes), it is far more likely that it leads to a higher
            // false positive rate. (Although, of course, not always. For
            // example, 'zq' is likely to have a very low false positive rate.)
            // But when we have 3 bytes, we have a really good chance of being
            // quite discriminatory and thus fast.
            //
            // We may still want to add some kind of limit on the number of
            // literals here, but keep in mind that Teddy already has its own
            // somewhat small limit (64 at time of writing). The main issue
            // here is that if 'is_fast' is false, it opens the door for the
            // reverse inner optimization to kick in. We really only want to
            // resort to the reverse inner optimization if we absolutely must.
            self.minimum_len >= 3
        }
    }
}
