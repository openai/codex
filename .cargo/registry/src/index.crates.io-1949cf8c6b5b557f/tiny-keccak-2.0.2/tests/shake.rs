use tiny_keccak::{Hasher, Shake, Xof};

#[test]
fn shake_xof_one() {
    let mut shake = Shake::v128();
    let mut output = [0; 32];
    let expected = b"\
        \x43\xE4\x1B\x45\xA6\x53\xF2\xA5\xC4\x49\x2C\x1A\xDD\x54\x45\x12\
        \xDD\xA2\x52\x98\x33\x46\x2B\x71\xA4\x1A\x45\xBE\x97\x29\x0B\x6F\
    ";

    for _ in 0..16 {
        shake.squeeze(&mut output);
    }

    assert_eq!(expected, &output);
}

#[test]
fn shake_xof_two() {
    let mut shake = Shake::v128();
    let mut output = [0; 32];
    let expected = b"\
        \x44\xC9\xFB\x35\x9F\xD5\x6A\xC0\xA9\xA7\x5A\x74\x3C\xFF\x68\x62\
        \xF1\x7D\x72\x59\xAB\x07\x52\x16\xC0\x69\x95\x11\x64\x3B\x64\x39\
    ";

    for _ in 0..10 {
        shake.update(&[0xa3; 20]);
    }

    for _ in 0..16 {
        shake.squeeze(&mut output);
    }

    assert_eq!(expected, &output);
}
