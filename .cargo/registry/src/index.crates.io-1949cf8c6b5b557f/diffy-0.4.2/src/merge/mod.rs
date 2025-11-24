use crate::{
    diff::DiffOptions,
    range::{DiffRange, Range, SliceLike},
    utils::Classifier,
};
use std::{cmp, fmt};

#[cfg(test)]
mod tests;

const DEFAULT_CONFLICT_MARKER_LENGTH: usize = 7;

enum Diff3Range<'ancestor, 'ours, 'theirs, T: ?Sized> {
    Equal(Range<'ancestor, T>, Range<'ours, T>, Range<'theirs, T>),
    Ancestor(Range<'ancestor, T>),
    AncestorOurs(Range<'ancestor, T>, Range<'ours, T>),
    AncestorTheirs(Range<'ancestor, T>, Range<'theirs, T>),
    Ours(Range<'ours, T>),
    Theirs(Range<'theirs, T>),
}

impl<T: ?Sized + fmt::Debug + SliceLike> fmt::Debug for Diff3Range<'_, '_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Diff3Range::Equal(range, ..) => write!(f, "Equal: {:?}", range.as_slice()),
            Diff3Range::Ancestor(range) => write!(f, "Ancestor: {:?}", range.as_slice()),
            Diff3Range::AncestorOurs(range, ..) => {
                write!(f, "AncestorOurs: {:?}", range.as_slice())
            }
            Diff3Range::AncestorTheirs(range, ..) => {
                write!(f, "AncestorTheirs: {:?}", range.as_slice())
            }
            Diff3Range::Ours(range) => write!(f, "Ours: {:?}", range.as_slice()),
            Diff3Range::Theirs(range) => write!(f, "Theirs: {:?}", range.as_slice()),
        }
    }
}

impl<T: ?Sized> Copy for Diff3Range<'_, '_, '_, T> {}

impl<T: ?Sized> Clone for Diff3Range<'_, '_, '_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

enum MergeRange<'ancestor, 'ours, 'theirs, T: ?Sized> {
    Equal(Range<'ancestor, T>, Range<'ours, T>, Range<'theirs, T>),
    Conflict(Range<'ancestor, T>, Range<'ours, T>, Range<'theirs, T>),
    Ours(Range<'ours, T>),
    Theirs(Range<'theirs, T>),
    Both(Range<'ours, T>, Range<'theirs, T>),
}

impl<T: ?Sized + fmt::Debug + SliceLike> fmt::Debug for MergeRange<'_, '_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeRange::Equal(range, ..) => write!(f, "Equal: {:?}", range.as_slice()),
            MergeRange::Conflict(ancestor, ours, theirs) => write!(
                f,
                "Conflict: ancestor: {:?} ours: {:?} theirs: {:?}",
                ancestor.as_slice(),
                ours.as_slice(),
                theirs.as_slice()
            ),
            MergeRange::Ours(range) => write!(f, "Ours: {:?}", range.as_slice()),
            MergeRange::Theirs(range) => write!(f, "Theirs: {:?}", range.as_slice()),
            MergeRange::Both(ours, theirs) => write!(
                f,
                "Both: ours: {:?} theirs: {:?}",
                ours.as_slice(),
                theirs.as_slice()
            ),
        }
    }
}

impl<T: ?Sized> Copy for MergeRange<'_, '_, '_, T> {}

impl<T: ?Sized> Clone for MergeRange<'_, '_, '_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Style used when rendering a conflict
#[derive(Copy, Clone, Debug)]
pub enum ConflictStyle {
    /// Renders conflicting lines from both files, separated by conflict markers.
    ///
    /// ```console
    /// <<<<<<< A
    /// lines in file A
    /// =======
    /// lines in file B
    /// >>>>>>> B
    /// ```
    Merge,

    /// Renders conflicting lines from both files including lines from the original files,
    /// separated by conflict markers.
    ///
    /// ```console
    /// <<<<<<< A
    /// lines in file A
    /// ||||||| Original
    /// lines in Original file
    /// =======
    /// lines in file B
    /// >>>>>>> B
    /// ```
    Diff3,
}

/// A collection of options for modifying the way a merge is performed
#[derive(Debug)]
pub struct MergeOptions {
    conflict_marker_length: usize,
    style: ConflictStyle,
}

