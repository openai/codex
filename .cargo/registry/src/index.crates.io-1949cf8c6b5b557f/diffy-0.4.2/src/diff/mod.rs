use crate::{
    patch::{Hunk, HunkRange, Line, Patch},
    range::{DiffRange, SliceLike},
    utils::Classifier,
};
use std::{borrow::Cow, cmp, ops};

mod cleanup;
mod myers;

#[cfg(test)]
mod tests;

// TODO determine if this should be exposed in the public API
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
enum Diff<'a, T: ?Sized> {
    Equal(&'a T),
    Delete(&'a T),
    Insert(&'a T),
}

impl<T: ?Sized> Copy for Diff<'_, T> {}

impl<T: ?Sized> Clone for Diff<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> From<DiffRange<'a, 'a, T>> for Diff<'a, T>
where
    T: ?Sized + SliceLike,
{
    fn from(diff: DiffRange<'a, 'a, T>) -> Self {
        match diff {
            DiffRange::Equal(range, _) => Diff::Equal(range.as_slice()),
            DiffRange::Delete(range) => Diff::Delete(range.as_slice()),
            DiffRange::Insert(range) => Diff::Insert(range.as_slice()),
        }
    }
}

/// A collection of options for modifying the way a diff is performed
#[derive(Debug)]
pub struct DiffOptions {
    compact: bool,
    context_len: usize,
    original_filename: Option<Cow<'static, str>>,
    modified_filename: Option<Cow<'static, str>>,
}

impl DiffOptions {
    /// Construct a new `DiffOptions` with default settings
    ///
    /// ## Defaults
    /// * context_len = 3
    pub fn new() -> Self {
        Self {
            compact: true,
            context_len: 3,
            original_filename: Some("original".into()),
            modified_filename: Some("modified".into()),
        }
    }

    /// Set the number of context lines that should be used when producing a patch
    pub fn set_context_len(&mut self, context_len: usize) -> &mut Self {
        self.context_len = context_len;
        self
    }

    /// Enable/Disable diff compaction. Compaction is a post-processing step which attempts to
    /// produce a prettier diff by reducing the number of edited blocks by shifting and merging
    /// edit blocks.
    // TODO determine if this should be exposed in the public API
    #[allow(dead_code)]
    fn set_compact(&mut self, compact: bool) -> &mut Self {
        self.compact = compact;
        self
    }

    /// Set the filename to be used in the patch for the original text
    ///
    /// If not set, the default value is "original".
    pub fn set_original_filename<T>(&mut self, filename: T) -> &mut Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.original_filename = Some(filename.into());
        self
    }

    /// Set the filename to be used in the patch for the modified text
    ///
    /// If not set, the default value is "modified".
    pub fn set_modified_filename<T>(&mut self, filename: T) -> &mut Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.modified_filename = Some(filename.into());
        self
    }

    // TODO determine if this should be exposed in the public API
    #[allow(dead_code)]
    fn diff<'a>(&self, original: &'a str, modified: &'a str) -> Vec<Diff<'a, str>> {
        let solution = myers::diff(original.as_bytes(), modified.as_bytes());

        let mut solution = solution
            .into_iter()
            .map(|diff_range| diff_range.to_str(original, modified))
            .collect();

        if self.compact {
            cleanup::compact(&mut solution);
        }

        solution.into_iter().map(Diff::from).collect()
    }

    /// Produce a Patch between two texts based on the configured options
    pub fn create_patch<'a>(&self, original: &'a str, modified: &'a str) -> Patch<'a, str> {
        let mut classifier = Classifier::default();
        let (old_lines, old_ids) = classifier.classify_lines(original);
        let (new_lines, new_ids) = classifier.classify_lines(modified);

        let solution = self.diff_slice(&old_ids, &new_ids);

        let hunks = to_hunks(&old_lines, &new_lines, &solution, self.context_len);
        Patch::new(
            self.original_filename.clone(),
            self.modified_filename.clone(),
            hunks,
        )
    }

    /// Create a patch between two potentially non-utf8 texts
    pub fn create_patch_bytes<'a>(
        &self,
        original: &'a [u8],
        modified: &'a [u8],
    ) -> Patch<'a, [u8]> {
        let mut classifier = Classifier::default();
        let (old_lines, old_ids) = classifier.classify_lines(original);
        let (new_lines, new_ids) = classifier.classify_lines(modified);

        let solution = self.diff_slice(&old_ids, &new_ids);

        let hunks = to_hunks(&old_lines, &new_lines, &solution, self.context_len);

        // helper function to convert a utf8 cow to a bytes cow
        fn cow_str_to_bytes(cow: Cow<'static, str>) -> Cow<'static, [u8]> {
            match cow {
                Cow::Borrowed(b) => Cow::Borrowed(b.as_bytes()),
                Cow::Owned(o) => Cow::Owned(o.into_bytes()),
            }
        }

        Patch::new(
            self.original_filename.clone().map(cow_str_to_bytes),
            self.modified_filename.clone().map(cow_str_to_bytes),
            hunks,
        )
    }

    pub(crate) fn diff_slice<'a, T: PartialEq>(
        &self,
        old: &'a [T],
        new: &'a [T],
    ) -> Vec<DiffRange<'a, 'a, [T]>> {
        let mut solution = myers::diff(old, new);

        if self.compact {
            cleanup::compact(&mut solution);
        }

        solution
    }
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self::new()
    }
}

