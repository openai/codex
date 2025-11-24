//! Brute-force algorithm for finding column minima.
//!
//! The functions here are mostly meant to be used for testing
//! correctness of the SMAWK implementation.
//!
//! **Note: this module is only available if you enable the `ndarray`
//! Cargo feature.**

use ndarray::{Array2, ArrayView1};

/// Compute lane minimum by brute force.
///
/// This does a simple scan through the lane (row or column).
#[inline]
pub fn lane_minimum<T: Ord>(lane: ArrayView1<'_, T>) -> usize {
    lane.iter()
        .enumerate()
        .min_by_key(|&(idx, elem)| (elem, idx))
        .map(|(idx, _)| idx)
        .expect("empty lane in matrix")
}

/// Compute row minima by brute force in O(*mn*) time.
///
/// This function implements a simple brute-force approach where each
/// matrix row is scanned completely. This means that the function
/// works on all matrices, not just Monge matrices.
///
/// # Examples
///
/// ```
/// let matrix = ndarray::arr2(&[[4, 2, 4, 3],
///                              [5, 3, 5, 3],
///                              [5, 3, 3, 1]]);
/// assert_eq!(smawk::brute_force::row_minima(&matrix),
///            vec![1, 1, 3]);
/// ```
///
/// # Panics
///
/// It is an error to call this on a matrix with zero columns.
pub fn row_minima<T: Ord>(matrix: &Array2<T>) -> Vec<usize> {
    matrix.rows().into_iter().map(lane_minimum).collect()
}

/// Compute column minima by brute force in O(*mn*) time.
///
/// This function implements a simple brute-force approach where each
/// matrix column is scanned completely. This means that the function
/// works on all matrices, not just Monge matrices.
///
/// # Examples
///
/// ```
/// let matrix = ndarray::arr2(&[[4, 2, 4, 3],
///                              [5, 3, 5, 3],
///                              [5, 3, 3, 1]]);
/// assert_eq!(smawk::brute_force::column_minima(&matrix),
///            vec![0, 0, 2, 2]);
/// ```
///
/// # Panics
///
/// It is an error to call this on a matrix with zero rows.
pub fn column_minima<T: Ord>(matrix: &Array2<T>) -> Vec<usize> {
    matrix.columns().into_iter().map(lane_minimum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::arr2;

    #[test]
    fn brute_force_1x1() {
        let matrix = arr2(&[[2]]);
        let minima = vec![0];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_2x1() {
        let matrix = arr2(&[
            [3], //
            [2],
        ]);
        let minima = vec![0, 0];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_1x2() {
        let matrix = arr2(&[[2, 1]]);
        let minima = vec![1];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_2x2() {
        let matrix = arr2(&[
            [3, 2], //
            [2, 1],
        ]);
        let minima = vec![1, 1];
        assert_eq!(row_minima(&matrix), minima);
        assert_eq!(column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_3x3() {
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
    fn brute_force_4x4() {
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
    fn brute_force_5x5() {
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
