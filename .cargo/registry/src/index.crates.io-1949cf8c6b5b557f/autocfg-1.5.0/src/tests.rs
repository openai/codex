use std::path::Path;

#[test]
fn version_cmp() {
    use super::version::Version;
    let v123 = Version::new(1, 2, 3);

    assert!(Version::new(1, 0, 0) < v123);
    assert!(Version::new(1, 2, 2) < v123);
    assert!(Version::new(1, 2, 3) == v123);
    assert!(Version::new(1, 2, 4) > v123);
    assert!(Version::new(1, 10, 0) > v123);
    assert!(Version::new(2, 0, 0) > v123);
}

#[test]
fn dir_does_not_contain_target() {
    assert!(!super::dir_contains_target(
        &Some("x86_64-unknown-linux-gnu".into()),
        Path::new("/project/target/debug/build/project-ea75983148559682/out"),
        None,
    ));
}

#[test]
fn dir_does_contain_target() {
    assert!(super::dir_contains_target(
        &Some("x86_64-unknown-linux-gnu".into()),
        Path::new(
            "/project/target/x86_64-unknown-linux-gnu/debug/build/project-0147aca016480b9d/out"
        ),
        None,
    ));
}

#[test]
fn dir_does_not_contain_target_with_custom_target_dir() {
    assert!(!super::dir_contains_target(
        &Some("x86_64-unknown-linux-gnu".into()),
        Path::new("/project/custom/debug/build/project-ea75983148559682/out"),
        Some("custom".into()),
    ));
}

#[test]
fn dir_does_contain_target_with_custom_target_dir() {
    assert!(super::dir_contains_target(
        &Some("x86_64-unknown-linux-gnu".into()),
        Path::new(
            "/project/custom/x86_64-unknown-linux-gnu/debug/build/project-0147aca016480b9d/out"
        ),
        Some("custom".into()),
    ));
}
