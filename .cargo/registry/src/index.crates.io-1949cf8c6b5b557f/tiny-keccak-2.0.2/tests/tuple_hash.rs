use tiny_keccak::{Hasher, TupleHash};

#[test]
fn test_tuple_hash128_one() {
    let te3 = b"\x00\x01\x02";
    let te6 = b"\x10\x11\x12\x13\x14\x15";
    let s0 = b"";
    let expected = b"\
        \xC5\xD8\x78\x6C\x1A\xFB\x9B\x82\x11\x1A\xB3\x4B\x65\xB2\xC0\x04\
        \x8F\xA6\x4E\x6D\x48\xE2\x63\x26\x4C\xE1\x70\x7D\x3F\xFC\x8E\xD1\
    ";
    let mut output = [0u8; 32];
    let mut hasher = TupleHash::v128(s0);
    hasher.update(te3);
    hasher.update(te6);
    hasher.finalize(&mut output);
    assert_eq!(expected, &output);
}

#[test]
fn test_tuple_hash128_two() {
    let te3 = b"\x00\x01\x02";
    let te6 = b"\x10\x11\x12\x13\x14\x15";
    let s1 = b"My Tuple App";
    let expected = b"\
        \x75\xCD\xB2\x0F\xF4\xDB\x11\x54\xE8\x41\xD7\x58\xE2\x41\x60\xC5\
        \x4B\xAE\x86\xEB\x8C\x13\xE7\xF5\xF4\x0E\xB3\x55\x88\xE9\x6D\xFB\
    ";
    let mut output = [0u8; 32];
    let mut hasher = TupleHash::v128(s1);
    hasher.update(te3);
    hasher.update(te6);
    hasher.finalize(&mut output);
    assert_eq!(expected, &output);
}

#[test]
fn test_tuple_hash128_three() {
    let te3 = b"\x00\x01\x02";
    let te6 = b"\x10\x11\x12\x13\x14\x15";
    let te9 = b"\x20\x21\x22\x23\x24\x25\x26\x27\x28";
    let s1 = b"My Tuple App";
    let expected = b"\
        \xE6\x0F\x20\x2C\x89\xA2\x63\x1E\xDA\x8D\x4C\x58\x8C\xA5\xFD\x07\
        \xF3\x9E\x51\x51\x99\x8D\xEC\xCF\x97\x3A\xDB\x38\x04\xBB\x6E\x84\
    ";
    let mut output = [0u8; 32];
    let mut hasher = TupleHash::v128(s1);
    hasher.update(te3);
    hasher.update(te6);
    hasher.update(te9);
    hasher.finalize(&mut output);
    assert_eq!(expected, &output);
}

#[test]
fn test_tuple_hash256() {
    let te3 = b"\x00\x01\x02";
    let te6 = b"\x10\x11\x12\x13\x14\x15";
    let s0 = b"";
    let expected = b"\
        \xCF\xB7\x05\x8C\xAC\xA5\xE6\x68\xF8\x1A\x12\xA2\x0A\x21\x95\xCE\
        \x97\xA9\x25\xF1\xDB\xA3\xE7\x44\x9A\x56\xF8\x22\x01\xEC\x60\x73\
        \x11\xAC\x26\x96\xB1\xAB\x5E\xA2\x35\x2D\xF1\x42\x3B\xDE\x7B\xD4\
        \xBB\x78\xC9\xAE\xD1\xA8\x53\xC7\x86\x72\xF9\xEB\x23\xBB\xE1\x94\
    ";
    let mut output = [0u8; 64];
    let mut hasher = TupleHash::v256(s0);
    hasher.update(te3);
    hasher.update(te6);
    hasher.finalize(&mut output);
    assert_eq!(expected as &[u8], &output as &[u8]);
}

#[test]
fn test_tuple_hash256_two() {
    let te3 = b"\x00\x01\x02";
    let te6 = b"\x10\x11\x12\x13\x14\x15";
    let s1 = b"My Tuple App";
    let expected = b"\
        \x14\x7C\x21\x91\xD5\xED\x7E\xFD\x98\xDB\xD9\x6D\x7A\xB5\xA1\x16\
        \x92\x57\x6F\x5F\xE2\xA5\x06\x5F\x3E\x33\xDE\x6B\xBA\x9F\x3A\xA1\
        \xC4\xE9\xA0\x68\xA2\x89\xC6\x1C\x95\xAA\xB3\x0A\xEE\x1E\x41\x0B\
        \x0B\x60\x7D\xE3\x62\x0E\x24\xA4\xE3\xBF\x98\x52\xA1\xD4\x36\x7E\
    ";
    let mut output = [0u8; 64];
    let mut hasher = TupleHash::v256(s1);
    hasher.update(te3);
    hasher.update(te6);
    hasher.finalize(&mut output);
    assert_eq!(expected as &[u8], &output as &[u8]);
}

#[test]
fn test_tuple_hash256_three() {
    let te3 = b"\x00\x01\x02";
    let te6 = b"\x10\x11\x12\x13\x14\x15";
    let te9 = b"\x20\x21\x22\x23\x24\x25\x26\x27\x28";
    let s1 = b"My Tuple App";
    let expected = b"\
        \x45\x00\x0B\xE6\x3F\x9B\x6B\xFD\x89\xF5\x47\x17\x67\x0F\x69\xA9\
        \xBC\x76\x35\x91\xA4\xF0\x5C\x50\xD6\x88\x91\xA7\x44\xBC\xC6\xE7\
        \xD6\xD5\xB5\xE8\x2C\x01\x8D\xA9\x99\xED\x35\xB0\xBB\x49\xC9\x67\
        \x8E\x52\x6A\xBD\x8E\x85\xC1\x3E\xD2\x54\x02\x1D\xB9\xE7\x90\xCE\
    ";
    let mut output = [0u8; 64];
    let mut hasher = TupleHash::v256(s1);
    hasher.update(te3);
    hasher.update(te6);
    hasher.update(te9);
    hasher.finalize(&mut output);
    assert_eq!(expected as &[u8], &output as &[u8]);
}
