use crate::util::{
    prefilter::PrefilterI,
    search::{MatchKind, Span},
};

#[derive(Clone, Debug)]
pub(crate) struct ByteSet([bool; 256]);

impl ByteSet {
    pub(crate) fn new<B: AsRef<[u8]>>(
        _kind: MatchKind,
        needles: &[B],
    ) -> Option<ByteSet> {
        #[cfg(not(feature = "perf-literal-multisubstring"))]
        {
            None
        }
        #[cfg(feature = "perf-literal-multisubstring")]
        {
            let mut set = [false; 256];
            for needle in needles.iter() {
                let needle = needle.as_ref();
                if needle.len() != 1 {
                    return None;
                }
                set[usize::from(needle[0])] = true;
            }
            Some(ByteSet(set))
        }
    }
}

impl PrefilterI for ByteSet {
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        haystack[span].iter().position(|&b| self.0[usize::from(b)]).map(|i| {
            let start = span.start + i;
            let end = start + 1;
            Span { start, end }
        })
    }

    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        let b = *haystack.get(span.start)?;
        if self.0[usize::from(b)] {
            Some(Span { start: span.start, end: span.start + 1 })
        } else {
            None
        }
    }

    fn memory_usage(&self) -> usize {
        0
    }

    fn is_fast(&self) -> bool {
        false
    }
}
