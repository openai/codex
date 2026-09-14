//! Keep command outcomes distinct without publishing arbitrary exit-code metric labels.

use super::command_outcome;
use pretty_assertions::assert_eq;

#[test]
fn command_results_distinguish_timeouts_transport_errors_and_nonzero_exits() {
    assert_eq!(
        [
            Some((0, false)),
            Some((1, false)),
            Some((0xc0000135u32 as i32, false)), // Windows DLL-not-found status.
            Some((0, true)),
            Some((1, true)),
            None,
        ]
        .map(command_outcome),
        [
            "success",
            "nonzero_exit",
            "nonzero_exit",
            "timeout",
            "timeout",
            "transport_error",
        ]
    );
}
