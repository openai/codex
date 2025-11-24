/// Some tests only make sense in contexts where they can re-exec the test
/// itself. Not all contexts support this, so you can call this method to find
/// out which case you are in.
pub fn cannot_reexec_the_test() -> bool {
    // These run in docker containers on CI where they can't re-exec the test,
    // so just skip these for CI. No other reason this can't run on those
    // platforms though.
    // Miri does not have support for re-execing a file
    cfg!(unix)
        && (cfg!(target_arch = "arm")
            || cfg!(target_arch = "aarch64")
            || cfg!(target_arch = "s390x"))
        || cfg!(miri)
}
