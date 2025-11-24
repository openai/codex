use indexmap::{indexmap, indexset};

#[test]
fn test_sort() {
    let m = indexmap! {
        1 => 2,
        7 => 1,
        2 => 2,
        3 => 3,
    };

    itertools::assert_equal(
        m.sorted_by(|_k1, v1, _k2, v2| v1.cmp(v2)),
        vec![(7, 1), (1, 2), (2, 2), (3, 3)],
    );
}

#[test]
fn test_sort_set() {
    let s = indexset! {
        1,
        7,
        2,
        3,
    };

    itertools::assert_equal(s.sorted_by(|v1, v2| v1.cmp(v2)), vec![1, 2, 3, 7]);
}
