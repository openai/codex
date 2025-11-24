#![feature(test)]
extern crate test;

use aes::Aes128;

cipher::block_encryptor_bench!(
    KeyIv: cbc::Encryptor<Aes128>,
    cbc_aes128_encrypt_block,
    cbc_aes128_encrypt_blocks,
);

cipher::block_decryptor_bench!(
    KeyIv: cbc::Decryptor<Aes128>,
    cbc_aes128_decrypt_block,
    cbc_aes128_decrypt_blocks,
);
