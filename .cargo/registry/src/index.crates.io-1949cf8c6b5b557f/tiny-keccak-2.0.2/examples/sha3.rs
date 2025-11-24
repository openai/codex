use tiny_keccak::{Hasher, Sha3};

fn main() {
    let mut sha3 = Sha3::v256();
    let mut output = [0; 32];
    let expected = b"\
        \x64\x4b\xcc\x7e\x56\x43\x73\x04\x09\x99\xaa\xc8\x9e\x76\x22\xf3\
        \xca\x71\xfb\xa1\xd9\x72\xfd\x94\xa3\x1c\x3b\xfb\xf2\x4e\x39\x38\
    ";

    sha3.update(b"hello");
    sha3.update(b" ");
    sha3.update(b"world");
    sha3.finalize(&mut output);

    assert_eq!(expected, &output);
}
