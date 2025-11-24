#![allow(dead_code)] // some code is tested for type checking only

use super::*;

derive_display!(TestErr, T, E);
#[derive(Debug, Error)]
enum TestErr<E, T> {
    Unit,
    NamedImplicitNoSource {
        field: T,
    },
    NamedImplicitSource {
        source: E,
        field: T,
    },
    NamedExplicitNoSource {
        #[error(not(source))]
        source: E,
        field: T,
    },
    NamedExplicitSource {
        #[error(source)]
        explicit_source: E,
        field: T,
    },
    NamedExplicitNoSourceRedundant {
        #[error(not(source))]
        field: T,
    },
    NamedExplicitSourceRedundant {
        #[error(source)]
        source: E,
        field: T,
    },
    NamedExplicitSuppressesImplicit {
        source: T,
        #[error(source)]
        field: E,
    },
    UnnamedImplicitNoSource(T, T),
    UnnamedImplicitSource(E),
    UnnamedExplicitNoSource(#[error(not(source))] E),
    UnnamedExplicitSource(#[error(source)] E, T),
    UnnamedExplicitNoSourceRedundant(#[error(not(source))] T, #[error(not(source))] T),
    UnnamedExplicitSourceRedundant(#[error(source)] E),
    NamedIgnore {
        #[error(ignore)]
        source: E,
        field: T,
    },
    UnnamedIgnore(#[error(ignore)] E),
    NamedIgnoreRedundant {
        #[error(ignore)]
        field: T,
    },
    UnnamedIgnoreRedundant(#[error(ignore)] T, #[error(ignore)] T),
    #[error(ignore)]
    NamedVariantIgnore {
        source: E,
        field: T,
    },
    #[error(ignore)]
    UnnamedVariantIgnore(E),
    #[error(ignore)]
    NamedVariantIgnoreRedundant {
        field: T,
    },
    #[error(ignore)]
    UnnamedVariantIgnoreRedundant(T, T),
}

#[test]
fn unit() {
    assert!(TestErr::<SimpleErr, i32>::Unit.source().is_none());
}

#[test]
fn named_implicit_no_source() {
    let err = TestErr::<SimpleErr, _>::NamedImplicitNoSource { field: 0 };

    assert!(err.source().is_none());
}

#[test]
fn named_implicit_source() {
    let err = TestErr::NamedImplicitSource {
        source: SimpleErr,
        field: 0,
    };

    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_explicit_no_source() {
    let err = TestErr::NamedExplicitNoSource {
        source: SimpleErr,
        field: 0,
    };

    assert!(err.source().is_none());
}

#[test]
fn named_explicit_source() {
    let err = TestErr::NamedExplicitSource {
        explicit_source: SimpleErr,
        field: 0,
    };

    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_explicit_no_source_redundant() {
    let err = TestErr::<SimpleErr, _>::NamedExplicitNoSourceRedundant { field: 0 };

    assert!(err.source().is_none());
}

#[test]
fn named_explicit_source_redundant() {
    let err = TestErr::NamedExplicitSourceRedundant {
        source: SimpleErr,
        field: 0,
    };

    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_explicit_suppresses_implicit() {
    let err = TestErr::NamedExplicitSuppressesImplicit {
        source: 0,
        field: SimpleErr,
    };

    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn unnamed_implicit_no_source() {
    let err = TestErr::<SimpleErr, _>::UnnamedImplicitNoSource(0, 0);

    assert!(err.source().is_none());
}

#[test]
fn unnamed_implicit_source() {
    let err = TestErr::<_, i32>::UnnamedImplicitSource(SimpleErr);

    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn unnamed_explicit_no_source() {
    let err = TestErr::<_, i32>::UnnamedExplicitNoSource(SimpleErr);

    assert!(err.source().is_none());
}

#[test]
fn unnamed_explicit_source() {
    let err = TestErr::UnnamedExplicitSource(SimpleErr, 0);

    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn unnamed_explicit_no_source_redundant() {
    let err = TestErr::<SimpleErr, _>::UnnamedExplicitNoSourceRedundant(0, 0);

    assert!(err.source().is_none());
}

#[test]
fn unnamed_explicit_source_redundant() {
    let err = TestErr::<_, i32>::UnnamedExplicitSourceRedundant(SimpleErr);

    assert!(err.source().is_some());
    assert!(err.source().unwrap().is::<SimpleErr>());
}

#[test]
fn named_ignore() {
    let err = TestErr::NamedIgnore {
        source: SimpleErr,
        field: 0,
    };

    assert!(err.source().is_none());
}

#[test]
fn unnamed_ignore() {
    let err = TestErr::<_, i32>::UnnamedIgnore(SimpleErr);

    assert!(err.source().is_none());
}

#[test]
fn named_ignore_redundant() {
    let err = TestErr::<SimpleErr, _>::NamedIgnoreRedundant { field: 0 };

    assert!(err.source().is_none());
}

#[test]
fn unnamed_ignore_redundant() {
    let err = TestErr::<SimpleErr, _>::UnnamedIgnoreRedundant(0, 0);

    assert!(err.source().is_none());
}

#[test]
fn named_variant_ignore() {
    let err = TestErr::NamedVariantIgnore {
        source: SimpleErr,
        field: 0,
    };

    assert!(err.source().is_none());
}

#[test]
fn unnamed_variant_ignore() {
    let err = TestErr::<_, i32>::UnnamedVariantIgnore(SimpleErr);

    assert!(err.source().is_none())
}

#[test]
fn named_variant_ignore_redundant() {
    let err = TestErr::<SimpleErr, _>::NamedVariantIgnoreRedundant { field: 0 };

    assert!(err.source().is_none());
}

#[test]
fn unnamed_variant_ignore_redundant() {
    let err = TestErr::<SimpleErr, _>::UnnamedVariantIgnoreRedundant(0, 0);

    assert!(err.source().is_none())
}
