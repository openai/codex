//! Recursive algorithm for finding column minima.
//!
//! The functions here are mostly meant to be used for testing
//! correctness of the SMAWK implementation.
//!
//! **Note: this module is only available if you enable the `ndarray`
//! Cargo feature.**

use ndarray::{s, Array2, ArrayView2, Axis};

/// Compute row minima in O(*m* + *n* log *m*) time.
///
/// This function computes row minima in a totally monotone matrix
/// using a recursive algorithm.
///
/// Running time on an *m* ✕ *n* matrix: O(*m* + *n* log *m*).
///
/// # Examples
///
/// ```
/// let matrix = ndarray::arr2(&[[4, 2, 4, 3],
///                              [5, 3, 5, 3],
///                              [5, 3, 3, 1]]);
/// assert_eq!(smawk::recursive::row_minima(&matrix),
///            vec![1, 1, 3]);
/// ```
///
/// # Panics
///
/// It is an error to call this on a matrix with zero columns.
pub fn row_minima<T: Ord>(matrix: &Array2<T>) -> Vec<usize> {
    let mut minima = vec![0; matrix.nrows()];
    recursive_inner(matrix.view(), &|| Direction::Row, 0, &mut minima);
    minima
}

/// Compute column minima in O(*n* + *m* log *n*) time.
///
/// This function computes column minima in a totally monotone matrix
/// using a recursive algorithm.
///
/// Running time on an *m* ✕ *n* matrix: O(*n* + *m* log *n*).
///
/// # Examples
///
/// ```
/// let matrix = ndarray::arr2(&[[4, 2, 4, 3],
///                              [5, 3, 5, 3],
///                              [5, 3, 3, 1]]);
/// assert_eq!(smawk::recursive::column_minima(&matrix),
///            vec![0, 0, 2, 2]);
/// ```
///
/// # Panics
///
/// It is an error to call this on a matrix with zero rows.
pub fn column_minima<T: Ord>(matrix: &Array2<T>) -> Vec<usize> {
    let mut minima = vec![0; matrix.ncols()];
    recursive_inner(matrix.view(), &|| Direction::Column, 0, &mut minima);
    minima
}

/// The type of minima (row or column) we compute.
enum Direction {
    Row,
    Column,
}

/// Compute the minima along the given direction (`Direction::Row` for
/// row minima and `Direction::Column` for column minima).
///
/// The direction is given as a generic function argument to allow
/// monomorphization to kick in. The function calls will be inlined
/// and optimized away and the result is that the compiler generates
/// differnet code for finding row and column minima.
fn recursive_inner<T: Ord, F: Fn() -> Direction>(
    matrix: ArrayView2<'_, T>,
    dir: &F,
    offset: usize,
    minima: &mut [usize],
) {
    if matrix.is_empty() {
        return;
    }

    let axis = match dir() {
        Direction::Row => Axis(0),
        Direction::Column => Axis(1),
    };
    let mid = matrix.len_of(axis) / 2;
    let min_idx = crate::brute_force::lane_minimum(matrix.index_axis(axis, mid));
    minima[mid] = offset + min_idx;

    if mid == 0 {
        return; // Matrix has a single row or column, so we're done.
    }

    let top_left = match dir() {
        Direction::Row => matrix.slice(s![..mid, ..(min_idx + 1)]),
        Direction::Column => matrix.slice(s![..(min_idx + 1), ..mid]),
    };
    let bot_right = match dir() {
        Direction::Row => matrix.slice(s![(mid + 1).., min_idx..]),
        Direction::Column => matrix.slice(s![min_idx.., (mid + 1)..]),
    };
    recursive_inner(top_left, dir, offset, &mut minima[..mid]);
    recursive_inner(bot_right, dir, offset + min_idx, &mut minima[mid + 1..]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::arr2;

    #[test]
    fn recursive_1x1() {
        let matrix = arr2(&[[2]]);
        let minima = vec![0];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_2x1() {
        let matrix = arr2(&[
            [3], //
            [2],
        ]);
        let minima = vec![0, 0];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_1x2() {
        let matrix = arr2(&[[2, 1]]);
        let minima = vec![1];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_2x2() {
        let matrix = arr2(&[
            [3, 2], //
            [2, 1],
        ]);
        let minima = vec![1, 1];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_3x3() {
        let matrix = arr2(&[
            [3, 4, 4], //
            [3, 4, 4],
            [2, 3, 3],
        ]);
        let minima = vec![0, 0, 0];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_4x4() {
        let matrix = arr2(&[
            [4, 5, 5, 5], //
            [2, 3, 3, 3],
            [2, 3, 3, 3],
            [2, 2, 2, 2],
        ]);
        let minima = vec![0, 0, 0, 0];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_5x5() {
        let matrix = arr2(&[
            [3, 2, 4, 5, 6],
            [2, 1, 3, 3, 4],
            [2, 1, 3, 3, 4],
            [3, 2, 4, 3, 4],
            [4, 3, 2, 1, 1],
        ]);
        let minima = vec![1, 1, 1, 1, 3];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }
}
