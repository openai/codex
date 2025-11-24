//! Path normalization.

use core::fmt;
use core::ops::Range;

use crate::parser::str::{find_split_hole, rfind};
use crate::spec::{Spec, UriSpec};

use super::pct_case::PctCaseNormalized;
use super::{Error, NormalizationMode, NormalizationOp};

/// Path that is (possibly) not yet processed or being processed.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Path<'a> {
    /// The result. No more processing is needed.
    Done(&'a str),
    /// Not yet completely processed path.
    NeedsProcessing(PathToNormalize<'a>),
}

/// Path that needs merge and/or dot segment removal.
///
/// # Invariants
///
/// If the first field (prefix field) is not `None`, it must end with a slash.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PathToNormalize<'a>(Option<&'a str>, &'a str);

impl<'a> PathToNormalize<'a> {
    /// Creates a `PathToNormalize` from the given single path.
    #[inline]
    #[must_use]
    pub(crate) fn from_single_path(path: &'a str) -> Self {
        Self(None, path)
    }

    /// Creates a `PathToNormalize` from the given base and reference paths to be resolved.
    #[must_use]
    pub(crate) fn from_paths_to_be_resolved(base: &'a str, reference: &'a str) -> Self {
        if reference.starts_with('/') {
            return Self(None, reference);
        }

        match rfind(base.as_bytes(), b'/') {
            Some(last_slash_pos) => Self(Some(&base[..=last_slash_pos]), reference),
            None => Self(None, reference),
        }
    }

    /// Returns true if the path is empty string.
    #[inline]
    #[must_use]
    fn is_empty(&self) -> bool {
        // If `self.0` is `Some(_)`, it ends with a slash, i.e. it is not empty.
        self.0.is_none() && self.1.is_empty()
    }

    /// Returns the length of the not yet normalized path.
    #[inline]
    #[must_use]
    pub(super) fn len(&self) -> usize {
        self.len_prefix() + self.1.len()
    }

    /// Returns the length of the prefix part.
    ///
    /// Returns 0 if the prefix part is empty.
    #[inline]
    #[must_use]
    fn len_prefix(&self) -> usize {
        self.0.map_or(0, |s| s.len())
    }

    /// Returns a byte at the given position.
    #[must_use]
    fn byte_at(&self, mut i: usize) -> Option<u8> {
        if let Some(prefix) = self.0 {
            if i < prefix.len() {
                return Some(prefix.as_bytes()[i]);
            }
            i -= prefix.len();
        }
        self.1.as_bytes().get(i).copied()
    }

    /// Returns the position of the next slash of the byte at the given position.
    #[must_use]
    fn find_next_slash(&self, scan_start: usize) -> Option<usize> {
        if let Some(prefix) = self.0 {
            let prefix_len = prefix.len();
            if scan_start < prefix_len {
                prefix[scan_start..].find('/').map(|rel| rel + scan_start)
            } else {
                let local_i = scan_start - prefix_len;
                self.1[local_i..].find('/').map(|rel| rel + scan_start)
            }
        } else {
            self.1[scan_start..].find('/').map(|rel| rel + scan_start)
        }
    }

    /// Removes the `len` characters from the beginning of `self`.
    fn remove_start(&mut self, len: usize) {
        if let Some(prefix) = self.0 {
            if let Some(suffix_trim_len) = len.checked_sub(prefix.len()) {
                self.0 = None;
                self.1 = &self.1[suffix_trim_len..];
            } else {
                self.0 = Some(&prefix[len..]);
            }
        } else {
            self.1 = &self.1[len..];
        }
    }

    /// Removes the prefix that are ignorable on normalization.
    // Skips the prefix dot segments without leading slashes (such as `./`,
    // `../`, and `../.././`).
    // This is necessary because such segments should be removed with the
    // FOLLOWING slashes, not leading slashes.
    fn remove_ignorable_prefix(&mut self) {
        while let Some(seg) = PathSegmentsIter::new(self).next() {
            if seg.has_leading_slash {
                // The first segment starting with a slash is not target.
                break;
            }
            match seg.kind(self) {
                SegmentKind::Dot | SegmentKind::DotDot => {
                    // Attempt to skip the following slash by `+ 1`.
                    let skip = self.len().min(seg.range.end + 1);
                    self.remove_start(skip);
                }
                SegmentKind::Normal => break,
            }
        }
    }
}

impl PathToNormalize<'_> {
    /// Writes the normalized path.
    pub(crate) fn fmt_write_normalize<S: Spec, W: fmt::Write>(
        &self,
        f: &mut W,
        op: NormalizationOp,
        authority_is_present: bool,
    ) -> fmt::Result {
        debug_assert!(
            self.0.map_or(true, |s| s.ends_with('/')),
            "[validity] the prefix field of `PathToNormalize` should end with a slash"
        );

        if self.is_empty() {
            return Ok(());
        }

        if (op.mode == NormalizationMode::PreserveAuthoritylessRelativePath)
            && !authority_is_present
            && self.byte_at(0) != Some(b'/')
        {
            // Treat the path as "opaque", i.e. do not apply dot segments removal.
            // See <https://github.com/lo48576/iri-string/issues/29>.
            debug_assert!(
                op.mode.case_pct_normalization(),
                "[consistency] case/pct normalization should still be applied"
            );
            if let Some(prefix) = self.0 {
                write!(f, "{}", PctCaseNormalized::<S>::new(prefix))?;
            }
            write!(f, "{}", PctCaseNormalized::<S>::new(self.1))?;
            return Ok(());
        }

        let mut rest = *self;

        // Skip the prefix dot segments without leading slashes (such as `./`,
        // `../`, and `../.././`).
        // This is necessary because such segments should be removed with the
        // FOLLOWING slashes, not leading slashes.
        rest.remove_ignorable_prefix();
        if rest.is_empty() {
            // Path consists of only `/.`s and `/..`s.
            // In this case, if the authority component is present, the result
            // should be `/`, not empty.
            if authority_is_present {
                f.write_char('/')?;
            }
            return Ok(());
        }

        // None: No segments are written yet.
        // Some(false): Something other than `/` is already written as the path.
        // Some(true): Only a `/` is written as the path.
        let mut only_a_slash_is_written = None;
        let mut too_deep_area_may_have_dot_segments = true;
        while !rest.is_empty() && too_deep_area_may_have_dot_segments {
            /// The size of the queue to track the path segments.
            ///
            /// This should be nonzero.
            const QUEUE_SIZE: usize = 8;

            {
                // Skip `/.` and `/..` segments at the head.
                let mut skipped_len = 0;
                for seg in PathSegmentsIter::new(&rest) {
                    match seg.kind(&rest) {
                        SegmentKind::Dot | SegmentKind::DotDot => {
                            debug_assert!(
                                seg.has_leading_slash,
                                "[consistency] `.` or `..` segments without a
                                 leading slash have already been skipped"
                            );
                            skipped_len = seg.range.end;
                        }
                        _ => break,
                    }
                }
                rest.remove_start(skipped_len);
                if rest.is_empty() {
                    // Finished with a dot segment.
                    // The last `/.` or `/..` should be replaced to `/`.
                    if !authority_is_present && (only_a_slash_is_written == Some(true)) {
                        // Insert a dot segment to break the prefix `//`.
                        // Without this, the path starts with `//` and it may
                        // be confused with the prefix of an authority.
                        f.write_str(".//")?;
                    } else {
                        f.write_char('/')?;
                    }
                    break;
                }
            }

            let mut queue: [Option<&'_ str>; QUEUE_SIZE] = Default::default();
            let mut level: usize = 0;
            let mut first_segment_has_leading_slash = false;

            // Find higher path segments.
            let mut end = 0;
            for seg in PathSegmentsIter::new(&rest) {
                let kind = seg.kind(&rest);
                match kind {
                    SegmentKind::Dot => {
                        too_deep_area_may_have_dot_segments = true;
                    }
                    SegmentKind::DotDot => {
                        level = level.saturating_sub(1);
                        too_deep_area_may_have_dot_segments = true;
                        if level < queue.len() {
                            queue[level] = None;
                        }
                    }
                    SegmentKind::Normal => {
                        if level < queue.len() {
                            queue[level] = Some(seg.segment(&rest));
                            too_deep_area_may_have_dot_segments = false;
                            end = seg.range.end;
                            if level == 0 {
                                first_segment_has_leading_slash = seg.has_leading_slash;
                            }
                        }
                        level += 1;
                    }
                }
            }

            // Write the path segments as possible, and update the internal state.
            for segname in queue.iter().flatten() {
                Self::emit_segment::<S, _>(
                    f,
                    &mut only_a_slash_is_written,
                    first_segment_has_leading_slash,
                    segname,
                    authority_is_present,
                    op,
                )?;
            }

            rest.remove_start(end);
        }

        if !rest.is_empty() {
            // No need of searching dot segments anymore.
            assert!(
                !too_deep_area_may_have_dot_segments,
                "[consistency] loop condition of the previous loop"
            );
            // Apply only normalization (if needed).
            for seg in PathSegmentsIter::new(&rest) {
                assert_eq!(
                    seg.kind(&rest),
                    SegmentKind::Normal,
                    "[consistency] already confirmed that there are no more dot segments"
                );
                let segname = seg.segment(&rest);
                Self::emit_segment::<S, _>(
                    f,
                    &mut only_a_slash_is_written,
                    seg.has_leading_slash,
                    segname,
                    authority_is_present,
                    op,
                )?;
            }
        }

        Ok(())
    }

    /// Emits a non-dot segment and update the current state.
    //
    // `first_segment_has_leading_slash` can be any value if the segment is not the first one.
    fn emit_segment<S: Spec, W: fmt::Write>(
        f: &mut W,
        only_a_slash_is_written: &mut Option<bool>,
        first_segment_has_leading_slash: bool,
        segname: &str,
        authority_is_present: bool,
        op: NormalizationOp,
    ) -> fmt::Result {
        // Omit the leading slash of the segment only if the segment is
        // the first one and marked as not having a leading slash.
        match *only_a_slash_is_written {
            None => {
                // First segment.
                // This pass can be possible if `./` is repeated `QUEUE_SIZE`
                // times at the beginning.
                if first_segment_has_leading_slash {
                    f.write_char('/')?;
                }
                *only_a_slash_is_written =
                    Some(first_segment_has_leading_slash && segname.is_empty());
            }
            Some(only_a_slash) => {
                if only_a_slash && !authority_is_present {
                    // Apply serialization like WHATWG URL Standard.
                    // This prevents `<scheme=foo>:<path=//bar>` from written as
                    // `foo://bar`, which is interpreted as
                    // `<scheme=foo>://<authority=bar>`. Prepending `./`, the
                    // serialization result would be `foo:/.//bar`, which is safe.
                    f.write_str("./")?;
                    *only_a_slash_is_written = Some(false);
                }
                f.write_char('/')?;
            }
        }

        // Write the segment name.
        if op.mode.case_pct_normalization() {
            write!(f, "{}", PctCaseNormalized::<S>::new(segname))
        } else {
            f.write_str(segname)
        }
    }

    /// Checks if the path is normalizable by RFC 3986 algorithm when the authority is absent.
    ///
    /// Returns `Ok(())` when normalizable, returns `Err(_)` if not.
    pub(crate) fn ensure_rfc3986_normalizable_with_authority_absent(&self) -> Result<(), Error> {
        /// A sink to get the prefix of the input.
        #[derive(Default)]
        struct PrefixRetriever {
            /// The buffer to remember the prefix of the input.
            buf: [u8; 3],
            /// The next write position in the buffer.
            cursor: usize,
        }
        impl PrefixRetriever {
            /// Returns the read prefix data.
            #[inline]
            #[must_use]
            fn as_bytes(&self) -> &[u8] {
                &self.buf[..self.cursor]
            }
        }
        impl fmt::Write for PrefixRetriever {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                if !s.is_empty() && (self.cursor >= self.buf.len()) {
                    // Enough bytes are read.
                    return Err(fmt::Error);
                }
                self.buf[self.cursor..]
                    .iter_mut()
                    .zip(s.bytes())
                    .for_each(|(dest, src)| *dest = src);
                self.cursor = self.cursor.saturating_add(s.len()).min(self.buf.len());
                Ok(())
            }
        }

        let mut prefix = PrefixRetriever::default();
        // The failure of this write indicates more than 3 characters are read.
        // This is safe to ignore since the check needs only 3 characters.
        let _ = self.fmt_write_normalize::<UriSpec, _>(
            &mut prefix,
            NormalizationOp {
                mode: NormalizationMode::None,
            },
            // Assume the authority is absent.
            false,
        );

        if prefix.as_bytes() == b"/./" {
            Err(Error::new())
        } else {
            Ok(())
        }
    }
}

