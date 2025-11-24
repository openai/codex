#![allow(dead_code)] // some code is tested for type checking only

use super::*;

#[test]
fn unit() {
    assert!(SimpleErr.source().is_none());
}

#[test]
fn named_implicit_no_source() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr {
        field: i32,
    }

    assert!(TestErr::default().source().is_none());
}

#[test]
fn named_implicit_source() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr {
        source: SimpleErr,
        field: i32,
    }

    let err = TestErr::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_explicit_no_source() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr {
        #[error(not(source))]
        source: SimpleErr,
        field: i32,
    }

    assert!(TestErr::default().source().is_none());
}

#[test]
fn named_explicit_source() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr {
        #[error(source)]
        explicit_source: SimpleErr,
        field: i32,
    }

    let err = TestErr::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_explicit_no_source_redundant() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr {
        #[error(not(source))]
        field: i32,
    }

    assert!(TestErr::default().source().is_none());
}

#[test]
fn named_explicit_source_redundant() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr {
        #[error(source)]
        source: SimpleErr,
        field: i32,
    }

    let err = TestErr::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_explicit_suppresses_implicit() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr {
        source: i32,
        #[error(source)]
        field: SimpleErr,
    }

    let err = TestErr::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn unnamed_implicit_no_source() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr(i32, i32);

    assert!(TestErr::default().source().is_none());
}

#[test]
fn unnamed_implicit_source() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr(SimpleErr);

    let err = TestErr::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn unnamed_explicit_no_source() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr(#[error(not(source))] SimpleErr);

    assert!(TestErr::default().source().is_none());
}

#[test]
fn unnamed_explicit_source() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr(#[error(source)] SimpleErr, i32);

    let err = TestErr::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn unnamed_explicit_no_source_redundant() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr(#[error(not(source))] i32, #[error(not(source))] i32);

    assert!(TestErr::default().source().is_none());
}

#[test]
fn unnamed_explicit_source_redundant() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr(#[error(source)] SimpleErr);

    let err = TestErr::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_ignore() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr {
        #[error(ignore)]
        source: SimpleErr,
        field: i32,
    }

    assert!(TestErr::default().source().is_none());
}

#[test]
fn unnamed_ignore() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr(#[error(ignore)] SimpleErr);

    assert!(TestErr::default().source().is_none());
}

#[test]
fn named_ignore_redundant() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr {
        #[error(ignore)]
        field: i32,
    }

    assert!(TestErr::default().source().is_none());
}

#[test]
fn unnamed_ignore_redundant() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    struct TestErr(#[error(ignore)] i32, #[error(ignore)] i32);

    assert!(TestErr::default().source().is_none());
}

#[test]
fn named_struct_ignore() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    #[error(ignore)]
    struct TestErr {
        source: SimpleErr,
        field: i32,
    }

    assert!(TestErr::default().source().is_none())
}

#[test]
fn unnamed_struct_ignore() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    #[error(ignore)]
    struct TestErr(SimpleErr);

    assert!(TestErr::default().source().is_none())
}

#[test]
fn named_struct_ignore_redundant() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    #[error(ignore)]
    struct TestErr {
        field: i32,
    }

    assert!(TestErr::default().source().is_none())
}

#[test]
fn unnamed_struct_ignore_redundant() {
    derive_display!(TestErr);
    #[derive(Default, Debug, Error)]
    #[error(ignore)]
    struct TestErr(i32, i32);

    assert!(TestErr::default().source().is_none())
}
