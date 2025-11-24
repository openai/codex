/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * AES-NI support for AES modes ecb, cbc, ofb, cfb, ctr.
 * This file is included by cipher_aes_hw.c
 */

#define cipher_hw_aesni_ofb128 ossl_cipher_hw_generic_ofb128
#define cipher_hw_aesni_cfb128 ossl_cipher_hw_generic_cfb128
#define cipher_hw_aesni_cfb8   ossl_cipher_hw_generic_cfb8
#define cipher_hw_aesni_cfb1   ossl_cipher_hw_generic_cfb1
#define cipher_hw_aesni_ctr    ossl_cipher_hw_generic_ctr

static int cipher_hw_aesni_initkey(PROV_CIPHER_CTX *dat,
                                   const unsigned char *key, size_t keylen)
{
    int ret;
    PROV_AES_CTX *adat = (PROV_AES_CTX *)dat;
    AES_KEY *ks = &adat->ks.ks;

    dat->ks = ks;

    if ((dat->mode == EVP_CIPH_ECB_MODE || dat->mode == EVP_CIPH_CBC_MODE)
        && !dat->enc) {
        ret = aesni_set_decrypt_key(key, keylen * 8, ks);
        dat->block = (block128_f) aesni_decrypt;
        dat->stream.cbc = dat->mode == EVP_CIPH_CBC_MODE ?
            (cbc128_f) aesni_cbc_encrypt : NULL;
    } else {
        ret = aesni_set_encrypt_key(key, keylen * 8, ks);
        dat->block = (block128_f) aesni_encrypt;
        if (dat->mode == EVP_CIPH_CBC_MODE)
            dat->stream.cbc = (cbc128_f) aesni_cbc_encrypt;
        else if (dat->mode == EVP_CIPH_CTR_MODE)
            dat->stream.ctr = (ctr128_f) aesni_ctr32_encrypt_blocks;
        else
            dat->stream.cbc = NULL;
    }

    if (ret < 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_KEY_SETUP_FAILED);
        return 0;
    }

    return 1;
}

static int cipher_hw_aesni_cbc(PROV_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    const AES_KEY *ks = ctx->ks;

    aesni_cbc_encrypt(in, out, len, ks, ctx->iv, ctx->enc);

    return 1;
}

static int cipher_hw_aesni_ecb(PROV_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t len)
{
    if (len < ctx->blocksize)
        return 1;

    aesni_ecb_encrypt(in, out, len, ctx->ks, ctx->enc);

    return 1;
}

#define PROV_CIPHER_HW_declare(mode)                                           \
static const PROV_CIPHER_HW aesni_##mode = {                                   \
    cipher_hw_aesni_initkey,                                                   \
    cipher_hw_aesni_##mode,                                                    \
    cipher_hw_aes_copyctx                                                      \
};
#define PROV_CIPHER_HW_select(mode)                                            \
if (AESNI_CAPABLE)                                                             \
    return &aesni_##mode;
