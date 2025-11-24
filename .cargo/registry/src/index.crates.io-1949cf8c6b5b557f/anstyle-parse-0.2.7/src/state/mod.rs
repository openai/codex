//! ANSI escape code parsing state machine

#[cfg(test)]
mod codegen;
mod definitions;
mod table;

#[cfg(test)]
pub(crate) use definitions::pack;
pub(crate) use definitions::unpack;
pub use definitions::Action;
pub use definitions::State;

/// Transition to next [`State`]
///
/// Note: This does not directly support UTF-8.
/// - If the data is validated as UTF-8 (e.g. `str`) or single-byte C1 control codes are
///   unsupported, then treat [`Action::BeginUtf8`] and [`Action::Execute`] for UTF-8 continuations
///   as [`Action::Print`].
/// - If the data is not validated, then a UTF-8 state machine will need to be implemented on top,
///   starting with [`Action::BeginUtf8`].
///
/// Note: When [`State::Anywhere`] is returned, revert back to the prior state.
#[inline]
pub const fn state_change(state: State, byte: u8) -> (State, Action) {
    // Handle state changes in the anywhere state before evaluating changes
    // for current state.
    let mut change = state_change_(State::Anywhere, byte);
    if change == 0 {
        change = state_change_(state, byte);
    }

    // Unpack into a state and action
    unpack(change)
}

#[inline]
const fn state_change_(state: State, byte: u8) -> u8 {
    let state_idx = state as usize;
    let byte_idx = byte as usize;

    table::STATE_CHANGES[state_idx][byte_idx]
}