/// Characteristic of a path.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PathCharacteristic {
    /// Absolute path, not special.
    CommonAbsolute,
    /// Absolute path, not special.
    CommonRelative,
    /// The first path segment of the relative path has one or more colon characters.
    RelativeFirstSegmentHasColon,
    /// The path starts with the double slash.
    StartsWithDoubleSlash,
}

impl PathCharacteristic {
    /// Returns true if the path is absolute.
    #[inline]
    #[must_use]
    pub(crate) fn is_absolute(self) -> bool {
        matches!(self, Self::CommonAbsolute | Self::StartsWithDoubleSlash)
    }

    /// Returns the characteristic of the path.
    pub(crate) fn from_path_to_display<S: Spec>(
        path: &PathToNormalize<'_>,
        op: NormalizationOp,
        authority_is_present: bool,
    ) -> Self {
        /// Dummy writer to get necessary values.
        #[derive(Default, Clone, Copy)]
        struct Writer {
            /// Result.
            result: Option<PathCharacteristic>,
            /// Whether the normalized path is absolute.
            is_absolute: Option<bool>,
        }
        impl fmt::Write for Writer {
            fn write_str(&mut self, mut s: &str) -> fmt::Result {
                if self.result.is_some() {
                    // Nothing more to do.
                    return Err(fmt::Error);
                }
                while !s.is_empty() {
                    if self.is_absolute.is_none() {
                        // The first input.
                        match s.strip_prefix('/') {
                            Some(rest) => {
                                self.is_absolute = Some(true);
                                s = rest;
                            }
                            None => {
                                self.is_absolute = Some(false);
                            }
                        }
                        continue;
                    }
                    if self.is_absolute == Some(true) {
                        let result = if s.starts_with('/') {
                            PathCharacteristic::StartsWithDoubleSlash
                        } else {
                            PathCharacteristic::CommonAbsolute
                        };
                        self.result = Some(result);
                        return Err(fmt::Error);
                    }
                    // Processing the first segment of the relative path.
                    match find_split_hole(s, b'/') {
                        Some((first_seg, _rest)) => {
                            let result = if first_seg.contains(':') {
                                PathCharacteristic::RelativeFirstSegmentHasColon
                            } else {
                                PathCharacteristic::CommonRelative
                            };
                            self.result = Some(result);
                            return Err(fmt::Error);
                        }
                        None => {
                            // `s` might not be the complete first segment.
                            if s.contains(':') {
                                self.result =
                                    Some(PathCharacteristic::RelativeFirstSegmentHasColon);
                                return Err(fmt::Error);
                            }
                            break;
                        }
                    }
                }
                Ok(())
            }
        }

        let mut writer = Writer::default();
        match path.fmt_write_normalize::<S, _>(&mut writer, op, authority_is_present) {
            // Empty path.
            Ok(_) => PathCharacteristic::CommonRelative,
            Err(_) => writer
                .result
                .expect("[consistency] the formatting quits early by `Err` when the check is done"),
        }
    }
}

