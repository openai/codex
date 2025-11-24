/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* AES CCM mode */

/*
 * This file uses the low level AES functions (which are deprecated for
 * non-internal use) in order to implement provider AES ciphers.
 */
#include "internal/deprecated.h"

#include "cipher_aes_ccm.h"

#define AES_HW_CCM_SET_KEY_FN(fn_set_enc_key, fn_blk, fn_ccm_enc, fn_ccm_dec)  \
    fn_set_enc_key(key, keylen * 8, &actx->ccm.ks.ks);                         \
    CRYPTO_ccm128_init(&ctx->ccm_ctx, ctx->m, ctx->l, &actx->ccm.ks.ks,        \
                       (block128_f)fn_blk);                                    \
    ctx->str = ctx->enc ? (ccm128_f)fn_ccm_enc : (ccm128_f)fn_ccm_dec;         \
    ctx->key_set = 1;

static int ccm_generic_aes_initkey(PROV_CCM_CTX *ctx, const unsigned char *key,
                                   size_t keylen)
{
    PROV_AES_CCM_CTX *actx = (PROV_AES_CCM_CTX *)ctx;

#ifdef HWAES_CAPABLE
    if (HWAES_CAPABLE) {
        AES_HW_CCM_SET_KEY_FN(HWAES_set_encrypt_key, HWAES_encrypt, NULL, NULL);
    } else
#endif /* HWAES_CAPABLE */

#ifdef VPAES_CAPABLE
    if (VPAES_CAPABLE) {
        AES_HW_CCM_SET_KEY_FN(vpaes_set_encrypt_key, vpaes_encrypt, NULL, NULL);
    } else
#endif
    {
        AES_HW_CCM_SET_KEY_FN(AES_set_encrypt_key, AES_encrypt, NULL, NULL)
    }
    return 1;
}

static const PROV_CCM_HW aes_ccm = {
    ccm_generic_aes_initkey,
    ossl_ccm_generic_setiv,
    ossl_ccm_generic_setaad,
    ossl_ccm_generic_auth_encrypt,
    ossl_ccm_generic_auth_decrypt,
    ossl_ccm_generic_gettag
};

#if defined(S390X_aes_128_CAPABLE)
# include "cipher_aes_ccm_hw_s390x.inc"
#elif defined(AESNI_CAPABLE)
# include "cipher_aes_ccm_hw_aesni.inc"
#elif defined(SPARC_AES_CAPABLE)
# include "cipher_aes_ccm_hw_t4.inc"
#elif defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 64
# include "cipher_aes_ccm_hw_rv64i.inc"
#elif defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 32
# include "cipher_aes_ccm_hw_rv32i.inc"
#else
const PROV_CCM_HW *ossl_prov_aes_hw_ccm(size_t keybits)
{
    return &aes_ccm;
}
#endif
