use tiny_keccak::{Hasher, Sha3};

#[test]
fn empty_sha3_256() {
    let sha3 = Sha3::v256();
    let mut output = [0; 32];
    let expected = b"\
        \xa7\xff\xc6\xf8\xbf\x1e\xd7\x66\x51\xc1\x47\x56\xa0\x61\xd6\x62\
        \xf5\x80\xff\x4d\xe4\x3b\x49\xfa\x82\xd8\x0a\x4b\x80\xf8\x43\x4a\
    ";
    sha3.finalize(&mut output);
    assert_eq!(expected, &output);
}

#[test]
fn hello_sha3_256() {
    let mut sha3 = Sha3::v256();
    let input = b"hello";
    let mut output = [0u8; 32];
    let expected = b"\
        \x33\x38\xbe\x69\x4f\x50\xc5\xf3\x38\x81\x49\x86\xcd\xf0\x68\x64\
        \x53\xa8\x88\xb8\x4f\x42\x4d\x79\x2a\xf4\xb9\x20\x23\x98\xf3\x92\
    ";
    sha3.update(input);
    sha3.finalize(&mut output);
    assert_eq!(expected, &output);
}

#[test]
fn hello_sha3_256_parts() {
    let mut sha3 = Sha3::v256();
    let input_a = b"hell";
    let input_b = b"o";
    let mut output = [0u8; 32];
    let expected = b"\
        \x33\x38\xbe\x69\x4f\x50\xc5\xf3\x38\x81\x49\x86\xcd\xf0\x68\x64\
        \x53\xa8\x88\xb8\x4f\x42\x4d\x79\x2a\xf4\xb9\x20\x23\x98\xf3\x92\
    ";
    sha3.update(input_a);
    sha3.update(input_b);
    sha3.finalize(&mut output);
    assert_eq!(expected, &output);
}

#[test]
fn hello_sha3_256_parts5() {
    let mut sha3 = Sha3::v256();
    let mut output = [0u8; 32];
    let expected = b"\
        \x33\x38\xbe\x69\x4f\x50\xc5\xf3\x38\x81\x49\x86\xcd\xf0\x68\x64\
        \x53\xa8\x88\xb8\x4f\x42\x4d\x79\x2a\xf4\xb9\x20\x23\x98\xf3\x92\
    ";
    sha3.update(b"h");
    sha3.update(b"e");
    sha3.update(b"l");
    sha3.update(b"l");
    sha3.update(b"o");
    sha3.finalize(&mut output);
    assert_eq!(expected, &output);
}

#[test]
fn long_string_sha3_512() {
    let mut sha3 = Sha3::v512();
    let input = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";
    let mut output = [0u8; 64];
    let expected = b"\
        \xf3\x2a\x94\x23\x55\x13\x51\xdf\x0a\x07\xc0\xb8\xc2\x0e\xb9\x72\
        \x36\x7c\x39\x8d\x61\x06\x60\x38\xe1\x69\x86\x44\x8e\xbf\xbc\x3d\
        \x15\xed\xe0\xed\x36\x93\xe3\x90\x5e\x9a\x8c\x60\x1d\x9d\x00\x2a\
        \x06\x85\x3b\x97\x97\xef\x9a\xb1\x0c\xbd\xe1\x00\x9c\x7d\x0f\x09\
    ";
    sha3.update(input);
    sha3.finalize(&mut output);
    assert_eq!(expected as &[u8], &output as &[u8]);
}

#[test]
fn long_string_sha3_512_parts() {
    let mut sha3 = Sha3::v512();
    let input_a = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ";
    let input_b = b"ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";
    let mut output = [0u8; 64];
    let expected = b"\
        \xf3\x2a\x94\x23\x55\x13\x51\xdf\x0a\x07\xc0\xb8\xc2\x0e\xb9\x72\
        \x36\x7c\x39\x8d\x61\x06\x60\x38\xe1\x69\x86\x44\x8e\xbf\xbc\x3d\
        \x15\xed\xe0\xed\x36\x93\xe3\x90\x5e\x9a\x8c\x60\x1d\x9d\x00\x2a\
        \x06\x85\x3b\x97\x97\xef\x9a\xb1\x0c\xbd\xe1\x00\x9c\x7d\x0f\x09\
    ";
    sha3.update(input_a);
    sha3.update(input_b);
    sha3.finalize(&mut output);
    assert_eq!(expected as &[u8], &output as &[u8]);
}
