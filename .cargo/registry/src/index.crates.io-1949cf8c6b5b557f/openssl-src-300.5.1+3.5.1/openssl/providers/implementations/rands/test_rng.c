/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdlib.h>
#include <openssl/core_dispatch.h>
#include <openssl/e_os2.h>
#include <openssl/params.h>
#include <openssl/core_names.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/randerr.h>
#include "prov/securitycheck.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/provider_util.h"
#include "prov/implementations.h"

static OSSL_FUNC_rand_newctx_fn test_rng_new;
static OSSL_FUNC_rand_freectx_fn test_rng_free;
static OSSL_FUNC_rand_instantiate_fn test_rng_instantiate;
static OSSL_FUNC_rand_uninstantiate_fn test_rng_uninstantiate;
static OSSL_FUNC_rand_generate_fn test_rng_generate;
static OSSL_FUNC_rand_reseed_fn test_rng_reseed;
static OSSL_FUNC_rand_nonce_fn test_rng_nonce;
static OSSL_FUNC_rand_settable_ctx_params_fn test_rng_settable_ctx_params;
static OSSL_FUNC_rand_set_ctx_params_fn test_rng_set_ctx_params;
static OSSL_FUNC_rand_gettable_ctx_params_fn test_rng_gettable_ctx_params;
static OSSL_FUNC_rand_get_ctx_params_fn test_rng_get_ctx_params;
static OSSL_FUNC_rand_verify_zeroization_fn test_rng_verify_zeroization;
static OSSL_FUNC_rand_enable_locking_fn test_rng_enable_locking;
static OSSL_FUNC_rand_lock_fn test_rng_lock;
static OSSL_FUNC_rand_unlock_fn test_rng_unlock;
static OSSL_FUNC_rand_get_seed_fn test_rng_get_seed;

typedef struct {
    void *provctx;
    unsigned int generate;
    int state;
    unsigned int strength;
    size_t max_request;
    unsigned char *entropy, *nonce;
    size_t entropy_len, entropy_pos, nonce_len;
    CRYPTO_RWLOCK *lock;
    uint32_t seed;
} PROV_TEST_RNG;

static void *test_rng_new(void *provctx, void *parent,
                          const OSSL_DISPATCH *parent_dispatch)
{
    PROV_TEST_RNG *t;

    t = OPENSSL_zalloc(sizeof(*t));
    if (t == NULL)
        return NULL;

    t->max_request = INT_MAX;
    t->provctx = provctx;
    t->state = EVP_RAND_STATE_UNINITIALISED;
    return t;
}

static void test_rng_free(void *vtest)
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;

    if (t == NULL)
        return;
    OPENSSL_free(t->entropy);
    OPENSSL_free(t->nonce);
    CRYPTO_THREAD_lock_free(t->lock);
    OPENSSL_free(t);
}

static int test_rng_instantiate(void *vtest, unsigned int strength,
                                int prediction_resistance,
                                const unsigned char *pstr, size_t pstr_len,
                                const OSSL_PARAM params[])
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;

    if (!test_rng_set_ctx_params(t, params) || strength > t->strength)
        return 0;

    t->state = EVP_RAND_STATE_READY;
    t->entropy_pos = 0;
    t->seed = 221953166;    /* Value doesn't matter, so long as it isn't zero */

    return 1;
}

static int test_rng_uninstantiate(void *vtest)
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;

    t->entropy_pos = 0;
    t->state = EVP_RAND_STATE_UNINITIALISED;
    return 1;
}

static unsigned char gen_byte(PROV_TEST_RNG *t)
{
    uint32_t n;

    /*
     * Implement the 32 bit xorshift as suggested by George Marsaglia in:
     *      https://doi.org/10.18637/jss.v008.i14
     *
     * This is a very fast PRNG so there is no need to extract bytes one at a
     * time and use the entire value each time.
     */
    n = t->seed;
    n ^= n << 13;
    n ^= n >> 17;
    n ^= n << 5;
    t->seed = n;

    return n & 0xff;
}

