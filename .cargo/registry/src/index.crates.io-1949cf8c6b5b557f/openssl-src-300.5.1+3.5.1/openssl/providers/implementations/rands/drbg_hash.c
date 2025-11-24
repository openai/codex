/*
 * Copyright 2011-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <openssl/sha.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/core_dispatch.h>
#include <openssl/proverr.h>
#include "internal/thread_once.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/provider_util.h"
#include "prov/implementations.h"
#include "drbg_local.h"
#include "crypto/evp.h"
#include "crypto/evp/evp_local.h"
#include "internal/provider.h"

static OSSL_FUNC_rand_newctx_fn drbg_hash_new_wrapper;
static OSSL_FUNC_rand_freectx_fn drbg_hash_free;
static OSSL_FUNC_rand_instantiate_fn drbg_hash_instantiate_wrapper;
static OSSL_FUNC_rand_uninstantiate_fn drbg_hash_uninstantiate_wrapper;
static OSSL_FUNC_rand_generate_fn drbg_hash_generate_wrapper;
static OSSL_FUNC_rand_reseed_fn drbg_hash_reseed_wrapper;
static OSSL_FUNC_rand_settable_ctx_params_fn drbg_hash_settable_ctx_params;
static OSSL_FUNC_rand_set_ctx_params_fn drbg_hash_set_ctx_params;
static OSSL_FUNC_rand_gettable_ctx_params_fn drbg_hash_gettable_ctx_params;
static OSSL_FUNC_rand_get_ctx_params_fn drbg_hash_get_ctx_params;
static OSSL_FUNC_rand_verify_zeroization_fn drbg_hash_verify_zeroization;

static int drbg_hash_set_ctx_params_locked(void *vctx, const OSSL_PARAM params[]);

/* 888 bits from SP800-90Ar1 10.1 table 2 */
#define HASH_PRNG_MAX_SEEDLEN    (888/8)

/* 440 bits from SP800-90Ar1 10.1 table 2 */
#define HASH_PRNG_SMALL_SEEDLEN   (440/8)

/* Determine what seedlen to use based on the block length */
#define MAX_BLOCKLEN_USING_SMALL_SEEDLEN (256/8)
#define INBYTE_IGNORE ((unsigned char)0xFF)

typedef struct rand_drbg_hash_st {
    PROV_DIGEST digest;
    EVP_MD_CTX *ctx;
    size_t blocklen;
    unsigned char V[HASH_PRNG_MAX_SEEDLEN];
    unsigned char C[HASH_PRNG_MAX_SEEDLEN];
    /* Temporary value storage: should always exceed max digest length */
    unsigned char vtmp[HASH_PRNG_MAX_SEEDLEN];
} PROV_DRBG_HASH;

/*
 * SP800-90Ar1 10.3.1 Derivation function using a Hash Function (Hash_df).
 * The input string used is composed of:
 *    inbyte - An optional leading byte (ignore if equal to INBYTE_IGNORE)
 *    in - input string 1 (A Non NULL value).
 *    in2 - optional input string (Can be NULL).
 *    in3 - optional input string (Can be NULL).
 *    These are concatenated as part of the DigestUpdate process.
 */