// TODO determine if this should be exposed in the public API
#[allow(dead_code)]
fn diff<'a>(original: &'a str, modified: &'a str) -> Vec<Diff<'a, str>> {
    DiffOptions::default().diff(original, modified)
}

/// Create a patch between two texts.
///
/// ```
/// # use diffy::create_patch;
/// let original = "\
/// I am afraid, however, that all I have known - that my story - will be forgotten.
/// I am afraid for the world that is to come.
/// Afraid that my plans will fail.
/// Afraid of a doom worse than the Deepness.
/// ";
///
/// let modified = "\
/// I am afraid, however, that all I have known - that my story - will be forgotten.
/// I am afraid for the world that is to come.
/// Afraid that Alendi will fail.
/// Afraid of a doom brought by the Deepness.
/// ";
///
/// let expected = "\
/// --- original
/// +++ modified
/// @@ -1,4 +1,4 @@
///  I am afraid, however, that all I have known - that my story - will be forgotten.
///  I am afraid for the world that is to come.
/// -Afraid that my plans will fail.
/// -Afraid of a doom worse than the Deepness.
/// +Afraid that Alendi will fail.
/// +Afraid of a doom brought by the Deepness.
/// ";
///
/// let patch = create_patch(original, modified);
/// assert_eq!(patch.to_string(), expected);
/// ```
pub fn create_patch<'a>(original: &'a str, modified: &'a str) -> Patch<'a, str> {
    DiffOptions::default().create_patch(original, modified)
}

/// Create a patch between two potentially non-utf8 texts
pub fn create_patch_bytes<'a>(original: &'a [u8], modified: &'a [u8]) -> Patch<'a, [u8]> {
    DiffOptions::default().create_patch_bytes(original, modified)
}

fn to_hunks<'a, T: ?Sized>(
    lines1: &[&'a T],
    lines2: &[&'a T],
    solution: &[DiffRange<[u64]>],
    context_len: usize,
) -> Vec<Hunk<'a, T>> {
    let edit_script = build_edit_script(solution);

    let mut hunks = Vec::new();

    let mut idx = 0;
    while let Some(mut script) = edit_script.get(idx) {
        let start1 = script.old.start.saturating_sub(context_len);
        let start2 = script.new.start.saturating_sub(context_len);

        let (mut end1, mut end2) = calc_end(
            context_len,
            lines1.len(),
            lines2.len(),
            script.old.end,
            script.new.end,
        );

        let mut lines = Vec::new();

        // Pre-context
        for line in lines2.get(start2..script.new.start).into_iter().flatten() {
            lines.push(Line::Context(*line));
        }

        loop {
            // Delete lines from text1
            for line in lines1.get(script.old.clone()).into_iter().flatten() {
                lines.push(Line::Delete(*line));
            }

            // Insert lines from text2
            for line in lines2.get(script.new.clone()).into_iter().flatten() {
                lines.push(Line::Insert(*line));
            }

            if let Some(s) = edit_script.get(idx + 1) {
                // Check to see if we can merge the hunks
                let start1_next =
                    cmp::min(s.old.start, lines1.len() - 1).saturating_sub(context_len);
                if start1_next < end1 {
                    // Context lines between hunks
                    for (_i1, i2) in (script.old.end..s.old.start).zip(script.new.end..s.new.start)
                    {
                        if let Some(line) = lines2.get(i2) {
                            lines.push(Line::Context(*line));
                        }
                    }

                    // Calc the new end
                    let (e1, e2) = calc_end(
                        context_len,
                        lines1.len(),
                        lines2.len(),
                        s.old.end,
                        s.new.end,
                    );

                    end1 = e1;
                    end2 = e2;
                    script = s;
                    idx += 1;
                    continue;
                }
            }

            break;
        }

        // Post-context
        for line in lines2.get(script.new.end..end2).into_iter().flatten() {
            lines.push(Line::Context(*line));
        }

        let len1 = end1 - start1;
        let old_range = HunkRange::new(if len1 > 0 { start1 + 1 } else { start1 }, len1);

        let len2 = end2 - start2;
        let new_range = HunkRange::new(if len2 > 0 { start2 + 1 } else { start2 }, len2);

        hunks.push(Hunk::new(old_range, new_range, None, lines));
        idx += 1;
    }

    hunks
}

