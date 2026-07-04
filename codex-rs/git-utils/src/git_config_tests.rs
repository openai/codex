use super::*;

#[test]
fn git_boolean_accepts_native_git_spellings() {
    for (value, expected) in [
        (b"true".as_slice(), true),
        (b"TRUE".as_slice(), true),
        (b"yes".as_slice(), true),
        (b"On".as_slice(), true),
        (b"1".as_slice(), true),
        (b"2".as_slice(), true),
        (b"-1".as_slice(), true),
        (b"+1".as_slice(), true),
        (b"01".as_slice(), true),
        (b"0x1".as_slice(), true),
        (b"0Xf".as_slice(), true),
        (b"010".as_slice(), true),
        (b"1k".as_slice(), true),
        (b"1M".as_slice(), true),
        (b"1g".as_slice(), true),
        (b" 1".as_slice(), true),
        (b"2147483647".as_slice(), true),
        (b"-2147483648".as_slice(), true),
        (b"-2g".as_slice(), true),
        (b"false".as_slice(), false),
        (b"FALSE".as_slice(), false),
        (b"no".as_slice(), false),
        (b"Off".as_slice(), false),
        (b"".as_slice(), false),
        (b"0".as_slice(), false),
        (b"-0".as_slice(), false),
        (b"+0".as_slice(), false),
        (b"00".as_slice(), false),
        (b"0x0".as_slice(), false),
        (b"0k".as_slice(), false),
    ] {
        assert_eq!(parse_git_boolean(value), Some(expected), "value {value:?}");
    }
}

#[test]
fn git_boolean_rejects_values_native_git_rejects() {
    for value in [
        b"08".as_slice(),
        b"-08".as_slice(),
        b"2147483648".as_slice(),
        b"-2147483649".as_slice(),
        b"9223372036854775807".as_slice(),
        b"2g".as_slice(),
        b"-3g".as_slice(),
        b"0x".as_slice(),
        b"0b1".as_slice(),
        b"1kb".as_slice(),
        b"1foo".as_slice(),
        b"1_0".as_slice(),
        b"1 ".as_slice(),
        b" ".as_slice(),
        b"+".as_slice(),
        b"-".as_slice(),
        b"not-a-bool".as_slice(),
        b"\xff".as_slice(),
    ] {
        assert_eq!(parse_git_boolean(value), None, "value {value:?}");
    }
}

#[test]
fn symmetric_git_boolean_parser_excludes_only_int_min_spellings() {
    for value in [
        b"-2147483648".as_slice(),
        b"-0x80000000".as_slice(),
        b"-020000000000".as_slice(),
        b"-2097152k".as_slice(),
        b"-2048m".as_slice(),
        b"-2g".as_slice(),
        b" -2G".as_slice(),
    ] {
        assert_eq!(parse_git_boolean(value), Some(true), "value {value:?}");
        assert_eq!(
            parse_git_boolean_symmetric_i32(value),
            None,
            "value {value:?}"
        );
    }

    for value in [
        b"0x1".as_slice(),
        b"010".as_slice(),
        b"1k".as_slice(),
        b"-1g".as_slice(),
        b" 1".as_slice(),
        b"-2147483647".as_slice(),
    ] {
        assert_eq!(
            parse_git_boolean_symmetric_i32(value),
            parse_git_boolean(value),
            "value {value:?}"
        );
    }
}

#[test]
fn path_containment_uses_component_boundaries() {
    let root = Path::new("/repo/root");
    assert!(path_is_within(Path::new("/repo/root"), root));
    assert!(path_is_within(Path::new("/repo/root/config"), root));
    assert!(!path_is_within(Path::new("/repo/rooted/config"), root));
    assert!(!path_is_within(Path::new("/repo"), root));
}