impl MergeOptions {
    /// Constructs a new `MergeOptions` with default settings
    ///
    /// ## Defaults
    /// * conflict_marker_length = 7
    /// * style = ConflictStyle::Diff3
    pub fn new() -> Self {
        Self {
            conflict_marker_length: DEFAULT_CONFLICT_MARKER_LENGTH,
            style: ConflictStyle::Diff3,
        }
    }

    /// Set the length of the conflict markers used when displaying a merge conflict
    pub fn set_conflict_marker_length(&mut self, conflict_marker_length: usize) -> &mut Self {
        self.conflict_marker_length = conflict_marker_length;
        self
    }

    /// Set the conflict style used when displaying a merge conflict
    pub fn set_conflict_style(&mut self, style: ConflictStyle) -> &mut Self {
        self.style = style;
        self
    }

    /// Merge two files, given a common ancestor, based on the configured options
    pub fn merge<'a>(
        &self,
        ancestor: &'a str,
        ours: &'a str,
        theirs: &'a str,
    ) -> Result<String, String> {
        let mut classifier = Classifier::default();
        let (ancestor_lines, ancestor_ids) = classifier.classify_lines(ancestor);
        let (our_lines, our_ids) = classifier.classify_lines(ours);
        let (their_lines, their_ids) = classifier.classify_lines(theirs);

        let opts = DiffOptions::default();
        let our_solution = opts.diff_slice(&ancestor_ids, &our_ids);
        let their_solution = opts.diff_slice(&ancestor_ids, &their_ids);

        let merged = merge_solutions(&our_solution, &their_solution);
        let mut merge = diff3_range_to_merge_range(&merged);

        cleanup_conflicts(&mut merge);

        output_result(
            &ancestor_lines,
            &our_lines,
            &their_lines,
            &merge,
            self.conflict_marker_length,
            self.style,
        )
    }

    /// Perform a 3-way merge between potentially non-utf8 texts
    pub fn merge_bytes<'a>(
        &self,
        ancestor: &'a [u8],
        ours: &'a [u8],
        theirs: &'a [u8],
    ) -> Result<Vec<u8>, Vec<u8>> {
        let mut classifier = Classifier::default();
        let (ancestor_lines, ancestor_ids) = classifier.classify_lines(ancestor);
        let (our_lines, our_ids) = classifier.classify_lines(ours);
        let (their_lines, their_ids) = classifier.classify_lines(theirs);

        let opts = DiffOptions::default();
        let our_solution = opts.diff_slice(&ancestor_ids, &our_ids);
        let their_solution = opts.diff_slice(&ancestor_ids, &their_ids);

        let merged = merge_solutions(&our_solution, &their_solution);
        let mut merge = diff3_range_to_merge_range(&merged);

        cleanup_conflicts(&mut merge);

        output_result_bytes(
            &ancestor_lines,
            &our_lines,
            &their_lines,
            &merge,
            self.conflict_marker_length,
            self.style,
        )
    }
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Merge two files given a common ancestor.
///
/// Returns `Ok(String)` upon a successful merge.
/// Returns `Err(String)` if there were conflicts, with the conflicting
/// regions marked with conflict markers.
///
/// ## Merging two files without conflicts
/// ```
/// # use diffy::merge;
/// let original = "\
/// Devotion
/// Dominion
/// Odium
/// Preservation
/// Ruin
/// Cultivation
/// Honor
/// Endowment
/// Autonomy
/// Ambition
/// ";
/// let a = "\
/// Odium
/// Preservation
/// Ruin
/// Cultivation
/// Endowment
/// Autonomy
/// ";
/// let b = "\
/// Devotion
/// Dominion
/// Odium
/// Harmony
/// Cultivation
/// Honor
/// Endowment
/// Autonomy
/// Ambition
/// ";
///
/// let expected = "\
/// Odium
/// Harmony
/// Cultivation
/// Endowment
/// Autonomy
/// ";
///
/// assert_eq!(merge(original, a, b).unwrap(), expected);
/// ```
pub fn merge<'a>(ancestor: &'a str, ours: &'a str, theirs: &'a str) -> Result<String, String> {
    MergeOptions::default().merge(ancestor, ours, theirs)
}

