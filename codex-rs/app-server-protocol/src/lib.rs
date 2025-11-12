mod export;
mod jsonrpc_lite;
mod protocol;

pub use export::generate_json;
pub use export::generate_ts;
pub use export::generate_types;
pub use jsonrpc_lite::*;
pub use protocol::common::*;
pub use protocol::v1::*;
pub use protocol::v2::*;

#[cfg(test)]
mod test_junit_reporting {
    #[test]
    fn test_that_fails_for_junit_demo() {
        // TODO: This file is going to be removed prior to merge.
        // This is just to test that JUnit + GitHub annotations are working.
        assert_eq!(
            2 + 2,
            5,
            "This test intentionally fails to test JUnit reporting"
        );
    }
}
