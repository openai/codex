use aes::*;
use cbc::{Decryptor, Encryptor};
use cipher::{block_mode_dec_test, block_mode_enc_test, iv_state_test};

iv_state_test!(aes128_cbc_enc_iv_state, Encryptor<Aes128>, encrypt);
iv_state_test!(aes128_cbc_dec_iv_state, Decryptor<Aes128>, decrypt);
iv_state_test!(aes192_cbc_enc_iv_state, Encryptor<Aes192>, encrypt);
iv_state_test!(aes192_cbc_dec_iv_state, Decryptor<Aes192>, decrypt);
iv_state_test!(aes256_cbc_enc_iv_state, Encryptor<Aes256>, encrypt);
iv_state_test!(aes256_cbc_dec_iv_state, Decryptor<Aes256>, decrypt);

// Test vectors from CVAP "AES Multiblock Message Test (MMT) Sample Vectors":
// <https://csrc.nist.gov/Projects/Cryptographic-Algorithm-Validation-Program/Block-Ciphers>
block_mode_enc_test!(aes128_cbc_enc_test, "aes128", Encryptor<Aes128>);
block_mode_dec_test!(aes128_cbc_dec_test, "aes128", Decryptor<Aes128>);
block_mode_enc_test!(aes128enc_cbc_enc_test, "aes128", Encryptor<Aes128Enc>);
block_mode_dec_test!(aes128dec_cbc_dec_test, "aes128", Decryptor<Aes128Dec>);
block_mode_enc_test!(aes192_cbc_enc_test, "aes192", Encryptor<Aes192>);
block_mode_dec_test!(aes192_cbc_dec_test, "aes192", Decryptor<Aes192>);
block_mode_enc_test!(aes192enc_cbc_enc_test, "aes192", Encryptor<Aes192Enc>);
block_mode_dec_test!(aes192dec_cbc_dec_test, "aes192", Decryptor<Aes192Dec>);
block_mode_enc_test!(aes256_cbc_enc_test, "aes256", Encryptor<Aes256>);
block_mode_dec_test!(aes256_cbc_dec_test, "aes256", Decryptor<Aes256>);
block_mode_enc_test!(aes256enc_cbc_enc_test, "aes256", Encryptor<Aes256Enc>);
block_mode_dec_test!(aes256dec_cbc_dec_test, "aes256", Decryptor<Aes256Dec>);
