//! Myers' diff algorithm.
//!
//! * time: `O((N+M)D)`
//! * space `O(N+M)`
//!
//! See [the original article by Eugene W. Myers](http://www.xmailserver.org/diff2.pdf)
//! describing it.
//!
//! The implementation of this algorithm is based on the implementation by
//! Brandon Williams.
//!
//! # Heuristics
//!
//! At present this implementation of Myers' does not implement any more advanced
//! heuristics that would solve some pathological cases.  For instance passing two
//! large and completely distinct sequences to the algorithm will make it spin
//! without making reasonable progress.  Currently the only protection in the
//! library against this is to pass a deadline to the diffing algorithm.
//!
//! For potential improvements here see [similar#15](https://github.com/mitsuhiko/similar/issues/15).

use std::ops::{Index, IndexMut, Range};

use crate::algorithms::utils::{common_prefix_len, common_suffix_len, is_empty_range};
use crate::algorithms::DiffHook;
use crate::deadline_support::{deadline_exceeded, Instant};

/// Myers' diff algorithm.
///
/// Diff `old`, between indices `old_range` and `new` between indices `new_range`.
pub fn diff<Old, New, D>(
    d: &mut D,
    old: &Old,
    old_range: Range<usize>,
    new: &New,
    new_range: Range<usize>,
) -> Result<(), D::Error>
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    D: DiffHook,
    New::Output: PartialEq<Old::Output>,
{
    diff_deadline(d, old, old_range, new, new_range, None)
}

/// Myers' diff algorithm with deadline.
///
/// Diff `old`, between indices `old_range` and `new` between indices `new_range`.
///
/// This diff is done with an optional deadline that defines the maximal
/// execution time permitted before it bails and falls back to an approximation.
pub fn diff_deadline<Old, New, D>(
    d: &mut D,
    old: &Old,
    old_range: Range<usize>,
    new: &New,
    new_range: Range<usize>,
    deadline: Option<Instant>,
) -> Result<(), D::Error>
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    D: DiffHook,
    New::Output: PartialEq<Old::Output>,
{
    let max_d = max_d(old_range.len(), new_range.len());
    let mut vb = V::new(max_d);
    let mut vf = V::new(max_d);
    conquer(
        d, old, old_range, new, new_range, &mut vf, &mut vb, deadline,
    )?;
    d.finish()
}

// A D-path is a path which starts at (0,0) that has exactly D non-diagonal
// edges. All D-paths consist of a (D - 1)-path followed by a non-diagonal edge
// and then a possibly empty sequence of diagonal edges called a snake.

/// `V` contains the endpoints of the furthest reaching `D-paths`. For each
/// recorded endpoint `(x,y)` in diagonal `k`, we only need to retain `x` because
/// `y` can be computed from `x - k`. In other words, `V` is an array of integers
/// where `V[k]` contains the row index of the endpoint of the furthest reaching
/// path in diagonal `k`.
///
/// We can't use a traditional Vec to represent `V` since we use `k` as an index
/// and it can take on negative values. So instead `V` is represented as a
/// light-weight wrapper around a Vec plus an `offset` which is the maximum value
/// `k` can take on in order to map negative `k`'s back to a value >= 0.
#[derive(Debug)]
struct V {
    offset: isize,
    v: Vec<usize>, // Look into initializing this to -1 and storing isize
}

impl V {
    fn new(max_d: usize) -> Self {
        Self {
            offset: max_d as isize,
            v: vec![0; 2 * max_d],
        }
    }

    fn len(&self) -> usize {
        self.v.len()
    }
}

impl Index<isize> for V {
    type Output = usize;

    fn index(&self, index: isize) -> &Self::Output {
        &self.v[(index + self.offset) as usize]
    }
}

impl IndexMut<isize> for V {
    fn index_mut(&mut self, index: isize) -> &mut Self::Output {
        &mut self.v[(index + self.offset) as usize]
    }
}

fn max_d(len1: usize, len2: usize) -> usize {
    // XXX look into reducing the need to have the additional '+ 1'
    (len1 + len2 + 1) / 2 + 1
}

#[inline(always)]
fn split_at(range: Range<usize>, at: usize) -> (Range<usize>, Range<usize>) {
    (range.start..at, at..range.end)
}

