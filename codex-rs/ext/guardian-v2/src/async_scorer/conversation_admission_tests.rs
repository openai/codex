#![cfg(test)]
//! Verifies that both policy backends share admission while recording an observation.

use std::sync::Arc;
use std::sync::TryLockError;

use super::ConversationBackend;
use super::LunaSampler;
use super::LunaSamplerError;
use super::MAX_OUTSTANDING;
use crate::async_scorer::sampler::tests::sampler_config;

#[test]
fn earlier_observation_keeps_the_last_available_slot() {
    let sampler = Arc::new(LunaSampler::new(sampler_config(
        "http://localhost".to_owned(),
    )));
    let previous = ConversationBackend::new(sampler, /*revision*/ 0, /*metrics*/ None);
    let current = previous.for_policy(/*revision*/ 1);
    let _occupied = (0..MAX_OUTSTANDING - 1)
        .map(|index| previous.reserve(|| index).1.expect("available slot"))
        .collect::<Vec<_>>();

    let (_, reservation) = current.reserve(|| {
        assert!(matches!(
            previous.capacity.try_lock(),
            Err(TryLockError::WouldBlock)
        ));
        MAX_OUTSTANDING - 1
    });
    assert!(reservation.is_ok());
    assert!(matches!(
        previous.reserve(|| MAX_OUTSTANDING).1,
        Err(LunaSamplerError::QueueFull)
    ));
}
