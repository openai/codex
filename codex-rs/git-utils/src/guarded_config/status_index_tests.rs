use super::*;
use pretty_assertions::assert_eq;

#[test]
fn candidates_require_canonical_regular_stage_zero_entries() {
    fn push_record(output: &mut Vec<u8>, mode: &[u8], oid: &[u8], stage: u8, path: &[u8]) {
        output.extend_from_slice(mode);
        output.push(b' ');
        output.extend_from_slice(oid);
        output.extend([b' ', stage, b'\t']);
        output.extend_from_slice(path);
        output.push(0);
    }

    let oid_sha1 = vec![b'1'; 40];
    let oid_sha256 = vec![b'a'; 64];
    let mut output = Vec::new();
    push_record(
        &mut output,
        b"100644",
        &oid_sha1,
        /*stage*/ b'0',
        b"file.txt",
    );
    push_record(
        &mut output,
        b"100755",
        &oid_sha256,
        /*stage*/ b'0',
        b"bin/tab\topaque-\xff",
    );
    push_record(
        &mut output,
        b"120000",
        &oid_sha1,
        /*stage*/ b'0',
        b"link",
    );
    push_record(
        &mut output,
        b"160000",
        &oid_sha1,
        /*stage*/ b'0',
        b"nested",
    );
    for stage in [b'1', b'2', b'3'] {
        push_record(&mut output, b"100644", &oid_sha1, stage, b"conflict");
    }

    assert_eq!(
        parse_status_filter_candidate_paths(&output, /*core_symlinks*/ true)
            .expect("canonical stage records with native symlinks"),
        vec![b"file.txt".to_vec(), b"bin/tab\topaque-\xff".to_vec()]
    );
    assert_eq!(
        parse_status_filter_candidate_paths(&output, /*core_symlinks*/ false)
            .expect("canonical stage records with emulated symlinks"),
        vec![
            b"file.txt".to_vec(),
            b"bin/tab\topaque-\xff".to_vec(),
            b"link".to_vec(),
        ]
    );
    assert_eq!(
        parse_status_filter_candidate_paths(b"", /*core_symlinks*/ true).expect("empty index"),
        Vec::<Vec<u8>>::new()
    );

    let malformed = [
        b"100644 1111111111111111111111111111111111111111 0\tunterminated".as_slice(),
        b"\0".as_slice(),
        b"100644 1111111111111111111111111111111111111111 0\t\0".as_slice(),
        b"100644 1111111111111111111111111111111111111111 0 no-tab\0".as_slice(),
        b"10064 1111111111111111111111111111111111111111 0\tbad-mode\0".as_slice(),
        b"040000 1111111111111111111111111111111111111111 0\tsparse-dir\0".as_slice(),
        b"100644 A111111111111111111111111111111111111111 0\tuppercase-oid\0".as_slice(),
        b"100644 1111111111111111111111111111111111111111 4\tbad-stage\0".as_slice(),
        b"100644  1111111111111111111111111111111111111111 0\textra-space\0".as_slice(),
        b"100644 1111111111111111111111111111111111111111 0\tfirst\0\0".as_slice(),
    ];
    for input in malformed {
        assert!(
            parse_status_filter_candidate_paths(input, /*core_symlinks*/ true).is_err(),
            "accepted malformed tracked-path inventory {input:?}"
        );
    }
}

#[test]
fn core_symlinks_screening_preserves_explicit_values_and_platform_defaults() {
    for windows in [false, true] {
        assert!(status_core_symlinks_for_filter_screening_on(
            Some(true),
            windows
        ));
        assert!(!status_core_symlinks_for_filter_screening_on(
            Some(false),
            windows
        ));
    }
    assert!(status_core_symlinks_for_filter_screening_on(
        /*configured*/ None, /*windows*/ false
    ));
    assert!(!status_core_symlinks_for_filter_screening_on(
        /*configured*/ None, /*windows*/ true
    ));
}
