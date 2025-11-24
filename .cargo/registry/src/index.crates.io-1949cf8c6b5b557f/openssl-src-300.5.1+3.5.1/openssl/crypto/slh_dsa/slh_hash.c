/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h" /* PKCS1_MGF1() */

#include <string.h>
#include <openssl/evp.h>
#include <openssl/core_names.h>
#include <openssl/rsa.h> /* PKCS1_MGF1() */
#include "slh_dsa_local.h"
#include "slh_dsa_key.h"

#define MAX_DIGEST_SIZE 64 /* SHA-512 is used for security category 3 & 5 */

static OSSL_SLH_HASHFUNC_H_MSG slh_hmsg_sha2;
static OSSL_SLH_HASHFUNC_PRF slh_prf_sha2;
static OSSL_SLH_HASHFUNC_PRF_MSG slh_prf_msg_sha2;
static OSSL_SLH_HASHFUNC_F slh_f_sha2;
static OSSL_SLH_HASHFUNC_H slh_h_sha2;
static OSSL_SLH_HASHFUNC_T slh_t_sha2;

static OSSL_SLH_HASHFUNC_H_MSG slh_hmsg_shake;
static OSSL_SLH_HASHFUNC_PRF slh_prf_shake;
static OSSL_SLH_HASHFUNC_PRF_MSG slh_prf_msg_shake;
static OSSL_SLH_HASHFUNC_F slh_f_shake;
static OSSL_SLH_HASHFUNC_H slh_h_shake;
static OSSL_SLH_HASHFUNC_T slh_t_shake;

static ossl_inline int xof_digest_3(EVP_MD_CTX *ctx,
                                    const uint8_t *in1, size_t in1_len,
                                    const uint8_t *in2, size_t in2_len,
                                    const uint8_t *in3, size_t in3_len,
                                    uint8_t *out, size_t out_len)
{
    return (EVP_DigestInit_ex2(ctx, NULL, NULL) == 1
            && EVP_DigestUpdate(ctx, in1, in1_len) == 1
            && EVP_DigestUpdate(ctx, in2, in2_len) == 1
            && EVP_DigestUpdate(ctx, in3, in3_len) == 1
            && EVP_DigestFinalXOF(ctx, out, out_len) == 1);
}

static ossl_inline int xof_digest_4(EVP_MD_CTX *ctx,
                                    const uint8_t *in1, size_t in1_len,
                                    const uint8_t *in2, size_t in2_len,
                                    const uint8_t *in3, size_t in3_len,
                                    const uint8_t *in4, size_t in4_len,
                                    uint8_t *out, size_t out_len)
{
    return (EVP_DigestInit_ex2(ctx, NULL, NULL) == 1
            && EVP_DigestUpdate(ctx, in1, in1_len) == 1
            && EVP_DigestUpdate(ctx, in2, in2_len) == 1
            && EVP_DigestUpdate(ctx, in3, in3_len) == 1
            && EVP_DigestUpdate(ctx, in4, in4_len) == 1
            && EVP_DigestFinalXOF(ctx, out, out_len) == 1);
}

/* See FIPS 205 Section 11.1 */
static int
slh_hmsg_shake(SLH_DSA_HASH_CTX *ctx, const uint8_t *r,
               const uint8_t *pk_seed, const uint8_t *pk_root,
               const uint8_t *msg, size_t msg_len,
               uint8_t *out, size_t out_len)
{
    const SLH_DSA_PARAMS *params = ctx->key->params;
    size_t m = params->m;
    size_t n = params->n;

    return xof_digest_4(ctx->md_ctx, r, n, pk_seed, n, pk_root, n,
                        msg, msg_len, out, m);
}

static int
slh_prf_shake(SLH_DSA_HASH_CTX *ctx,
              const uint8_t *pk_seed, const uint8_t *sk_seed,
              const uint8_t *adrs, uint8_t *out, size_t out_len)
{
    const SLH_DSA_PARAMS *params = ctx->key->params;
    size_t n = params->n;

    return xof_digest_3(ctx->md_ctx, pk_seed, n, adrs, SLH_ADRS_SIZE,
                        sk_seed, n, out, n);
}

