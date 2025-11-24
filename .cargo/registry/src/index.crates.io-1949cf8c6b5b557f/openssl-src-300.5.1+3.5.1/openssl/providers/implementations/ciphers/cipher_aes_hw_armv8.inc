/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Crypto extension support for AES modes ecb, cbc, ofb, cfb, ctr.
 * This file is included by cipher_aes_hw.c
 */

static int cipher_hw_aes_arm_initkey(PROV_CIPHER_CTX *dat,
                                                       const unsigned char *key,
                                                       size_t keylen)
{
    int ret = cipher_hw_aes_initkey(dat, key, keylen);
    if (AES_UNROLL12_EOR3_CAPABLE && dat->mode == EVP_CIPH_CTR_MODE)
        dat->stream.ctr = (ctr128_f)HWAES_ctr32_encrypt_blocks_unroll12_eor3;

    return ret;
}

#define PROV_CIPHER_HW_declare(mode)                                           \
static const PROV_CIPHER_HW aes_arm_##mode = {                                 \
    cipher_hw_aes_arm_initkey,                                                 \
    ossl_cipher_hw_generic_##mode,                                             \
    cipher_hw_aes_copyctx                                                      \
};
#define PROV_CIPHER_HW_select(mode)                                            \
if (ARMv8_HWAES_CAPABLE)                                                         \
    return &aes_arm_##mode;