static int hash_df(PROV_DRBG *drbg, unsigned char *out,
                   const unsigned char inbyte,
                   const unsigned char *in, size_t inlen,
                   const unsigned char *in2, size_t in2len,
                   const unsigned char *in3, size_t in3len)
{
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)drbg->data;
    EVP_MD_CTX *ctx = hash->ctx;
    unsigned char *vtmp = hash->vtmp;
    /* tmp = counter || num_bits_returned || [inbyte] */
    unsigned char tmp[1 + 4 + 1];
    int tmp_sz = 0;
    size_t outlen = drbg->seedlen;
    size_t num_bits_returned = outlen * 8;
    /*
     * No need to check outlen size here, as the standard only ever needs
     * seedlen bytes which is always less than the maximum permitted.
     */

    /* (Step 3) counter = 1 (tmp[0] is the 8 bit counter) */
    tmp[tmp_sz++] = 1;
    /* tmp[1..4] is the fixed 32 bit no_of_bits_to_return */
    tmp[tmp_sz++] = (unsigned char)((num_bits_returned >> 24) & 0xff);
    tmp[tmp_sz++] = (unsigned char)((num_bits_returned >> 16) & 0xff);
    tmp[tmp_sz++] = (unsigned char)((num_bits_returned >> 8) & 0xff);
    tmp[tmp_sz++] = (unsigned char)(num_bits_returned & 0xff);
    /* Tack the additional input byte onto the end of tmp if it exists */
    if (inbyte != INBYTE_IGNORE)
        tmp[tmp_sz++] = inbyte;

    /* (Step 4) */
    for (;;) {
        /*
         * (Step 4.1) out = out || Hash(tmp || in || [in2] || [in3])
         *            (where tmp = counter || num_bits_returned || [inbyte])
         */
        if (!(EVP_DigestInit_ex(ctx, ossl_prov_digest_md(&hash->digest), NULL)
                && EVP_DigestUpdate(ctx, tmp, tmp_sz)
                && EVP_DigestUpdate(ctx, in, inlen)
                && (in2 == NULL || EVP_DigestUpdate(ctx, in2, in2len))
                && (in3 == NULL || EVP_DigestUpdate(ctx, in3, in3len))))
            return 0;

        if (outlen < hash->blocklen) {
            if (!EVP_DigestFinal(ctx, vtmp, NULL))
                return 0;
            memcpy(out, vtmp, outlen);
            OPENSSL_cleanse(vtmp, hash->blocklen);
            break;
        } else if (!EVP_DigestFinal(ctx, out, NULL)) {
            return 0;
        }

        outlen -= hash->blocklen;
        if (outlen == 0)
            break;
        /* (Step 4.2) counter++ */
        tmp[0]++;
        out += hash->blocklen;
    }
    return 1;
}

/* Helper function that just passes 2 input parameters to hash_df() */
static int hash_df1(PROV_DRBG *drbg, unsigned char *out,
                    const unsigned char in_byte,
                    const unsigned char *in1, size_t in1len)
{
    return hash_df(drbg, out, in_byte, in1, in1len, NULL, 0, NULL, 0);
}

/*
 * Add 2 byte buffers together. The first elements in each buffer are the top
 * most bytes. The result is stored in the dst buffer.
 * The final carry is ignored i.e: dst =  (dst + in) mod (2^seedlen_bits).
 * where dst size is drbg->seedlen, and inlen <= drbg->seedlen.
 */
static int add_bytes(PROV_DRBG *drbg, unsigned char *dst,
                     unsigned char *in, size_t inlen)
{
    size_t i;
    int result;
    const unsigned char *add;
    unsigned char carry = 0, *d;

    assert(drbg->seedlen >= 1 && inlen >= 1 && inlen <= drbg->seedlen);

    d = &dst[drbg->seedlen - 1];
    add = &in[inlen - 1];

    for (i = inlen; i > 0; i--, d--, add--) {
        result = *d + *add + carry;
        carry = (unsigned char)(result >> 8);
        *d = (unsigned char)(result & 0xff);
    }

    if (carry != 0) {
        /* Add the carry to the top of the dst if inlen is not the same size */
        for (i = drbg->seedlen - inlen; i > 0; --i, d--) {
            *d += 1;     /* Carry can only be 1 */
            if (*d != 0) /* exit if carry doesn't propagate to the next byte */
                break;
        }
    }
    return 1;
}

/* V = (V + Hash(inbyte || V  || [additional_input]) mod (2^seedlen) */
static int add_hash_to_v(PROV_DRBG *drbg, unsigned char inbyte,
                         const unsigned char *adin, size_t adinlen)
{
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)drbg->data;
    EVP_MD_CTX *ctx = hash->ctx;

    return EVP_DigestInit_ex(ctx, ossl_prov_digest_md(&hash->digest), NULL)
           && EVP_DigestUpdate(ctx, &inbyte, 1)
           && EVP_DigestUpdate(ctx, hash->V, drbg->seedlen)
           && (adin == NULL || EVP_DigestUpdate(ctx, adin, adinlen))
           && EVP_DigestFinal(ctx, hash->vtmp, NULL)
           && add_bytes(drbg, hash->V, hash->vtmp, hash->blocklen);
}

