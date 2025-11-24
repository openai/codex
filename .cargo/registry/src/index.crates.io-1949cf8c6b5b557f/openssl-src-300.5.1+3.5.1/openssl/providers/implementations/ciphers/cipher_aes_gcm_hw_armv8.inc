/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Crypto extension support for AES GCM.
 * This file is included by cipher_aes_gcm_hw.c
 */

size_t armv8_aes_gcm_encrypt(const unsigned char *in, unsigned char *out, size_t len,
                             const void *key, unsigned char ivec[16], u64 *Xi)
{
    AES_KEY *aes_key = (AES_KEY *)key;
    size_t align_bytes = len - len % 16;

    switch(aes_key->rounds) {
        case 10:
            if (IS_CPU_SUPPORT_UNROLL8_EOR3()) {
                unroll8_eor3_aes_gcm_enc_128_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            } else {
                aes_gcm_enc_128_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            }
            break;
        case 12:
            if (IS_CPU_SUPPORT_UNROLL8_EOR3()) {
                unroll8_eor3_aes_gcm_enc_192_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            } else {
                aes_gcm_enc_192_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            }
            break;
        case 14:
            if (IS_CPU_SUPPORT_UNROLL8_EOR3()) {
                unroll8_eor3_aes_gcm_enc_256_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            } else {
                aes_gcm_enc_256_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            }
            break;
    }
    return align_bytes;
}

size_t armv8_aes_gcm_decrypt(const unsigned char *in, unsigned char *out, size_t len,
                             const void *key, unsigned char ivec[16], u64 *Xi)
{
    AES_KEY *aes_key = (AES_KEY *)key;
    size_t align_bytes = len - len % 16;

    switch(aes_key->rounds) {
        case 10:
            if (IS_CPU_SUPPORT_UNROLL8_EOR3()) {
                unroll8_eor3_aes_gcm_dec_128_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            } else {
                aes_gcm_dec_128_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            }
            break;
        case 12:
            if (IS_CPU_SUPPORT_UNROLL8_EOR3()) {
                unroll8_eor3_aes_gcm_dec_192_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            } else {
                aes_gcm_dec_192_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            }
            break;
        case 14:
            if (IS_CPU_SUPPORT_UNROLL8_EOR3()) {
                unroll8_eor3_aes_gcm_dec_256_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            } else {
                aes_gcm_dec_256_kernel(in, align_bytes * 8, out, (uint64_t *)Xi, ivec, key);
            }
            break;
    }
    return align_bytes;
}

static int armv8_aes_gcm_initkey(PROV_GCM_CTX *ctx, const unsigned char *key,
                                 size_t keylen)
{
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;
    AES_KEY *ks = &actx->ks.ks;

    if (AES_UNROLL12_EOR3_CAPABLE) {
        GCM_HW_SET_KEY_CTR_FN(ks, aes_v8_set_encrypt_key, aes_v8_encrypt,
                              aes_v8_ctr32_encrypt_blocks_unroll12_eor3);
    } else {
        GCM_HW_SET_KEY_CTR_FN(ks, aes_v8_set_encrypt_key, aes_v8_encrypt,
                              aes_v8_ctr32_encrypt_blocks);
    }
    return 1;
}


static const PROV_GCM_HW armv8_aes_gcm = {
    armv8_aes_gcm_initkey,
    ossl_gcm_setiv,
    ossl_gcm_aad_update,
    generic_aes_gcm_cipher_update,
    ossl_gcm_cipher_final,
    ossl_gcm_one_shot
};

const PROV_GCM_HW *ossl_prov_aes_hw_gcm(size_t keybits)
{
    return AES_PMULL_CAPABLE ? &armv8_aes_gcm : &aes_gcm;
}
