use super::DropBomb;
use super::UNJOINED_CHILD_MESSAGE;
use crate::test_support::panic_message;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use tracing_test::traced_test;

#[test]
fn disarmed_drop_bomb_does_not_report_an_error() {
    let mut bomb = DropBomb::new();
    bomb.disarm();
}

#[cfg(debug_assertions)]
#[test]
fn armed_drop_bomb_panics() {
    let panic = catch_unwind(AssertUnwindSafe(|| drop(DropBomb::new())))
        .expect_err("armed drop bomb should panic");

    assert_eq!(panic_message(panic.as_ref()), UNJOINED_CHILD_MESSAGE);
}

#[cfg(not(debug_assertions))]
#[test]
#[traced_test]
fn armed_drop_bomb_logs_an_error() {
    drop(DropBomb::new());

    assert!(logs_contain(UNJOINED_CHILD_MESSAGE));
}

#[test]
#[traced_test]
fn armed_drop_bomb_logs_instead_of_panicking_during_unwind() {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _bomb = DropBomb::new();
        panic!("outer panic");
    }))
    .expect_err("outer panic should propagate");

    assert_eq!(panic_message(panic.as_ref()), "outer panic");
    assert!(logs_contain(UNJOINED_CHILD_MESSAGE));
}
