use crate::util::{
    prefilter::PrefilterI,
    search::{MatchKind, Span},
};

#[derive(Clone, Debug)]
pub(crate) struct Memmem {
    #[cfg(not(all(feature = "std", feature = "perf-literal-substring")))]
    _unused: (),
    #[cfg(all(feature = "std", feature = "perf-literal-substring"))]
    finder: memchr::memmem::Finder<'static>,
}

impl Memmem {
    pub(crate) fn new<B: AsRef<[u8]>>(
        _kind: MatchKind,
        needles: &[B],
    ) -> Option<Memmem> {
        #[cfg(not(all(feature = "std", feature = "perf-literal-substring")))]
        {
            None
        }
        #[cfg(all(feature = "std", feature = "perf-literal-substring"))]
        {
            if needles.len() != 1 {
                return None;
            }
            let needle = needles[0].as_ref();
            let finder = memchr::memmem::Finder::new(needle).into_owned();
            Some(Memmem { finder })
        }
    }
}

impl PrefilterI for Memmem {
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(all(feature = "std", feature = "perf-literal-substring")))]
        {
            unreachable!()
        }
        #[cfg(all(feature = "std", feature = "perf-literal-substring"))]
        {
            self.finder.find(&haystack[span]).map(|i| {
                let start = span.start + i;
                let end = start + self.finder.needle().len();
                Span { start, end }
            })
        }
    }

    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(all(feature = "std", feature = "perf-literal-substring")))]
        {
            unreachable!()
        }
        #[cfg(all(feature = "std", feature = "perf-literal-substring"))]
        {
            let needle = self.finder.needle();
            if haystack[span].starts_with(needle) {
                Some(Span { end: span.start + needle.len(), ..span })
            } else {
                None
            }
        }
    }

    fn memory_usage(&self) -> usize {
        #[cfg(not(all(feature = "std", feature = "perf-literal-substring")))]
        {
            unreachable!()
        }
        #[cfg(all(feature = "std", feature = "perf-literal-substring"))]
        {
            self.finder.needle().len()
        }
    }

    fn is_fast(&self) -> bool {
        #[cfg(not(all(feature = "std", feature = "perf-literal-substring")))]
        {
            unreachable!()
        }
        #[cfg(all(feature = "std", feature = "perf-literal-substring"))]
        {
            true
        }
    }
}
