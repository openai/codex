use log::{log, log_enabled, Log, Metadata, Record};

macro_rules! all_log_macros {
    ($($arg:tt)*) => ({
        ::log::trace!($($arg)*);
        ::log::debug!($($arg)*);
        ::log::info!($($arg)*);
        ::log::warn!($($arg)*);
        ::log::error!($($arg)*);
    });
}

// Not `Copy`
struct Logger;

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        false
    }
    fn log(&self, _: &Record) {}
    fn flush(&self) {}
}

#[test]
fn no_args() {
    let logger = Logger;

    for lvl in log::Level::iter() {
        log!(lvl, "hello");
        log!(lvl, "hello",);

        log!(target: "my_target", lvl, "hello");
        log!(target: "my_target", lvl, "hello",);

        log!(logger: logger, lvl, "hello");
        log!(logger: logger, lvl, "hello",);

        log!(logger: logger, target: "my_target", lvl, "hello");
        log!(logger: logger, target: "my_target", lvl, "hello",);
    }

    all_log_macros!("hello");
    all_log_macros!("hello",);

    all_log_macros!(target: "my_target", "hello");
    all_log_macros!(target: "my_target", "hello",);

    all_log_macros!(logger: logger, "hello");
    all_log_macros!(logger: logger, "hello",);

    all_log_macros!(logger: logger, target: "my_target", "hello");
    all_log_macros!(logger: logger, target: "my_target", "hello",);
}

#[test]
fn anonymous_args() {
    for lvl in log::Level::iter() {
        log!(lvl, "hello {}", "world");
        log!(lvl, "hello {}", "world",);

        log!(target: "my_target", lvl, "hello {}", "world");
        log!(target: "my_target", lvl, "hello {}", "world",);

        log!(lvl, "hello {}", "world");
        log!(lvl, "hello {}", "world",);
    }

    all_log_macros!("hello {}", "world");
    all_log_macros!("hello {}", "world",);

    all_log_macros!(target: "my_target", "hello {}", "world");
    all_log_macros!(target: "my_target", "hello {}", "world",);

    let logger = Logger;

    all_log_macros!(logger: logger, "hello {}", "world");
    all_log_macros!(logger: logger, "hello {}", "world",);

    all_log_macros!(logger: logger, target: "my_target", "hello {}", "world");
    all_log_macros!(logger: logger, target: "my_target", "hello {}", "world",);
}

#[test]
fn named_args() {
    for lvl in log::Level::iter() {
        log!(lvl, "hello {world}", world = "world");
        log!(lvl, "hello {world}", world = "world",);

        log!(target: "my_target", lvl, "hello {world}", world = "world");
        log!(target: "my_target", lvl, "hello {world}", world = "world",);

        log!(lvl, "hello {world}", world = "world");
        log!(lvl, "hello {world}", world = "world",);
    }

    all_log_macros!("hello {world}", world = "world");
    all_log_macros!("hello {world}", world = "world",);

    all_log_macros!(target: "my_target", "hello {world}", world = "world");
    all_log_macros!(target: "my_target", "hello {world}", world = "world",);

    let logger = Logger;

    all_log_macros!(logger: logger, "hello {world}", world = "world");
    all_log_macros!(logger: logger, "hello {world}", world = "world",);

    all_log_macros!(logger: logger, target: "my_target", "hello {world}", world = "world");
    all_log_macros!(logger: logger, target: "my_target", "hello {world}", world = "world",);
}

#[test]
fn inlined_args() {
    let world = "world";

    for lvl in log::Level::iter() {
        log!(lvl, "hello {world}");
        log!(lvl, "hello {world}",);

        log!(target: "my_target", lvl, "hello {world}");
        log!(target: "my_target", lvl, "hello {world}",);

        log!(lvl, "hello {world}");
        log!(lvl, "hello {world}",);
    }

    all_log_macros!("hello {world}");
    all_log_macros!("hello {world}",);

    all_log_macros!(target: "my_target", "hello {world}");
    all_log_macros!(target: "my_target", "hello {world}",);

    let logger = Logger;

    all_log_macros!(logger: logger, "hello {world}");
    all_log_macros!(logger: logger, "hello {world}",);

    all_log_macros!(logger: logger, target: "my_target", "hello {world}");
    all_log_macros!(logger: logger, target: "my_target", "hello {world}",);
}

