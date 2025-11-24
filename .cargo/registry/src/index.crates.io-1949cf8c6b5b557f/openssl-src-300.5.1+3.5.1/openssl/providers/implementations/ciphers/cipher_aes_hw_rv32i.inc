/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * RISC-V 32 ZKND ZKNE support for AES modes ecb, cbc, ofb, cfb, ctr.
 * This file is included by cipher_aes_hw.c
 */

#define cipher_hw_rv32i_zknd_zkne_cbc    ossl_cipher_hw_generic_cbc
#define cipher_hw_rv32i_zknd_zkne_ecb    ossl_cipher_hw_generic_ecb
#define cipher_hw_rv32i_zknd_zkne_ofb128 ossl_cipher_hw_generic_ofb128
#define cipher_hw_rv32i_zknd_zkne_cfb128 ossl_cipher_hw_generic_cfb128
#define cipher_hw_rv32i_zknd_zkne_cfb8   ossl_cipher_hw_generic_cfb8
#define cipher_hw_rv32i_zknd_zkne_cfb1   ossl_cipher_hw_generic_cfb1
#define cipher_hw_rv32i_zknd_zkne_ctr    ossl_cipher_hw_generic_ctr

#define cipher_hw_rv32i_zbkb_zknd_zkne_cbc    ossl_cipher_hw_generic_cbc
#define cipher_hw_rv32i_zbkb_zknd_zkne_ecb    ossl_cipher_hw_generic_ecb
#define cipher_hw_rv32i_zbkb_zknd_zkne_ofb128 ossl_cipher_hw_generic_ofb128
#define cipher_hw_rv32i_zbkb_zknd_zkne_cfb128 ossl_cipher_hw_generic_cfb128
#define cipher_hw_rv32i_zbkb_zknd_zkne_cfb8   ossl_cipher_hw_generic_cfb8
#define cipher_hw_rv32i_zbkb_zknd_zkne_cfb1   ossl_cipher_hw_generic_cfb1
#define cipher_hw_rv32i_zbkb_zknd_zkne_ctr    ossl_cipher_hw_generic_ctr

static int cipher_hw_rv32i_zknd_zkne_initkey(PROV_CIPHER_CTX *dat,
                                             const unsigned char *key, size_t keylen)
{
    int ret;
    PROV_AES_CTX *adat = (PROV_AES_CTX *)dat;
    AES_KEY *ks = &adat->ks.ks;

    dat->ks = ks;

    if ((dat->mode == EVP_CIPH_ECB_MODE || dat->mode == EVP_CIPH_CBC_MODE)
        && !dat->enc) {
        ret = rv32i_zknd_zkne_set_decrypt_key(key, keylen * 8, ks);
        dat->block = (block128_f) rv32i_zknd_decrypt;
        dat->stream.cbc = NULL;
    } else {
        ret = rv32i_zkne_set_encrypt_key(key, keylen * 8, ks);
        dat->block = (block128_f) rv32i_zkne_encrypt;
        dat->stream.cbc = NULL;
    }

    if (ret < 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_KEY_SETUP_FAILED);
        return 0;
    }

    return 1;
}

static int cipher_hw_rv32i_zbkb_zknd_zkne_initkey(PROV_CIPHER_CTX *dat,
                                                  const unsigned char *key, size_t keylen)
{
    int ret;
    PROV_AES_CTX *adat = (PROV_AES_CTX *)dat;
    AES_KEY *ks = &adat->ks.ks;

    dat->ks = ks;

    if ((dat->mode == EVP_CIPH_ECB_MODE || dat->mode == EVP_CIPH_CBC_MODE)
        && !dat->enc) {
        ret = rv32i_zbkb_zknd_zkne_set_decrypt_key(key, keylen * 8, ks);
        dat->block = (block128_f) rv32i_zknd_decrypt;
        dat->stream.cbc = NULL;
    } else {
        ret = rv32i_zbkb_zkne_set_encrypt_key(key, keylen * 8, ks);
        dat->block = (block128_f) rv32i_zkne_encrypt;
        dat->stream.cbc = NULL;
    }

    if (ret < 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_KEY_SETUP_FAILED);
        return 0;
    }

    return 1;
}

#define PROV_CIPHER_HW_declare(mode)                                           \
static const PROV_CIPHER_HW rv32i_zknd_zkne_##mode = {                         \
    cipher_hw_rv32i_zknd_zkne_initkey,                                         \
    cipher_hw_rv32i_zknd_zkne_##mode,                                          \
    cipher_hw_aes_copyctx                                                      \
};                                                                             \
static const PROV_CIPHER_HW rv32i_zbkb_zknd_zkne_##mode = {                    \
    cipher_hw_rv32i_zbkb_zknd_zkne_initkey,                                    \
    cipher_hw_rv32i_zbkb_zknd_zkne_##mode,                                     \
    cipher_hw_aes_copyctx                                                      \
};
#define PROV_CIPHER_HW_select(mode)                                            \
if (RISCV_HAS_ZBKB_AND_ZKND_AND_ZKNE())                                        \
    return &rv32i_zbkb_zknd_zkne_##mode;                                       \
if (RISCV_HAS_ZKND_AND_ZKNE())                                                 \
    return &rv32i_zknd_zkne_##mode;
