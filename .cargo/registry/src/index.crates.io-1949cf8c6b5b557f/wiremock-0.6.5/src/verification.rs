use crate::mock::Times;

/// A report returned by an `MountedMock` detailing what the user expectations were and
/// how many calls were actually received since the mock was mounted on the server.
#[derive(Clone)]
pub(crate) struct VerificationReport {
    /// The mock name specified by the user.
    pub(crate) mock_name: Option<String>,
    /// What users specified
    pub(crate) expectation_range: Times,
    /// Actual number of received requests that matched the specification
    pub(crate) n_matched_requests: u64,
    /// The position occupied by the mock that generated the report within its parent
    /// [`MountedMockSet`](crate::mock_set::MountedMockSet) collection of `MountedMock`s.
    ///
    /// E.g. `0` if it is the first mock that we try to match against an incoming request, `1`
    /// if it is the second, etc.
    pub(crate) position_in_set: usize,
}

impl VerificationReport {
    pub(crate) fn error_message(&self) -> String {
        if let Some(ref mock_name) = self.mock_name {
            format!(
                "{}.\n\tExpected range of matching incoming requests: {}\n\tNumber of matched incoming requests: {}",
                mock_name, self.expectation_range, self.n_matched_requests
            )
        } else {
            format!(
                "Mock #{}.\n\tExpected range of matching incoming requests: {}\n\tNumber of matched incoming requests: {}",
                self.position_in_set, self.expectation_range, self.n_matched_requests
            )
        }
    }

    pub(crate) fn is_satisfied(&self) -> bool {
        self.expectation_range.contains(self.n_matched_requests)
    }
}

pub(crate) enum VerificationOutcome {
    /// The expectations set on all active mocks were satisfied.
    Success,
    /// The expectations set for one or more of the active mocks were not satisfied.
    /// All failed expectations are returned.
    Failure(Vec<VerificationReport>),
}
