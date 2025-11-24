// For creating backtraces with different addresses in tests.
#![allow(clippy::redundant_closure, clippy::redundant_closure_call)] // for testing
#![allow(dead_code)] // some code is tested for type checking only

use core::error::{request_ref, request_value};

use super::*;

#[test]
fn named_implicit_no_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<T> {
        field: T,
    }

    assert!(request_ref::<Backtrace>(&TestErr::<i32>::default()).is_none());
}

#[test]
fn named_implicit_backtrace_by_field_name() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        backtrace: MyBacktrace,
        field: T,
    }

    type MyBacktrace = Backtrace;

    let err = TestErr {
        backtrace: Backtrace::force_capture(),
        field: 0,
    };
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err);
}

#[test]
fn named_implicit_backtrace_by_field_type() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        implicit_backtrace: Backtrace,
        field: T,
    }

    let err = TestErr {
        implicit_backtrace: Backtrace::force_capture(),
        field: 0,
    };
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, implicit_backtrace);
}

#[test]
fn named_explicit_no_backtrace_by_field_name() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(not(backtrace))]
        backtrace: MyBacktrace,
        field: T,
    }

    type MyBacktrace = Backtrace;

    assert!(request_ref::<Backtrace>(&TestErr {
        backtrace: Backtrace::force_capture(),
        field: 0
    })
    .is_none());
}

#[test]
fn named_explicit_no_backtrace_by_field_type() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(not(backtrace))]
        implicit_backtrace: Backtrace,
        field: T,
    }

    assert!(request_ref::<Backtrace>(&TestErr {
        implicit_backtrace: Backtrace::force_capture(),
        field: 0
    })
    .is_none());
}

#[test]
fn named_explicit_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(backtrace)]
        explicit_backtrace: MyBacktrace,
        field: T,
    }

    type MyBacktrace = Backtrace;

    let err = TestErr {
        explicit_backtrace: Backtrace::force_capture(),
        field: 0,
    };
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, explicit_backtrace);
}

#[test]
fn named_explicit_no_backtrace_redundant() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(not(backtrace))]
        not_backtrace: MyBacktrace,
        #[error(not(backtrace))]
        field: T,
    }

    type MyBacktrace = Backtrace;

    assert!(request_ref::<Backtrace>(&TestErr {
        not_backtrace: Backtrace::force_capture(),
        field: 0
    })
    .is_none());
}

#[test]
fn named_explicit_backtrace_by_field_name_redundant() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(backtrace)]
        backtrace: MyBacktrace,
        field: T,
    }

    type MyBacktrace = Backtrace;

    let err = TestErr {
        backtrace: Backtrace::force_capture(),
        field: 0,
    };
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err);
}

#[test]
fn named_explicit_backtrace_by_field_type_redundant() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(backtrace)]
        implicit_backtrace: Backtrace,
        field: T,
    }

    let err = TestErr {
        implicit_backtrace: Backtrace::force_capture(),
        field: 0,
    };
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, implicit_backtrace);
}

#[test]
fn named_explicit_suppresses_implicit() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(backtrace)]
        not_backtrace: MyBacktrace,
        backtrace: Backtrace,
        field: T,
    }

    type MyBacktrace = Backtrace;

    let err = TestErr {
        not_backtrace: Backtrace::force_capture(),
        backtrace: (|| Backtrace::force_capture())(), // ensure backtraces are different
        field: 0,
    };

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, not_backtrace);
    assert_bt!(!=, err);
}

#[test]
fn named_implicit_no_backtrace_from_source() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(source)]
        err: T,
    }

    let err = TestErr {
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
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(source, not(backtrace))]
        err: T,
    }

    let err = TestErr {
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
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(backtrace, source)]
        err: T,
    }

    let err = TestErr {
        err: BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
    };

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, request_ref::<Backtrace>(&err.err).unwrap());
}

#[test]
fn named_implicit_different_source_and_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(source)]
        err: T,
        backtrace: Backtrace,
    }

    let err = TestErr {
        err: BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
        backtrace: (|| Backtrace::force_capture())(), // ensure backtraces are different
    };

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, backtrace);
    assert_bt!(!=, err, request_ref::<Backtrace>(&err.err).unwrap());
}

