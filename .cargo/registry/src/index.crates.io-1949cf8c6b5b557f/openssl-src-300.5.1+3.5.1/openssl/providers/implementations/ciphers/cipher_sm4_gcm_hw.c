/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * Generic support for SM4 GCM.
 */

#include "cipher_sm4_gcm.h"
#include "crypto/sm4_platform.h"

# define SM4_GCM_HW_SET_KEY_CTR_FN(ks, fn_set_enc_key, fn_block, fn_ctr)       \
    fn_set_enc_key(key, ks);                                                   \
    CRYPTO_gcm128_init(&ctx->gcm, ks, (block128_f)fn_block);                   \
    ctx->ctr = (ctr128_f)fn_ctr;                                               \
    ctx->key_set = 1;

static int sm4_gcm_initkey(PROV_GCM_CTX *ctx, const unsigned char *key,
                           size_t keylen)
{
    PROV_SM4_GCM_CTX *actx = (PROV_SM4_GCM_CTX *)ctx;
    SM4_KEY *ks = &actx->ks.ks;

# ifdef HWSM4_CAPABLE
    if (HWSM4_CAPABLE) {
#  ifdef HWSM4_ctr32_encrypt_blocks
        SM4_GCM_HW_SET_KEY_CTR_FN(ks, HWSM4_set_encrypt_key, HWSM4_encrypt,
                                  HWSM4_ctr32_encrypt_blocks);
#  else /* HWSM4_ctr32_encrypt_blocks */
        SM4_GCM_HW_SET_KEY_CTR_FN(ks, HWSM4_set_encrypt_key, HWSM4_encrypt, NULL);
#  endif
    } else
# endif /* HWSM4_CAPABLE */

#ifdef VPSM4_EX_CAPABLE
    if (VPSM4_EX_CAPABLE) {
        SM4_GCM_HW_SET_KEY_CTR_FN(ks, vpsm4_ex_set_encrypt_key, vpsm4_ex_encrypt,
                                  vpsm4_ex_ctr32_encrypt_blocks);
    } else
#endif /* VPSM4_EX_CAPABLE */

# ifdef VPSM4_CAPABLE
    if (VPSM4_CAPABLE) {
        SM4_GCM_HW_SET_KEY_CTR_FN(ks, vpsm4_set_encrypt_key, vpsm4_encrypt,
                                  vpsm4_ctr32_encrypt_blocks);
    } else
# endif /* VPSM4_CAPABLE */
    {
        SM4_GCM_HW_SET_KEY_CTR_FN(ks, ossl_sm4_set_key, ossl_sm4_encrypt, NULL);
    }

    return 1;
}

static int hw_gcm_cipher_update(PROV_GCM_CTX *ctx, const unsigned char *in,
                                size_t len, unsigned char *out)
{
    if (ctx->enc) {
        if (ctx->ctr != NULL) {
            if (CRYPTO_gcm128_encrypt_ctr32(&ctx->gcm, in, out, len, ctx->ctr))
                return 0;
        } else {
            if (CRYPTO_gcm128_encrypt(&ctx->gcm, in, out, len))
                return 0;
        }
    } else {
        if (ctx->ctr != NULL) {
            if (CRYPTO_gcm128_decrypt_ctr32(&ctx->gcm, in, out, len, ctx->ctr))
                return 0;
        } else {
            if (CRYPTO_gcm128_decrypt(&ctx->gcm, in, out, len))
                return 0;
        }
    }
    return 1;
}

static const PROV_GCM_HW sm4_gcm = {
    sm4_gcm_initkey,
    ossl_gcm_setiv,
    ossl_gcm_aad_update,
    hw_gcm_cipher_update,
    ossl_gcm_cipher_final,
    ossl_gcm_one_shot
};

#if defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 64
# include "cipher_sm4_gcm_hw_rv64i.inc"
#else
const PROV_GCM_HW *ossl_prov_sm4_hw_gcm(size_t keybits)
{
    return &sm4_gcm;
}
#endif
