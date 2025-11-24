use crate::util::{
    prefilter::PrefilterI,
    search::{MatchKind, Span},
};

#[derive(Clone, Debug)]
pub(crate) struct Memchr(u8);

impl Memchr {
    pub(crate) fn new<B: AsRef<[u8]>>(
        _kind: MatchKind,
        needles: &[B],
    ) -> Option<Memchr> {
        #[cfg(not(feature = "perf-literal-substring"))]
        {
            None
        }
        #[cfg(feature = "perf-literal-substring")]
        {
            if needles.len() != 1 {
                return None;
            }
            if needles[0].as_ref().len() != 1 {
                return None;
            }
            Some(Memchr(needles[0].as_ref()[0]))
        }
    }
}

impl PrefilterI for Memchr {
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(feature = "perf-literal-substring"))]
        {
            unreachable!()
        }
        #[cfg(feature = "perf-literal-substring")]
        {
            memchr::memchr(self.0, &haystack[span]).map(|i| {
                let start = span.start + i;
                let end = start + 1;
                Span { start, end }
            })
        }
    }

    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        let b = *haystack.get(span.start)?;
        if self.0 == b {
            Some(Span { start: span.start, end: span.start + 1 })
        } else {
            None
        }
    }

    fn memory_usage(&self) -> usize {
        0
    }

    fn is_fast(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Memchr2(u8, u8);

impl Memchr2 {
    pub(crate) fn new<B: AsRef<[u8]>>(
        _kind: MatchKind,
        needles: &[B],
    ) -> Option<Memchr2> {
        #[cfg(not(feature = "perf-literal-substring"))]
        {
            None
        }
        #[cfg(feature = "perf-literal-substring")]
        {
            if needles.len() != 2 {
                return None;
            }
            if !needles.iter().all(|n| n.as_ref().len() == 1) {
                return None;
            }
            let b1 = needles[0].as_ref()[0];
            let b2 = needles[1].as_ref()[0];
            Some(Memchr2(b1, b2))
        }
    }
}

impl PrefilterI for Memchr2 {
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(feature = "perf-literal-substring"))]
        {
            unreachable!()
        }
        #[cfg(feature = "perf-literal-substring")]
        {
            memchr::memchr2(self.0, self.1, &haystack[span]).map(|i| {
                let start = span.start + i;
                let end = start + 1;
                Span { start, end }
            })
        }
    }

    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        let b = *haystack.get(span.start)?;
        if self.0 == b || self.1 == b {
            Some(Span { start: span.start, end: span.start + 1 })
        } else {
            None
        }
    }

    fn memory_usage(&self) -> usize {
        0
    }

    fn is_fast(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Memchr3(u8, u8, u8);

impl Memchr3 {
    pub(crate) fn new<B: AsRef<[u8]>>(
        _kind: MatchKind,
        needles: &[B],
    ) -> Option<Memchr3> {
        #[cfg(not(feature = "perf-literal-substring"))]
        {
            None
        }
        #[cfg(feature = "perf-literal-substring")]
        {
            if needles.len() != 3 {
                return None;
            }
            if !needles.iter().all(|n| n.as_ref().len() == 1) {
                return None;
            }
            let b1 = needles[0].as_ref()[0];
            let b2 = needles[1].as_ref()[0];
            let b3 = needles[2].as_ref()[0];
            Some(Memchr3(b1, b2, b3))
        }
    }
}

impl PrefilterI for Memchr3 {
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(feature = "perf-literal-substring"))]
        {
            unreachable!()
        }
        #[cfg(feature = "perf-literal-substring")]
        {
            memchr::memchr3(self.0, self.1, self.2, &haystack[span]).map(|i| {
                let start = span.start + i;
                let end = start + 1;
                Span { start, end }
            })
        }
    }

    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        let b = *haystack.get(span.start)?;
        if self.0 == b || self.1 == b || self.2 == b {
            Some(Span { start: span.start, end: span.start + 1 })
        } else {
            None
        }
    }

    fn memory_usage(&self) -> usize {
        0
    }

    fn is_fast(&self) -> bool {
        true
    }
}