#[test]
fn named_explicit_different_source_and_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T> {
        #[error(source)]
        err: T,
        #[error(backtrace)]
        backtrace: Backtrace,
    }

    let err = TestErr {
        err: BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
        backtrace: (|| Backtrace::force_capture())(), // ensure backtraces are different
    };

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, backtrace);
    assert_bt!(!=, err, request_ref::<Backtrace>(&err.err).unwrap());
}

#[test]
fn unnamed_implicit_no_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Default, Debug, Error)]
    struct TestErr<T>(T, T);

    assert!(request_ref::<Backtrace>(&TestErr::<i32>::default()).is_none());
}

#[test]
fn unnamed_implicit_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(Backtrace, T, T);

    let err = TestErr(Backtrace::force_capture(), 0, 0);
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .0);
}

#[test]
fn unnamed_explicit_no_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(#[error(not(backtrace))] Backtrace, T);

    assert!(
        request_ref::<Backtrace>(&TestErr(Backtrace::force_capture(), 0)).is_none()
    );
}

#[test]
fn unnamed_explicit_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(#[error(backtrace)] MyBacktrace, T, T);

    type MyBacktrace = Backtrace;

    let err = TestErr(Backtrace::force_capture(), 0, 0);
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .0);
}

#[test]
fn unnamed_explicit_no_backtrace_redundant() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(
        #[error(not(backtrace))] MyBacktrace,
        #[error(not(backtrace))] T,
    );

    type MyBacktrace = Backtrace;

    assert!(
        request_ref::<Backtrace>(&TestErr(Backtrace::force_capture(), 0)).is_none()
    );
}

#[test]
fn unnamed_explicit_backtrace_redundant() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(#[error(backtrace)] Backtrace, T, T);

    let err = TestErr(Backtrace::force_capture(), 0, 0);
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .0);
}

#[test]
fn unnamed_explicit_suppresses_implicit() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(#[error(backtrace)] MyBacktrace, Backtrace, T);

    type MyBacktrace = Backtrace;

    let err = TestErr(
        Backtrace::force_capture(),
        (|| Backtrace::force_capture())(), // ensure backtraces are different
        0,
    );

    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_bt!(==, err, .0);
    assert_bt!(!=, err, .1);
}

#[test]
fn unnamed_implicit_no_backtrace_from_source() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(T);

    let err = TestErr(BacktraceErr {
        backtrace: Backtrace::force_capture(),
    });

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_none());
    assert!(request_value::<i32>(&err).is_none());
}

#[test]
fn unnamed_explicit_no_backtrace_from_source() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(#[error(not(backtrace))] T);

    let err = TestErr(BacktraceErr {
        backtrace: Backtrace::force_capture(),
    });

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_none());
    assert!(request_value::<i32>(&err).is_none());
}

#[test]
fn unnamed_explicit_backtrace_from_source() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(#[error(backtrace)] T);

    let err = TestErr(BacktraceErr {
        backtrace: Backtrace::force_capture(),
    });

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, request_ref::<Backtrace>(&err.0).unwrap());
}

#[test]
fn unnamed_implicit_different_source_and_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(#[error(source)] T, Backtrace);

    let err = TestErr(
        BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
        (|| Backtrace::force_capture())(), // ensure backtraces are different
    );

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, .1);
    assert_bt!(!=, err, request_ref::<Backtrace>(&err.0).unwrap());
}

#[test]
fn unnamed_explicit_different_source_and_backtrace() {
    derive_display!(TestErr, T);
    #[derive(Debug, Error)]
    struct TestErr<T>(#[error(source)] T, #[error(backtrace)] Backtrace);

    let err = TestErr(
        BacktraceErr {
            backtrace: Backtrace::force_capture(),
        },
        (|| Backtrace::force_capture())(), // ensure backtraces are different
    );

    assert!(err.source().is_some());
    assert!(request_ref::<Backtrace>(&err).is_some());
    assert_eq!(request_value::<i32>(&err), Some(42));
    assert_bt!(==, err, .1);
    assert_bt!(!=, err, request_ref::<Backtrace>(&err.0).unwrap());
}
