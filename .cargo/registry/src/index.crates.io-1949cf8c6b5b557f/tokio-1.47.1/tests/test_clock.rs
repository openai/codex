#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use tokio::time::{self, Duration, Instant};

#[tokio::test]
async fn resume_lets_time_move_forward_instead_of_resetting_it() {
    let start = Instant::now();
    time::pause();
    time::advance(Duration::from_secs(10)).await;
    let advanced_by_ten_secs = Instant::now();
    assert!(advanced_by_ten_secs - start > Duration::from_secs(10));
    assert!(advanced_by_ten_secs - start < Duration::from_secs(11));
    time::resume();
    assert!(advanced_by_ten_secs < Instant::now());
    assert!(Instant::now() - advanced_by_ten_secs < Duration::from_secs(1));
}

#[tokio::test]
async fn can_pause_after_resume() {
    let start = Instant::now();
    time::pause();
    time::advance(Duration::from_secs(10)).await;
    time::resume();
    time::pause();
    time::advance(Duration::from_secs(10)).await;
    assert!(Instant::now() - start > Duration::from_secs(20));
    assert!(Instant::now() - start < Duration::from_secs(21));
}

#[tokio::test]
#[should_panic]
async fn freezing_time_while_frozen_panics() {
    time::pause();
    time::pause();
}

#[tokio::test]
#[should_panic]
async fn advancing_time_when_time_is_not_frozen_panics() {
    time::advance(Duration::from_secs(1)).await;
}

#[tokio::test]
#[should_panic]
async fn resuming_time_when_not_frozen_panics() {
    time::pause();
    time::resume();
    time::resume();
}