/*
 * The Hashgen() as listed in SP800-90Ar1 10.1.1.4 Hash_DRBG_Generate_Process.
 *
 * drbg contains the current value of V.
 * outlen is the requested number of bytes.
 * out is a buffer to return the generated bits.
 *
 * The algorithm to generate the bits is:
 *     data = V
 *     w = NULL
 *     for (i = 1 to m) {
 *        W = W || Hash(data)
 *        data = (data + 1) mod (2^seedlen)
 *     }
 *     out = Leftmost(W, outlen)
 *
 * Returns zero if an error occurs otherwise it returns 1.
 */
static int hash_gen(PROV_DRBG *drbg, unsigned char *out, size_t outlen)
{
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)drbg->data;
    unsigned char one = 1;

    if (outlen == 0)
        return 1;
    memcpy(hash->vtmp, hash->V, drbg->seedlen);
    for (;;) {
        if (!EVP_DigestInit_ex(hash->ctx, ossl_prov_digest_md(&hash->digest),
                               NULL)
                || !EVP_DigestUpdate(hash->ctx, hash->vtmp, drbg->seedlen))
            return 0;

        if (outlen < hash->blocklen) {
            if (!EVP_DigestFinal(hash->ctx, hash->vtmp, NULL))
                return 0;
            memcpy(out, hash->vtmp, outlen);
            return 1;
        } else {
            if (!EVP_DigestFinal(hash->ctx, out, NULL))
                return 0;
            outlen -= hash->blocklen;
            if (outlen == 0)
                break;
            out += hash->blocklen;
        }
        add_bytes(drbg, hash->vtmp, &one, 1);
    }
    return 1;
}

/*
 * SP800-90Ar1 10.1.1.2 Hash_DRBG_Instantiate_Process:
 *
 * ent is entropy input obtained from a randomness source of length ent_len.
 * nonce is a string of bytes of length nonce_len.
 * pstr is a personalization string received from an application. May be NULL.
 *
 * Returns zero if an error occurs otherwise it returns 1.
 */
static int drbg_hash_instantiate(PROV_DRBG *drbg,
                                 const unsigned char *ent, size_t ent_len,
                                 const unsigned char *nonce, size_t nonce_len,
                                 const unsigned char *pstr, size_t pstr_len)
{
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)drbg->data;

    EVP_MD_CTX_free(hash->ctx);
    hash->ctx = EVP_MD_CTX_new();

    /* (Step 1-3) V = Hash_df(entropy||nonce||pers, seedlen) */
    return hash->ctx != NULL
           && hash_df(drbg, hash->V, INBYTE_IGNORE,
                      ent, ent_len, nonce, nonce_len, pstr, pstr_len)
           /* (Step 4) C = Hash_df(0x00||V, seedlen) */
           && hash_df1(drbg, hash->C, 0x00, hash->V, drbg->seedlen);
}

static int drbg_hash_instantiate_wrapper(void *vdrbg, unsigned int strength,
                                         int prediction_resistance,
                                         const unsigned char *pstr,
                                         size_t pstr_len,
                                         const OSSL_PARAM params[])
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    int ret = 0;

    if (drbg->lock != NULL && !CRYPTO_THREAD_write_lock(drbg->lock))
        return 0;

    if (!ossl_prov_is_running()
            || !drbg_hash_set_ctx_params_locked(drbg, params))
        goto err;
    ret = ossl_prov_drbg_instantiate(drbg, strength, prediction_resistance,
                                     pstr, pstr_len);
 err:
    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);
    return ret;
}

/*
 * SP800-90Ar1 10.1.1.3 Hash_DRBG_Reseed_Process:
 *
 * ent is entropy input bytes obtained from a randomness source.
 * addin is additional input received from an application. May be NULL.
 *
 * Returns zero if an error occurs otherwise it returns 1.
 */
static int drbg_hash_reseed(PROV_DRBG *drbg,
                            const unsigned char *ent, size_t ent_len,
                            const unsigned char *adin, size_t adin_len)
{
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)drbg->data;

    /* (Step 1-2) V = Hash_df(0x01 || V || entropy_input || additional_input) */
    /* V about to be updated so use C as output instead */
    if (!hash_df(drbg, hash->C, 0x01, hash->V, drbg->seedlen, ent, ent_len,
                 adin, adin_len))
        return 0;
    memcpy(hash->V, hash->C, drbg->seedlen);
    /* (Step 4) C = Hash_df(0x00||V, seedlen) */
    return hash_df1(drbg, hash->C, 0x00, hash->V, drbg->seedlen);
}