static int test_rng_generate(void *vtest, unsigned char *out, size_t outlen,
                             unsigned int strength, int prediction_resistance,
                             const unsigned char *adin, size_t adin_len)
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;
    size_t i;

    if (strength > t->strength)
        return 0;
    if (t->generate) {
        for (i = 0; i < outlen; i++)
            out[i] = gen_byte(t);
    } else {
        if (t->entropy_len - t->entropy_pos < outlen)
            return 0;

        memcpy(out, t->entropy + t->entropy_pos, outlen);
        t->entropy_pos += outlen;
    }
    return 1;
}

static int test_rng_reseed(ossl_unused void *vtest,
                           ossl_unused int prediction_resistance,
                           ossl_unused const unsigned char *ent,
                           ossl_unused size_t ent_len,
                           ossl_unused const unsigned char *adin,
                           ossl_unused size_t adin_len)
{
    return 1;
}

static size_t test_rng_nonce(void *vtest, unsigned char *out,
                             unsigned int strength, size_t min_noncelen,
                             size_t max_noncelen)
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;
    size_t i;

    if (strength > t->strength)
        return 0;

    if (t->generate) {
        for (i = 0; i < min_noncelen; i++)
            out[i] = gen_byte(t);
        return min_noncelen;
    }

    if (t->nonce == NULL)
        return 0;
    i = t->nonce_len > max_noncelen ? max_noncelen : t->nonce_len;
    if (out != NULL)
        memcpy(out, t->nonce, i);
    return i;
}

static int test_rng_get_ctx_params(void *vtest, OSSL_PARAM params[])
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STATE);
    if (p != NULL && !OSSL_PARAM_set_int(p, t->state))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STRENGTH);
    if (p != NULL && !OSSL_PARAM_set_int(p, t->strength))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_MAX_REQUEST);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, t->max_request))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_GENERATE);
    if (p != NULL && !OSSL_PARAM_set_uint(p, t->generate))
        return 0;

#ifdef FIPS_MODULE
    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_FIPS_APPROVED_INDICATOR);
    if (p != NULL && !OSSL_PARAM_set_int(p, 0))
        return 0;
#endif  /* FIPS_MODULE */
    return 1;
}

static const OSSL_PARAM *test_rng_gettable_ctx_params(ossl_unused void *vtest,
                                                      ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_int(OSSL_RAND_PARAM_STATE, NULL),
        OSSL_PARAM_uint(OSSL_RAND_PARAM_STRENGTH, NULL),
        OSSL_PARAM_size_t(OSSL_RAND_PARAM_MAX_REQUEST, NULL),
        OSSL_PARAM_uint(OSSL_RAND_PARAM_GENERATE, NULL),
        OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

static int test_rng_set_ctx_params(void *vtest, const OSSL_PARAM params[])
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;
    const OSSL_PARAM *p;
    void *ptr = NULL;
    size_t size = 0;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_RAND_PARAM_STRENGTH);
    if (p != NULL && !OSSL_PARAM_get_uint(p, &t->strength))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_RAND_PARAM_TEST_ENTROPY);
    if (p != NULL) {
        if (!OSSL_PARAM_get_octet_string(p, &ptr, 0, &size))
            return 0;
        OPENSSL_free(t->entropy);
        t->entropy = ptr;
        t->entropy_len = size;
        t->entropy_pos = 0;
        ptr = NULL;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_RAND_PARAM_TEST_NONCE);
    if (p != NULL) {
        if (!OSSL_PARAM_get_octet_string(p, &ptr, 0, &size))
            return 0;
        OPENSSL_free(t->nonce);
        t->nonce = ptr;
        t->nonce_len = size;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_RAND_PARAM_MAX_REQUEST);
    if (p != NULL && !OSSL_PARAM_get_size_t(p, &t->max_request))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_RAND_PARAM_GENERATE);
    if (p != NULL && !OSSL_PARAM_get_uint(p, &t->generate))
        return 0;
    return 1;
}

