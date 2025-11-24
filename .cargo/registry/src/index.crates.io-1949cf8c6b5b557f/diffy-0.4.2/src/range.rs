use std::{cmp, fmt::Debug, ops};

// Range type inspired by the Range type used in [dissimilar](https://docs.rs/dissimilar)
#[derive(Debug)]
pub struct Range<'a, T: ?Sized> {
    inner: &'a T,
    offset: usize,
    len: usize,
}

impl<T: ?Sized> Copy for Range<'_, T> {}

impl<T: ?Sized> Clone for Range<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T: ?Sized> Range<'a, T> {
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn inner(&self) -> &'a T {
        self.inner
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    #[allow(dead_code)]
    pub fn range(&self) -> ops::Range<usize> {
        self.offset..self.offset + self.len
    }

    pub fn grow_up(&mut self, adjust: usize) {
        self.offset -= adjust;
        self.len += adjust;
    }

    pub fn grow_down(&mut self, adjust: usize) {
        self.len += adjust;
    }

    pub fn shrink_front(&mut self, adjust: usize) {
        self.offset += adjust;
        self.len -= adjust;
    }

    pub fn shrink_back(&mut self, adjust: usize) {
        self.len -= adjust;
    }

    pub fn shift_up(&mut self, adjust: usize) {
        self.offset -= adjust
    }

    pub fn shift_down(&mut self, adjust: usize) {
        self.offset += adjust;
    }

    pub fn slice(&self, bounds: impl RangeBounds) -> Self {
        let (offset, len) = bounds.index(self.len);
        Range {
            inner: self.inner,
            offset: self.offset + offset,
            len,
        }
    }

    pub fn get(&self, bounds: impl RangeBounds) -> Option<Self> {
        let (offset, len) = bounds.try_index(self.len)?;
        Some(Range {
            inner: self.inner,
            offset: self.offset + offset,
            len,
        })
    }

    pub fn split_at(&self, mid: usize) -> (Self, Self) {
        (self.slice(..mid), self.slice(mid..))
    }
}

impl<'a, T> Range<'a, T>
where
    T: ?Sized + SliceLike,
{
    pub fn new(inner: &'a T, bounds: impl RangeBounds) -> Self {
        let (offset, len) = bounds.index(inner.len());
        Range { inner, offset, len }
    }

    #[allow(dead_code)]
    pub fn empty() -> Range<'a, T> {
        Range {
            inner: T::empty(),
            offset: 0,
            len: 0,
        }
    }

    pub fn as_slice(&self) -> &'a T {
        self.inner.as_slice(self.offset..self.offset + self.len)
    }

    pub fn common_prefix_len(&self, other: Range<'_, T>) -> usize {
        self.as_slice().common_prefix_len(other.as_slice())
    }

    pub fn common_suffix_len(&self, other: Range<'_, T>) -> usize {
        self.as_slice().common_suffix_len(other.as_slice())
    }

    #[allow(dead_code)]
    pub fn common_overlap_len(&self, other: Range<'_, T>) -> usize {
        self.as_slice().common_overlap_len(other.as_slice())
    }

    #[allow(dead_code)]
    pub fn starts_with(&self, prefix: Range<'_, T>) -> bool {
        self.as_slice().starts_with(prefix.as_slice())
    }

    #[allow(dead_code)]
    pub fn ends_with(&self, suffix: Range<'_, T>) -> bool {
        self.as_slice().ends_with(suffix.as_slice())
    }
}

pub trait RangeBounds: Sized + Clone + Debug {
    // Returns (offset, len).
    fn try_index(self, len: usize) -> Option<(usize, usize)>;

    fn index(self, len: usize) -> (usize, usize) {
        match self.clone().try_index(len) {
            Some(range) => range,
            None => panic!("index out of range, index={:?}, len={}", self, len),
        }
    }
}

impl RangeBounds for ops::Range<usize> {
    fn try_index(self, len: usize) -> Option<(usize, usize)> {
        if self.start <= self.end && self.end <= len {
            Some((self.start, self.end - self.start))
        } else {
            None
        }
    }
}

impl RangeBounds for ops::RangeFrom<usize> {
    fn try_index(self, len: usize) -> Option<(usize, usize)> {
        if self.start <= len {
            Some((self.start, len - self.start))
        } else {
            None
        }
    }
}

impl RangeBounds for ops::RangeTo<usize> {
    fn try_index(self, len: usize) -> Option<(usize, usize)> {
        if self.end <= len {
            Some((0, self.end))
        } else {
            None
        }
    }
}

