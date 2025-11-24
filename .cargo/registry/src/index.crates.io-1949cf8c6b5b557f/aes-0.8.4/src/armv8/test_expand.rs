use super::{expand_key, inv_expanded_keys};
use core::arch::aarch64::*;
use hex_literal::hex;

/// FIPS 197, Appendix A.1: AES-128 Cipher Key
/// user input, unaligned buffer
const AES128_KEY: [u8; 16] = hex!("2b7e151628aed2a6abf7158809cf4f3c");

/// FIPS 197 Appendix A.1: Expansion of a 128-bit Cipher Key
/// library controlled, aligned buffer
const AES128_EXP_KEYS: [[u8; 16]; 11] = [
    AES128_KEY,
    hex!("a0fafe1788542cb123a339392a6c7605"),
    hex!("f2c295f27a96b9435935807a7359f67f"),
    hex!("3d80477d4716fe3e1e237e446d7a883b"),
    hex!("ef44a541a8525b7fb671253bdb0bad00"),
    hex!("d4d1c6f87c839d87caf2b8bc11f915bc"),
    hex!("6d88a37a110b3efddbf98641ca0093fd"),
    hex!("4e54f70e5f5fc9f384a64fb24ea6dc4f"),
    hex!("ead27321b58dbad2312bf5607f8d292f"),
    hex!("ac7766f319fadc2128d12941575c006e"),
    hex!("d014f9a8c9ee2589e13f0cc8b6630ca6"),
];

/// Inverse expanded keys for [`AES128_EXPANDED_KEYS`]
const AES128_EXP_INVKEYS: [[u8; 16]; 11] = [
    hex!("d014f9a8c9ee2589e13f0cc8b6630ca6"),
    hex!("0c7b5a631319eafeb0398890664cfbb4"),
    hex!("df7d925a1f62b09da320626ed6757324"),
    hex!("12c07647c01f22c7bc42d2f37555114a"),
    hex!("6efcd876d2df54807c5df034c917c3b9"),
    hex!("6ea30afcbc238cf6ae82a4b4b54a338d"),
    hex!("90884413d280860a12a128421bc89739"),
    hex!("7c1f13f74208c219c021ae480969bf7b"),
    hex!("cc7505eb3e17d1ee82296c51c9481133"),
    hex!("2b3708a7f262d405bc3ebdbf4b617d62"),
    AES128_KEY,
];

/// FIPS 197, Appendix A.2: AES-192 Cipher Key
/// user input, unaligned buffer
const AES192_KEY: [u8; 24] = hex!("8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b");

/// FIPS 197 Appendix A.2: Expansion of a 192-bit Cipher Key
/// library controlled, aligned buffer
const AES192_EXP_KEYS: [[u8; 16]; 13] = [
    hex!("8e73b0f7da0e6452c810f32b809079e5"),
    hex!("62f8ead2522c6b7bfe0c91f72402f5a5"),
    hex!("ec12068e6c827f6b0e7a95b95c56fec2"),
    hex!("4db7b4bd69b5411885a74796e92538fd"),
    hex!("e75fad44bb095386485af05721efb14f"),
    hex!("a448f6d94d6dce24aa326360113b30e6"),
    hex!("a25e7ed583b1cf9a27f939436a94f767"),
    hex!("c0a69407d19da4e1ec1786eb6fa64971"),
    hex!("485f703222cb8755e26d135233f0b7b3"),
    hex!("40beeb282f18a2596747d26b458c553e"),
    hex!("a7e1466c9411f1df821f750aad07d753"),
    hex!("ca4005388fcc5006282d166abc3ce7b5"),
    hex!("e98ba06f448c773c8ecc720401002202"),
];

/// FIPS 197, Appendix A.3: AES-256 Cipher Key
/// user input, unaligned buffer
const AES256_KEY: [u8; 32] =
    hex!("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");

/// FIPS 197 Appendix A.3: Expansion of a 256-bit Cipher Key
/// library controlled, aligned buffer
const AES256_EXP_KEYS: [[u8; 16]; 15] = [
    hex!("603deb1015ca71be2b73aef0857d7781"),
    hex!("1f352c073b6108d72d9810a30914dff4"),
    hex!("9ba354118e6925afa51a8b5f2067fcde"),
    hex!("a8b09c1a93d194cdbe49846eb75d5b9a"),
    hex!("d59aecb85bf3c917fee94248de8ebe96"),
    hex!("b5a9328a2678a647983122292f6c79b3"),
    hex!("812c81addadf48ba24360af2fab8b464"),
    hex!("98c5bfc9bebd198e268c3ba709e04214"),
    hex!("68007bacb2df331696e939e46c518d80"),
    hex!("c814e20476a9fb8a5025c02d59c58239"),
    hex!("de1369676ccc5a71fa2563959674ee15"),
    hex!("5886ca5d2e2f31d77e0af1fa27cf73c3"),
    hex!("749c47ab18501ddae2757e4f7401905a"),
    hex!("cafaaae3e4d59b349adf6acebd10190d"),
    hex!("fe4890d1e6188d0b046df344706c631e"),
];

fn load_expanded_keys<const N: usize>(input: [[u8; 16]; N]) -> [uint8x16_t; N] {
    let mut output = [unsafe { vdupq_n_u8(0) }; N];

    for (src, dst) in input.iter().zip(output.iter_mut()) {
        *dst = unsafe { vld1q_u8(src.as_ptr()) }
    }

    output
}

fn store_expanded_keys<const N: usize>(input: [uint8x16_t; N]) -> [[u8; 16]; N] {
    let mut output = [[0u8; 16]; N];

    for (src, dst) in input.iter().zip(output.iter_mut()) {
        unsafe { vst1q_u8(dst.as_mut_ptr(), *src) }
    }

    output
}

#[test]
fn aes128_key_expansion() {
    let ek = unsafe { expand_key(&AES128_KEY) };
    assert_eq!(store_expanded_keys(ek), AES128_EXP_KEYS);
}

#[test]
fn aes128_key_expansion_inv() {
    let mut ek = load_expanded_keys(AES128_EXP_KEYS);
    unsafe { inv_expanded_keys(&mut ek) };
    assert_eq!(store_expanded_keys(ek), AES128_EXP_INVKEYS);
}

#[test]
fn aes192_key_expansion() {
    let ek = unsafe { expand_key(&AES192_KEY) };
    assert_eq!(store_expanded_keys(ek), AES192_EXP_KEYS);
}

#[test]
fn aes256_key_expansion() {
    let ek = unsafe { expand_key(&AES256_KEY) };
    assert_eq!(store_expanded_keys(ek), AES256_EXP_KEYS);
}