/// Perform a 3-way merge between potentially non-utf8 texts
pub fn merge_bytes<'a>(
    ancestor: &'a [u8],
    ours: &'a [u8],
    theirs: &'a [u8],
) -> Result<Vec<u8>, Vec<u8>> {
    MergeOptions::default().merge_bytes(ancestor, ours, theirs)
}

fn merge_solutions<'ancestor, 'ours, 'theirs, T: ?Sized + SliceLike>(
    our_solution: &[DiffRange<'ancestor, 'ours, T>],
    their_solution: &[DiffRange<'ancestor, 'theirs, T>],
) -> Vec<Diff3Range<'ancestor, 'ours, 'theirs, T>> {
    let mut our_solution = our_solution.iter().copied();
    let mut their_solution = their_solution.iter().copied();
    let mut ours = our_solution.next();
    let mut theirs = their_solution.next();

    let mut solution = Vec::new();

    while ours.is_some() || theirs.is_some() {
        let merge_range = match (ours, theirs) {
            //
            // Inserts can't easily be checked to see if they match each other
            //
            (Some(DiffRange::Insert(range)), _) => {
                ours.take();
                Diff3Range::Ours(range)
            }
            (_, Some(DiffRange::Insert(range))) => {
                theirs.take();
                Diff3Range::Theirs(range)
            }

            (
                Some(DiffRange::Equal(ancestor1, our_range)),
                Some(DiffRange::Equal(ancestor2, their_range)),
            ) => {
                assert_eq!(ancestor1.offset(), ancestor2.offset());
                let len = cmp::min(ancestor1.len(), ancestor2.len());

                shrink_front(&mut ours, len);
                shrink_front(&mut theirs, len);

                Diff3Range::Equal(
                    ancestor1.slice(..len),
                    our_range.slice(..len),
                    their_range.slice(..len),
                )
            }

            (Some(DiffRange::Equal(ancestor1, our_range)), Some(DiffRange::Delete(ancestor2))) => {
                assert_eq!(ancestor1.offset(), ancestor2.offset());
                let len = cmp::min(ancestor1.len(), ancestor2.len());

                shrink_front(&mut ours, len);
                shrink_front(&mut theirs, len);

                Diff3Range::AncestorOurs(ancestor1.slice(..len), our_range.slice(..len))
            }

            (
                Some(DiffRange::Delete(ancestor1)),
                Some(DiffRange::Equal(ancestor2, their_range)),
            ) => {
                assert_eq!(ancestor1.offset(), ancestor2.offset());
                let len = cmp::min(ancestor1.len(), ancestor2.len());

                shrink_front(&mut ours, len);
                shrink_front(&mut theirs, len);

                Diff3Range::AncestorTheirs(ancestor2.slice(..len), their_range.slice(..len))
            }

            (Some(DiffRange::Delete(ancestor1)), Some(DiffRange::Delete(ancestor2))) => {
                assert_eq!(ancestor1.offset(), ancestor2.offset());
                let len = cmp::min(ancestor1.len(), ancestor2.len());

                shrink_front(&mut ours, len);
                shrink_front(&mut theirs, len);

                Diff3Range::Ancestor(ancestor1.slice(..len))
            }

            //
            // Unreachable cases
            //
            (Some(DiffRange::Equal(..)), None)
            | (Some(DiffRange::Delete(_)), None)
            | (None, Some(DiffRange::Equal(..)))
            | (None, Some(DiffRange::Delete(_)))
            | (None, None) => unreachable!("Equal/Delete should match up"),
        };

        solution.push(merge_range);

        if ours.map_or(true, |range| range.is_empty()) {
            ours = our_solution.next();
        }
        if theirs.map_or(true, |range| range.is_empty()) {
            theirs = their_solution.next();
        }
    }

    solution
}

fn shrink_front<T: ?Sized + SliceLike>(maybe_range: &mut Option<DiffRange<T>>, len: usize) {
    if let Some(range) = maybe_range {
        range.shrink_front(len)
    }
}

