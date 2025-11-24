/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * Sparc t4 support for AES modes ecb, cbc, ofb, cfb, ctr.
 * This file is included by cipher_aes_hw.c
 */

static int cipher_hw_aes_t4_initkey(PROV_CIPHER_CTX *dat,
                                    const unsigned char *key, size_t keylen)
{
    int ret, bits;
    PROV_AES_CTX *adat = (PROV_AES_CTX *)dat;
    AES_KEY *ks = &adat->ks.ks;

    dat->ks = (const void *)ks; /* used by cipher_hw_generic_XXX */

    bits = keylen * 8;
    if ((dat->mode == EVP_CIPH_ECB_MODE || dat->mode == EVP_CIPH_CBC_MODE)
        && !dat->enc) {
        ret = 0;
        aes_t4_set_decrypt_key(key, bits, ks);
        dat->block = (block128_f)aes_t4_decrypt;
        switch (bits) {
        case 128:
            dat->stream.cbc = dat->mode == EVP_CIPH_CBC_MODE ?
                (cbc128_f)aes128_t4_cbc_decrypt : NULL;
            break;
        case 192:
            dat->stream.cbc = dat->mode == EVP_CIPH_CBC_MODE ?
                (cbc128_f)aes192_t4_cbc_decrypt : NULL;
            break;
        case 256:
            dat->stream.cbc = dat->mode == EVP_CIPH_CBC_MODE ?
                (cbc128_f)aes256_t4_cbc_decrypt : NULL;
            break;
        default:
            ret = -1;
        }
    } else {
        ret = 0;
        aes_t4_set_encrypt_key(key, bits, ks);
        dat->block = (block128_f)aes_t4_encrypt;
        switch (bits) {
        case 128:
            if (dat->mode == EVP_CIPH_CBC_MODE)
                dat->stream.cbc = (cbc128_f)aes128_t4_cbc_encrypt;
            else if (dat->mode == EVP_CIPH_CTR_MODE)
                dat->stream.ctr = (ctr128_f)aes128_t4_ctr32_encrypt;
            else
                dat->stream.cbc = NULL;
            break;
        case 192:
            if (dat->mode == EVP_CIPH_CBC_MODE)
                dat->stream.cbc = (cbc128_f)aes192_t4_cbc_encrypt;
            else if (dat->mode == EVP_CIPH_CTR_MODE)
                dat->stream.ctr = (ctr128_f)aes192_t4_ctr32_encrypt;
            else
                dat->stream.cbc = NULL;
            break;
        case 256:
            if (dat->mode == EVP_CIPH_CBC_MODE)
                dat->stream.cbc = (cbc128_f)aes256_t4_cbc_encrypt;
            else if (dat->mode == EVP_CIPH_CTR_MODE)
                dat->stream.ctr = (ctr128_f)aes256_t4_ctr32_encrypt;
            else
                dat->stream.cbc = NULL;
            break;
        default:
            ret = -1;
        }
    }

    if (ret < 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_KEY_SETUP_FAILED);
        return 0;
    }

    return 1;
}

#define PROV_CIPHER_HW_declare(mode)                                           \
static const PROV_CIPHER_HW aes_t4_##mode = {                                  \
    cipher_hw_aes_t4_initkey,                                                  \
    ossl_cipher_hw_generic_##mode,                                             \
    cipher_hw_aes_copyctx                                                      \
};
#define PROV_CIPHER_HW_select(mode)                                            \
    if (SPARC_AES_CAPABLE)                                                     \
        return &aes_t4_##mode;
