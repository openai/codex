#![allow(dead_code)] // some code is tested for type checking only

use super::*;

#[test]
fn named_implicit_no_source() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<T> {
        field: T,
    }

    assert!(TestErr::<i32>::default().source().is_none());
}

#[test]
fn named_implicit_source() {
    derive_display!(TestErr, E, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<E, T> {
        source: E,
        field: T,
    }

    let err = TestErr::<SimpleErr, i32>::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_explicit_no_source() {
    derive_display!(TestErr, E, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<E, T> {
        #[error(not(source))]
        source: E,
        field: T,
    }

    let err = TestErr::<SimpleErr, i32>::default();
    assert!(err.source().is_none());
}

#[test]
fn named_explicit_source() {
    derive_display!(TestErr, E, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<E, T> {
        #[error(source)]
        explicit_source: E,
        field: T,
    }

    let err = TestErr::<SimpleErr, i32>::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_explicit_no_source_redundant() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<T> {
        #[error(not(source))]
        field: T,
    }

    assert!(TestErr::<i32>::default().source().is_none());
}

#[test]
fn named_explicit_source_redundant() {
    derive_display!(TestErr, E, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<E, T> {
        #[error(source)]
        source: E,
        field: T,
    }

    let err = TestErr::<SimpleErr, i32>::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_explicit_suppresses_implicit() {
    derive_display!(TestErr, E, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<E, T> {
        source: E,
        #[error(source)]
        field: T,
    }

    let err = TestErr::<i32, SimpleErr>::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn unnamed_implicit_no_source() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<T>(T, T);

    assert!(TestErr::<i32>::default().source().is_none());
}

#[test]
fn unnamed_implicit_source() {
    derive_display!(TestErr, E);
    #[derive(Default, Debug, Error)]
    struct TestErr<E>(E);

    let err = TestErr::<SimpleErr>::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn unnamed_explicit_no_source() {
    derive_display!(TestErr, E);
    #[derive(Default, Debug, Error)]
    struct TestErr<E>(#[error(not(source))] E);

    assert!(TestErr::<SimpleErr>::default().source().is_none());
}

#[test]
fn unnamed_explicit_source() {
    derive_display!(TestErr, E, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<E, T>(#[error(source)] E, T);

    let err = TestErr::<SimpleErr, i32>::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn unnamed_explicit_no_source_redundant() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<T>(#[error(not(source))] T, #[error(not(source))] T);

    assert!(TestErr::<i32>::default().source().is_none());
}

#[test]
fn unnamed_explicit_source_redundant() {
    derive_display!(TestErr, E);
    #[derive(Default, Debug, Error)]
    struct TestErr<E>(#[error(source)] E);

    let err = TestErr::<SimpleErr>::default();
    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_ignore() {
    derive_display!(TestErr, E, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<E, T> {
        #[error(ignore)]
        source: E,
        field: T,
    }

    assert!(TestErr::<SimpleErr, i32>::default().source().is_none());
}

#[test]
fn unnamed_ignore() {
    derive_display!(TestErr, E);
    #[derive(Default, Debug, Error)]
    struct TestErr<E>(#[error(ignore)] E);

    assert!(TestErr::<SimpleErr>::default().source().is_none());
}

#[test]
fn named_ignore_redundant() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<T> {
        #[error(ignore)]
        field: T,
    }

    assert!(TestErr::<i32>::default().source().is_none());
}

#[test]
fn unnamed_ignore_redundant() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<T>(#[error(ignore)] T, #[error(ignore)] T);

    assert!(TestErr::<i32>::default().source().is_none());
}

#[test]
fn named_struct_ignore() {
    derive_display!(TestErr, E, T);
    #[derive(Default, Debug, Error)]
    #[error(ignore)]
    struct TestErr<E, T> {
        source: E,
        field: T,
    }

    assert!(TestErr::<SimpleErr, i32>::default().source().is_none())
}

#[test]
fn unnamed_struct_ignore() {
    derive_display!(TestErr, E);
    #[derive(Default, Debug, Error)]
    #[error(ignore)]
    struct TestErr<E>(E);

    assert!(TestErr::<SimpleErr>::default().source().is_none())
}

#[test]
fn named_struct_ignore_redundant() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    #[error(ignore)]
    struct TestErr<T> {
        field: T,
    }

    assert!(TestErr::<i32>::default().source().is_none())
}

#[test]
fn unnamed_struct_ignore_redundant() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    #[error(ignore)]
    struct TestErr<T>(T, T);

    assert!(TestErr::<i32>::default().source().is_none())
}
