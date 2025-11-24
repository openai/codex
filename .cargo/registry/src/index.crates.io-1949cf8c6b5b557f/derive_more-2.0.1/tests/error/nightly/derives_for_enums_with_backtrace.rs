// For creating backtraces with different addresses in tests.
#![allow(clippy::redundant_closure, clippy::redundant_closure_call)] // for testing
#![allow(dead_code)] // some code is tested for type checking only

use core::error::{request_ref, request_value};

use super::*;

derive_display!(TestErr);
#[derive(Debug, Error)]
enum TestErr {
    Unit,
    NamedImplicitNoBacktrace {
        field: i32,
    },
    NamedImplicitBacktraceByFieldName {
        backtrace: MyBacktrace,
        field: i32,
    },
    NamedImplicitBacktraceByFieldType {
        implicit_backtrace: Backtrace,
        field: i32,
    },
    NamedExplicitNoBacktraceByFieldName {
        #[error(not(backtrace))]
        backtrace: MyBacktrace,
        field: i32,
    },
    NamedExplicitNoBacktraceByFieldType {
        #[error(not(backtrace))]
        implicit_backtrace: Backtrace,
        field: i32,
    },
    NamedExplicitBacktrace {
        #[error(backtrace)]
        explicit_backtrace: MyBacktrace,
        field: i32,
    },
    NamedExplicitNoBacktraceRedundant {
        #[error(not(backtrace))]
        not_backtrace: MyBacktrace,
        #[error(not(backtrace))]
        field: i32,
    },
    NamedExplicitBacktraceByFieldNameRedundant {
        #[error(backtrace)]
        backtrace: MyBacktrace,
        field: i32,
    },
    NamedExplicitBacktraceByFieldTypeRedundant {
        #[error(backtrace)]
        implicit_backtrace: Backtrace,
        field: i32,
    },
    NamedExplicitSuppressesImplicit {
        #[error(backtrace)]
        not_backtrace: MyBacktrace,
        backtrace: Backtrace,
        field: i32,
    },
    NamedImplicitNoBacktraceFromSource {
        #[error(source)]
        err: BacktraceErr,
    },
    NamedExplicitNoBacktraceFromSource {
        #[error(source, not(backtrace))]
        err: BacktraceErr,
    },
    NamedExplicitBacktraceFromSource {
        #[error(backtrace, source)]
        err: BacktraceErr,
    },
    NamedImplicitDifferentSourceAndBacktrace {
        #[error(source)]
        err: BacktraceErr,
        backtrace: Backtrace,
    },
    NamedExplicitDifferentSourceAndBacktrace {
        #[error(source)]
        err: BacktraceErr,
        #[error(backtrace)]
        backtrace: Backtrace,
    },
    UnnamedImplicitNoBacktrace(i32, i32),
    UnnamedImplicitBacktrace(Backtrace, i32, i32),
    UnnamedExplicitNoBacktrace(#[error(not(backtrace))] Backtrace, i32),
    UnnamedExplicitBacktrace(#[error(backtrace)] MyBacktrace, i32, i32),
    UnnamedExplicitNoBacktraceRedundant(
        #[error(not(backtrace))] MyBacktrace,
        #[error(not(backtrace))] i32,
    ),
    UnnamedExplicitBacktraceRedundant(#[error(backtrace)] Backtrace, i32, i32),
    UnnamedExplicitSuppressesImplicit(#[error(backtrace)] MyBacktrace, Backtrace, i32),
    UnnamedImplicitNoBacktraceFromSource(BacktraceErr),
    UnnamedExplicitNoBacktraceFromSource(#[error(not(backtrace))] BacktraceErr),
    UnnamedExplicitBacktraceFromSource(#[error(backtrace)] BacktraceErr),
    UnnamedImplicitDifferentSourceAndBacktrace(
        #[error(source)] BacktraceErr,
        Backtrace,
    ),
    UnnamedExplicitDifferentSourceAndBacktrace(
        #[error(source)] BacktraceErr,
        #[error(backtrace)] Backtrace,
    ),
}

impl TestErr {
    fn get_stored_backtrace(&self) -> &Backtrace {
        match self {
            Self::NamedImplicitBacktraceByFieldName { backtrace, .. }
            | Self::NamedImplicitBacktraceByFieldType {
                implicit_backtrace: backtrace,
                ..
            }
            | Self::NamedExplicitBacktrace {
                explicit_backtrace: backtrace,
                ..
            }
            | Self::NamedExplicitBacktraceByFieldNameRedundant { backtrace, .. }
            | Self::NamedExplicitBacktraceByFieldTypeRedundant {
                implicit_backtrace: backtrace,
                ..
            }
            | Self::NamedExplicitSuppressesImplicit {
                not_backtrace: backtrace,
                ..
            }
            | Self::NamedImplicitDifferentSourceAndBacktrace { backtrace, .. }
            | Self::NamedExplicitDifferentSourceAndBacktrace { backtrace, .. }
            | Self::UnnamedImplicitBacktrace(backtrace, _, _)
            | Self::UnnamedExplicitBacktrace(backtrace, _, _)
            | Self::UnnamedExplicitBacktraceRedundant(backtrace, _, _)
            | Self::UnnamedExplicitSuppressesImplicit(backtrace, _, _)
            | Self::UnnamedImplicitDifferentSourceAndBacktrace(_, backtrace)
            | Self::UnnamedExplicitDifferentSourceAndBacktrace(_, backtrace) => {
                backtrace
            }
            _ => panic!("ERROR IN TEST IMPLEMENTATION"),
        }
    }

    fn get_unused_backtrace(&self) -> &Backtrace {
        match self {
            Self::NamedExplicitSuppressesImplicit { backtrace, .. } => backtrace,
            Self::UnnamedExplicitSuppressesImplicit(_, backtrace, _) => backtrace,
            _ => panic!("ERROR IN TEST IMPLEMENTATION"),
        }
    }

    fn get_source_backtrace(&self) -> &Backtrace {
        request_ref(match self {
            Self::NamedExplicitBacktraceFromSource { err }
            | Self::NamedExplicitDifferentSourceAndBacktrace { err, .. }
            | Self::NamedImplicitDifferentSourceAndBacktrace { err, .. }
            | Self::UnnamedExplicitBacktraceFromSource(err)
            | Self::UnnamedExplicitDifferentSourceAndBacktrace(err, ..)
            | Self::UnnamedImplicitDifferentSourceAndBacktrace(err, ..) => err,
            _ => panic!("ERROR IN TEST IMPLEMENTATION"),
        })
        .unwrap()
    }
}

type MyBacktrace = Backtrace;

#[test]
fn unit() {
    assert!(request_ref::<Backtrace>(&TestErr::Unit).is_none());
}

#[test]
fn named_implicit_no_backtrace() {
    let err = TestErr::NamedImplicitNoBacktrace { field: 0 };

    assert!(request_ref::<Backtrace>(&err).is_none());
}

#[test]
fn named_implicit_backtrace_by_field_name() {
    let err = TestErr::NamedImplicitBacktraceByFieldName {
        backtrace: Backtrace::force_capture(),
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
}

#[test]
fn named_implicit_backtrace_by_field_type() {
    let err = TestErr::NamedImplicitBacktraceByFieldType {
        implicit_backtrace: Backtrace::force_capture(),
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
}

#[test]
fn named_explicit_no_backtrace_by_field_name() {
    let err = TestErr::NamedExplicitNoBacktraceByFieldName {
        backtrace: Backtrace::force_capture(),
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_none());
}

#[test]
fn named_explicit_no_backtrace_by_field_type() {
    let err = TestErr::NamedExplicitNoBacktraceByFieldType {
        implicit_backtrace: Backtrace::force_capture(),
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_none());
}

#[test]
fn named_explicit_backtrace() {
    let err = TestErr::NamedExplicitBacktrace {
        explicit_backtrace: Backtrace::force_capture(),
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
}

#[test]
fn named_explicit_no_backtrace_redundant() {
    let err = TestErr::NamedExplicitNoBacktraceRedundant {
        not_backtrace: Backtrace::force_capture(),
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_none());
}

#[test]
fn named_explicit_backtrace_by_field_name_redundant() {
    let err = TestErr::NamedExplicitBacktraceByFieldNameRedundant {
        backtrace: Backtrace::force_capture(),
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
}

#[test]
fn named_explicit_backtrace_by_field_type_redundant() {
    let err = TestErr::NamedExplicitBacktraceByFieldTypeRedundant {
        implicit_backtrace: Backtrace::force_capture(),
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
}

#[test]
fn named_explicit_suppresses_implicit() {
    let err = TestErr::NamedExplicitSuppressesImplicit {
        not_backtrace: Backtrace::force_capture(),
        backtrace: (|| Backtrace::force_capture())(), // ensure backtraces are different
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
    assert_bt!(!=, err, .get_unused_backtrace);
}

#[test]
fn named_implicit_no_backtrace_from_source() {
    let err = TestErr::NamedImplicitNoBacktraceFromSource {
        err: BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
    };

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_none());
    assert!(request_value::<i32>(&err).is_none());
}

#[test]
fn named_explicit_no_backtrace_from_source() {
    let err = TestErr::NamedExplicitNoBacktraceFromSource {
        err: BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
    };

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_none());
    assert!(request_value::<i32>(&err).is_none());
}

#[test]
fn named_explicit_backtrace_from_source() {
    let err = TestErr::NamedExplicitBacktraceFromSource {
        err: BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
    };

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, .get_source_backtrace);
}

#[test]
fn named_implicit_different_source_and_backtrace() {
    let err = TestErr::NamedImplicitDifferentSourceAndBacktrace {
        err: BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
        backtrace: (|| Backtrace::force_capture())(), // ensure backtraces are different
    };

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, .get_stored_backtrace);
    assert_bt!(!=, err, .get_source_backtrace);
}

#[test]
fn named_explicit_different_source_and_backtrace() {
    let err = TestErr::NamedExplicitDifferentSourceAndBacktrace {
        err: BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
        backtrace: (|| Backtrace::force_capture())(), // ensure backtraces are different
    };

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, .get_stored_backtrace);
    assert_bt!(!=, err, .get_source_backtrace);
}

#[test]
fn unnamed_implicit_no_backtrace() {
    let err = TestErr::UnnamedImplicitNoBacktrace(0, 0);

    assert!(request_ref::<Backtrace>(&err).is_none());
}

#[test]
fn unnamed_implicit_backtrace() {
    let err = TestErr::UnnamedImplicitBacktrace(Backtrace::force_capture(), 0, 0);

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
}

#[test]
fn unnamed_explicit_no_backtrace() {
    let err = TestErr::UnnamedExplicitNoBacktrace(Backtrace::force_capture(), 0);

    assert!(request_ref::<Backtrace>(&err).is_none());
}

#[test]
fn unnamed_explicit_backtrace() {
    let err = TestErr::UnnamedExplicitBacktrace(Backtrace::force_capture(), 0, 0);

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
}

#[test]
fn unnamed_explicit_no_backtrace_redundant() {
    let err =
        TestErr::UnnamedExplicitNoBacktraceRedundant(Backtrace::force_capture(), 0);

    assert!(request_ref::<Backtrace>(&err).is_none());
}

#[test]
fn unnamed_explicit_backtrace_redundant() {
    let err =
        TestErr::UnnamedExplicitBacktraceRedundant(Backtrace::force_capture(), 0, 0);

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
}

#[test]
fn unnamed_explicit_suppresses_implicit() {
    let err = TestErr::UnnamedExplicitSuppressesImplicit(
        Backtrace::force_capture(),
        (|| Backtrace::force_capture())(), // ensure backtraces are different
        0,
    );

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .get_stored_backtrace);
    assert_bt!(!=, err, .get_unused_backtrace);
}

#[test]
fn unnamed_implicit_no_backtrace_from_source() {
    let err = TestErr::UnnamedImplicitNoBacktraceFromSource(BacktraceErr {
        backtrace: Backtrace::force_capture(),
    });

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_none());
    assert!(request_value::<i32>(&err).is_none());
}

#[test]
fn unnamed_explicit_no_backtrace_from_source() {
    let err = TestErr::UnnamedExplicitNoBacktraceFromSource(BacktraceErr {
        backtrace: Backtrace::force_capture(),
    });

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_none());
    assert!(request_value::<i32>(&err).is_none());
}

#[test]
fn unnamed_explicit_backtrace_from_source() {
    let err = TestErr::UnnamedExplicitBacktraceFromSource(BacktraceErr {
        backtrace: Backtrace::force_capture(),
    });

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, .get_source_backtrace);
}

#[test]
fn unnamed_implicit_different_source_and_backtrace() {
    let err = TestErr::UnnamedImplicitDifferentSourceAndBacktrace(
        BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
        (|| Backtrace::force_capture())(), // ensure backtraces are different
    );

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, .get_stored_backtrace);
    assert_bt!(!=, err, .get_source_backtrace);
}

#[test]
fn unnamed_explicit_different_source_and_backtrace() {
    let err = TestErr::UnnamedExplicitDifferentSourceAndBacktrace(
        BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
        (|| Backtrace::force_capture())(), // ensure backtraces are different
    );

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, .get_stored_backtrace);
    assert_bt!(!=, err, .get_source_backtrace);
}