fn diff3_range_to_merge_range<'ancestor, 'ours, 'theirs, T: ?Sized + SliceLike>(
    solution: &[Diff3Range<'ancestor, 'ours, 'theirs, T>],
) -> Vec<MergeRange<'ancestor, 'ours, 'theirs, T>> {
    let mut ancestor: Option<Range<'ancestor, T>> = None;
    let mut ours: Option<Range<'ours, T>> = None;
    let mut theirs: Option<Range<'theirs, T>> = None;

    let mut merge = Vec::new();

    for &diff3 in solution {
        match diff3 {
            Diff3Range::Equal(ancestor_range, our_range, their_range) => {
                if let Some(merge_range) =
                    create_merge_range(ancestor.take(), ours.take(), theirs.take())
                {
                    merge.push(merge_range);
                }
                merge.push(MergeRange::Equal(ancestor_range, our_range, their_range));
            }
            Diff3Range::Ancestor(range) => {
                set_or_merge_range(&mut ancestor, range);
                set_or_merge_range(&mut ours, Range::empty());
                set_or_merge_range(&mut theirs, Range::empty());
            }
            Diff3Range::AncestorOurs(ancestor_range, our_range) => {
                set_or_merge_range(&mut ancestor, ancestor_range);
                set_or_merge_range(&mut ours, our_range);
            }
            Diff3Range::AncestorTheirs(ancestor_range, their_range) => {
                set_or_merge_range(&mut ancestor, ancestor_range);
                set_or_merge_range(&mut theirs, their_range);
            }
            Diff3Range::Ours(range) => set_or_merge_range(&mut ours, range),
            Diff3Range::Theirs(range) => set_or_merge_range(&mut theirs, range),
        }
    }

    if let Some(merge_range) = create_merge_range(ancestor.take(), ours.take(), theirs.take()) {
        merge.push(merge_range);
    }

    merge
}

fn set_or_merge_range<'a, T: ?Sized>(range1: &mut Option<Range<'a, T>>, range2: Range<'a, T>) {
    if let Some(range1) = range1 {
        if range1.is_empty() {
            *range1 = range2;
        } else if !range2.is_empty() {
            assert_eq!(range1.offset() + range1.len(), range2.offset());
            range1.grow_down(range2.len());
        }
    } else {
        *range1 = Some(range2);
    }
}

fn create_merge_range<'ancestor, 'ours, 'theirs, T: ?Sized + SliceLike>(
    ancestor: Option<Range<'ancestor, T>>,
    ours: Option<Range<'ours, T>>,
    theirs: Option<Range<'theirs, T>>,
) -> Option<MergeRange<'ancestor, 'ours, 'theirs, T>> {
    match (ancestor, ours, theirs) {
        (Some(ancestor), Some(ours), Some(theirs)) => {
            Some(MergeRange::Conflict(ancestor, ours, theirs))
        }
        (None, Some(ours), Some(theirs)) => {
            Some(MergeRange::Conflict(Range::empty(), ours, theirs))
        }
        (None, Some(ours), None) => Some(MergeRange::Ours(ours)),
        (None, None, Some(theirs)) => Some(MergeRange::Theirs(theirs)),

        (Some(ancestor), None, Some(theirs)) => {
            Some(MergeRange::Conflict(ancestor, Range::empty(), theirs))
        }
        (Some(ancestor), Some(ours), None) => {
            Some(MergeRange::Conflict(ancestor, ours, Range::empty()))
        }

        (Some(_), None, None) | (None, None, None) => None,
    }
}

#[allow(clippy::needless_lifetimes)]
fn cleanup_conflicts<'ancestor, 'ours, 'theirs, T: ?Sized + SliceLike + PartialEq>(
    solution: &mut [MergeRange<'ancestor, 'ours, 'theirs, T>],
) {
    let mut pointer = 0;

    // TODO this could probably be more sophisticated:
    // e.g. run the diff algorithm on the conflict area
    while let Some(&merge) = solution.get(pointer) {
        if let MergeRange::Conflict(ancestor, ours, theirs) = merge {
            // If the ranges in the conflict end up being the same on both sides then we can
            // eliminate the conflict
            if ours.as_slice() == theirs.as_slice() {
                solution[pointer] = MergeRange::Both(ours, theirs);
            // If either ours or theirs exactly matches ancestor then we can also eliminate the
            // conflict
            } else if ancestor.as_slice() == ours.as_slice() {
                solution[pointer] = MergeRange::Theirs(theirs);
            } else if ancestor.as_slice() == theirs.as_slice() {
                solution[pointer] = MergeRange::Ours(ours);
            }
        }
        pointer += 1;
    }
}

