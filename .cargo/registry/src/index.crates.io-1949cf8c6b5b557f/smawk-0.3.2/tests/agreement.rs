#![cfg(feature = "ndarray")]

use ndarray::{s, Array2};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use smawk::{brute_force, online_column_minima, recursive};

mod random_monge;
use random_monge::random_monge_matrix;

/// Check that the brute force, recursive, and SMAWK functions
/// give identical results on a large number of randomly generated
/// Monge matrices.
#[test]
fn column_minima_agree() {
    let sizes = vec![1, 2, 3, 4, 5, 10, 15, 20, 30];
    let mut rng = ChaCha20Rng::seed_from_u64(0);
    for _ in 0..4 {
        for m in sizes.clone().iter() {
            for n in sizes.clone().iter() {
                let matrix: Array2<i32> = random_monge_matrix(*m, *n, &mut rng);

                // Compute and test row minima.
                let brute_force = brute_force::row_minima(&matrix);
                let recursive = recursive::row_minima(&matrix);
                let smawk = smawk::row_minima(&matrix);
                assert_eq!(
                    brute_force, recursive,
                    "recursive and brute force differs on:\n{:?}",
                    matrix
                );
                assert_eq!(
                    brute_force, smawk,
                    "SMAWK and brute force differs on:\n{:?}",
                    matrix
                );

                // Do the same for the column minima.
                let brute_force = brute_force::column_minima(&matrix);
                let recursive = recursive::column_minima(&matrix);
                let smawk = smawk::column_minima(&matrix);
                assert_eq!(
                    brute_force, recursive,
                    "recursive and brute force differs on:\n{:?}",
                    matrix
                );
                assert_eq!(
                    brute_force, smawk,
                    "SMAWK and brute force differs on:\n{:?}",
                    matrix
                );
            }
        }
    }
}

/// Check that the brute force and online SMAWK functions give
/// identical results on a large number of randomly generated
/// Monge matrices.
#[test]
fn online_agree() {
    let sizes = vec![1, 2, 3, 4, 5, 10, 15, 20, 30, 50];
    let mut rng = ChaCha20Rng::seed_from_u64(0);
    for _ in 0..5 {
        for &size in &sizes {
            // Random totally monotone square matrix of the
            // desired size.
            let mut matrix: Array2<i32> = random_monge_matrix(size, size, &mut rng);

            // Adjust matrix so the column minima are above the
            // diagonal. The brute_force::column_minima will still
            // work just fine on such a mangled Monge matrix.
            let max = *matrix.iter().max().unwrap_or(&0);
            for idx in 0..(size as isize) {
                // Using the maximum value of the matrix instead
                // of i32::max_value() makes for prettier matrices
                // in case we want to print them.
                matrix.slice_mut(s![idx..idx + 1, ..idx + 1]).fill(max);
            }

            // The online algorithm always returns the initial
            // value for the left-most column -- without
            // inspecting the column at all. So we fill the
            // left-most column with this value to have the brute
            // force algorithm do the same.
            let initial = 42;
            matrix.slice_mut(s![0.., ..1]).fill(initial);

            // Brute-force computation of column minima, returned
            // in the same form as online_column_minima.
            let brute_force = brute_force::column_minima(&matrix)
                .iter()
                .enumerate()
                .map(|(j, &i)| (i, matrix[[i, j]]))
                .collect::<Vec<_>>();
            let online = online_column_minima(initial, size, |_, i, j| matrix[[i, j]]);
            assert_eq!(
                brute_force, online,
                "brute force and online differ on:\n{:3?}",
                matrix
            );
        }
    }
}