static int drbg_hash_reseed_wrapper(void *vdrbg, int prediction_resistance,
                                    const unsigned char *ent, size_t ent_len,
                                    const unsigned char *adin, size_t adin_len)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;

    return ossl_prov_drbg_reseed(drbg, prediction_resistance, ent, ent_len,
                                 adin, adin_len);
}

/*
 * SP800-90Ar1 10.1.1.4 Hash_DRBG_Generate_Process:
 *
 * Generates pseudo random bytes using the drbg.
 * out is a buffer to fill with outlen bytes of pseudo random data.
 * addin is additional input received from an application. May be NULL.
 *
 * Returns zero if an error occurs otherwise it returns 1.
 */
static int drbg_hash_generate(PROV_DRBG *drbg,
                              unsigned char *out, size_t outlen,
                              const unsigned char *adin, size_t adin_len)
{
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)drbg->data;
    unsigned char counter[4];
    int reseed_counter = drbg->generate_counter;

    counter[0] = (unsigned char)((reseed_counter >> 24) & 0xff);
    counter[1] = (unsigned char)((reseed_counter >> 16) & 0xff);
    counter[2] = (unsigned char)((reseed_counter >> 8) & 0xff);
    counter[3] = (unsigned char)(reseed_counter & 0xff);

    return hash->ctx != NULL
           && (adin == NULL
           /* (Step 2) if adin != NULL then V = V + Hash(0x02||V||adin) */
               || adin_len == 0
               || add_hash_to_v(drbg, 0x02, adin, adin_len))
           /* (Step 3) Hashgen(outlen, V) */
           && hash_gen(drbg, out, outlen)
           /* (Step 4/5) H = V = (V + Hash(0x03||V) mod (2^seedlen_bits) */
           && add_hash_to_v(drbg, 0x03, NULL, 0)
           /* (Step 5) V = (V + H + C + reseed_counter) mod (2^seedlen_bits) */
           /* V = (V + C) mod (2^seedlen_bits) */
           && add_bytes(drbg, hash->V, hash->C, drbg->seedlen)
           /* V = (V + reseed_counter) mod (2^seedlen_bits) */
           && add_bytes(drbg, hash->V, counter, 4);
}

static int drbg_hash_generate_wrapper
    (void *vdrbg, unsigned char *out, size_t outlen, unsigned int strength,
     int prediction_resistance, const unsigned char *adin, size_t adin_len)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;

    return ossl_prov_drbg_generate(drbg, out, outlen, strength,
                                   prediction_resistance, adin, adin_len);
}

static int drbg_hash_uninstantiate(PROV_DRBG *drbg)
{
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)drbg->data;

    OPENSSL_cleanse(hash->V, sizeof(hash->V));
    OPENSSL_cleanse(hash->C, sizeof(hash->C));
    OPENSSL_cleanse(hash->vtmp, sizeof(hash->vtmp));
    return ossl_prov_drbg_uninstantiate(drbg);
}

static int drbg_hash_uninstantiate_wrapper(void *vdrbg)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    int ret;

    if (drbg->lock != NULL && !CRYPTO_THREAD_write_lock(drbg->lock))
        return 0;

    ret = drbg_hash_uninstantiate(drbg);

    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);

    return ret;
}

static int drbg_hash_verify_zeroization(void *vdrbg)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)drbg->data;
    int ret = 0;

    if (drbg->lock != NULL && !CRYPTO_THREAD_read_lock(drbg->lock))
        return 0;

    PROV_DRBG_VERIFY_ZEROIZATION(hash->V);
    PROV_DRBG_VERIFY_ZEROIZATION(hash->C);
    PROV_DRBG_VERIFY_ZEROIZATION(hash->vtmp);

    ret = 1;
 err:
    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);
    return ret;
}

