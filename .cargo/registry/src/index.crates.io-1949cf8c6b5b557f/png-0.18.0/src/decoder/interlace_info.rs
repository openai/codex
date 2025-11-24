use std::ops::Range;

use crate::adam7::{Adam7Info, Adam7Iterator};

/// Describes which interlacing algorithm applies to a decoded row.
///
/// PNG (2003) specifies two interlace modes, but reserves future extensions.
///
/// See also [Reader.next_interlaced_row](crate::Reader::next_interlaced_row).
#[derive(Clone, Copy, Debug)]
pub enum InterlaceInfo {
    /// The `null` method means no interlacing.
    Null(NullInfo),
    /// [The `Adam7` algorithm](https://en.wikipedia.org/wiki/Adam7_algorithm) derives its name
    /// from doing 7 passes over the image, only decoding a subset of all pixels in each pass.
    /// The following table shows pictorially what parts of each 8x8 area of the image is found in
    /// each pass:
    ///
    /// ```txt
    /// 1 6 4 6 2 6 4 6
    /// 7 7 7 7 7 7 7 7
    /// 5 6 5 6 5 6 5 6
    /// 7 7 7 7 7 7 7 7
    /// 3 6 4 6 3 6 4 6
    /// 7 7 7 7 7 7 7 7
    /// 5 6 5 6 5 6 5 6
    /// 7 7 7 7 7 7 7 7
    /// ```
    Adam7(Adam7Info),
}

#[derive(Clone, Copy, Debug)]
pub struct NullInfo {
    line: u32,
}

impl InterlaceInfo {
    pub(crate) fn line_number(&self) -> u32 {
        match self {
            InterlaceInfo::Null(NullInfo { line }) => *line,
            InterlaceInfo::Adam7(Adam7Info { line, .. }) => *line,
        }
    }

    pub(crate) fn get_adam7_info(&self) -> Option<&Adam7Info> {
        match self {
            InterlaceInfo::Null(_) => None,
            InterlaceInfo::Adam7(adam7info) => Some(adam7info),
        }
    }
}

pub(crate) struct InterlaceInfoIter(IterImpl);

impl InterlaceInfoIter {
    pub fn empty() -> Self {
        Self(IterImpl::None(0..0))
    }

    pub fn new(width: u32, height: u32, interlaced: bool) -> Self {
        if interlaced {
            Self(IterImpl::Adam7(Adam7Iterator::new(width, height)))
        } else {
            Self(IterImpl::None(0..height))
        }
    }
}

impl Iterator for InterlaceInfoIter {
    type Item = InterlaceInfo;

    fn next(&mut self) -> Option<InterlaceInfo> {
        match self.0 {
            IterImpl::Adam7(ref mut adam7) => Some(InterlaceInfo::Adam7(adam7.next()?)),
            IterImpl::None(ref mut height) => Some(InterlaceInfo::Null(NullInfo {
                line: height.next()?,
            })),
        }
    }
}

enum IterImpl {
    None(Range<u32>),
    Adam7(Adam7Iterator),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn null() {
        assert_eq!(
            InterlaceInfoIter::new(8, 8, false)
                .map(|info| info.line_number())
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5, 6, 7],
        );
    }

    #[test]
    fn adam7() {
        assert_eq!(
            InterlaceInfoIter::new(8, 8, true)
                .map(|info| info.line_number())
                .collect::<Vec<_>>(),
            vec![
                0, // pass 1
                0, // pass 2
                0, // pass 3
                0, 1, // pass 4
                0, 1, // pass 5
                0, 1, 2, 3, // pass 6
                0, 1, 2, 3, // pass 7
            ],
        );
    }

    #[test]
    fn empty() {
        assert_eq!(
            InterlaceInfoIter::empty()
                .map(|info| info.line_number())
                .collect::<Vec<_>>(),
            vec![],
        );
    }
}