impl RangeBounds for ops::RangeFull {
    fn try_index(self, len: usize) -> Option<(usize, usize)> {
        Some((0, len))
    }
}

pub trait SliceLike: ops::Index<ops::Range<usize>> {
    fn len(&self) -> usize;
    fn empty<'a>() -> &'a Self;
    fn as_slice(&self, range: ops::Range<usize>) -> &Self;
    fn common_prefix_len(&self, other: &Self) -> usize;
    fn common_suffix_len(&self, other: &Self) -> usize;
    fn common_overlap_len(&self, other: &Self) -> usize;
    fn starts_with(&self, prefix: &Self) -> bool;
    fn ends_with(&self, suffix: &Self) -> bool;
}

impl SliceLike for str {
    fn len(&self) -> usize {
        self.len()
    }

    fn empty<'a>() -> &'a str {
        ""
    }

    fn as_slice(&self, range: ops::Range<usize>) -> &str {
        &self[range]
    }

    fn common_prefix_len(&self, other: &str) -> usize {
        for ((i, ch1), ch2) in self.char_indices().zip(other.chars()) {
            if ch1 != ch2 {
                return i;
            }
        }
        cmp::min(self.len(), other.len())
    }

    fn common_suffix_len(&self, other: &str) -> usize {
        for ((i, ch1), ch2) in self.char_indices().rev().zip(other.chars().rev()) {
            if ch1 != ch2 {
                return self.len() - i - ch1.len_utf8();
            }
        }
        cmp::min(self.len(), other.len())
    }

    // returns length of overlap of prefix of `self` with suffic of `other`
    fn common_overlap_len(&self, mut other: &str) -> usize {
        let mut this = self;
        // Eliminate the null case
        if this.is_empty() || other.is_empty() {
            return 0;
        }

        match this.len().cmp(&other.len()) {
            cmp::Ordering::Greater => {
                let mut end = other.len();
                while !this.is_char_boundary(end) {
                    end -= 1;
                }

                this = &this[..end];
            }
            cmp::Ordering::Less => {
                let mut start = other.len() - this.len();
                while !other.is_char_boundary(start) {
                    start += 1;
                }

                other = &other[start..]
            }
            cmp::Ordering::Equal => {}
        }

        // Quick check for the worst case.
        if this == other {
            return this.len();
        }

        // Start by looking for a single character match
        // and increase length until no match is found.
        // Performance analysis: https://neil.fraser.name/news/2010/11/04/
        let mut best = 0;
        let mut length = 0;
        for (i, c) in other.char_indices().rev() {
            let pattern = &other[i..];
            let found = match this.find(pattern) {
                Some(found) => found,
                None => return best,
            };

            length += c.len_utf8();
            if found == 0 {
                best = length;
            }
        }

        best
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.starts_with(prefix)
    }

    fn ends_with(&self, suffix: &str) -> bool {
        self.ends_with(suffix)
    }
}

impl<T> SliceLike for [T]
where
    T: PartialEq,
{
    fn len(&self) -> usize {
        self.len()
    }

    fn empty<'a>() -> &'a [T] {
        &[]
    }

    fn as_slice(&self, range: ops::Range<usize>) -> &[T] {
        &self[range]
    }

    fn common_prefix_len(&self, other: &[T]) -> usize {
        for (i, (item1, item2)) in self.iter().zip(other.iter()).enumerate() {
            if item1 != item2 {
                return i;
            }
        }
        cmp::min(self.len(), other.len())
    }

    fn common_suffix_len(&self, other: &[T]) -> usize {
        for (i, (item1, item2)) in self.iter().rev().zip(other.iter().rev()).enumerate() {
            if item1 != item2 {
                return i;
            }
        }
        cmp::min(self.len(), other.len())
    }

    // returns length of overlap of prefix of `self` with suffic of `other`
    //TODO make a more efficient solution
    fn common_overlap_len(&self, other: &[T]) -> usize {
        let mut len = cmp::min(self.len(), other.len());

        while len > 0 {
            if self[..len] == other[other.len() - len..] {
                break;
            }
            len -= 1;
        }

        len
    }

    fn starts_with(&self, prefix: &Self) -> bool {
        self.starts_with(prefix)
    }

    fn ends_with(&self, suffix: &Self) -> bool {
        self.ends_with(suffix)
    }
}