/// A `Snake` is a sequence of diagonal edges in the edit graph.  Normally
/// a snake has a start end end point (and it is possible for a snake to have
/// a length of zero, meaning the start and end points are the same) however
/// we do not need the end point which is why it's not implemented here.
///
/// The divide part of a divide-and-conquer strategy. A D-path has D+1 snakes
/// some of which may be empty. The divide step requires finding the ceil(D/2) +
/// 1 or middle snake of an optimal D-path. The idea for doing so is to
/// simultaneously run the basic algorithm in both the forward and reverse
/// directions until furthest reaching forward and reverse paths starting at
/// opposing corners 'overlap'.
fn find_middle_snake<Old, New>(
    old: &Old,
    old_range: Range<usize>,
    new: &New,
    new_range: Range<usize>,
    vf: &mut V,
    vb: &mut V,
    deadline: Option<Instant>,
) -> Option<(usize, usize)>
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    New::Output: PartialEq<Old::Output>,
{
    let n = old_range.len();
    let m = new_range.len();

    // By Lemma 1 in the paper, the optimal edit script length is odd or even as
    // `delta` is odd or even.
    let delta = n as isize - m as isize;
    let odd = delta & 1 == 1;

    // The initial point at (0, -1)
    vf[1] = 0;
    // The initial point at (N, M+1)
    vb[1] = 0;

    // We only need to explore ceil(D/2) + 1
    let d_max = max_d(n, m);
    assert!(vf.len() >= d_max);
    assert!(vb.len() >= d_max);

    for d in 0..d_max as isize {
        // are we running for too long?
        if deadline_exceeded(deadline) {
            break;
        }

        // Forward path
        for k in (-d..=d).rev().step_by(2) {
            let mut x = if k == -d || (k != d && vf[k - 1] < vf[k + 1]) {
                vf[k + 1]
            } else {
                vf[k - 1] + 1
            };
            let y = (x as isize - k) as usize;

            // The coordinate of the start of a snake
            let (x0, y0) = (x, y);
            //  While these sequences are identical, keep moving through the
            //  graph with no cost
            if x < old_range.len() && y < new_range.len() {
                let advance = common_prefix_len(
                    old,
                    old_range.start + x..old_range.end,
                    new,
                    new_range.start + y..new_range.end,
                );
                x += advance;
            }

            // This is the new best x value
            vf[k] = x;

            // Only check for connections from the forward search when N - M is
            // odd and when there is a reciprocal k line coming from the other
            // direction.
            if odd && (k - delta).abs() <= (d - 1) {
                // TODO optimize this so we don't have to compare against n
                if vf[k] + vb[-(k - delta)] >= n {
                    // Return the snake
                    return Some((x0 + old_range.start, y0 + new_range.start));
                }
            }
        }

        // Backward path
        for k in (-d..=d).rev().step_by(2) {
            let mut x = if k == -d || (k != d && vb[k - 1] < vb[k + 1]) {
                vb[k + 1]
            } else {
                vb[k - 1] + 1
            };
            let mut y = (x as isize - k) as usize;

            // The coordinate of the start of a snake
            if x < n && y < m {
                let advance = common_suffix_len(
                    old,
                    old_range.start..old_range.start + n - x,
                    new,
                    new_range.start..new_range.start + m - y,
                );
                x += advance;
                y += advance;
            }

            // This is the new best x value
            vb[k] = x;

            if !odd && (k - delta).abs() <= d {
                // TODO optimize this so we don't have to compare against n
                if vb[k] + vf[-(k - delta)] >= n {
                    // Return the snake
                    return Some((n - x + old_range.start, m - y + new_range.start));
                }
            }
        }

        // TODO: Maybe there's an opportunity to optimize and bail early?
    }

    // deadline reached
    None
}

#[allow(clippy::too_many_arguments)]
fn conquer<Old, New, D>(
    d: &mut D,
    old: &Old,
    mut old_range: Range<usize>,
    new: &New,
    mut new_range: Range<usize>,
    vf: &mut V,
    vb: &mut V,
    deadline: Option<Instant>,
) -> Result<(), D::Error>
where
    Old: Index<usize> + ?Sized,
    New: Index<usize> + ?Sized,
    D: DiffHook,
    New::Output: PartialEq<Old::Output>,
{
    // Check for common prefix
    let common_prefix_len = common_prefix_len(old, old_range.clone(), new, new_range.clone());
    if common_prefix_len > 0 {
        d.equal(old_range.start, new_range.start, common_prefix_len)?;
    }
    old_range.start += common_prefix_len;
    new_range.start += common_prefix_len;

    // Check for common suffix
    let common_suffix_len = common_suffix_len(old, old_range.clone(), new, new_range.clone());
    let common_suffix = (
        old_range.end - common_suffix_len,
        new_range.end - common_suffix_len,
    );
    old_range.end -= common_suffix_len;
    new_range.end -= common_suffix_len;

    if is_empty_range(&old_range) && is_empty_range(&new_range) {
        // Do nothing
    } else if is_empty_range(&new_range) {
        d.delete(old_range.start, old_range.len(), new_range.start)?;
    } else if is_empty_range(&old_range) {
        d.insert(old_range.start, new_range.start, new_range.len())?;
    } else if let Some((x_start, y_start)) = find_middle_snake(
        old,
        old_range.clone(),
        new,
        new_range.clone(),
        vf,
        vb,
        deadline,
    ) {
        let (old_a, old_b) = split_at(old_range, x_start);
        let (new_a, new_b) = split_at(new_range, y_start);
        conquer(d, old, old_a, new, new_a, vf, vb, deadline)?;
        conquer(d, old, old_b, new, new_b, vf, vb, deadline)?;
    } else {
        d.delete(
            old_range.start,
            old_range.end - old_range.start,
            new_range.start,
        )?;
        d.insert(
            old_range.start,
            new_range.start,
            new_range.end - new_range.start,
        )?;
    }

    if common_suffix_len > 0 {
        d.equal(common_suffix.0, common_suffix.1, common_suffix_len)?;
    }

    Ok(())
}

