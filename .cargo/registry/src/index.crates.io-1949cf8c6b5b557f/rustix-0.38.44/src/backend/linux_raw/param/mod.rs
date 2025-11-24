// With "use-explicitly-provided-auxv" enabled, we expect to be initialized
// with an explicit `rustix::param::init` call.
//
// With "use-libc-auxv" enabled, use libc's `getauxval`.
//
// Otherwise, we read aux values from /proc/self/auxv.
#[cfg_attr(feature = "use-explicitly-provided-auxv", path = "init.rs")]
#[cfg_attr(
    all(
        not(feature = "use-explicitly-provided-auxv"),
        feature = "use-libc-auxv"
    ),
    path = "libc_auxv.rs"
)]
pub(crate) mod auxv;
