use itertools::EitherOrBoth;
use itertools::free::merge_join_by;

#[test]
fn empty() {
    let left: Vec<u32> = vec![];
    let right: Vec<u32> = vec![];
    let expected_result: Vec<EitherOrBoth<u32, u32>> = vec![];
    let actual_result = merge_join_by(left, right, |l, r| l.cmp(r))
        .collect::<Vec<_>>();
    assert_eq!(expected_result, actual_result);
}

#[test]
fn left_only() {
    let left: Vec<u32> = vec![1,2,3];
    let right: Vec<u32> = vec![];
    let expected_result: Vec<EitherOrBoth<u32, u32>> = vec![
        EitherOrBoth::Left(1),
        EitherOrBoth::Left(2),
        EitherOrBoth::Left(3)
    ];
    let actual_result = merge_join_by(left, right, |l, r| l.cmp(r))
        .collect::<Vec<_>>();
    assert_eq!(expected_result, actual_result);
}

#[test]
fn right_only() {
    let left: Vec<u32> = vec![];
    let right: Vec<u32> = vec![1,2,3];
    let expected_result: Vec<EitherOrBoth<u32, u32>> = vec![
        EitherOrBoth::Right(1),
        EitherOrBoth::Right(2),
        EitherOrBoth::Right(3)
    ];
    let actual_result = merge_join_by(left, right, |l, r| l.cmp(r))
        .collect::<Vec<_>>();
    assert_eq!(expected_result, actual_result);
}

#[test]
fn first_left_then_right() {
    let left: Vec<u32> = vec![1,2,3];
    let right: Vec<u32> = vec![4,5,6];
    let expected_result: Vec<EitherOrBoth<u32, u32>> = vec![
        EitherOrBoth::Left(1),
        EitherOrBoth::Left(2),
        EitherOrBoth::Left(3),
        EitherOrBoth::Right(4),
        EitherOrBoth::Right(5),
        EitherOrBoth::Right(6)
    ];
    let actual_result = merge_join_by(left, right, |l, r| l.cmp(r))
        .collect::<Vec<_>>();
    assert_eq!(expected_result, actual_result);
}

#[test]
fn first_right_then_left() {
    let left: Vec<u32> = vec![4,5,6];
    let right: Vec<u32> = vec![1,2,3];
    let expected_result: Vec<EitherOrBoth<u32, u32>> = vec![
        EitherOrBoth::Right(1),
        EitherOrBoth::Right(2),
        EitherOrBoth::Right(3),
        EitherOrBoth::Left(4),
        EitherOrBoth::Left(5),
        EitherOrBoth::Left(6)
    ];
    let actual_result = merge_join_by(left, right, |l, r| l.cmp(r))
        .collect::<Vec<_>>();
    assert_eq!(expected_result, actual_result);
}

#[test]
fn interspersed_left_and_right() {
    let left: Vec<u32> = vec![1,3,5];
    let right: Vec<u32> = vec![2,4,6];
    let expected_result: Vec<EitherOrBoth<u32, u32>> = vec![
        EitherOrBoth::Left(1),
        EitherOrBoth::Right(2),
        EitherOrBoth::Left(3),
        EitherOrBoth::Right(4),
        EitherOrBoth::Left(5),
        EitherOrBoth::Right(6)
    ];
    let actual_result = merge_join_by(left, right, |l, r| l.cmp(r))
        .collect::<Vec<_>>();
    assert_eq!(expected_result, actual_result);
}

#[test]
fn overlapping_left_and_right() {
    let left: Vec<u32> = vec![1,3,4,6];
    let right: Vec<u32> = vec![2,3,4,5];
    let expected_result: Vec<EitherOrBoth<u32, u32>> = vec![
        EitherOrBoth::Left(1),
        EitherOrBoth::Right(2),
        EitherOrBoth::Both(3, 3),
        EitherOrBoth::Both(4, 4),
        EitherOrBoth::Right(5),
        EitherOrBoth::Left(6)
    ];
    let actual_result = merge_join_by(left, right, |l, r| l.cmp(r))
        .collect::<Vec<_>>();
    assert_eq!(expected_result, actual_result);
}
