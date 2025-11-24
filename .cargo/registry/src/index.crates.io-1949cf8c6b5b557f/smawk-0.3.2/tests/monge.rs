#![cfg(feature = "ndarray")]

use ndarray::{arr2, Array, Array2};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use smawk::monge::is_monge;

mod random_monge;
use random_monge::{random_monge_matrix, MongePrim};

#[test]
fn random_monge() {
    let mut rng = ChaCha20Rng::seed_from_u64(0);
    let matrix: Array2<u8> = random_monge_matrix(5, 5, &mut rng);

    assert!(is_monge(&matrix));
    assert_eq!(
        matrix,
        arr2(&[
            [2, 3, 4, 4, 5],
            [5, 5, 6, 6, 7],
            [3, 3, 4, 4, 5],
            [5, 2, 3, 3, 4],
            [5, 2, 3, 3, 4]
        ])
    );
}

#[test]
fn monge_constant_rows() {
    let mut rng = ChaCha20Rng::seed_from_u64(0);
    let matrix: Array2<u8> = MongePrim::ConstantRows.to_matrix(5, 4, &mut rng);
    assert!(is_monge(&matrix));
    for row in matrix.rows() {
        let elem = row[0];
        assert_eq!(row, Array::from_elem(matrix.ncols(), elem));
    }
}

#[test]
fn monge_constant_cols() {
    let mut rng = ChaCha20Rng::seed_from_u64(0);
    let matrix: Array2<u8> = MongePrim::ConstantCols.to_matrix(5, 4, &mut rng);
    assert!(is_monge(&matrix));
    for column in matrix.columns() {
        let elem = column[0];
        assert_eq!(column, Array::from_elem(matrix.nrows(), elem));
    }
}

#[test]
fn monge_upper_right_ones() {
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    let matrix: Array2<u8> = MongePrim::UpperRightOnes.to_matrix(5, 4, &mut rng);
    assert!(is_monge(&matrix));
    assert_eq!(
        matrix,
        arr2(&[
            [0, 0, 1, 1],
            [0, 0, 1, 1],
            [0, 0, 1, 1],
            [0, 0, 0, 0],
            [0, 0, 0, 0]
        ])
    );
}

#[test]
fn monge_lower_left_ones() {
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    let matrix: Array2<u8> = MongePrim::LowerLeftOnes.to_matrix(5, 4, &mut rng);
    assert!(is_monge(&matrix));
    assert_eq!(
        matrix,
        arr2(&[
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [1, 1, 0, 0],
            [1, 1, 0, 0],
            [1, 1, 0, 0]
        ])
    );
}
