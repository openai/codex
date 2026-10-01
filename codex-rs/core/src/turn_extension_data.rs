//! Carries host-owned data through settings acceptance and into each new turn.

use codex_extension_api::ExtensionDataInit;

/// A request that also replaces the host data inherited by future turns.
///
/// Core accepts or rejects both together. Steering leaves the running turn's
/// data unchanged. Plain requests preserve the saved data; an empty initializer
/// clears it. Values should be immutable: initializers share their values by `Arc`.
/// This data is not persisted, so hosts must supply it again on cold resume.
#[derive(Debug)]
pub struct WithTurnExtensionData<T> {
    pub(crate) request: T,
    pub(crate) turn_extension_init: Option<ExtensionDataInit>,
}

impl<T> WithTurnExtensionData<T> {
    pub fn new(request: T, turn_extension_init: ExtensionDataInit) -> Self {
        Self {
            request,
            turn_extension_init: Some(turn_extension_init),
        }
    }
}

impl<T> From<T> for WithTurnExtensionData<T> {
    fn from(request: T) -> Self {
        Self {
            request,
            turn_extension_init: None,
        }
    }
}
