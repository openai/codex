/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for AES GCM mode */

/*
 * This file uses the low level AES functions (which are deprecated for
 * non-internal use) in order to implement provider AES ciphers.
 */
#include "internal/deprecated.h"

#include "cipher_aes_gcm.h"

static int aes_gcm_initkey(PROV_GCM_CTX *ctx, const unsigned char *key,
                                   size_t keylen)
{
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;
    AES_KEY *ks = &actx->ks.ks;

# ifdef HWAES_CAPABLE
    if (HWAES_CAPABLE) {
#  ifdef HWAES_ctr32_encrypt_blocks
        GCM_HW_SET_KEY_CTR_FN(ks, HWAES_set_encrypt_key, HWAES_encrypt,
                              HWAES_ctr32_encrypt_blocks);
#  else
        GCM_HW_SET_KEY_CTR_FN(ks, HWAES_set_encrypt_key, HWAES_encrypt, NULL);
#  endif /* HWAES_ctr32_encrypt_blocks */
    } else
# endif /* HWAES_CAPABLE */

# ifdef BSAES_CAPABLE
    if (BSAES_CAPABLE) {
        GCM_HW_SET_KEY_CTR_FN(ks, AES_set_encrypt_key, AES_encrypt,
                              ossl_bsaes_ctr32_encrypt_blocks);
    } else
# endif /* BSAES_CAPABLE */

# ifdef VPAES_CAPABLE
    if (VPAES_CAPABLE) {
        GCM_HW_SET_KEY_CTR_FN(ks, vpaes_set_encrypt_key, vpaes_encrypt, NULL);
    } else
# endif /* VPAES_CAPABLE */

    {
# ifdef AES_CTR_ASM
        GCM_HW_SET_KEY_CTR_FN(ks, AES_set_encrypt_key, AES_encrypt,
                              AES_ctr32_encrypt);
# else
        GCM_HW_SET_KEY_CTR_FN(ks, AES_set_encrypt_key, AES_encrypt, NULL);
# endif /* AES_CTR_ASM */
    }
    return 1;
}

static int generic_aes_gcm_cipher_update(PROV_GCM_CTX *ctx, const unsigned char *in,
                                         size_t len, unsigned char *out)
{
    if (ctx->enc) {
        if (ctx->ctr != NULL) {
#if defined(AES_GCM_ASM)
            size_t bulk = 0;

            if (len >= AES_GCM_ENC_BYTES && AES_GCM_ASM(ctx)) {
                size_t res = (16 - ctx->gcm.mres) % 16;

                if (CRYPTO_gcm128_encrypt(&ctx->gcm, in, out, res))
                    return 0;

                bulk = AES_gcm_encrypt(in + res, out + res, len - res,
                                       ctx->gcm.key,
                                       ctx->gcm.Yi.c, ctx->gcm.Xi.u);

                ctx->gcm.len.u[1] += bulk;
                bulk += res;
            }
            if (CRYPTO_gcm128_encrypt_ctr32(&ctx->gcm, in + bulk, out + bulk,
                                            len - bulk, ctx->ctr))
                return 0;
#else
            if (CRYPTO_gcm128_encrypt_ctr32(&ctx->gcm, in, out, len, ctx->ctr))
                return 0;
#endif /* AES_GCM_ASM */
        } else {
            if (CRYPTO_gcm128_encrypt(&ctx->gcm, in, out, len))
                return 0;
        }
    } else {
        if (ctx->ctr != NULL) {
#if defined(AES_GCM_ASM)
            size_t bulk = 0;

            if (len >= AES_GCM_DEC_BYTES && AES_GCM_ASM(ctx)) {
                size_t res = (16 - ctx->gcm.mres) % 16;

                if (CRYPTO_gcm128_decrypt(&ctx->gcm, in, out, res))
                    return 0;

                bulk = AES_gcm_decrypt(in + res, out + res, len - res,
                                       ctx->gcm.key,
                                       ctx->gcm.Yi.c, ctx->gcm.Xi.u);

                ctx->gcm.len.u[1] += bulk;
                bulk += res;
            }
            if (CRYPTO_gcm128_decrypt_ctr32(&ctx->gcm, in + bulk, out + bulk,
                                            len - bulk, ctx->ctr))
                return 0;
#else
            if (CRYPTO_gcm128_decrypt_ctr32(&ctx->gcm, in, out, len, ctx->ctr))
                return 0;
#endif /* AES_GCM_ASM */
        } else {
            if (CRYPTO_gcm128_decrypt(&ctx->gcm, in, out, len))
                return 0;
        }
    }
    return 1;
}

static const PROV_GCM_HW aes_gcm = {
    aes_gcm_initkey,
    ossl_gcm_setiv,
    ossl_gcm_aad_update,
    generic_aes_gcm_cipher_update,
    ossl_gcm_cipher_final,
    ossl_gcm_one_shot
};

#if defined(S390X_aes_128_CAPABLE)
# include "cipher_aes_gcm_hw_s390x.inc"
#elif defined(AESNI_CAPABLE)
# include "cipher_aes_gcm_hw_aesni.inc"
#elif defined(SPARC_AES_CAPABLE)
# include "cipher_aes_gcm_hw_t4.inc"
#elif defined(AES_PMULL_CAPABLE) && defined(AES_GCM_ASM)
# include "cipher_aes_gcm_hw_armv8.inc"
#elif defined(PPC_AES_GCM_CAPABLE) && defined(_ARCH_PPC64)
# include "cipher_aes_gcm_hw_ppc.inc"
#elif defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 64
# include "cipher_aes_gcm_hw_rv64i.inc"
#elif defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 32
# include "cipher_aes_gcm_hw_rv32i.inc"
#else
const PROV_GCM_HW *ossl_prov_aes_hw_gcm(size_t keybits)
{
    return &aes_gcm;
}
#endif