#[test]
fn test_find_middle_snake() {
    let a = &b"ABCABBA"[..];
    let b = &b"CBABAC"[..];
    let max_d = max_d(a.len(), b.len());
    let mut vf = V::new(max_d);
    let mut vb = V::new(max_d);
    let (x_start, y_start) =
        find_middle_snake(a, 0..a.len(), b, 0..b.len(), &mut vf, &mut vb, None).unwrap();
    assert_eq!(x_start, 4);
    assert_eq!(y_start, 1);
}

#[test]
fn test_diff() {
    let a: &[usize] = &[0, 1, 2, 3, 4];
    let b: &[usize] = &[0, 1, 2, 9, 4];

    let mut d = crate::algorithms::Replace::new(crate::algorithms::Capture::new());
    diff(&mut d, a, 0..a.len(), b, 0..b.len()).unwrap();
    insta::assert_debug_snapshot!(d.into_inner().ops());
}

#[test]
fn test_contiguous() {
    let a: &[usize] = &[0, 1, 2, 3, 4, 4, 4, 5];
    let b: &[usize] = &[0, 1, 2, 8, 9, 4, 4, 7];

    let mut d = crate::algorithms::Replace::new(crate::algorithms::Capture::new());
    diff(&mut d, a, 0..a.len(), b, 0..b.len()).unwrap();
    insta::assert_debug_snapshot!(d.into_inner().ops());
}

#[test]
fn test_pat() {
    let a: &[usize] = &[0, 1, 3, 4, 5];
    let b: &[usize] = &[0, 1, 4, 5, 8, 9];

    let mut d = crate::algorithms::Capture::new();
    diff(&mut d, a, 0..a.len(), b, 0..b.len()).unwrap();
    insta::assert_debug_snapshot!(d.ops());
}

#[test]
fn test_deadline_reached() {
    use std::ops::Index;
    use std::time::Duration;

    let a = (0..100).collect::<Vec<_>>();
    let mut b = (0..100).collect::<Vec<_>>();
    b[10] = 99;
    b[50] = 99;
    b[25] = 99;

    struct SlowIndex<'a>(&'a [usize]);

    impl Index<usize> for SlowIndex<'_> {
        type Output = usize;

        fn index(&self, index: usize) -> &Self::Output {
            std::thread::sleep(Duration::from_millis(1));
            &self.0[index]
        }
    }

    let slow_a = SlowIndex(&a);
    let slow_b = SlowIndex(&b);

    // don't give it enough time to do anything interesting
    let mut d = crate::algorithms::Replace::new(crate::algorithms::Capture::new());
    diff_deadline(
        &mut d,
        &slow_a,
        0..a.len(),
        &slow_b,
        0..b.len(),
        Some(Instant::now() + Duration::from_millis(50)),
    )
    .unwrap();
    insta::assert_debug_snapshot!(d.into_inner().ops());
}

#[test]
fn test_finish_called() {
    struct HasRunFinish(bool);

    impl DiffHook for HasRunFinish {
        type Error = ();
        fn finish(&mut self) -> Result<(), Self::Error> {
            self.0 = true;
            Ok(())
        }
    }

    let mut d = HasRunFinish(false);
    let slice = &[1, 2];
    let slice2 = &[1, 2, 3];
    diff(&mut d, slice, 0..slice.len(), slice2, 0..slice2.len()).unwrap();
    assert!(d.0);

    let mut d = HasRunFinish(false);
    let slice = &[1, 2];
    diff(&mut d, slice, 0..slice.len(), slice, 0..slice.len()).unwrap();
    assert!(d.0);

    let mut d = HasRunFinish(false);
    let slice: &[u8] = &[];
    diff(&mut d, slice, 0..slice.len(), slice, 0..slice.len()).unwrap();
    assert!(d.0);
}
