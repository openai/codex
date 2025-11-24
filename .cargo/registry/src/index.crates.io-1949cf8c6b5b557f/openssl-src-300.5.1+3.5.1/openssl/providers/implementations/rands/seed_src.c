/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
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
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "crypto/rand.h"
#include "crypto/rand_pool.h"

static OSSL_FUNC_rand_newctx_fn seed_src_new;
static OSSL_FUNC_rand_freectx_fn seed_src_free;
static OSSL_FUNC_rand_instantiate_fn seed_src_instantiate;
static OSSL_FUNC_rand_uninstantiate_fn seed_src_uninstantiate;
static OSSL_FUNC_rand_generate_fn seed_src_generate;
static OSSL_FUNC_rand_reseed_fn seed_src_reseed;
static OSSL_FUNC_rand_gettable_ctx_params_fn seed_src_gettable_ctx_params;
static OSSL_FUNC_rand_get_ctx_params_fn seed_src_get_ctx_params;
static OSSL_FUNC_rand_verify_zeroization_fn seed_src_verify_zeroization;
static OSSL_FUNC_rand_enable_locking_fn seed_src_enable_locking;
static OSSL_FUNC_rand_lock_fn seed_src_lock;
static OSSL_FUNC_rand_unlock_fn seed_src_unlock;
static OSSL_FUNC_rand_get_seed_fn seed_get_seed;
static OSSL_FUNC_rand_clear_seed_fn seed_clear_seed;

typedef struct {
    void *provctx;
    int state;
} PROV_SEED_SRC;

static void *seed_src_new(void *provctx, void *parent,
                          const OSSL_DISPATCH *parent_dispatch)
{
    PROV_SEED_SRC *s;

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

static void seed_src_free(void *vseed)
{
    OPENSSL_free(vseed);
}

static int seed_src_instantiate(void *vseed, unsigned int strength,
                                int prediction_resistance,
                                const unsigned char *pstr, size_t pstr_len,
                                ossl_unused const OSSL_PARAM params[])
{
    PROV_SEED_SRC *s = (PROV_SEED_SRC *)vseed;

    s->state = EVP_RAND_STATE_READY;
    return 1;
}

static int seed_src_uninstantiate(void *vseed)
{
    PROV_SEED_SRC *s = (PROV_SEED_SRC *)vseed;

    s->state = EVP_RAND_STATE_UNINITIALISED;
    return 1;
}

static int seed_src_generate(void *vseed, unsigned char *out, size_t outlen,
                             unsigned int strength,
                             ossl_unused int prediction_resistance,
                             const unsigned char *adin,
                             size_t adin_len)
{
    PROV_SEED_SRC *s = (PROV_SEED_SRC *)vseed;
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

    /* Get entropy by polling system entropy sources. */
    entropy_available = ossl_pool_acquire_entropy(pool);

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

static int seed_src_reseed(void *vseed,
                           ossl_unused int prediction_resistance,
                           ossl_unused const unsigned char *ent,
                           ossl_unused size_t ent_len,
                           ossl_unused const unsigned char *adin,
                           ossl_unused size_t adin_len)
{
    PROV_SEED_SRC *s = (PROV_SEED_SRC *)vseed;

    if (s->state != EVP_RAND_STATE_READY) {
        ERR_raise(ERR_LIB_PROV,
                  s->state == EVP_RAND_STATE_ERROR ? PROV_R_IN_ERROR_STATE
                                                   : PROV_R_NOT_INSTANTIATED);
        return 0;
    }
    return 1;
}

static int seed_src_get_ctx_params(void *vseed, OSSL_PARAM params[])
{
    PROV_SEED_SRC *s = (PROV_SEED_SRC *)vseed;
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

static const OSSL_PARAM *seed_src_gettable_ctx_params(ossl_unused void *vseed,
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

static int seed_src_verify_zeroization(ossl_unused void *vseed)
{
    return 1;
}

static size_t seed_get_seed(void *vseed, unsigned char **pout,
                            int entropy, size_t min_len, size_t max_len,
                            int prediction_resistance,
                            const unsigned char *adin, size_t adin_len)
{
    size_t ret = 0;
    size_t entropy_available = 0;
    RAND_POOL *pool;

    pool = ossl_rand_pool_new(entropy, 1, min_len, max_len);
    if (pool == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_RAND_LIB);
        return 0;
    }

    /* Get entropy by polling system entropy sources. */
    entropy_available = ossl_pool_acquire_entropy(pool);

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

static void seed_clear_seed(ossl_unused void *vdrbg,
                            unsigned char *out, size_t outlen)
{
    OPENSSL_secure_clear_free(out, outlen);
}

static int seed_src_enable_locking(ossl_unused void *vseed)
{
    return 1;
}

int seed_src_lock(ossl_unused void *vctx)
{
    return 1;
}

void seed_src_unlock(ossl_unused void *vctx)
{
}

const OSSL_DISPATCH ossl_seed_src_functions[] = {
    { OSSL_FUNC_RAND_NEWCTX, (void(*)(void))seed_src_new },
    { OSSL_FUNC_RAND_FREECTX, (void(*)(void))seed_src_free },
    { OSSL_FUNC_RAND_INSTANTIATE,
      (void(*)(void))seed_src_instantiate },
    { OSSL_FUNC_RAND_UNINSTANTIATE,
      (void(*)(void))seed_src_uninstantiate },
    { OSSL_FUNC_RAND_GENERATE, (void(*)(void))seed_src_generate },
    { OSSL_FUNC_RAND_RESEED, (void(*)(void))seed_src_reseed },
    { OSSL_FUNC_RAND_ENABLE_LOCKING, (void(*)(void))seed_src_enable_locking },
    { OSSL_FUNC_RAND_LOCK, (void(*)(void))seed_src_lock },
    { OSSL_FUNC_RAND_UNLOCK, (void(*)(void))seed_src_unlock },
    { OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS,
      (void(*)(void))seed_src_gettable_ctx_params },
    { OSSL_FUNC_RAND_GET_CTX_PARAMS, (void(*)(void))seed_src_get_ctx_params },
    { OSSL_FUNC_RAND_VERIFY_ZEROIZATION,
      (void(*)(void))seed_src_verify_zeroization },
    { OSSL_FUNC_RAND_GET_SEED, (void(*)(void))seed_get_seed },
    { OSSL_FUNC_RAND_CLEAR_SEED, (void(*)(void))seed_clear_seed },
    OSSL_DISPATCH_END
};