#[derive(Debug)]
pub enum DiffRange<'a, 'b, T: ?Sized> {
    Equal(Range<'a, T>, Range<'b, T>),
    Delete(Range<'a, T>),
    Insert(Range<'b, T>),
}

impl<T: ?Sized> Copy for DiffRange<'_, '_, T> {}

impl<T: ?Sized> Clone for DiffRange<'_, '_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'tmp, 'a: 'tmp, 'b: 'tmp, T> DiffRange<'a, 'b, T>
where
    T: ?Sized + SliceLike,
{
    pub fn inner(&self) -> Range<'tmp, T> {
        match *self {
            DiffRange::Equal(range, _) | DiffRange::Delete(range) | DiffRange::Insert(range) => {
                range
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner().is_empty()
    }

    pub fn len(&self) -> usize {
        self.inner().len()
    }

    pub fn grow_up(&mut self, adjust: usize) {
        self.for_each(|range| range.grow_up(adjust));
    }

    pub fn grow_down(&mut self, adjust: usize) {
        self.for_each(|range| range.grow_down(adjust));
    }

    pub fn shrink_front(&mut self, adjust: usize) {
        self.for_each(|range| range.shrink_front(adjust));
    }

    pub fn shrink_back(&mut self, adjust: usize) {
        self.for_each(|range| range.shrink_back(adjust));
    }

    pub fn shift_up(&mut self, adjust: usize) {
        self.for_each(|range| range.shift_up(adjust));
    }

    pub fn shift_down(&mut self, adjust: usize) {
        self.for_each(|range| range.shift_down(adjust));
    }

    fn for_each(&mut self, f: impl Fn(&mut Range<'_, T>)) {
        match self {
            DiffRange::Equal(range1, range2) => {
                f(range1);
                f(range2);
            }
            DiffRange::Delete(range) => f(range),
            DiffRange::Insert(range) => f(range),
        }
    }
}

impl<'a, 'b> DiffRange<'a, 'b, [u8]> {
    pub fn to_str(self, text1: &'a str, text2: &'b str) -> DiffRange<'a, 'b, str> {
        fn boundary_down(text: &str, pos: usize) -> usize {
            let mut adjust = 0;
            while !text.is_char_boundary(pos - adjust) {
                adjust += 1;
            }
            adjust
        }

        fn boundary_up(text: &str, pos: usize) -> usize {
            let mut adjust = 0;
            while !text.is_char_boundary(pos + adjust) {
                adjust += 1;
            }
            adjust
        }

        match self {
            DiffRange::Equal(range1, range2) => {
                debug_assert_eq!(range1.inner().as_ptr(), text1.as_ptr());
                debug_assert_eq!(range2.inner().as_ptr(), text2.as_ptr());
                let mut offset1 = range1.offset();
                let mut len1 = range1.len();
                let mut offset2 = range2.offset();
                let mut len2 = range2.len();

                let adjust = boundary_up(text1, offset1);
                offset1 += adjust;
                len1 -= adjust;
                offset2 += adjust;
                len2 -= adjust;
                let adjust = boundary_down(text1, offset1 + len1);
                len1 -= adjust;
                len2 -= adjust;

                DiffRange::Equal(
                    Range::new(text1, offset1..offset1 + len1),
                    Range::new(text2, offset2..offset2 + len2),
                )
            }
            DiffRange::Delete(range) => {
                debug_assert_eq!(range.inner().as_ptr(), text1.as_ptr());
                let mut offset = range.offset();
                let mut len = range.len();
                let adjust = boundary_down(text1, offset);
                offset -= adjust;
                len += adjust;
                let adjust = boundary_up(text1, offset + len);
                len += adjust;
                DiffRange::Delete(Range::new(text1, offset..offset + len))
            }
            DiffRange::Insert(range) => {
                debug_assert_eq!(range.inner().as_ptr(), text2.as_ptr());
                let mut offset = range.offset();
                let mut len = range.len();
                let adjust = boundary_down(text2, offset);
                offset -= adjust;
                len += adjust;
                let adjust = boundary_up(text2, offset + len);
                len += adjust;
                DiffRange::Insert(Range::new(text2, offset..offset + len))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_prefix() {
        let text1 = Range::new("abc", ..);
        let text2 = Range::new("xyz", ..);
        assert_eq!(0, text1.common_prefix_len(text2), "Null case");
        let text1 = Range::new(b"abc".as_ref(), ..);
        let text2 = Range::new(b"xyz".as_ref(), ..);
        assert_eq!(0, text1.common_prefix_len(text2), "Null case");

        let text1 = Range::new("1234abcdef", ..);
        let text2 = Range::new("1234xyz", ..);
        assert_eq!(4, text1.common_prefix_len(text2), "Non-null case");
        let text1 = Range::new(b"1234abcdef".as_ref(), ..);
        let text2 = Range::new(b"1234xyz".as_ref(), ..);
        assert_eq!(4, text1.common_prefix_len(text2), "Non-null case");

        let text1 = Range::new("1234", ..);
        let text2 = Range::new("1234xyz", ..);
        assert_eq!(4, text1.common_prefix_len(text2), "Whole case");

        let text1 = Range::new(b"1234".as_ref(), ..);
        let text2 = Range::new(b"1234xyz".as_ref(), ..);
        assert_eq!(4, text1.common_prefix_len(text2), "Whole case");

        let snowman = "\u{2603}";
        let comet = "\u{2604}";
        let text1 = Range::new(snowman, ..);
        let text2 = Range::new(comet, ..);
        assert_eq!(0, text1.common_prefix_len(text2), "Unicode case");
        let text1 = Range::new(snowman.as_bytes(), ..);
        let text2 = Range::new(comet.as_bytes(), ..);
        assert_eq!(2, text1.common_prefix_len(text2), "Unicode case");
    }

    #[test]
    fn test_common_suffix() {
        let text1 = Range::new("abc", ..);
        let text2 = Range::new("xyz", ..);
        assert_eq!(0, text1.common_suffix_len(text2), "Null case");
        let text1 = Range::new(b"abc".as_ref(), ..);
        let text2 = Range::new(b"xyz".as_ref(), ..);
        assert_eq!(0, text1.common_suffix_len(text2), "Null case");

        let text1 = Range::new("abcdef1234", ..);
        let text2 = Range::new("xyz1234", ..);
        assert_eq!(4, text1.common_suffix_len(text2), "Non-null case");
        let text1 = Range::new(b"abcdef1234".as_ref(), ..);
        let text2 = Range::new(b"xyz1234".as_ref(), ..);
        assert_eq!(4, text1.common_suffix_len(text2), "Non-null case");

        let text1 = Range::new("1234", ..);
        let text2 = Range::new("xyz1234", ..);
        assert_eq!(4, text1.common_suffix_len(text2), "Whole case");
        let text1 = Range::new(b"1234".as_ref(), ..);
        let text2 = Range::new(b"xyz1234".as_ref(), ..);
        assert_eq!(4, text1.common_suffix_len(text2), "Whole case");
    }

    #[test]
    fn test_common_overlap() {
        let text1 = Range::empty();
        let text2 = Range::new("abcd", ..);
        assert_eq!(0, text1.common_overlap_len(text2), "Null case");
        let text1 = Range::empty();
        let text2 = Range::new(b"abcd".as_ref(), ..);
        assert_eq!(0, text1.common_overlap_len(text2), "Null case");

        let text1 = Range::new("abcd", ..);
        let text2 = Range::new("abc", ..);
        assert_eq!(3, text1.common_overlap_len(text2), "Whole case");
        let text1 = Range::new(b"abcd".as_ref(), ..);
        let text2 = Range::new(b"abc".as_ref(), ..);
        assert_eq!(3, text1.common_overlap_len(text2), "Whole case");

        let text1 = Range::new("123456", ..);
        let text2 = Range::new("abcd", ..);
        assert_eq!(0, text1.common_overlap_len(text2), "No overlap");
        let text1 = Range::new(b"123456".as_ref(), ..);
        let text2 = Range::new(b"abcd".as_ref(), ..);
        assert_eq!(0, text1.common_overlap_len(text2), "No overlap");

        let text1 = Range::new("xxxabcd", ..);
        let text2 = Range::new("123456xxx", ..);
        assert_eq!(3, text1.common_overlap_len(text2), "Overlap");
        let text1 = Range::new(b"xxxabcd".as_ref(), ..);
        let text2 = Range::new(b"123456xxx".as_ref(), ..);
        assert_eq!(3, text1.common_overlap_len(text2), "Overlap");

        // Some overly clever languages (C#) may treat ligatures as equal to their
        // component letters. E.g. U+FB01 == 'fi'
        let text1 = Range::new("fi", ..);
        let text2 = Range::new("\u{fb01}i", ..);
        assert_eq!(0, text1.common_overlap_len(text2), "Unicode");
    }
}
