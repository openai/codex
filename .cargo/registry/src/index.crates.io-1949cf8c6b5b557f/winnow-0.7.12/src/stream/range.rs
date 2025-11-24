/// A range bounded inclusively for counting parses performed
///
/// This is flexible in what can be converted to a [Range]:
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::prelude::*;
/// # use winnow::token::any;
/// # use winnow::combinator::repeat;
/// # fn inner(input: &mut &str) -> ModalResult<char> {
/// #     any.parse_next(input)
/// # }
/// # let mut input = "0123456789012345678901234567890123456789";
/// # let input = &mut input;
/// let parser: Vec<_> = repeat(5, inner).parse_next(input).unwrap();
/// # let mut input = "0123456789012345678901234567890123456789";
/// # let input = &mut input;
/// let parser: Vec<_> = repeat(.., inner).parse_next(input).unwrap();
/// # let mut input = "0123456789012345678901234567890123456789";
/// # let input = &mut input;
/// let parser: Vec<_> = repeat(1.., inner).parse_next(input).unwrap();
/// # let mut input = "0123456789012345678901234567890123456789";
/// # let input = &mut input;
/// let parser: Vec<_> = repeat(5..8, inner).parse_next(input).unwrap();
/// # let mut input = "0123456789012345678901234567890123456789";
/// # let input = &mut input;
/// let parser: Vec<_> = repeat(5..=8, inner).parse_next(input).unwrap();
/// # }
/// ```
#[derive(PartialEq, Eq, Copy, Clone)]
pub struct Range {
    pub(crate) start_inclusive: usize,
    pub(crate) end_inclusive: Option<usize>,
}

impl Range {
    #[inline(always)]
    fn raw(start_inclusive: usize, end_inclusive: Option<usize>) -> Self {
        Self {
            start_inclusive,
            end_inclusive,
        }
    }
}

impl crate::lib::std::ops::RangeBounds<usize> for Range {
    #[inline(always)]
    fn start_bound(&self) -> crate::lib::std::ops::Bound<&usize> {
        crate::lib::std::ops::Bound::Included(&self.start_inclusive)
    }

    #[inline(always)]
    fn end_bound(&self) -> crate::lib::std::ops::Bound<&usize> {
        if let Some(end_inclusive) = &self.end_inclusive {
            crate::lib::std::ops::Bound::Included(end_inclusive)
        } else {
            crate::lib::std::ops::Bound::Unbounded
        }
    }
}

impl From<usize> for Range {
    #[inline(always)]
    fn from(fixed: usize) -> Self {
        (fixed..=fixed).into()
    }
}

impl From<crate::lib::std::ops::Range<usize>> for Range {
    #[inline(always)]
    fn from(range: crate::lib::std::ops::Range<usize>) -> Self {
        let start_inclusive = range.start;
        let end_inclusive = Some(range.end.saturating_sub(1));
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl From<crate::lib::std::ops::RangeFull> for Range {
    #[inline(always)]
    fn from(_: crate::lib::std::ops::RangeFull) -> Self {
        let start_inclusive = 0;
        let end_inclusive = None;
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl From<crate::lib::std::ops::RangeFrom<usize>> for Range {
    #[inline(always)]
    fn from(range: crate::lib::std::ops::RangeFrom<usize>) -> Self {
        let start_inclusive = range.start;
        let end_inclusive = None;
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl From<crate::lib::std::ops::RangeTo<usize>> for Range {
    #[inline(always)]
    fn from(range: crate::lib::std::ops::RangeTo<usize>) -> Self {
        let start_inclusive = 0;
        let end_inclusive = Some(range.end.saturating_sub(1));
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl From<crate::lib::std::ops::RangeInclusive<usize>> for Range {
    #[inline(always)]
    fn from(range: crate::lib::std::ops::RangeInclusive<usize>) -> Self {
        let start_inclusive = *range.start();
        let end_inclusive = Some(*range.end());
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl From<crate::lib::std::ops::RangeToInclusive<usize>> for Range {
    #[inline(always)]
    fn from(range: crate::lib::std::ops::RangeToInclusive<usize>) -> Self {
        let start_inclusive = 0;
        let end_inclusive = Some(range.end);
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl crate::lib::std::fmt::Display for Range {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        self.start_inclusive.fmt(f)?;
        match self.end_inclusive {
            Some(e) if e == self.start_inclusive => {}
            Some(e) => {
                "..=".fmt(f)?;
                e.fmt(f)?;
            }
            None => {
                "..".fmt(f)?;
            }
        }
        Ok(())
    }
}

impl crate::lib::std::fmt::Debug for Range {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        write!(f, "{self}")
    }
}