static int
slh_prf_msg_shake(SLH_DSA_HASH_CTX *ctx, const uint8_t *sk_prf,
                  const uint8_t *opt_rand, const uint8_t *msg, size_t msg_len,
                  WPACKET *pkt)
{
    unsigned char out[SLH_MAX_N];
    const SLH_DSA_PARAMS *params = ctx->key->params;
    size_t n = params->n;

    return xof_digest_3(ctx->md_ctx, sk_prf, n, opt_rand, n, msg, msg_len, out, n)
        && WPACKET_memcpy(pkt, out, n);
}

static int
slh_f_shake(SLH_DSA_HASH_CTX *ctx, const uint8_t *pk_seed, const uint8_t *adrs,
            const uint8_t *m1, size_t m1_len, uint8_t *out, size_t out_len)
{
    const SLH_DSA_PARAMS *params = ctx->key->params;
    size_t n = params->n;

    return xof_digest_3(ctx->md_ctx, pk_seed, n, adrs, SLH_ADRS_SIZE, m1, m1_len, out, n);
}

static int
slh_h_shake(SLH_DSA_HASH_CTX *ctx, const uint8_t *pk_seed, const uint8_t *adrs,
            const uint8_t *m1, const uint8_t *m2, uint8_t *out, size_t out_len)
{
    const SLH_DSA_PARAMS *params = ctx->key->params;
    size_t n = params->n;

    return xof_digest_4(ctx->md_ctx, pk_seed, n, adrs, SLH_ADRS_SIZE, m1, n, m2, n, out, n);
}

static int
slh_t_shake(SLH_DSA_HASH_CTX *ctx, const uint8_t *pk_seed, const uint8_t *adrs,
            const uint8_t *ml, size_t ml_len, uint8_t *out, size_t out_len)
{
    const SLH_DSA_PARAMS *params = ctx->key->params;
    size_t n = params->n;

    return xof_digest_3(ctx->md_ctx, pk_seed, n, adrs, SLH_ADRS_SIZE, ml, ml_len, out, n);
}

static ossl_inline int
digest_4(EVP_MD_CTX *ctx,
         const uint8_t *in1, size_t in1_len, const uint8_t *in2, size_t in2_len,
         const uint8_t *in3, size_t in3_len, const uint8_t *in4, size_t in4_len,
         uint8_t *out)
{
    return (EVP_DigestInit_ex2(ctx, NULL, NULL) == 1
            && EVP_DigestUpdate(ctx, in1, in1_len) == 1
            && EVP_DigestUpdate(ctx, in2, in2_len) == 1
            && EVP_DigestUpdate(ctx, in3, in3_len) == 1
            && EVP_DigestUpdate(ctx, in4, in4_len) == 1
            && EVP_DigestFinal_ex(ctx, out, NULL) == 1);
}

/* FIPS 205 Section 11.2.1 and 11.2.2 */

static int
slh_hmsg_sha2(SLH_DSA_HASH_CTX *hctx, const uint8_t *r, const uint8_t *pk_seed,
              const uint8_t *pk_root, const uint8_t *msg, size_t msg_len,
              uint8_t *out, size_t out_len)
{
    const SLH_DSA_PARAMS *params = hctx->key->params;
    size_t m = params->m;
    size_t n = params->n;
    uint8_t seed[2 * SLH_MAX_N + MAX_DIGEST_SIZE];
    int sz = EVP_MD_get_size(hctx->key->md_big);
    size_t seed_len = (size_t)sz + 2 * n;

    memcpy(seed, r, n);
    memcpy(seed + n, pk_seed, n);
    return digest_4(hctx->md_big_ctx, r, n, pk_seed, n, pk_root, n, msg, msg_len,
                    seed + 2 * n)
        && (PKCS1_MGF1(out, m, seed, seed_len, hctx->key->md_big) == 0);
}