fn output_result<'a, T: ?Sized>(
    ancestor: &[&'a str],
    ours: &[&'a str],
    theirs: &[&'a str],
    merge: &[MergeRange<T>],
    marker_len: usize,
    style: ConflictStyle,
) -> Result<String, String> {
    let mut conflicts = 0;
    let mut output = String::new();

    for merge_range in merge {
        match merge_range {
            MergeRange::Equal(range, ..) => {
                output.extend(ancestor[range.range()].iter().copied());
            }
            MergeRange::Conflict(ancestor_range, ours_range, theirs_range) => {
                add_conflict_marker(&mut output, '<', marker_len, Some("ours"));
                output.extend(ours[ours_range.range()].iter().copied());

                if let ConflictStyle::Diff3 = style {
                    add_conflict_marker(&mut output, '|', marker_len, Some("original"));
                    output.extend(ancestor[ancestor_range.range()].iter().copied());
                }

                add_conflict_marker(&mut output, '=', marker_len, None);
                output.extend(theirs[theirs_range.range()].iter().copied());
                add_conflict_marker(&mut output, '>', marker_len, Some("theirs"));
                conflicts += 1;
            }
            MergeRange::Ours(range) => {
                output.extend(ours[range.range()].iter().copied());
            }
            MergeRange::Theirs(range) => {
                output.extend(theirs[range.range()].iter().copied());
            }
            MergeRange::Both(range, _) => {
                output.extend(ours[range.range()].iter().copied());
            }
        }
    }

    if conflicts != 0 {
        Err(output)
    } else {
        Ok(output)
    }
}

fn add_conflict_marker(
    output: &mut String,
    marker: char,
    marker_len: usize,
    filename: Option<&str>,
) {
    for _ in 0..marker_len {
        output.push(marker);
    }

    if let Some(filename) = filename {
        output.push(' ');
        output.push_str(filename);
    }
    output.push('\n');
}

fn output_result_bytes<'a, T: ?Sized>(
    ancestor: &[&'a [u8]],
    ours: &[&'a [u8]],
    theirs: &[&'a [u8]],
    merge: &[MergeRange<T>],
    marker_len: usize,
    style: ConflictStyle,
) -> Result<Vec<u8>, Vec<u8>> {
    let mut conflicts = 0;
    let mut output: Vec<u8> = Vec::new();

    for merge_range in merge {
        match merge_range {
            MergeRange::Equal(range, ..) => {
                ancestor[range.range()]
                    .iter()
                    .for_each(|line| output.extend_from_slice(line));
            }
            MergeRange::Conflict(ancestor_range, ours_range, theirs_range) => {
                add_conflict_marker_bytes(&mut output, b'<', marker_len, Some(b"ours"));
                ours[ours_range.range()]
                    .iter()
                    .for_each(|line| output.extend_from_slice(line));

                if let ConflictStyle::Diff3 = style {
                    add_conflict_marker_bytes(&mut output, b'|', marker_len, Some(b"original"));
                    ancestor[ancestor_range.range()]
                        .iter()
                        .for_each(|line| output.extend_from_slice(line));
                }

                add_conflict_marker_bytes(&mut output, b'=', marker_len, None);
                theirs[theirs_range.range()]
                    .iter()
                    .for_each(|line| output.extend_from_slice(line));
                add_conflict_marker_bytes(&mut output, b'>', marker_len, Some(b"theirs"));
                conflicts += 1;
            }
            MergeRange::Ours(range) => {
                ours[range.range()]
                    .iter()
                    .for_each(|line| output.extend_from_slice(line));
            }
            MergeRange::Theirs(range) => {
                theirs[range.range()]
                    .iter()
                    .for_each(|line| output.extend_from_slice(line));
            }
            MergeRange::Both(range, _) => {
                ours[range.range()]
                    .iter()
                    .for_each(|line| output.extend_from_slice(line));
            }
        }
    }

    if conflicts != 0 {
        Err(output)
    } else {
        Ok(output)
    }
}

fn add_conflict_marker_bytes(
    output: &mut Vec<u8>,
    marker: u8,
    marker_len: usize,
    filename: Option<&[u8]>,
) {
    for _ in 0..marker_len {
        output.push(marker);
    }

    if let Some(filename) = filename {
        output.push(b' ');
        output.extend_from_slice(filename);
    }
    output.push(b'\n');
}
