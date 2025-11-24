/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/rand.h>
#include <openssl/core_dispatch.h>
#include <openssl/e_os2.h>
#include <openssl/params.h>
#include <openssl/core_names.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/randerr.h>
#include <openssl/proverr.h>
#include <openssl/self_test.h>
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "crypto/rand.h"
#include "crypto/rand_pool.h"

#ifndef OPENSSL_NO_JITTER
# include <jitterentropy.h>

# define JITTER_MAX_NUM_TRIES 3

static OSSL_FUNC_rand_newctx_fn jitter_new;
static OSSL_FUNC_rand_freectx_fn jitter_free;
static OSSL_FUNC_rand_instantiate_fn jitter_instantiate;
static OSSL_FUNC_rand_uninstantiate_fn jitter_uninstantiate;
static OSSL_FUNC_rand_generate_fn jitter_generate;
static OSSL_FUNC_rand_reseed_fn jitter_reseed;
static OSSL_FUNC_rand_gettable_ctx_params_fn jitter_gettable_ctx_params;
static OSSL_FUNC_rand_get_ctx_params_fn jitter_get_ctx_params;
static OSSL_FUNC_rand_verify_zeroization_fn jitter_verify_zeroization;
static OSSL_FUNC_rand_enable_locking_fn jitter_enable_locking;
static OSSL_FUNC_rand_lock_fn jitter_lock;
static OSSL_FUNC_rand_unlock_fn jitter_unlock;
static OSSL_FUNC_rand_get_seed_fn jitter_get_seed;
static OSSL_FUNC_rand_clear_seed_fn jitter_clear_seed;

typedef struct {
    void *provctx;
    int state;
} PROV_JITTER;

static size_t get_jitter_random_value(PROV_JITTER *s, unsigned char *buf, size_t len);

/*
 * Acquire entropy from jitterentropy library
 *
 * Returns the total entropy count, if it exceeds the requested
 * entropy count. Otherwise, returns an entropy count of 0.
 */
static size_t ossl_prov_acquire_entropy_from_jitter(PROV_JITTER *s,
                                                    RAND_POOL *pool)
{
    size_t bytes_needed;
    unsigned char *buffer;

    bytes_needed = ossl_rand_pool_bytes_needed(pool, 1 /* entropy_factor */);
    if (bytes_needed > 0) {
        buffer = ossl_rand_pool_add_begin(pool, bytes_needed);

        if (buffer != NULL) {
            if (get_jitter_random_value(s, buffer, bytes_needed) == bytes_needed) {
                ossl_rand_pool_add_end(pool, bytes_needed, 8 * bytes_needed);
            } else {
                ossl_rand_pool_add_end(pool, 0, 0);
            }
        }
    }

    return ossl_rand_pool_entropy_available(pool);
}

/* Obtain random bytes from the jitter library */
static size_t get_jitter_random_value(PROV_JITTER *s,
                                      unsigned char *buf, size_t len)
{
    struct rand_data *jitter_ec = NULL;
    ssize_t result = 0;
    size_t num_tries;

    /* Retry intermittent failures, then give up */
    for (num_tries = 0; num_tries < JITTER_MAX_NUM_TRIES; num_tries++) {
        /* Allocate a fresh collector */
        jitter_ec = jent_entropy_collector_alloc(0, JENT_FORCE_FIPS);
        if (jitter_ec == NULL)
            continue;

        /* Do not use _safe API as per typical security policies */
        result = jent_read_entropy(jitter_ec, (char *) buf, len);
        jent_entropy_collector_free(jitter_ec);

        /*
         * Permanent Failure
         * https://github.com/smuellerDD/jitterentropy-library/blob/master/doc/jitterentropy.3#L234
         */
        if (result < -5) {
            ossl_set_error_state(OSSL_SELF_TEST_TYPE_CRNG);
            break;
        }

        /* Success */
        if (result >= 0 && (size_t)result == len)
            return len;
    }

    /* Permanent failure or too many intermittent failures */
    s->state = EVP_RAND_STATE_ERROR;
    ERR_raise_data(ERR_LIB_RAND, RAND_R_ERROR_RETRIEVING_ENTROPY,
                   "jent_read_entropy (%d)", result);
    return 0;
}

