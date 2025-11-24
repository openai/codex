/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * Fujitsu SPARC64 X support for camellia modes.
 * This file is included by cipher_camellia_hw.c
 */

static int cipher_hw_camellia_t4_initkey(PROV_CIPHER_CTX *dat,
                                         const unsigned char *key,
                                         size_t keylen)
{
    int ret = 0, bits, mode = dat->mode;
    PROV_CAMELLIA_CTX *adat = (PROV_CAMELLIA_CTX *)dat;
    CAMELLIA_KEY *ks = &adat->ks.ks;

    dat->ks = ks;
    bits = keylen * 8;

    cmll_t4_set_key(key, bits, ks);

    if (dat->enc || (mode != EVP_CIPH_ECB_MODE && mode != EVP_CIPH_CBC_MODE)) {
        dat->block = (block128_f) cmll_t4_encrypt;
        switch (bits) {
        case 128:
            if (mode == EVP_CIPH_CBC_MODE)
                dat->stream.cbc = (cbc128_f) cmll128_t4_cbc_encrypt;
            else if (mode == EVP_CIPH_CTR_MODE)
                dat->stream.ctr = (ctr128_f) cmll128_t4_ctr32_encrypt;
            else
                dat->stream.cbc = NULL;
            break;
        case 192:
        case 256:
            if (mode == EVP_CIPH_CBC_MODE)
                dat->stream.cbc = (cbc128_f) cmll256_t4_cbc_encrypt;
            else if (mode == EVP_CIPH_CTR_MODE)
                dat->stream.ctr = (ctr128_f) cmll256_t4_ctr32_encrypt;
            else
                dat->stream.cbc = NULL;
            break;
        default:
            ret = -1;
            break;
        }
    } else {
        dat->block = (block128_f) cmll_t4_decrypt;
        switch (bits) {
        case 128:
            dat->stream.cbc = mode == EVP_CIPH_CBC_MODE ?
                (cbc128_f) cmll128_t4_cbc_decrypt : NULL;
            break;
        case 192:
        case 256:
            dat->stream.cbc = mode == EVP_CIPH_CBC_MODE ?
                (cbc128_f) cmll256_t4_cbc_decrypt : NULL;
            break;
        default:
            ret = -1;
            break;
        }
    }
    if (ret < 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_KEY_SETUP_FAILED);
        return 0;
    }
    return 1;
}

#define PROV_CIPHER_HW_declare(mode)                                           \
static const PROV_CIPHER_HW t4_camellia_##mode = {                             \
    cipher_hw_camellia_t4_initkey,                                             \
    ossl_cipher_hw_generic_##mode,                                             \
    cipher_hw_camellia_copyctx                                                 \
};
#define PROV_CIPHER_HW_select(mode)                                            \
if (SPARC_CMLL_CAPABLE)                                                        \
    return &t4_camellia_##mode;
