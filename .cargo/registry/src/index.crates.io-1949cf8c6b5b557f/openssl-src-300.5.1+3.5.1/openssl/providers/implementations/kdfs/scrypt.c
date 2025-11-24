/*
 * Copyright 2017-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/err.h>
#include <openssl/core_names.h>
#include <openssl/proverr.h>
#include "crypto/evp.h"
#include "internal/numbers.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/provider_util.h"

#ifndef OPENSSL_NO_SCRYPT

static OSSL_FUNC_kdf_newctx_fn kdf_scrypt_new;
static OSSL_FUNC_kdf_dupctx_fn kdf_scrypt_dup;
static OSSL_FUNC_kdf_freectx_fn kdf_scrypt_free;
static OSSL_FUNC_kdf_reset_fn kdf_scrypt_reset;
static OSSL_FUNC_kdf_derive_fn kdf_scrypt_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn kdf_scrypt_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn kdf_scrypt_set_ctx_params;
static OSSL_FUNC_kdf_gettable_ctx_params_fn kdf_scrypt_gettable_ctx_params;
static OSSL_FUNC_kdf_get_ctx_params_fn kdf_scrypt_get_ctx_params;

static int scrypt_alg(const char *pass, size_t passlen,
                      const unsigned char *salt, size_t saltlen,
                      uint64_t N, uint64_t r, uint64_t p, uint64_t maxmem,
                      unsigned char *key, size_t keylen, EVP_MD *sha256,
                      OSSL_LIB_CTX *libctx, const char *propq);

typedef struct {
    OSSL_LIB_CTX *libctx;
    char *propq;
    unsigned char *pass;
    size_t pass_len;
    unsigned char *salt;
    size_t salt_len;
    uint64_t N;
    uint64_t r, p;
    uint64_t maxmem_bytes;
    EVP_MD *sha256;
} KDF_SCRYPT;

static void kdf_scrypt_init(KDF_SCRYPT *ctx);

static void *kdf_scrypt_new_inner(OSSL_LIB_CTX *libctx)
{
    KDF_SCRYPT *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;
    ctx->libctx = libctx;
    kdf_scrypt_init(ctx);
    return ctx;
}

static void *kdf_scrypt_new(void *provctx)
{
    return kdf_scrypt_new_inner(PROV_LIBCTX_OF(provctx));
}

static void kdf_scrypt_free(void *vctx)
{
    KDF_SCRYPT *ctx = (KDF_SCRYPT *)vctx;

    if (ctx != NULL) {
        OPENSSL_free(ctx->propq);
        EVP_MD_free(ctx->sha256);
        kdf_scrypt_reset(ctx);
        OPENSSL_free(ctx);
    }
}

static void kdf_scrypt_reset(void *vctx)
{
    KDF_SCRYPT *ctx = (KDF_SCRYPT *)vctx;

    OPENSSL_free(ctx->salt);
    ctx->salt = NULL;
    OPENSSL_clear_free(ctx->pass, ctx->pass_len);
    ctx->pass = NULL;
    kdf_scrypt_init(ctx);
}

static void *kdf_scrypt_dup(void *vctx)
{
    const KDF_SCRYPT *src = (const KDF_SCRYPT *)vctx;
    KDF_SCRYPT *dest;

    dest = kdf_scrypt_new_inner(src->libctx);
    if (dest != NULL) {
        if (src->sha256 != NULL && !EVP_MD_up_ref(src->sha256))
            goto err;
        if (src->propq != NULL) {
            dest->propq = OPENSSL_strdup(src->propq);
            if (dest->propq == NULL)
                goto err;
        }
        if (!ossl_prov_memdup(src->salt, src->salt_len,
                              &dest->salt, &dest->salt_len)
                || !ossl_prov_memdup(src->pass, src->pass_len,
                                     &dest->pass , &dest->pass_len))
            goto err;
        dest->N = src->N;
        dest->r = src->r;
        dest->p = src->p;
        dest->maxmem_bytes = src->maxmem_bytes;
        dest->sha256 = src->sha256;
    }
    return dest;

 err:
    kdf_scrypt_free(dest);
    return NULL;
}

static void kdf_scrypt_init(KDF_SCRYPT *ctx)
{
    /* Default values are the most conservative recommendation given in the
     * original paper of C. Percival. Derivation uses roughly 1 GiB of memory
     * for this parameter choice (approx. 128 * r * N * p bytes).
     */
    ctx->N = 1 << 20;
    ctx->r = 8;
    ctx->p = 1;
    ctx->maxmem_bytes = 1025 * 1024 * 1024;
}