static void *jitter_new(void *provctx, void *parent,
                        const OSSL_DISPATCH *parent_dispatch)
{
    PROV_JITTER *s;

    if (parent != NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_SEED_SOURCES_MUST_NOT_HAVE_A_PARENT);
        return NULL;
    }

    s = OPENSSL_zalloc(sizeof(*s));
    if (s == NULL)
        return NULL;

    s->provctx = provctx;
    s->state = EVP_RAND_STATE_UNINITIALISED;
    return s;
}

static void jitter_free(void *vseed)
{
    OPENSSL_free(vseed);
}

static int jitter_instantiate(void *vseed, unsigned int strength,
                              int prediction_resistance,
                              const unsigned char *pstr,
                              size_t pstr_len,
                              ossl_unused const OSSL_PARAM params[])
{
    PROV_JITTER *s = (PROV_JITTER *)vseed;
    int ret;

    if ((ret = jent_entropy_init_ex(0, JENT_FORCE_FIPS)) != 0) {
        ERR_raise_data(ERR_LIB_RAND, RAND_R_ERROR_RETRIEVING_ENTROPY,
                       "jent_entropy_init_ex (%d)", ret);
        s->state = EVP_RAND_STATE_ERROR;
        return 0;
    }

    s->state = EVP_RAND_STATE_READY;
    return 1;
}

static int jitter_uninstantiate(void *vseed)
{
    PROV_JITTER *s = (PROV_JITTER *)vseed;

    s->state = EVP_RAND_STATE_UNINITIALISED;
    return 1;
}

static int jitter_generate(void *vseed, unsigned char *out, size_t outlen,
                           unsigned int strength,
                           ossl_unused int prediction_resistance,
                           ossl_unused const unsigned char *adin,
                           ossl_unused size_t adin_len)
{
    PROV_JITTER *s = (PROV_JITTER *)vseed;
    size_t entropy_available;
    RAND_POOL *pool;

    if (s->state != EVP_RAND_STATE_READY) {
        ERR_raise(ERR_LIB_PROV,
                  s->state == EVP_RAND_STATE_ERROR ? PROV_R_IN_ERROR_STATE
                                                   : PROV_R_NOT_INSTANTIATED);
        return 0;
    }

    pool = ossl_rand_pool_new(strength, 1, outlen, outlen);
    if (pool == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_RAND_LIB);
        return 0;
    }

    /* Get entropy from jitter entropy library. */
    entropy_available = ossl_prov_acquire_entropy_from_jitter(s, pool);

    if (entropy_available > 0) {
        if (!ossl_rand_pool_adin_mix_in(pool, adin, adin_len)) {
            ossl_rand_pool_free(pool);
            return 0;
        }
        memcpy(out, ossl_rand_pool_buffer(pool), ossl_rand_pool_length(pool));
    }

    ossl_rand_pool_free(pool);
    return entropy_available > 0;
}

static int jitter_reseed(void *vseed,
                         ossl_unused int prediction_resistance,
                         ossl_unused const unsigned char *ent,
                         ossl_unused size_t ent_len,
                         ossl_unused const unsigned char *adin,
                         ossl_unused size_t adin_len)
{
    PROV_JITTER *s = (PROV_JITTER *)vseed;

    if (s->state != EVP_RAND_STATE_READY) {
        ERR_raise(ERR_LIB_PROV,
                  s->state == EVP_RAND_STATE_ERROR ? PROV_R_IN_ERROR_STATE
                                                   : PROV_R_NOT_INSTANTIATED);
        return 0;
    }
    return 1;
}

static int jitter_get_ctx_params(void *vseed, OSSL_PARAM params[])
{
    PROV_JITTER *s = (PROV_JITTER *)vseed;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STATE);
    if (p != NULL && !OSSL_PARAM_set_int(p, s->state))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STRENGTH);
    if (p != NULL && !OSSL_PARAM_set_int(p, 1024))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_MAX_REQUEST);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, 128))
        return 0;
    return 1;
}

