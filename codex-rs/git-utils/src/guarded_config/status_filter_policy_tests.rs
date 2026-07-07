use super::status_filter_sentinel_probe_selects_driver;
use super::status_filter_special_pathspec;
use pretty_assertions::assert_eq;

#[cfg(unix)]
#[test]
fn status_filter_special_pathspec_preserves_raw_literal_paths() {
    use std::os::unix::ffi::OsStringExt;

    let path = b"dir/a*?[\\:)-\xff";
    for (driver, requirement) in [
        ("set", "filter"),
        ("unset", "-filter"),
        ("unspecified", "!filter"),
    ] {
        let actual = status_filter_special_pathspec(path, driver)
            .expect("bounded raw Status pathspec")
            .into_vec();
        let mut expected = format!(":(top,literal,attr:{requirement})").into_bytes();
        expected.extend_from_slice(path);
        assert_eq!(actual, expected, "{driver}");
    }
}

#[test]
fn status_filter_sentinel_probe_requires_exact_bounded_stage_records() {
    let path = b"dir/file.txt";
    assert!(
        status_filter_sentinel_probe_selects_driver(b"", path).expect("empty exact output"),
        "no special-state match conservatively retains the literal driver"
    );
    assert!(
        !status_filter_sentinel_probe_selects_driver(b"dir/file.txt\0", path)
            .expect("one exact stage")
    );
    assert!(
        !status_filter_sentinel_probe_selects_driver(
            b"dir/file.txt\0dir/file.txt\0dir/file.txt\0",
            path,
        )
        .expect("three exact stages"),
        "three unmerged stages may repeat the exact path"
    );
    for malformed in [
        b"dir/file.txt".as_slice(),
        b"other.txt\0".as_slice(),
        b"dir/file.txt\0dir/file.txt\0dir/file.txt\0dir/file.txt\0".as_slice(),
    ] {
        assert!(
            status_filter_sentinel_probe_selects_driver(malformed, path).is_err(),
            "accepted malformed sentinel output {malformed:?}"
        );
    }
}