/// Path segment kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentKind {
    /// `.` or the equivalents.
    Dot,
    /// `..` or the equivalents.
    DotDot,
    /// Other normal (not special) segments.
    Normal,
}

impl SegmentKind {
    /// Creates a new `SegmentKind` from the given segment name.
    #[must_use]
    fn from_segment(s: &str) -> Self {
        match s {
            "." | "%2E" | "%2e" => SegmentKind::Dot,
            ".." | ".%2E" | ".%2e" | "%2E." | "%2E%2E" | "%2E%2e" | "%2e." | "%2e%2E"
            | "%2e%2e" => SegmentKind::DotDot,
            _ => SegmentKind::Normal,
        }
    }
}

/// A segment with optional leading slash.
#[derive(Debug, Clone)]
struct PathSegment {
    /// Presence of a leading slash.
    has_leading_slash: bool,
    /// Range of the segment name (without any slashes).
    range: Range<usize>,
}

impl PathSegment {
    /// Returns the segment without any slashes.
    #[inline]
    #[must_use]
    fn segment<'a>(&self, path: &PathToNormalize<'a>) -> &'a str {
        if let Some(prefix) = path.0 {
            let prefix_len = prefix.len();
            if self.range.end <= prefix_len {
                &prefix[self.range.clone()]
            } else {
                let range = (self.range.start - prefix_len)..(self.range.end - prefix_len);
                &path.1[range]
            }
        } else {
            &path.1[self.range.clone()]
        }
    }

    /// Returns the segment kind.
    #[inline]
    #[must_use]
    fn kind(&self, path: &PathToNormalize<'_>) -> SegmentKind {
        SegmentKind::from_segment(self.segment(path))
    }
}

