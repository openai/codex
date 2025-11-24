/// This file tests `smallvec!` without actually having the macro in scope.
/// This forces any recursion to use a `$crate` prefix to reliably find itself.

#[test]
fn smallvec() {
    let mut vec: smallvec::SmallVec<[i32; 2]>;

    macro_rules! check {
        ($init:tt) => {
            vec = smallvec::smallvec! $init;
            assert_eq!(*vec, *vec! $init);
        }
    }

    check!([0; 0]);
    check!([1; 1]);
    check!([2; 2]);
    check!([3; 3]);

    check!([]);
    check!([1]);
    check!([1, 2]);
    check!([1, 2, 3]);
}
