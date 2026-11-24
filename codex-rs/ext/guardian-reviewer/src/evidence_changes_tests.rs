#![cfg(test)]

//! Checks coalescing across subscription and committed evidence changes.

use super::GuardianEvidenceChanges;
use pretty_assertions::assert_eq;

#[test]
fn coalesces_changes_and_catches_up_new_subscribers() {
    let changes = GuardianEvidenceChanges::default();
    changes.notify();
    let mut updates = changes.subscribe();
    assert_eq!(*updates.borrow_and_update(), 1);
    assert!(!updates.has_changed().expect("feed remains open"));

    changes.notify();
    changes.notify();
    assert_eq!(*updates.borrow_and_update(), 3);
    assert!(!updates.has_changed().expect("feed remains open"));
}