/// Iterator of path segments.
struct PathSegmentsIter<'a> {
    /// Path.
    path: &'a PathToNormalize<'a>,
    /// Current cursor position.
    cursor: usize,
}

impl<'a> PathSegmentsIter<'a> {
    /// Creates a new iterator of path segments.
    #[inline]
    #[must_use]
    fn new(path: &'a PathToNormalize<'a>) -> Self {
        Self { path, cursor: 0 }
    }
}

impl Iterator for PathSegmentsIter<'_> {
    type Item = PathSegment;

    fn next(&mut self) -> Option<Self::Item> {
        let path_len = self.path.len();
        if self.cursor >= path_len {
            return None;
        }
        let has_leading_slash = self.path.byte_at(self.cursor) == Some(b'/');

        let prefix_len = self.path.len_prefix();
        if (prefix_len != 0) && (self.cursor == prefix_len - 1) {
            debug_assert!(has_leading_slash);
            let end = self.path.1.find('/').unwrap_or(self.path.1.len()) + prefix_len;
            self.cursor = end;
            return Some(PathSegment {
                has_leading_slash,
                range: prefix_len..end,
            });
        }

        if has_leading_slash {
            // Skip the leading slash.
            self.cursor += 1;
        };
        let start = self.cursor;
        self.cursor = self.path.find_next_slash(self.cursor).unwrap_or(path_len);

        Some(PathSegment {
            has_leading_slash,
            range: start..self.cursor,
        })
    }
}