fn calc_end(
    context_len: usize,
    text1_len: usize,
    text2_len: usize,
    script1_end: usize,
    script2_end: usize,
) -> (usize, usize) {
    let post_context_len = cmp::min(
        context_len,
        cmp::min(
            text1_len.saturating_sub(script1_end),
            text2_len.saturating_sub(script2_end),
        ),
    );

    let end1 = script1_end + post_context_len;
    let end2 = script2_end + post_context_len;

    (end1, end2)
}

#[derive(Debug)]
struct EditRange {
    old: ops::Range<usize>,
    new: ops::Range<usize>,
}

impl EditRange {
    fn new(old: ops::Range<usize>, new: ops::Range<usize>) -> Self {
        Self { old, new }
    }
}

fn build_edit_script<T>(solution: &[DiffRange<[T]>]) -> Vec<EditRange> {
    let mut idx_a = 0;
    let mut idx_b = 0;

    let mut edit_script: Vec<EditRange> = Vec::new();
    let mut script = None;

    for diff in solution {
        match diff {
            DiffRange::Equal(range1, range2) => {
                idx_a += range1.len();
                idx_b += range2.len();
                if let Some(script) = script.take() {
                    edit_script.push(script);
                }
            }
            DiffRange::Delete(range) => {
                match &mut script {
                    Some(s) => s.old.end += range.len(),
                    None => {
                        script = Some(EditRange::new(idx_a..idx_a + range.len(), idx_b..idx_b));
                    }
                }
                idx_a += range.len();
            }
            DiffRange::Insert(range) => {
                match &mut script {
                    Some(s) => s.new.end += range.len(),
                    None => {
                        script = Some(EditRange::new(idx_a..idx_a, idx_b..idx_b + range.len()));
                    }
                }
                idx_b += range.len();
            }
        }
    }

    if let Some(script) = script.take() {
        edit_script.push(script);
    }

    edit_script
}

#[cfg(test)]
mod test {
    use super::DiffOptions;

    #[test]
    fn set_original_and_modified_filenames() {
        let original = "\
I am afraid, however, that all I have known - that my story - will be forgotten.
I am afraid for the world that is to come.
Afraid that my plans will fail.
Afraid of a doom worse than the Deepness.
";
        let modified = "\
I am afraid, however, that all I have known - that my story - will be forgotten.
I am afraid for the world that is to come.
Afraid that Alendi will fail.
Afraid of a doom brought by the Deepness.
";
        let expected = "\
--- the old version
+++ the better version
@@ -1,4 +1,4 @@
 I am afraid, however, that all I have known - that my story - will be forgotten.
 I am afraid for the world that is to come.
-Afraid that my plans will fail.
-Afraid of a doom worse than the Deepness.
+Afraid that Alendi will fail.
+Afraid of a doom brought by the Deepness.
";

        let patch = DiffOptions::new()
            .set_original_filename("the old version")
            .set_modified_filename("the better version")
            .create_patch(original, modified);

        assert_eq!(patch.to_string(), expected);
    }
}