static int scrypt_set_membuf(unsigned char **buffer, size_t *buflen,
                             const OSSL_PARAM *p)
{
    OPENSSL_clear_free(*buffer, *buflen);
    *buffer = NULL;
    *buflen = 0;

    if (p->data_size == 0) {
        if ((*buffer = OPENSSL_malloc(1)) == NULL)
            return 0;
    } else if (p->data != NULL) {
        if (!OSSL_PARAM_get_octet_string(p, (void **)buffer, 0, buflen))
            return 0;
    }
    return 1;
}

static int set_digest(KDF_SCRYPT *ctx)
{
    EVP_MD_free(ctx->sha256);
    ctx->sha256 = EVP_MD_fetch(ctx->libctx, "sha256", ctx->propq);
    if (ctx->sha256 == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_UNABLE_TO_LOAD_SHA256);
        return 0;
    }
    return 1;
}

static int set_property_query(KDF_SCRYPT *ctx, const char *propq)
{
    OPENSSL_free(ctx->propq);
    ctx->propq = NULL;
    if (propq != NULL) {
        ctx->propq = OPENSSL_strdup(propq);
        if (ctx->propq == NULL)
            return 0;
    }
    return 1;
}

static int kdf_scrypt_derive(void *vctx, unsigned char *key, size_t keylen,
                             const OSSL_PARAM params[])
{
    KDF_SCRYPT *ctx = (KDF_SCRYPT *)vctx;

    if (!ossl_prov_is_running() || !kdf_scrypt_set_ctx_params(ctx, params))
        return 0;

    if (ctx->pass == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_PASS);
        return 0;
    }

    if (ctx->salt == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_SALT);
        return 0;
    }

    if (ctx->sha256 == NULL && !set_digest(ctx))
        return 0;

    return scrypt_alg((char *)ctx->pass, ctx->pass_len, ctx->salt,
                      ctx->salt_len, ctx->N, ctx->r, ctx->p,
                      ctx->maxmem_bytes, key, keylen, ctx->sha256,
                      ctx->libctx, ctx->propq);
}

static int is_power_of_two(uint64_t value)
{
    return (value != 0) && ((value & (value - 1)) == 0);
}

static int kdf_scrypt_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    KDF_SCRYPT *ctx = vctx;
    uint64_t u64_value;

    if (ossl_param_is_empty(params))
        return 1;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_PASSWORD)) != NULL)
        if (!scrypt_set_membuf(&ctx->pass, &ctx->pass_len, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SALT)) != NULL)
        if (!scrypt_set_membuf(&ctx->salt, &ctx->salt_len, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SCRYPT_N))
        != NULL) {
        if (!OSSL_PARAM_get_uint64(p, &u64_value)
            || u64_value <= 1
            || !is_power_of_two(u64_value))
            return 0;
        ctx->N = u64_value;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SCRYPT_R))
        != NULL) {
        if (!OSSL_PARAM_get_uint64(p, &u64_value) || u64_value < 1)
            return 0;
        ctx->r = u64_value;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SCRYPT_P))
        != NULL) {
        if (!OSSL_PARAM_get_uint64(p, &u64_value) || u64_value < 1)
            return 0;
        ctx->p = u64_value;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SCRYPT_MAXMEM))
        != NULL) {
        if (!OSSL_PARAM_get_uint64(p, &u64_value) || u64_value < 1)
            return 0;
        ctx->maxmem_bytes = u64_value;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_PROPERTIES);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING
            || !set_property_query(ctx, p->data)
            || !set_digest(ctx))
            return 0;
    }
    return 1;
}

static const OSSL_PARAM *kdf_scrypt_settable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_PASSWORD, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SALT, NULL, 0),
        OSSL_PARAM_uint64(OSSL_KDF_PARAM_SCRYPT_N, NULL),
        OSSL_PARAM_uint32(OSSL_KDF_PARAM_SCRYPT_R, NULL),
        OSSL_PARAM_uint32(OSSL_KDF_PARAM_SCRYPT_P, NULL),
        OSSL_PARAM_uint64(OSSL_KDF_PARAM_SCRYPT_MAXMEM, NULL),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int kdf_scrypt_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_SIZE)) != NULL)
        return OSSL_PARAM_set_size_t(p, SIZE_MAX);
    return -2;
}

static const OSSL_PARAM *kdf_scrypt_gettable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_KDF_PARAM_SIZE, NULL),
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_kdf_scrypt_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))kdf_scrypt_new },
    { OSSL_FUNC_KDF_DUPCTX, (void(*)(void))kdf_scrypt_dup },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))kdf_scrypt_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))kdf_scrypt_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))kdf_scrypt_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_scrypt_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))kdf_scrypt_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_scrypt_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))kdf_scrypt_get_ctx_params },
    OSSL_DISPATCH_END
};

