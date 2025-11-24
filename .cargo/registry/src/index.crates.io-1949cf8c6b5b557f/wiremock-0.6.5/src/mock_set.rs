use crate::request::BodyPrintLimit;
use crate::{
    ErrorResponse,
    mounted_mock::MountedMock,
    verification::{VerificationOutcome, VerificationReport},
};
use crate::{Mock, Request};
use http_body_util::Full;
use hyper::body::Bytes;
use log::debug;
use std::{
    ops::{Index, IndexMut},
    sync::{Arc, atomic::AtomicBool},
};
use tokio::sync::Notify;
use tokio::time::{Sleep, sleep};

/// The collection of mocks used by a `MockServer` instance to match against
/// incoming requests.
///
/// New mocks are added to `MountedMockSet` every time [`MockServer::register`](crate::MockServer::register),
/// [`MockServer::register_as_scoped`](crate::MockServer::register_as_scoped) or
/// [`Mock::mount`](crate::Mock::mount) are called.
pub(crate) struct MountedMockSet {
    mocks: Vec<(MountedMock, MountedMockState)>,
    /// A counter that keeps track of how many times [`MountedMockSet::reset`] has been called.
    /// It starts at `0` and gets incremented for each invocation.
    ///
    /// We need `generation` to know if a [`MockId`] points to an [`MountedMock`] that has been
    /// removed via [`MountedMockSet::reset`].
    generation: u16,
    body_print_limit: BodyPrintLimit,
}

/// A `MockId` is an opaque index that uniquely identifies an [`MountedMock`] inside an [`MountedMockSet`].
///
/// The only way to create a `MockId` is calling [`MountedMockSet::register`].
#[derive(Copy, Clone)]
pub(crate) struct MockId {
    index: usize,
    /// The generation of [`MountedMockSet`] when [`MountedMockSet::register`] was called.
    /// It allows [`MountedMockSet`] to check that the [`MountedMock`] our [`MockId`] points to is still in
    /// the set (i.e. the set has not been wiped by a [`MountedMockSet::reset`] call).
    generation: u16,
}

impl MountedMockSet {
    /// Create a new instance of `MountedMockSet`.
    pub(crate) fn new(body_print_limit: BodyPrintLimit) -> MountedMockSet {
        MountedMockSet {
            mocks: vec![],
            generation: 0,
            body_print_limit,
        }
    }

    pub(crate) async fn handle_request(
        &mut self,
        request: Request,
    ) -> Result<(hyper::Response<Full<Bytes>>, Option<Sleep>), ErrorResponse> {
        debug!("Handling request.");
        let mut response_template: Option<_> = None;
        self.mocks.sort_by_key(|(m, _)| m.specification.priority);
        for (mock, mock_state) in &mut self.mocks {
            if *mock_state == MountedMockState::OutOfScope {
                continue;
            }
            if mock.matches(&request) {
                response_template = Some(mock.response_template(&request));
                break;
            }
        }
        if let Some(response_template) = response_template {
            match response_template {
                Ok(response_template) => {
                    let delay = response_template.delay().map(sleep);
                    Ok((response_template.generate_response(), delay))
                }
                Err(err) => Err(err),
            }
        } else {
            let mut msg = "Got unexpected request:\n".to_string();
            _ = request.print_with_limit(&mut msg, self.body_print_limit);
            debug!("{}", msg);
            let not_found_response = hyper::Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                .body(Full::default())
                .unwrap();
            Ok((not_found_response, None))
        }
    }

    pub(crate) fn register(&mut self, mock: Mock) -> (Arc<(Notify, AtomicBool)>, MockId) {
        let n_registered_mocks = self.mocks.len();
        let active_mock = MountedMock::new(mock, n_registered_mocks);
        let notify = active_mock.notify();
        self.mocks.push((active_mock, MountedMockState::InScope));
        (
            notify,
            MockId {
                index: self.mocks.len() - 1,
                generation: self.generation,
            },
        )
    }

    pub(crate) fn reset(&mut self) {
        self.mocks = vec![];
        self.generation += 1;
    }

    /// Mark one of the mocks in the set as out of scope.
    ///
    /// It will stop matching against incoming requests, regardless of its specification.
    pub(crate) fn deactivate(&mut self, mock_id: MockId) {
        let mock = &mut self[mock_id];
        mock.1 = MountedMockState::OutOfScope;
    }

