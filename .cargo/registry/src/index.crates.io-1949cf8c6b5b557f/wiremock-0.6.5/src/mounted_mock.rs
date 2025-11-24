use std::sync::{Arc, atomic::AtomicBool};

use tokio::sync::Notify;

use crate::{
    ErrorResponse, Match, Mock, Request, ResponseTemplate, verification::VerificationReport,
};

/// Given the behaviour specification as a [`Mock`], keep track of runtime information
/// concerning this mock - e.g. how many times it matched on a incoming request.
pub(crate) struct MountedMock {
    pub(crate) specification: Mock,
    n_matched_requests: u64,
    /// The position occupied by this mock within the parent [`MountedMockSet`](crate::mock_set::MountedMockSet)
    /// collection of `MountedMock`s.
    ///
    /// E.g. `0` if this is the first mock that we try to match against an incoming request, `1`
    /// if it is the second, etc.
    position_in_set: usize,

    // matched requests:
    matched_requests: Vec<crate::Request>,

    notify: Arc<(Notify, AtomicBool)>,
}

impl MountedMock {
    pub(crate) fn new(specification: Mock, position_in_set: usize) -> Self {
        Self {
            specification,
            n_matched_requests: 0,
            position_in_set,
            matched_requests: Vec::new(),
            notify: Arc::new((Notify::new(), AtomicBool::new(false))),
        }
    }

    /// This is NOT the same of `matches` from the `Match` trait!
    /// Key difference: we are talking a mutable reference to `self` in order to capture
    /// additional information (e.g. how many requests we matched so far) or change behaviour
    /// after a certain threshold has been crossed (e.g. start returning `false` for all requests
    /// once enough requests have been matched according to `max_n_matches`).
    pub(crate) fn matches(&mut self, request: &Request) -> bool {
        if Some(self.n_matched_requests) == self.specification.max_n_matches {
            // Skip the actual check if we are already at our maximum of matched requests.
            false
        } else {
            let matched = self
                .specification
                .matchers
                .iter()
                .all(|matcher| matcher.matches(request));

            if matched {
                // Increase match count
                self.n_matched_requests += 1;
                // Keep track of request
                self.matched_requests.push(request.clone());

                // notification of satisfaction
                if self.verify().is_satisfied() {
                    // always set the satisfaction flag **before** raising the event
                    self.notify
                        .1
                        .store(true, std::sync::atomic::Ordering::Release);
                    self.notify.0.notify_waiters();
                }
            }

            matched
        }
    }

    /// Verify if this mock has verified the expectations set at creation time
    /// over the number of invocations.
    pub(crate) fn verify(&self) -> VerificationReport {
        VerificationReport {
            mock_name: self.specification.name.clone(),
            n_matched_requests: self.n_matched_requests,
            expectation_range: self.specification.expectation_range.clone(),
            position_in_set: self.position_in_set,
        }
    }

    pub(crate) fn response_template(
        &self,
        request: &Request,
    ) -> Result<ResponseTemplate, ErrorResponse> {
        self.specification.response_template(request)
    }

    pub(crate) fn received_requests(&self) -> Vec<crate::Request> {
        self.matched_requests.clone()
    }

    pub(crate) fn notify(&self) -> Arc<(Notify, AtomicBool)> {
        self.notify.clone()
    }
}