#define R(a,b) (((a) << (b)) | ((a) >> (32 - (b))))
static void salsa208_word_specification(uint32_t inout[16])
{
    int i;
    uint32_t x[16];

    memcpy(x, inout, sizeof(x));
    for (i = 8; i > 0; i -= 2) {
        x[4] ^= R(x[0] + x[12], 7);
        x[8] ^= R(x[4] + x[0], 9);
        x[12] ^= R(x[8] + x[4], 13);
        x[0] ^= R(x[12] + x[8], 18);
        x[9] ^= R(x[5] + x[1], 7);
        x[13] ^= R(x[9] + x[5], 9);
        x[1] ^= R(x[13] + x[9], 13);
        x[5] ^= R(x[1] + x[13], 18);
        x[14] ^= R(x[10] + x[6], 7);
        x[2] ^= R(x[14] + x[10], 9);
        x[6] ^= R(x[2] + x[14], 13);
        x[10] ^= R(x[6] + x[2], 18);
        x[3] ^= R(x[15] + x[11], 7);
        x[7] ^= R(x[3] + x[15], 9);
        x[11] ^= R(x[7] + x[3], 13);
        x[15] ^= R(x[11] + x[7], 18);
        x[1] ^= R(x[0] + x[3], 7);
        x[2] ^= R(x[1] + x[0], 9);
        x[3] ^= R(x[2] + x[1], 13);
        x[0] ^= R(x[3] + x[2], 18);
        x[6] ^= R(x[5] + x[4], 7);
        x[7] ^= R(x[6] + x[5], 9);
        x[4] ^= R(x[7] + x[6], 13);
        x[5] ^= R(x[4] + x[7], 18);
        x[11] ^= R(x[10] + x[9], 7);
        x[8] ^= R(x[11] + x[10], 9);
        x[9] ^= R(x[8] + x[11], 13);
        x[10] ^= R(x[9] + x[8], 18);
        x[12] ^= R(x[15] + x[14], 7);
        x[13] ^= R(x[12] + x[15], 9);
        x[14] ^= R(x[13] + x[12], 13);
        x[15] ^= R(x[14] + x[13], 18);
    }
    for (i = 0; i < 16; ++i)
        inout[i] += x[i];
    OPENSSL_cleanse(x, sizeof(x));
}

static void scryptBlockMix(uint32_t *B_, uint32_t *B, uint64_t r)
{
    uint64_t i, j;
    uint32_t X[16], *pB;

    memcpy(X, B + (r * 2 - 1) * 16, sizeof(X));
    pB = B;
    for (i = 0; i < r * 2; i++) {
        for (j = 0; j < 16; j++)
            X[j] ^= *pB++;
        salsa208_word_specification(X);
        memcpy(B_ + (i / 2 + (i & 1) * r) * 16, X, sizeof(X));
    }
    OPENSSL_cleanse(X, sizeof(X));
}

static void scryptROMix(unsigned char *B, uint64_t r, uint64_t N,
                        uint32_t *X, uint32_t *T, uint32_t *V)
{
    unsigned char *pB;
    uint32_t *pV;
    uint64_t i, k;

    /* Convert from little endian input */
    for (pV = V, i = 0, pB = B; i < 32 * r; i++, pV++) {
        *pV = *pB++;
        *pV |= *pB++ << 8;
        *pV |= *pB++ << 16;
        *pV |= (uint32_t)*pB++ << 24;
    }

    for (i = 1; i < N; i++, pV += 32 * r)
        scryptBlockMix(pV, pV - 32 * r, r);

    scryptBlockMix(X, V + (N - 1) * 32 * r, r);

    for (i = 0; i < N; i++) {
        uint32_t j;
        j = X[16 * (2 * r - 1)] % N;
        pV = V + 32 * r * j;
        for (k = 0; k < 32 * r; k++)
            T[k] = X[k] ^ *pV++;
        scryptBlockMix(X, T, r);
    }
    /* Convert output to little endian */
    for (i = 0, pB = B; i < 32 * r; i++) {
        uint32_t xtmp = X[i];
        *pB++ = xtmp & 0xff;
        *pB++ = (xtmp >> 8) & 0xff;
        *pB++ = (xtmp >> 16) & 0xff;
        *pB++ = (xtmp >> 24) & 0xff;
    }
}

#ifndef SIZE_MAX
# define SIZE_MAX    ((size_t)-1)
#endif

/*
 * Maximum power of two that will fit in uint64_t: this should work on
 * most (all?) platforms.
 */