#[test]
fn enabled() {
    let logger = Logger;

    for lvl in log::Level::iter() {
        let _enabled = log_enabled!(lvl);
        let _enabled = log_enabled!(target: "my_target", lvl);
        let _enabled = log_enabled!(logger: logger, target: "my_target", lvl);
        let _enabled = log_enabled!(logger: logger, lvl);
    }
}

#[test]
fn expr() {
    let logger = Logger;

    for lvl in log::Level::iter() {
        log!(lvl, "hello");

        log!(logger: logger, lvl, "hello");
    }
}

#[test]
#[cfg(feature = "kv")]
fn kv_no_args() {
    let logger = Logger;

    for lvl in log::Level::iter() {
        log!(target: "my_target", lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello");
        log!(lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello");

        log!(logger: logger, target: "my_target", lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello");
        log!(logger: logger, lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello");
    }

    all_log_macros!(target: "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello");
    all_log_macros!(target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello");
    all_log_macros!(cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello");

    all_log_macros!(logger: logger, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello");
    all_log_macros!(logger: logger, target: "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello");
}

#[test]
#[cfg(feature = "kv")]
fn kv_expr_args() {
    let logger = Logger;

    for lvl in log::Level::iter() {
        log!(target: "my_target", lvl, cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");

        log!(lvl, target = "my_target", cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");
        log!(lvl, cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");

        log!(logger: logger, target: "my_target", lvl, cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");

        log!(logger: logger, lvl, target = "my_target", cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");
        log!(logger: logger, lvl, cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");
    }

    all_log_macros!(target: "my_target", cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");
    all_log_macros!(target = "my_target", cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");
    all_log_macros!(cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");

    all_log_macros!(logger: logger, target: "my_target", cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");
    all_log_macros!(logger: logger, target = "my_target", cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");
    all_log_macros!(logger: logger, cat_math = { let mut x = 0; x += 1; x + 1 }; "hello");
}

#[test]
#[cfg(feature = "kv")]
fn kv_anonymous_args() {
    let logger = Logger;

    for lvl in log::Level::iter() {
        log!(target: "my_target", lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");
        log!(lvl, target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");

        log!(lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");

        log!(logger: logger, target: "my_target", lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");
        log!(logger: logger, lvl, target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");

        log!(logger: logger, lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");
    }

    all_log_macros!(target: "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");
    all_log_macros!(target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");
    all_log_macros!(cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");

    all_log_macros!(logger: logger, target: "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");
    all_log_macros!(logger: logger, target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");
    all_log_macros!(logger: logger, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {}", "world");
}

#[test]
#[cfg(feature = "kv")]
fn kv_named_args() {
    let logger = Logger;

    for lvl in log::Level::iter() {
        log!(target: "my_target", lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");
        log!(lvl, target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");

        log!(lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");

        log!(logger: logger, target: "my_target", lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");
        log!(logger: logger, lvl, target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");

        log!(logger: logger, lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");
    }

    all_log_macros!(target: "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");
    all_log_macros!(target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");
    all_log_macros!(cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");

    all_log_macros!(logger: logger, target: "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");
    all_log_macros!(logger: logger, target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");
    all_log_macros!(logger: logger, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}", world = "world");
}

#[test]
#[cfg(feature = "kv")]
fn kv_ident() {
    let cat_1 = "chashu";
    let cat_2 = "nori";

    all_log_macros!(cat_1, cat_2:%, cat_count = 2; "hello {world}", world = "world");
}

#[test]
#[cfg(feature = "kv")]
fn kv_expr_context() {
    match "chashu" {
        cat_1 => {
            log::info!(target: "target", cat_1 = cat_1, cat_2 = "nori"; "hello {}", "cats");
        }
    };
}

#[test]
fn implicit_named_args() {
    let world = "world";

    for lvl in log::Level::iter() {
        log!(lvl, "hello {world}");
        log!(lvl, "hello {world}",);

        log!(target: "my_target", lvl, "hello {world}");
        log!(target: "my_target", lvl, "hello {world}",);

        log!(lvl, "hello {world}");
        log!(lvl, "hello {world}",);
    }

    all_log_macros!("hello {world}");
    all_log_macros!("hello {world}",);

    all_log_macros!(target: "my_target", "hello {world}");
    all_log_macros!(target: "my_target", "hello {world}",);

    #[cfg(feature = "kv")]
    all_log_macros!(target = "my_target"; "hello {world}");
    #[cfg(feature = "kv")]
    all_log_macros!(target = "my_target"; "hello {world}",);
}

#[test]
#[cfg(feature = "kv")]
fn kv_implicit_named_args() {
    let world = "world";

    for lvl in log::Level::iter() {
        log!(target: "my_target", lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}");

        log!(lvl, cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}");
    }

    all_log_macros!(target: "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}");
    all_log_macros!(target = "my_target", cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}");
    all_log_macros!(cat_1 = "chashu", cat_2 = "nori", cat_count = 2; "hello {world}");
}

#[test]
#[cfg(feature = "kv")]
fn kv_string_keys() {
    for lvl in log::Level::iter() {
        log!(target: "my_target", lvl, "also dogs" = "Fílos", "key/that-can't/be/an/ident" = "hi"; "hello {world}", world = "world");
    }

    all_log_macros!(target: "my_target", "also dogs" = "Fílos", "key/that-can't/be/an/ident" = "hi"; "hello {world}", world = "world");
}

#[test]
#[cfg(feature = "kv")]
fn kv_common_value_types() {
    all_log_macros!(
        u8 = 42u8,
        u16 = 42u16,
        u32 = 42u32,
        u64 = 42u64,
        u128 = 42u128,
        i8 = -42i8,
        i16 = -42i16,
        i32 = -42i32,
        i64 = -42i64,
        i128 = -42i128,
        f32 = 4.2f32,
        f64 = -4.2f64,
        bool = true,
        str = "string";
        "hello world"
    );
}

#[test]
#[cfg(feature = "kv")]
fn kv_debug() {
    all_log_macros!(
        a:? = 42,
        b:debug = 42;
        "hello world"
    );
}

#[test]
#[cfg(feature = "kv")]
fn kv_display() {
    all_log_macros!(
        a:% = 42,
        b:display = 42;
        "hello world"
    );
}

#[test]
#[cfg(feature = "kv_std")]
fn kv_error() {
    all_log_macros!(
        a:err = std::io::Error::new(std::io::ErrorKind::Other, "an error");
        "hello world"
    );
}

#[test]
#[cfg(feature = "kv_sval")]
fn kv_sval() {
    all_log_macros!(
        a:sval = 42;
        "hello world"
    );
}

#[test]
#[cfg(feature = "kv_serde")]
fn kv_serde() {
    all_log_macros!(
        a:serde = 42;
        "hello world"
    );
}

#[test]
fn logger_short_lived() {
    all_log_macros!(logger: Logger, "hello");
    all_log_macros!(logger: &Logger, "hello");
}

#[test]
fn logger_expr() {
    all_log_macros!(logger: {
        let logger = Logger;
        logger
    }, "hello");
}

/// Some and None (from Option) are used in the macros.
#[derive(Debug)]
enum Type {
    Some,
    None,
}

#[test]
fn regression_issue_494() {
    use self::Type::*;
    all_log_macros!("some message: {:?}, {:?}", None, Some);
}
