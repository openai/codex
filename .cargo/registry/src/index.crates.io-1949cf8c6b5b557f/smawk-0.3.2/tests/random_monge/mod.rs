//! Test functionality for generating random Monge matrices.

// The code is put here so we can reuse it in different integration
// tests, without Cargo finding it when `cargo test` is run. See the
// section on "Submodules in Integration Tests" in
// https://doc.rust-lang.org/book/ch11-03-test-organization.html

use ndarray::{s, Array2};
use num_traits::PrimInt;
use rand::distributions::{Distribution, Standard};
use rand::Rng;

/// A Monge matrix can be decomposed into one of these primitive
/// building blocks.
#[derive(Copy, Clone)]
pub enum MongePrim {
    ConstantRows,
    ConstantCols,
    UpperRightOnes,
    LowerLeftOnes,
}

impl MongePrim {
    /// Generate a Monge matrix from a primitive.
    pub fn to_matrix<T: PrimInt, R: Rng>(&self, m: usize, n: usize, rng: &mut R) -> Array2<T>
    where
        Standard: Distribution<T>,
    {
        let mut matrix = Array2::from_elem((m, n), T::zero());
        // Avoid panic in UpperRightOnes and LowerLeftOnes below.
        if m == 0 || n == 0 {
            return matrix;
        }

        match *self {
            MongePrim::ConstantRows => {
                for mut row in matrix.rows_mut() {
                    if rng.gen::<bool>() {
                        row.fill(T::one())
                    }
                }
            }
            MongePrim::ConstantCols => {
                for mut col in matrix.columns_mut() {
                    if rng.gen::<bool>() {
                        col.fill(T::one())
                    }
                }
            }
            MongePrim::UpperRightOnes => {
                let i = rng.gen_range(0..(m + 1) as isize);
                let j = rng.gen_range(0..(n + 1) as isize);
                matrix.slice_mut(s![..i, -j..]).fill(T::one());
            }
            MongePrim::LowerLeftOnes => {
                let i = rng.gen_range(0..(m + 1) as isize);
                let j = rng.gen_range(0..(n + 1) as isize);
                matrix.slice_mut(s![-i.., ..j]).fill(T::one());
            }
        }

        matrix
    }
}

/// Generate a random Monge matrix.
pub fn random_monge_matrix<R: Rng, T: PrimInt>(m: usize, n: usize, rng: &mut R) -> Array2<T>
where
    Standard: Distribution<T>,
{
    let monge_primitives = [
        MongePrim::ConstantRows,
        MongePrim::ConstantCols,
        MongePrim::LowerLeftOnes,
        MongePrim::UpperRightOnes,
    ];
    let mut matrix = Array2::from_elem((m, n), T::zero());
    for _ in 0..(m + n) {
        let monge = monge_primitives[rng.gen_range(0..monge_primitives.len())];
        matrix = matrix + monge.to_matrix(m, n, rng);
    }
    matrix
}