#define LOG2_UINT64_MAX         (sizeof(uint64_t) * 8 - 1)

/*
 * Maximum value of p * r:
 * p <= ((2^32-1) * hLen) / MFLen =>
 * p <= ((2^32-1) * 32) / (128 * r) =>
 * p * r <= (2^30-1)
 */

#define SCRYPT_PR_MAX   ((1 << 30) - 1)

static int scrypt_alg(const char *pass, size_t passlen,
                      const unsigned char *salt, size_t saltlen,
                      uint64_t N, uint64_t r, uint64_t p, uint64_t maxmem,
                      unsigned char *key, size_t keylen, EVP_MD *sha256,
                      OSSL_LIB_CTX *libctx, const char *propq)
{
    int rv = 0;
    unsigned char *B;
    uint32_t *X, *V, *T;
    uint64_t i, Blen, Vlen;

    /* Sanity check parameters */
    /* initial check, r,p must be non zero, N >= 2 and a power of 2 */
    if (r == 0 || p == 0 || N < 2 || (N & (N - 1)))
        return 0;
    /* Check p * r < SCRYPT_PR_MAX avoiding overflow */
    if (p > SCRYPT_PR_MAX / r) {
        ERR_raise(ERR_LIB_EVP, EVP_R_MEMORY_LIMIT_EXCEEDED);
        return 0;
    }

    /*
     * Need to check N: if 2^(128 * r / 8) overflows limit this is
     * automatically satisfied since N <= UINT64_MAX.
     */

    if (16 * r <= LOG2_UINT64_MAX) {
        if (N >= (((uint64_t)1) << (16 * r))) {
            ERR_raise(ERR_LIB_EVP, EVP_R_MEMORY_LIMIT_EXCEEDED);
            return 0;
        }
    }

    /* Memory checks: check total allocated buffer size fits in uint64_t */

    /*
     * B size in section 5 step 1.S
     * Note: we know p * 128 * r < UINT64_MAX because we already checked
     * p * r < SCRYPT_PR_MAX
     */
    Blen = p * 128 * r;
    /*
     * Yet we pass it as integer to PKCS5_PBKDF2_HMAC... [This would
     * have to be revised when/if PKCS5_PBKDF2_HMAC accepts size_t.]
     */
    if (Blen > INT_MAX) {
        ERR_raise(ERR_LIB_EVP, EVP_R_MEMORY_LIMIT_EXCEEDED);
        return 0;
    }

    /*
     * Check 32 * r * (N + 2) * sizeof(uint32_t) fits in uint64_t
     * This is combined size V, X and T (section 4)
     */
    i = UINT64_MAX / (32 * sizeof(uint32_t));
    if (N + 2 > i / r) {
        ERR_raise(ERR_LIB_EVP, EVP_R_MEMORY_LIMIT_EXCEEDED);
        return 0;
    }
    Vlen = 32 * r * (N + 2) * sizeof(uint32_t);

    /* check total allocated size fits in uint64_t */
    if (Blen > UINT64_MAX - Vlen) {
        ERR_raise(ERR_LIB_EVP, EVP_R_MEMORY_LIMIT_EXCEEDED);
        return 0;
    }

    /* Check that the maximum memory doesn't exceed a size_t limits */
    if (maxmem > SIZE_MAX)
        maxmem = SIZE_MAX;

    if (Blen + Vlen > maxmem) {
        ERR_raise(ERR_LIB_EVP, EVP_R_MEMORY_LIMIT_EXCEEDED);
        return 0;
    }

    /* If no key return to indicate parameters are OK */
    if (key == NULL)
        return 1;

    B = OPENSSL_malloc((size_t)(Blen + Vlen));
    if (B == NULL)
        return 0;
    X = (uint32_t *)(B + Blen);
    T = X + 32 * r;
    V = T + 32 * r;
    if (ossl_pkcs5_pbkdf2_hmac_ex(pass, passlen, salt, saltlen, 1, sha256,
                                  (int)Blen, B, libctx, propq) == 0)
        goto err;

    for (i = 0; i < p; i++)
        scryptROMix(B + 128 * r * i, r, N, X, T, V);

    if (ossl_pkcs5_pbkdf2_hmac_ex(pass, passlen, B, (int)Blen, 1, sha256,
                                  keylen, key, libctx, propq) == 0)
        goto err;
    rv = 1;
 err:
    if (rv == 0)
        ERR_raise(ERR_LIB_EVP, EVP_R_PBKDF2_ERROR);

    OPENSSL_clear_free(B, (size_t)(Blen + Vlen));
    return rv;
}

#endif
