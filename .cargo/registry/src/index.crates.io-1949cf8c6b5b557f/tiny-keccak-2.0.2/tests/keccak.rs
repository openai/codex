use tiny_keccak::{Hasher, Keccak};

#[test]
fn empty_keccak() {
    let keccak = Keccak::v256();
    let mut output = [0; 32];
    let expected = b"\
        \xc5\xd2\x46\x01\x86\xf7\x23\x3c\x92\x7e\x7d\xb2\xdc\xc7\x03\xc0\
        \xe5\x00\xb6\x53\xca\x82\x27\x3b\x7b\xfa\xd8\x04\x5d\x85\xa4\x70\
    ";

    keccak.finalize(&mut output);
    assert_eq!(expected, &output);
}

#[test]
fn string_keccak_256() {
    let mut keccak = Keccak::v256();
    let mut in_and_out: [u8; 32] = [0; 32];
    for i in 1..6 {
        in_and_out[i as usize - 1] = i
    }
    let expected = b"\
        \x7d\x87\xc5\xea\x75\xf7\x37\x8b\xb7\x01\xe4\x04\xc5\x06\x39\x16\
        \x1a\xf3\xef\xf6\x62\x93\xe9\xf3\x75\xb5\xf1\x7e\xb5\x04\x76\xf4\
    ";
    keccak.update(&in_and_out[0..5]);
    keccak.finalize(&mut in_and_out);
    assert_eq!(expected, &in_and_out);
}
