/*
 * Copyright 2021-2022 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2021, Intel Corporation. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * AVX512 VAES + VPCLMULDQD support for AES GCM.
 * This file is included by cipher_aes_gcm_hw_aesni.inc
 */

#undef VAES_GCM_ENABLED
#if (defined(__x86_64) || defined(__x86_64__) || \
     defined(_M_AMD64) || defined(_M_X64))
# define VAES_GCM_ENABLED

/* Returns non-zero when AVX512F + VAES + VPCLMULDQD combination is available */
int ossl_vaes_vpclmulqdq_capable(void);

# define OSSL_AES_GCM_UPDATE(direction)                                 \
    void ossl_aes_gcm_ ## direction ## _avx512(const void *ks,          \
                                               void *gcm128ctx,         \
                                               unsigned int *pblocklen, \
                                               const unsigned char *in, \
                                               size_t len,              \
                                               unsigned char *out);

OSSL_AES_GCM_UPDATE(encrypt)
OSSL_AES_GCM_UPDATE(decrypt)

void ossl_aes_gcm_init_avx512(const void *ks, void *gcm128ctx);
void ossl_aes_gcm_setiv_avx512(const void *ks, void *gcm128ctx,
                               const unsigned char *iv, size_t ivlen);
void ossl_aes_gcm_update_aad_avx512(void *gcm128ctx, const unsigned char *aad,
                                    size_t aadlen);
void ossl_aes_gcm_finalize_avx512(void *gcm128ctx, unsigned int pblocklen);

void ossl_gcm_gmult_avx512(u64 Xi[2], const void *gcm128ctx);

static int vaes_gcm_setkey(PROV_GCM_CTX *ctx, const unsigned char *key,
                           size_t keylen)
{
    GCM128_CONTEXT *gcmctx = &ctx->gcm;
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;
    AES_KEY *ks = &actx->ks.ks;

    aesni_set_encrypt_key(key, keylen * 8, ks);
    memset(gcmctx, 0, sizeof(*gcmctx));
    gcmctx->key = ks;
    ctx->key_set = 1;

    ossl_aes_gcm_init_avx512(ks, gcmctx);

    return 1;
}

static int vaes_gcm_setiv(PROV_GCM_CTX *ctx, const unsigned char *iv,
                          size_t ivlen)
{
    GCM128_CONTEXT *gcmctx = &ctx->gcm;

    gcmctx->Yi.u[0] = 0;           /* Current counter */
    gcmctx->Yi.u[1] = 0;
    gcmctx->Xi.u[0] = 0;           /* AAD hash */
    gcmctx->Xi.u[1] = 0;
    gcmctx->len.u[0] = 0;          /* AAD length */
    gcmctx->len.u[1] = 0;          /* Message length */
    gcmctx->ares = 0;
    gcmctx->mres = 0;

    /* IV is limited by 2^64 bits, thus 2^61 bytes */
    if (ivlen > (U64(1) << 61))
        return 0;

    ossl_aes_gcm_setiv_avx512(gcmctx->key, gcmctx, iv, ivlen);

    return 1;
}

static int vaes_gcm_aadupdate(PROV_GCM_CTX *ctx,
                              const unsigned char *aad,
                              size_t aad_len)
{
    GCM128_CONTEXT *gcmctx = &ctx->gcm;
    u64 alen = gcmctx->len.u[0];
    unsigned int ares;
    size_t i, lenBlks;

    /* Bad sequence: call of AAD update after message processing */
    if (gcmctx->len.u[1] > 0)
        return 0;

    alen += aad_len;
    /* AAD is limited by 2^64 bits, thus 2^61 bytes */
    if ((alen > (U64(1) << 61)) || (alen < aad_len))
        return 0;

    gcmctx->len.u[0] = alen;

    ares = gcmctx->ares;
    /* Partial AAD block left from previous AAD update calls */
    if (ares > 0) {
        /*
         * Fill partial block buffer till full block
         * (note, the hash is stored reflected)
         */
        while (ares > 0 && aad_len > 0) {
            gcmctx->Xi.c[15 - ares] ^= *(aad++);
            --aad_len;
            ares = (ares + 1) % AES_BLOCK_SIZE;
        }
        /* Full block gathered */
        if (ares == 0) {
            ossl_gcm_gmult_avx512(gcmctx->Xi.u, gcmctx);
        } else { /* no more AAD */
            gcmctx->ares = ares;
            return 1;
        }
    }

    /* Bulk AAD processing */
    lenBlks = aad_len & ((size_t)(-AES_BLOCK_SIZE));
    if (lenBlks > 0) {
        ossl_aes_gcm_update_aad_avx512(gcmctx, aad, lenBlks);
        aad += lenBlks;
        aad_len -= lenBlks;
    }

    /* Add remaining AAD to the hash (note, the hash is stored reflected) */
    if (aad_len > 0) {
        ares = aad_len;
        for (i = 0; i < aad_len; i++)
            gcmctx->Xi.c[15 - i] ^= aad[i];
    }

    gcmctx->ares = ares;

    return 1;
}

static int vaes_gcm_cipherupdate(PROV_GCM_CTX *ctx, const unsigned char *in,
                                 size_t len, unsigned char *out)
{
    GCM128_CONTEXT *gcmctx = &ctx->gcm;
    u64 mlen = gcmctx->len.u[1];

    mlen += len;
    if (mlen > ((U64(1) << 36) - 32) || (mlen < len))
        return 0;

    gcmctx->len.u[1] = mlen;

    /* Finalize GHASH(AAD) if AAD partial blocks left unprocessed */
    if (gcmctx->ares > 0) {
        ossl_gcm_gmult_avx512(gcmctx->Xi.u, gcmctx);
        gcmctx->ares = 0;
    }

    if (ctx->enc)
        ossl_aes_gcm_encrypt_avx512(gcmctx->key, gcmctx, &gcmctx->mres, in, len, out);
    else
        ossl_aes_gcm_decrypt_avx512(gcmctx->key, gcmctx, &gcmctx->mres, in, len, out);

    return 1;
}

static int vaes_gcm_cipherfinal(PROV_GCM_CTX *ctx, unsigned char *tag)
{
    GCM128_CONTEXT *gcmctx = &ctx->gcm;
    unsigned int *res = &gcmctx->mres;

    /* Finalize AAD processing */
    if (gcmctx->ares > 0)
        res = &gcmctx->ares;

    ossl_aes_gcm_finalize_avx512(gcmctx, *res);

    if (ctx->enc) {
        ctx->taglen = GCM_TAG_MAX_SIZE;
        memcpy(tag, gcmctx->Xi.c,
               ctx->taglen <= sizeof(gcmctx->Xi.c) ? ctx->taglen :
               sizeof(gcmctx->Xi.c));
        *res = 0;
    } else {
        return !CRYPTO_memcmp(gcmctx->Xi.c, tag, ctx->taglen);
    }

    return 1;
}

static const PROV_GCM_HW vaes_gcm = {
    vaes_gcm_setkey,
    vaes_gcm_setiv,
    vaes_gcm_aadupdate,
    vaes_gcm_cipherupdate,
    vaes_gcm_cipherfinal,
    ossl_gcm_one_shot
};

#endif