static const OSSL_PARAM *jitter_gettable_ctx_params(ossl_unused void *vseed,
                                                    ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_int(OSSL_RAND_PARAM_STATE, NULL),
        OSSL_PARAM_uint(OSSL_RAND_PARAM_STRENGTH, NULL),
        OSSL_PARAM_size_t(OSSL_RAND_PARAM_MAX_REQUEST, NULL),
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

static int jitter_verify_zeroization(ossl_unused void *vseed)
{
    return 1;
}

static size_t jitter_get_seed(void *vseed, unsigned char **pout,
                              int entropy, size_t min_len,
                              size_t max_len,
                              int prediction_resistance,
                              const unsigned char *adin,
                              size_t adin_len)
{
    size_t ret = 0;
    size_t entropy_available = 0;
    RAND_POOL *pool;
    PROV_JITTER *s = (PROV_JITTER *)vseed;

    pool = ossl_rand_pool_new(entropy, 1, min_len, max_len);
    if (pool == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_RAND_LIB);
        return 0;
    }

    /* Get entropy from jitter entropy library. */
    entropy_available = ossl_prov_acquire_entropy_from_jitter(s, pool);

    if (entropy_available > 0
        && ossl_rand_pool_adin_mix_in(pool, adin, adin_len)) {
        ret = ossl_rand_pool_length(pool);
        *pout = ossl_rand_pool_detach(pool);
    } else {
        ERR_raise(ERR_LIB_PROV, PROV_R_ENTROPY_SOURCE_STRENGTH_TOO_WEAK);
    }
    ossl_rand_pool_free(pool);
    return ret;
}

# ifndef OPENSSL_NO_FIPS_JITTER
size_t ossl_rand_jitter_get_seed(unsigned char **pout, int entropy, size_t min_len, size_t max_len)
{
    size_t ret = 0;
    OSSL_PARAM params[1] = { OSSL_PARAM_END };
    PROV_JITTER *s = jitter_new(NULL, NULL, NULL);

    if (s == NULL)
        return ret;
    if (!jitter_instantiate(s, 0, 0, NULL, 0, params))
        goto end;
    ret = jitter_get_seed(s, pout, entropy, min_len, max_len, 0, NULL, 0);
 end:
    jitter_free(s);
    return ret;
}
# endif

static void jitter_clear_seed(ossl_unused void *vdrbg,
                              unsigned char *out, size_t outlen)
{
    OPENSSL_secure_clear_free(out, outlen);
}

static int jitter_enable_locking(ossl_unused void *vseed)
{
    return 1;
}

int jitter_lock(ossl_unused void *vctx)
{
    return 1;
}

void jitter_unlock(ossl_unused void *vctx)
{
}

const OSSL_DISPATCH ossl_jitter_functions[] = {
    { OSSL_FUNC_RAND_NEWCTX, (void(*)(void))jitter_new },
    { OSSL_FUNC_RAND_FREECTX, (void(*)(void))jitter_free },
    { OSSL_FUNC_RAND_INSTANTIATE,
      (void(*)(void))jitter_instantiate },
    { OSSL_FUNC_RAND_UNINSTANTIATE,
      (void(*)(void))jitter_uninstantiate },
    { OSSL_FUNC_RAND_GENERATE, (void(*)(void))jitter_generate },
    { OSSL_FUNC_RAND_RESEED, (void(*)(void))jitter_reseed },
    { OSSL_FUNC_RAND_ENABLE_LOCKING, (void(*)(void))jitter_enable_locking },
    { OSSL_FUNC_RAND_LOCK, (void(*)(void))jitter_lock },
    { OSSL_FUNC_RAND_UNLOCK, (void(*)(void))jitter_unlock },
    { OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS,
      (void(*)(void))jitter_gettable_ctx_params },
    { OSSL_FUNC_RAND_GET_CTX_PARAMS, (void(*)(void))jitter_get_ctx_params },
    { OSSL_FUNC_RAND_VERIFY_ZEROIZATION,
      (void(*)(void))jitter_verify_zeroization },
    { OSSL_FUNC_RAND_GET_SEED, (void(*)(void))jitter_get_seed },
    { OSSL_FUNC_RAND_CLEAR_SEED, (void(*)(void))jitter_clear_seed },
    OSSL_DISPATCH_END
};
#else
NON_EMPTY_TRANSLATION_UNIT
#endif
