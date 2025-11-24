use crate::util::{
    prefilter::PrefilterI,
    search::{MatchKind, Span},
};

#[derive(Clone, Debug)]
pub(crate) struct AhoCorasick {
    #[cfg(not(feature = "perf-literal-multisubstring"))]
    _unused: (),
    #[cfg(feature = "perf-literal-multisubstring")]
    ac: aho_corasick::AhoCorasick,
}

impl AhoCorasick {
    pub(crate) fn new<B: AsRef<[u8]>>(
        kind: MatchKind,
        needles: &[B],
    ) -> Option<AhoCorasick> {
        #[cfg(not(feature = "perf-literal-multisubstring"))]
        {
            None
        }
        #[cfg(feature = "perf-literal-multisubstring")]
        {
            // We used to use `aho_corasick::MatchKind::Standard` here when
            // `kind` was `MatchKind::All`, but this is not correct. The
            // "standard" Aho-Corasick match semantics are to report a match
            // immediately as soon as it is seen, but `All` isn't like that.
            // In particular, with "standard" semantics, given the needles
            // "abc" and "b" and the haystack "abc," it would report a match
            // at offset 1 before a match at offset 0. This is never what we
            // want in the context of the regex engine, regardless of whether
            // we have leftmost-first or 'all' semantics. Namely, we always
            // want the leftmost match.
            let ac_match_kind = match kind {
                MatchKind::LeftmostFirst | MatchKind::All => {
                    aho_corasick::MatchKind::LeftmostFirst
                }
            };
            // This is kind of just an arbitrary number, but basically, if we
            // have a small enough set of literals, then we try to use the VERY
            // memory hungry DFA. Otherwise, we whimp out and use an NFA. The
            // upshot is that the NFA is quite lean and decently fast. Faster
            // than a naive Aho-Corasick NFA anyway.
            let ac_kind = if needles.len() <= 500 {
                aho_corasick::AhoCorasickKind::DFA
            } else {
                aho_corasick::AhoCorasickKind::ContiguousNFA
            };
            let result = aho_corasick::AhoCorasick::builder()
                .kind(Some(ac_kind))
                .match_kind(ac_match_kind)
                .start_kind(aho_corasick::StartKind::Both)
                // We try to handle all of the prefilter cases in the super
                // module, and only use Aho-Corasick for the actual automaton.
                // The aho-corasick crate does have some extra prefilters,
                // namely, looking for rare bytes to feed to memchr{,2,3}
                // instead of just the first byte. If we end up wanting
                // those---and they are somewhat tricky to implement---then
                // we could port them to this crate.
                //
                // The main reason for doing things this way is so we have a
                // complete and easy to understand picture of which prefilters
                // are available and how they work. Otherwise it seems too
                // easy to get into a situation where we have a prefilter
                // layered on top of prefilter, and that might have unintended
                // consequences.
                .prefilter(false)
                .build(needles);
            let ac = match result {
                Ok(ac) => ac,
                Err(_err) => {
                    debug!("aho-corasick prefilter failed to build: {}", _err);
                    return None;
                }
            };
            Some(AhoCorasick { ac })
        }
    }
}

impl PrefilterI for AhoCorasick {
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(feature = "perf-literal-multisubstring"))]
        {
            unreachable!()
        }
        #[cfg(feature = "perf-literal-multisubstring")]
        {
            let input =
                aho_corasick::Input::new(haystack).span(span.start..span.end);
            self.ac
                .find(input)
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
            let input = aho_corasick::Input::new(haystack)
                .anchored(aho_corasick::Anchored::Yes)
                .span(span.start..span.end);
            self.ac
                .find(input)
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
            self.ac.memory_usage()
        }
    }

    fn is_fast(&self) -> bool {
        #[cfg(not(feature = "perf-literal-multisubstring"))]
        {
            unreachable!()
        }
        #[cfg(feature = "perf-literal-multisubstring")]
        {
            // Aho-Corasick is never considered "fast" because it's never
            // going to be even close to an order of magnitude faster than the
            // regex engine itself (assuming a DFA is used). In fact, it is
            // usually slower. The magic of Aho-Corasick is that it can search
            // a *large* number of literals with a relatively small amount of
            // memory. The regex engines are far more wasteful.
            //
            // Aho-Corasick may be "fast" when the regex engine corresponds
            // to, say, the PikeVM. That happens when the lazy DFA couldn't be
            // built or used for some reason. But in these cases, the regex
            // itself is likely quite big and we're probably hosed no matter
            // what we do. (In this case, the best bet is for the caller to
            // increase some of the memory limits on the hybrid cache capacity
            // and hope that's enough.)
            false
        }
    }
}
