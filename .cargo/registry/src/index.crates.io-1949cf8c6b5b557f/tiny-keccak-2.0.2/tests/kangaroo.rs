use tiny_keccak::{Hasher, KangarooTwelve};

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|j| (j % 251) as u8).collect()
}

fn test_kangaroo_twelve<A: AsRef<[u8]>, B: AsRef<[u8]>>(
    custom_string: A,
    message: B,
    output_len: usize,
    expected: &[u8],
) {
    let mut kangaroo = KangarooTwelve::new(custom_string.as_ref());
    kangaroo.update(message.as_ref());
    let mut res = vec![0; output_len];
    kangaroo.finalize(&mut res);
    assert_eq!(&res[output_len - expected.len()..], expected);
}

#[test]
fn empty_kangaroo_twelve() {
    let expected = b"\
        \x1a\xc2\xd4\x50\xfc\x3b\x42\x05\xd1\x9d\xa7\xbf\xca\x1b\x37\x51\
        \x3c\x08\x03\x57\x7a\xc7\x16\x7f\x06\xfe\x2c\xe1\xf0\xef\x39\xe5\
    ";
    test_kangaroo_twelve("", "", 32, expected);
}

#[test]
fn kangaroo_twelve_long() {
    let expected = b"\
        \xe8\xdc\x56\x36\x42\xf7\x22\x8c\x84\x68\x4c\x89\x84\x05\xd3\xa8\
        \x34\x79\x91\x58\xc0\x79\xb1\x28\x80\x27\x7a\x1d\x28\xe2\xff\x6d\
    ";
    test_kangaroo_twelve("", "", 10032, expected);
}

#[test]
fn kangaroo_twelve_with_message() {
    let expected = b"\
        \x2b\xda\x92\x45\x0e\x8b\x14\x7f\x8a\x7c\xb6\x29\xe7\x84\xa0\x58\
        \xef\xca\x7c\xf7\xd8\x21\x8e\x02\xd3\x45\xdf\xaa\x65\x24\x4a\x1f\
    ";
    test_kangaroo_twelve("", pattern(1), 32, expected);
}

#[test]
fn kangaroo_twelve_with_message2() {
    let expected = b"\
        \x6b\xf7\x5f\xa2\x23\x91\x98\xdb\x47\x72\xe3\x64\x78\xf8\xe1\x9b\
        \x0f\x37\x12\x05\xf6\xa9\xa9\x3a\x27\x3f\x51\xdf\x37\x12\x28\x88\
    ";
    test_kangaroo_twelve("", pattern(17), 32, expected);
}

#[test]
fn kangaroo_twelve_with_custom_string() {
    let expected = b"\
        \xfa\xb6\x58\xdb\x63\xe9\x4a\x24\x61\x88\xbf\x7a\xf6\x9a\x13\x30\
        \x45\xf4\x6e\xe9\x84\xc5\x6e\x3c\x33\x28\xca\xaf\x1a\xa1\xa5\x83\
    ";
    test_kangaroo_twelve(pattern(1), "", 32, expected);
}

#[test]
fn kangaroo_twelve_with_custom_string_and_message() {
    let expected = b"\
        \xd8\x48\xc5\x06\x8c\xed\x73\x6f\x44\x62\x15\x9b\x98\x67\xfd\x4c\
        \x20\xb8\x08\xac\xc3\xd5\xbc\x48\xe0\xb0\x6b\xa0\xa3\x76\x2e\xc4\
    ";
    test_kangaroo_twelve(pattern(41), &[0xff], 32, expected);
}

#[test]
fn kangaroo_twelve_with_custom_string_and_message2() {
    let expected = b"\
        \x75\xd2\xf8\x6a\x2e\x64\x45\x66\x72\x6b\x4f\xbc\xfc\x56\x57\xb9\
        \xdb\xcf\x07\x0c\x7b\x0d\xca\x06\x45\x0a\xb2\x91\xd7\x44\x3b\xcf\
    ";
    test_kangaroo_twelve(
        pattern(68921),
        &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        32,
        expected,
    );
}
