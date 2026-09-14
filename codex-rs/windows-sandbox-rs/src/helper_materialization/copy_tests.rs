//! Tests the legacy helper copy and freshness contract.

use super::CopyOutcome;
use super::copy_from_source_if_needed;
use super::destination_is_fresh;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::TempDir;

#[test]
fn copy_from_source_if_needed_copies_missing_destination() {
    let tmp = TempDir::new().expect("tempdir");
    let source = tmp.path().join("source.exe");
    let destination = tmp.path().join("bin").join("helper.exe");

    fs::write(&source, b"runner-v1").expect("write source");

    let outcome = copy_from_source_if_needed(&source, &destination).expect("copy helper");

    assert_eq!(CopyOutcome::ReCopied, outcome);
    assert_eq!(
        b"runner-v1".as_slice(),
        fs::read(&destination).expect("read destination")
    );
}

#[test]
fn destination_is_fresh_uses_size_and_mtime() {
    let tmp = TempDir::new().expect("tempdir");
    let source = tmp.path().join("source.exe");
    let destination = tmp.path().join("destination.exe");

    fs::write(&destination, b"same-size").expect("write destination");
    std::thread::sleep(std::time::Duration::from_secs(1));
    fs::write(&source, b"same-size").expect("write source");
    assert!(!destination_is_fresh(&source, &destination).expect("stale metadata"));

    fs::write(&destination, b"same-size").expect("rewrite destination");
    assert!(destination_is_fresh(&source, &destination).expect("fresh metadata"));
}

#[test]
fn copy_from_source_if_needed_reuses_fresh_destination() {
    let tmp = TempDir::new().expect("tempdir");
    let source = tmp.path().join("source.exe");
    let destination = tmp.path().join("bin").join("helper.exe");

    fs::write(&source, b"runner-v1").expect("write source");
    copy_from_source_if_needed(&source, &destination).expect("initial copy");

    let outcome = copy_from_source_if_needed(&source, &destination).expect("revalidate helper");

    assert_eq!(CopyOutcome::Reused, outcome);
    assert_eq!(
        b"runner-v1".as_slice(),
        fs::read(&destination).expect("read destination")
    );
}