static int drbg_hash_new(PROV_DRBG *ctx)
{
    PROV_DRBG_HASH *hash;

    hash = OPENSSL_secure_zalloc(sizeof(*hash));
    if (hash == NULL)
        return 0;

    OSSL_FIPS_IND_INIT(ctx)

    ctx->data = hash;
    ctx->seedlen = HASH_PRNG_MAX_SEEDLEN;
    ctx->max_entropylen = DRBG_MAX_LENGTH;
    ctx->max_noncelen = DRBG_MAX_LENGTH;
    ctx->max_perslen = DRBG_MAX_LENGTH;
    ctx->max_adinlen = DRBG_MAX_LENGTH;

    /* Maximum number of bits per request = 2^19  = 2^16 bytes */
    ctx->max_request = 1 << 16;
    return 1;
}

static void *drbg_hash_new_wrapper(void *provctx, void *parent,
                                   const OSSL_DISPATCH *parent_dispatch)
{
    return ossl_rand_drbg_new(provctx, parent, parent_dispatch,
                              &drbg_hash_new, &drbg_hash_free,
                              &drbg_hash_instantiate, &drbg_hash_uninstantiate,
                              &drbg_hash_reseed, &drbg_hash_generate);
}

static void drbg_hash_free(void *vdrbg)
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    PROV_DRBG_HASH *hash;

    if (drbg != NULL && (hash = (PROV_DRBG_HASH *)drbg->data) != NULL) {
        EVP_MD_CTX_free(hash->ctx);
        ossl_prov_digest_reset(&hash->digest);
        OPENSSL_secure_clear_free(hash, sizeof(*hash));
    }
    ossl_rand_drbg_free(drbg);
}

static int drbg_hash_get_ctx_params(void *vdrbg, OSSL_PARAM params[])
{
    PROV_DRBG *drbg = (PROV_DRBG *)vdrbg;
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)drbg->data;
    const EVP_MD *md;
    OSSL_PARAM *p;
    int ret = 0, complete = 0;

    if (!ossl_drbg_get_ctx_params_no_lock(drbg, params, &complete))
        return 0;

    if (complete)
        return 1;

    if (drbg->lock != NULL && !CRYPTO_THREAD_read_lock(drbg->lock))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_DRBG_PARAM_DIGEST);
    if (p != NULL) {
        md = ossl_prov_digest_md(&hash->digest);
        if (md == NULL || !OSSL_PARAM_set_utf8_string(p, EVP_MD_get0_name(md)))
            goto err;
    }

    ret = ossl_drbg_get_ctx_params(drbg, params);
 err:
    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);

    return ret;
}

static const OSSL_PARAM *drbg_hash_gettable_ctx_params(ossl_unused void *vctx,
                                                       ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_DRBG_PARAM_DIGEST, NULL, 0),
        OSSL_PARAM_DRBG_GETTABLE_CTX_COMMON,
        OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

static int drbg_fetch_digest_from_prov(const OSSL_PARAM params[],
                                       OSSL_LIB_CTX *libctx,
                                       EVP_MD **digest)
{
    OSSL_PROVIDER *prov = NULL;
    const OSSL_PARAM *p;
    EVP_MD *md = NULL;
    int ret = 0;

    if (digest == NULL)
        return 0;

    if ((p = OSSL_PARAM_locate_const(params,
                                     OSSL_PROV_PARAM_CORE_PROV_NAME)) == NULL)
        return 0;
    if (p->data_type != OSSL_PARAM_UTF8_STRING)
        return 0;
    if ((prov = ossl_provider_find(libctx, (const char *)p->data, 1)) == NULL)
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_DIGEST);
    if (p == NULL) {
        ret = 1;
        goto done;
    }

    if (p->data_type != OSSL_PARAM_UTF8_STRING)
        goto done;

    md = evp_digest_fetch_from_prov(prov, (const char *)p->data, NULL);
    if (md) {
        EVP_MD_free(*digest);
        *digest = md;
        ret = 1;
    }

done:
    ossl_provider_free(prov);
    return ret;
}

