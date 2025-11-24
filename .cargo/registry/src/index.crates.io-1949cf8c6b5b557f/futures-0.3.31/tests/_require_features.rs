#[cfg(not(all(
    feature = "std",
    feature = "alloc",
    feature = "async-await",
    feature = "compat",
    feature = "io-compat",
    feature = "executor",
    feature = "thread-pool",
)))]
compile_error!(
    "`futures` tests must have all stable features activated: \
    use `--all-features` or `--features default,thread-pool,io-compat`"
);
