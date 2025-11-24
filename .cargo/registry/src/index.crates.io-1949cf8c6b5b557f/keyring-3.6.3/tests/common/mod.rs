#![allow(dead_code)] // not all of these utilities are used by all tests

/// When tests fail, they leave keys behind, and those keys
/// have to be cleaned up before the tests can be run again
/// in order to avoid bad results.  So it's a lot easier just
/// to have tests use a random string for key names to avoid
/// the conflicts, and then do any needed cleanup once everything
/// is working correctly.  So tests make keys with these functions.
/// When tests fail, they leave keys behind, and those keys
/// have to be cleaned up before the tests can be run again
/// in order to avoid bad results.  So it's a lot easier just
/// to have tests use a random string for key names to avoid
/// the conflicts, and then do any needed cleanup once everything
/// is working correctly.  So we export this function for tests to use.
pub fn generate_random_string_of_len(len: usize) -> String {
    use fastrand;
    use std::iter::repeat_with;
    repeat_with(fastrand::alphanumeric).take(len).collect()
}

pub fn generate_random_string() -> String {
    generate_random_string_of_len(30)
}

pub fn generate_random_bytes_of_len(len: usize) -> Vec<u8> {
    use fastrand;
    use std::iter::repeat_with;
    repeat_with(|| fastrand::u8(..)).take(len).collect()
}

pub fn init_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}