    /// Verify that expectations have been met for **all** [`MountedMock`]s in the set.
    pub(crate) fn verify_all(&self) -> VerificationOutcome {
        let failed_verifications: Vec<VerificationReport> = self
            .mocks
            .iter()
            .filter(|(_, state)| *state == MountedMockState::InScope)
            .map(|(m, _)| m.verify())
            .filter(|verification_report| !verification_report.is_satisfied())
            .collect();
        if failed_verifications.is_empty() {
            VerificationOutcome::Success
        } else {
            VerificationOutcome::Failure(failed_verifications)
        }
    }

    /// Verify that expectations have been met for the [`MountedMock`] corresponding to the specified [`MockId`].
    pub(crate) fn verify(&self, mock_id: MockId) -> VerificationReport {
        let (mock, _) = &self[mock_id];
        mock.verify()
    }
}

impl IndexMut<MockId> for MountedMockSet {
    fn index_mut(&mut self, index: MockId) -> &mut Self::Output {
        if index.generation != self.generation {
            panic!(
                "The mock you are trying to access is no longer active. It has been deleted from the active set via `reset` - you should not hold on to a `MockId` after you call `reset`!."
            )
        }
        &mut self.mocks[index.index]
    }
}

impl Index<MockId> for MountedMockSet {
    type Output = (MountedMock, MountedMockState);

    fn index(&self, index: MockId) -> &Self::Output {
        if index.generation != self.generation {
            panic!(
                "The mock you are trying to access is no longer active. It has been deleted from the active set via `reset` - you should not hold on to a `MockId` after you call `reset`!."
            )
        }
        &self.mocks[index.index]
    }
}

/// A [`MountedMock`] can either be global (i.e. registered using [`crate::MockServer::register`]) or
/// scoped (i.e. registered using [`crate::MockServer::register_as_scoped`]).
///
/// [`MountedMock`]s must currently be in scope to be matched against incoming requests.
/// Out of scope [`MountedMock`]s are skipped when trying to match an incoming request.
///
/// # Implementation Rationale
///
/// An alternative approach would be removing a [`MountedMock`] from the [`MountedMockSet`] when it goes
/// out of scope.
/// This would create an issue for the stability of [`MockId`]s: removing an element from the vector
/// of [`MountedMock`]s in [`MountedMockSet`] would invalidate the ids of all mocks registered after
/// the removed one.
///
/// Attaching a state to the mocks in the vector, instead, allows us to ensure id stability while
/// achieving the desired behaviour.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub(crate) enum MountedMockState {
    InScope,
    OutOfScope,
}

#[cfg(test)]
mod tests {
    use crate::matchers::path;
    use crate::mock_set::{MountedMockSet, MountedMockState};
    use crate::request::BodyPrintLimit;
    use crate::{Mock, ResponseTemplate};

    fn test_mock_set() -> MountedMockSet {
        MountedMockSet::new(BodyPrintLimit::Unlimited)
    }

    #[test]
    fn generation_is_incremented_for_every_reset() {
        let mut set = test_mock_set();
        assert_eq!(set.generation, 0);

        for i in 1..10 {
            set.reset();
            assert_eq!(set.generation, i);
        }
    }

    #[test]
    #[should_panic]
    fn accessing_a_mock_id_after_a_reset_triggers_a_panic() {
        // Assert
        let mut set = test_mock_set();
        let mock = Mock::given(path("/")).respond_with(ResponseTemplate::new(200));
        let (_, mock_id) = set.register(mock);

        // Act
        set.reset();

        // Assert
        let _ = &set[mock_id];
    }

    #[test]
    fn deactivating_a_mock_does_not_invalidate_other_ids() {
        // Assert
        let mut set = test_mock_set();
        let first_mock = Mock::given(path("/")).respond_with(ResponseTemplate::new(200));
        let second_mock = Mock::given(path("/hello")).respond_with(ResponseTemplate::new(500));
        let (_, first_mock_id) = set.register(first_mock);
        let (_, second_mock_id) = set.register(second_mock);

        // Act
        set.deactivate(first_mock_id);

        // Assert
        let first_mock = &set[first_mock_id];
        assert_eq!(first_mock.1, MountedMockState::OutOfScope);
        let second_mock = &set[second_mock_id];
        assert_eq!(second_mock.1, MountedMockState::InScope);
    }
}
