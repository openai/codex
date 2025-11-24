use crate::prelude::*;
use either1::Either;

#[test]
fn either() {
    test!(Either<i32, Either<bool, ()>>)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            Either::Left(123),
            Either::Right(Either::Left(true)),
            Either::Right(Either::Right(())),
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}
