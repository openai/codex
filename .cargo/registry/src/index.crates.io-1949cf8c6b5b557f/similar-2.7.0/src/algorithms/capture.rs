use std::convert::Infallible;

use crate::algorithms::DiffHook;
use crate::{group_diff_ops, DiffOp};

/// A [`DiffHook`] that captures all diff operations.
#[derive(Default, Clone)]
pub struct Capture(Vec<DiffOp>);

impl Capture {
    /// Creates a new capture hook.
    pub fn new() -> Capture {
        Capture::default()
    }

    /// Converts the capture hook into a vector of ops.
    pub fn into_ops(self) -> Vec<DiffOp> {
        self.0
    }

    /// Isolate change clusters by eliminating ranges with no changes.
    ///
    /// This is equivalent to calling [`group_diff_ops`] on [`Capture::into_ops`].
    pub fn into_grouped_ops(self, n: usize) -> Vec<Vec<DiffOp>> {
        group_diff_ops(self.into_ops(), n)
    }

    /// Accesses the captured operations.
    pub fn ops(&self) -> &[DiffOp] {
        &self.0
    }
}

impl DiffHook for Capture {
    type Error = Infallible;

    #[inline(always)]
    fn equal(&mut self, old_index: usize, new_index: usize, len: usize) -> Result<(), Self::Error> {
        self.0.push(DiffOp::Equal {
            old_index,
            new_index,
            len,
        });
        Ok(())
    }

    #[inline(always)]
    fn delete(
        &mut self,
        old_index: usize,
        old_len: usize,
        new_index: usize,
    ) -> Result<(), Self::Error> {
        self.0.push(DiffOp::Delete {
            old_index,
            old_len,
            new_index,
        });
        Ok(())
    }

    #[inline(always)]
    fn insert(
        &mut self,
        old_index: usize,
        new_index: usize,
        new_len: usize,
    ) -> Result<(), Self::Error> {
        self.0.push(DiffOp::Insert {
            old_index,
            new_index,
            new_len,
        });
        Ok(())
    }

    #[inline(always)]
    fn replace(
        &mut self,
        old_index: usize,
        old_len: usize,
        new_index: usize,
        new_len: usize,
    ) -> Result<(), Self::Error> {
        self.0.push(DiffOp::Replace {
            old_index,
            old_len,
            new_index,
            new_len,
        });
        Ok(())
    }
}

#[test]
fn test_capture_hook_grouping() {
    use crate::algorithms::{diff_slices, Algorithm, Replace};

    let rng = (1..100).collect::<Vec<_>>();
    let mut rng_new = rng.clone();
    rng_new[10] = 1000;
    rng_new[13] = 1000;
    rng_new[16] = 1000;
    rng_new[34] = 1000;

    let mut d = Replace::new(Capture::new());
    diff_slices(Algorithm::Myers, &mut d, &rng, &rng_new).unwrap();

    let ops = d.into_inner().into_grouped_ops(3);
    let tags = ops
        .iter()
        .map(|group| group.iter().map(|x| x.as_tag_tuple()).collect::<Vec<_>>())
        .collect::<Vec<_>>();

    insta::assert_debug_snapshot!(ops);
    insta::assert_debug_snapshot!(tags);
}