static const OSSL_PARAM *test_rng_settable_ctx_params(ossl_unused void *vtest,
                                                      ossl_unused void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_RAND_PARAM_TEST_NONCE, NULL, 0),
        OSSL_PARAM_uint(OSSL_RAND_PARAM_STRENGTH, NULL),
        OSSL_PARAM_size_t(OSSL_RAND_PARAM_MAX_REQUEST, NULL),
        OSSL_PARAM_uint(OSSL_RAND_PARAM_GENERATE, NULL),
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int test_rng_verify_zeroization(ossl_unused void *vtest)
{
    return 1;
}

static size_t test_rng_get_seed(void *vtest, unsigned char **pout,
                                int entropy, size_t min_len, size_t max_len,
                                ossl_unused int prediction_resistance,
                                ossl_unused const unsigned char *adin,
                                ossl_unused size_t adin_len)
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;

    *pout = t->entropy;
    return  t->entropy_len > max_len ? max_len : t->entropy_len;
}

static int test_rng_enable_locking(void *vtest)
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;

    if (t != NULL && t->lock == NULL) {
        t->lock = CRYPTO_THREAD_lock_new();
        if (t->lock == NULL) {
            ERR_raise(ERR_LIB_PROV, RAND_R_FAILED_TO_CREATE_LOCK);
            return 0;
        }
    }
    return 1;
}

static int test_rng_lock(void *vtest)
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;

    if (t == NULL || t->lock == NULL)
        return 1;
    return CRYPTO_THREAD_write_lock(t->lock);
}

static void test_rng_unlock(void *vtest)
{
    PROV_TEST_RNG *t = (PROV_TEST_RNG *)vtest;

    if (t != NULL && t->lock != NULL)
        CRYPTO_THREAD_unlock(t->lock);
}

const OSSL_DISPATCH ossl_test_rng_functions[] = {
    { OSSL_FUNC_RAND_NEWCTX, (void(*)(void))test_rng_new },
    { OSSL_FUNC_RAND_FREECTX, (void(*)(void))test_rng_free },
    { OSSL_FUNC_RAND_INSTANTIATE,
      (void(*)(void))test_rng_instantiate },
    { OSSL_FUNC_RAND_UNINSTANTIATE,
      (void(*)(void))test_rng_uninstantiate },
    { OSSL_FUNC_RAND_GENERATE, (void(*)(void))test_rng_generate },
    { OSSL_FUNC_RAND_RESEED, (void(*)(void))test_rng_reseed },
    { OSSL_FUNC_RAND_NONCE, (void(*)(void))test_rng_nonce },
    { OSSL_FUNC_RAND_ENABLE_LOCKING, (void(*)(void))test_rng_enable_locking },
    { OSSL_FUNC_RAND_LOCK, (void(*)(void))test_rng_lock },
    { OSSL_FUNC_RAND_UNLOCK, (void(*)(void))test_rng_unlock },
    { OSSL_FUNC_RAND_SETTABLE_CTX_PARAMS,
      (void(*)(void))test_rng_settable_ctx_params },
    { OSSL_FUNC_RAND_SET_CTX_PARAMS, (void(*)(void))test_rng_set_ctx_params },
    { OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS,
      (void(*)(void))test_rng_gettable_ctx_params },
    { OSSL_FUNC_RAND_GET_CTX_PARAMS, (void(*)(void))test_rng_get_ctx_params },
    { OSSL_FUNC_RAND_VERIFY_ZEROIZATION,
      (void(*)(void))test_rng_verify_zeroization },
    { OSSL_FUNC_RAND_GET_SEED, (void(*)(void))test_rng_get_seed },
    OSSL_DISPATCH_END
};
