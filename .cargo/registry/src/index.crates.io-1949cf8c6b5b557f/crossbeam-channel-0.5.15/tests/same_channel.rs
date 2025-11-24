use std::time::Duration;

use crossbeam_channel::{after, bounded, never, tick, unbounded};

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn after_same_channel() {
    let r = after(ms(50));

    let r2 = r.clone();
    assert!(r.same_channel(&r2));

    let r3 = after(ms(50));
    assert!(!r.same_channel(&r3));
    assert!(!r2.same_channel(&r3));

    let r4 = after(ms(100));
    assert!(!r.same_channel(&r4));
    assert!(!r2.same_channel(&r4));
}

#[test]
fn array_same_channel() {
    let (s, r) = bounded::<usize>(1);

    let s2 = s.clone();
    assert!(s.same_channel(&s2));

    let r2 = r.clone();
    assert!(r.same_channel(&r2));

    let (s3, r3) = bounded::<usize>(1);
    assert!(!s.same_channel(&s3));
    assert!(!s2.same_channel(&s3));
    assert!(!r.same_channel(&r3));
    assert!(!r2.same_channel(&r3));
}

#[test]
fn list_same_channel() {
    let (s, r) = unbounded::<usize>();

    let s2 = s.clone();
    assert!(s.same_channel(&s2));

    let r2 = r.clone();
    assert!(r.same_channel(&r2));

    let (s3, r3) = unbounded::<usize>();
    assert!(!s.same_channel(&s3));
    assert!(!s2.same_channel(&s3));
    assert!(!r.same_channel(&r3));
    assert!(!r2.same_channel(&r3));
}

#[test]
fn never_same_channel() {
    let r = never::<usize>();

    let r2 = r.clone();
    assert!(r.same_channel(&r2));

    // Never channel are always equal to one another.
    let r3 = never::<usize>();
    assert!(r.same_channel(&r3));
    assert!(r2.same_channel(&r3));
}

#[test]
fn tick_same_channel() {
    let r = tick(ms(50));

    let r2 = r.clone();
    assert!(r.same_channel(&r2));

    let r3 = tick(ms(50));
    assert!(!r.same_channel(&r3));
    assert!(!r2.same_channel(&r3));

    let r4 = tick(ms(100));
    assert!(!r.same_channel(&r4));
    assert!(!r2.same_channel(&r4));
}

#[test]
fn zero_same_channel() {
    let (s, r) = bounded::<usize>(0);

    let s2 = s.clone();
    assert!(s.same_channel(&s2));

    let r2 = r.clone();
    assert!(r.same_channel(&r2));

    let (s3, r3) = bounded::<usize>(0);
    assert!(!s.same_channel(&s3));
    assert!(!s2.same_channel(&s3));
    assert!(!r.same_channel(&r3));
    assert!(!r2.same_channel(&r3));
}

#[test]
fn different_flavors_same_channel() {
    let (s1, r1) = bounded::<usize>(0);
    let (s2, r2) = unbounded::<usize>();

    assert!(!s1.same_channel(&s2));
    assert!(!r1.same_channel(&r2));
}