static int
slh_prf_msg_sha2(SLH_DSA_HASH_CTX *hctx,
                 const uint8_t *sk_prf, const uint8_t *opt_rand,
                 const uint8_t *msg, size_t msg_len, WPACKET *pkt)
{
    int ret;
    const SLH_DSA_KEY *key = hctx->key;
    EVP_MAC_CTX *mctx = hctx->hmac_ctx;
    const SLH_DSA_PARAMS *prms = key->params;
    size_t n = prms->n;
    uint8_t mac[MAX_DIGEST_SIZE] = {0};
    OSSL_PARAM *p = NULL;
    OSSL_PARAM params[3];

    /*
     * Due to the way HMAC works, it is not possible to do this code early
     * in hmac_ctx_new() since it requires a key in order to set the digest.
     * So we do a lazy update here on the first call.
     */
    if (hctx->hmac_digest_used == 0) {
        p = params;
        /* The underlying digest to be used */
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_DIGEST,
                                                (char *)EVP_MD_get0_name(key->md_big), 0);
        if (key->propq != NULL)
            *p++ = OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_PROPERTIES,
                                                    (char *)key->propq, 0);
        *p = OSSL_PARAM_construct_end();
        p = params;
        hctx->hmac_digest_used = 1;
    }

    ret = EVP_MAC_init(mctx, sk_prf, n, p) == 1
        && EVP_MAC_update(mctx, opt_rand, n) == 1
        && EVP_MAC_update(mctx, msg, msg_len) == 1
        && EVP_MAC_final(mctx, mac, NULL, sizeof(mac)) == 1
        && WPACKET_memcpy(pkt, mac, n); /* Truncate output to n bytes */
    return ret;
}

static ossl_inline int
do_hash(EVP_MD_CTX *ctx, size_t n, const uint8_t *pk_seed, const uint8_t *adrs,
        const uint8_t *m, size_t m_len, size_t b, uint8_t *out, size_t out_len)
{
    int ret;
    uint8_t zeros[128] = { 0 };
    uint8_t digest[MAX_DIGEST_SIZE];

    ret = digest_4(ctx, pk_seed, n, zeros, b - n, adrs, SLH_ADRSC_SIZE,
                   m, m_len, digest);
    /* Truncated returned value is n = 16 bytes */
    memcpy(out, digest, n);
    return ret;
}

static int
slh_prf_sha2(SLH_DSA_HASH_CTX *hctx, const uint8_t *pk_seed,
             const uint8_t *sk_seed, const uint8_t *adrs,
             uint8_t *out, size_t out_len)
{
    size_t n = hctx->key->params->n;

    return do_hash(hctx->md_ctx, n, pk_seed, adrs, sk_seed, n,
                   OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND1, out, out_len);
}

static int
slh_f_sha2(SLH_DSA_HASH_CTX *hctx, const uint8_t *pk_seed, const uint8_t *adrs,
           const uint8_t *m1, size_t m1_len, uint8_t *out, size_t out_len)
{
    return do_hash(hctx->md_ctx, hctx->key->params->n, pk_seed, adrs, m1, m1_len,
                   OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND1, out, out_len);
}

static int
slh_h_sha2(SLH_DSA_HASH_CTX *hctx, const uint8_t *pk_seed, const uint8_t *adrs,
           const uint8_t *m1, const uint8_t *m2, uint8_t *out, size_t out_len)
{
    uint8_t m[SLH_MAX_N * 2];
    const SLH_DSA_PARAMS *prms = hctx->key->params;
    size_t n = prms->n;

    memcpy(m, m1, n);
    memcpy(m + n, m2, n);
    return do_hash(hctx->md_big_ctx, n, pk_seed, adrs, m, 2 * n,
                   prms->sha2_h_and_t_bound, out, out_len);
}

static int
slh_t_sha2(SLH_DSA_HASH_CTX *hctx, const uint8_t *pk_seed, const uint8_t *adrs,
           const uint8_t *ml, size_t ml_len, uint8_t *out, size_t out_len)
{
    const SLH_DSA_PARAMS *prms = hctx->key->params;

    return do_hash(hctx->md_big_ctx, prms->n, pk_seed, adrs, ml, ml_len,
                   prms->sha2_h_and_t_bound, out, out_len);
}

const SLH_HASH_FUNC *ossl_slh_get_hash_fn(int is_shake)
{
    static const SLH_HASH_FUNC methods[] = {
        {
            slh_hmsg_shake,
            slh_prf_shake,
            slh_prf_msg_shake,
            slh_f_shake,
            slh_h_shake,
            slh_t_shake
        },
        {
            slh_hmsg_sha2,
            slh_prf_sha2,
            slh_prf_msg_sha2,
            slh_f_sha2,
            slh_h_sha2,
            slh_t_sha2
        }
    };
    return &methods[is_shake ? 0 : 1];
}
