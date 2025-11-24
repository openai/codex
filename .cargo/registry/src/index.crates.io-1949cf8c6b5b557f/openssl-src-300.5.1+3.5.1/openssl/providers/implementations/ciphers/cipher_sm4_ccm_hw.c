/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * Generic support for SM4 CCM.
 */

#include "cipher_sm4_ccm.h"
#include "crypto/sm4_platform.h"

#define SM4_HW_CCM_SET_KEY_FN(fn_set_enc_key, fn_blk, fn_ccm_enc, fn_ccm_dec)  \
    fn_set_enc_key(key, &actx->ks.ks);                                         \
    CRYPTO_ccm128_init(&ctx->ccm_ctx, ctx->m, ctx->l, &actx->ks.ks,            \
                       (block128_f)fn_blk);                                    \
    ctx->str = ctx->enc ? (ccm128_f)fn_ccm_enc : (ccm128_f)fn_ccm_dec;         \
    ctx->key_set = 1;

static int ccm_sm4_initkey(PROV_CCM_CTX *ctx,
                           const unsigned char *key, size_t keylen)
{
    PROV_SM4_CCM_CTX *actx = (PROV_SM4_CCM_CTX *)ctx;

#ifdef HWSM4_CAPABLE
    if (HWSM4_CAPABLE) {
        SM4_HW_CCM_SET_KEY_FN(HWSM4_set_encrypt_key, HWSM4_encrypt, NULL, NULL);
    } else
#endif /* HWSM4_CAPABLE */

#ifdef VPSM4_EX_CAPABLE
    if (VPSM4_EX_CAPABLE) {
        SM4_HW_CCM_SET_KEY_FN(vpsm4_ex_set_encrypt_key, vpsm4_ex_encrypt, NULL,
                              NULL);
    } else
#endif /* VPSM4_EX_CAPABLE */

#ifdef VPSM4_CAPABLE
    if (VPSM4_CAPABLE) {
        SM4_HW_CCM_SET_KEY_FN(vpsm4_set_encrypt_key, vpsm4_encrypt, NULL, NULL);
    } else
#endif /* VPSM4_CAPABLE */
    {
        SM4_HW_CCM_SET_KEY_FN(ossl_sm4_set_key, ossl_sm4_encrypt, NULL, NULL);
    }
  return 1;
}

static const PROV_CCM_HW ccm_sm4 = {
    ccm_sm4_initkey,
    ossl_ccm_generic_setiv,
    ossl_ccm_generic_setaad,
    ossl_ccm_generic_auth_encrypt,
    ossl_ccm_generic_auth_decrypt,
    ossl_ccm_generic_gettag
};

#if defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 64
# include "cipher_sm4_ccm_hw_rv64i.inc"
#else
const PROV_CCM_HW *ossl_prov_sm4_hw_ccm(size_t keybits)
{
    return &ccm_sm4;
}
#endif