static int drbg_hash_set_ctx_params_locked(void *vctx, const OSSL_PARAM params[])
{
    PROV_DRBG *ctx = (PROV_DRBG *)vctx;
    PROV_DRBG_HASH *hash = (PROV_DRBG_HASH *)ctx->data;
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);
    EVP_MD *prov_md = NULL;
    const EVP_MD *md;
    int md_size;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(ctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_DRBG_PARAM_FIPS_DIGEST_CHECK))
        return 0;

    /* try to fetch digest from provider */
    (void)ERR_set_mark();
    if (!drbg_fetch_digest_from_prov(params, libctx, &prov_md)) {
        (void)ERR_pop_to_mark();
        /* fall back to full implementation search */
        if (!ossl_prov_digest_load_from_params(&hash->digest, params, libctx))
            return 0;
    } else {
        (void)ERR_clear_last_mark();
        if (prov_md)
            ossl_prov_digest_set_md(&hash->digest, prov_md);
    }

    md = ossl_prov_digest_md(&hash->digest);
    if (md != NULL) {
        if (!ossl_drbg_verify_digest(ctx, libctx, md))
            return 0;   /* Error already raised for us */

        /* These are taken from SP 800-90 10.1 Table 2 */
        md_size = EVP_MD_get_size(md);
        if (md_size <= 0)
            return 0;
        hash->blocklen = md_size;
        /* See SP800-57 Part1 Rev4 5.6.1 Table 3 */
        ctx->strength = 64 * (hash->blocklen >> 3);
        if (ctx->strength > 256)
            ctx->strength = 256;
        if (hash->blocklen > MAX_BLOCKLEN_USING_SMALL_SEEDLEN)
            ctx->seedlen = HASH_PRNG_MAX_SEEDLEN;
        else
            ctx->seedlen = HASH_PRNG_SMALL_SEEDLEN;

        ctx->min_entropylen = ctx->strength / 8;
        ctx->min_noncelen = ctx->min_entropylen / 2;
    }

    return ossl_drbg_set_ctx_params(ctx, params);
}

static int drbg_hash_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_DRBG *drbg = (PROV_DRBG *)vctx;
    int ret;

    if (drbg->lock != NULL && !CRYPTO_THREAD_write_lock(drbg->lock))
        return 0;

    ret = drbg_hash_set_ctx_params_locked(vctx, params);

    if (drbg->lock != NULL)
        CRYPTO_THREAD_unlock(drbg->lock);

    return ret;
}

static const OSSL_PARAM *drbg_hash_settable_ctx_params(ossl_unused void *vctx,
                                                       ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_DRBG_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_DRBG_PARAM_DIGEST, NULL, 0),
        OSSL_PARAM_DRBG_SETTABLE_CTX_COMMON,
        OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_DRBG_PARAM_FIPS_DIGEST_CHECK)
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

const OSSL_DISPATCH ossl_drbg_hash_functions[] = {
    { OSSL_FUNC_RAND_NEWCTX, (void(*)(void))drbg_hash_new_wrapper },
    { OSSL_FUNC_RAND_FREECTX, (void(*)(void))drbg_hash_free },
    { OSSL_FUNC_RAND_INSTANTIATE,
      (void(*)(void))drbg_hash_instantiate_wrapper },
    { OSSL_FUNC_RAND_UNINSTANTIATE,
      (void(*)(void))drbg_hash_uninstantiate_wrapper },
    { OSSL_FUNC_RAND_GENERATE, (void(*)(void))drbg_hash_generate_wrapper },
    { OSSL_FUNC_RAND_RESEED, (void(*)(void))drbg_hash_reseed_wrapper },
    { OSSL_FUNC_RAND_ENABLE_LOCKING, (void(*)(void))ossl_drbg_enable_locking },
    { OSSL_FUNC_RAND_LOCK, (void(*)(void))ossl_drbg_lock },
    { OSSL_FUNC_RAND_UNLOCK, (void(*)(void))ossl_drbg_unlock },
    { OSSL_FUNC_RAND_SETTABLE_CTX_PARAMS,
      (void(*)(void))drbg_hash_settable_ctx_params },
    { OSSL_FUNC_RAND_SET_CTX_PARAMS, (void(*)(void))drbg_hash_set_ctx_params },
    { OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS,
      (void(*)(void))drbg_hash_gettable_ctx_params },
    { OSSL_FUNC_RAND_GET_CTX_PARAMS, (void(*)(void))drbg_hash_get_ctx_params },
    { OSSL_FUNC_RAND_VERIFY_ZEROIZATION,
      (void(*)(void))drbg_hash_verify_zeroization },
    { OSSL_FUNC_RAND_GET_SEED, (void(*)(void))ossl_drbg_get_seed },
    { OSSL_FUNC_RAND_CLEAR_SEED, (void(*)(void))ossl_drbg_clear_seed },
    OSSL_DISPATCH_END
};
