/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Implementation of SP 800-90B section 4.4 Approved Continuous Health Tests.
 */

#include <string.h>
#include <openssl/evp.h>
#include <openssl/core_dispatch.h>
#include <openssl/params.h>
#include <openssl/self_test.h>
#include <openssl/proverr.h>
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/implementations.h"
#include "internal/cryptlib.h"
#include "crypto/rand_pool.h"
#include "drbg_local.h"
#include "prov/seeding.h"
#include "crypto/context.h"

static OSSL_FUNC_rand_newctx_fn crng_test_new;
static OSSL_FUNC_rand_freectx_fn crng_test_free;
static OSSL_FUNC_rand_instantiate_fn crng_test_instantiate;
static OSSL_FUNC_rand_uninstantiate_fn crng_test_uninstantiate;
static OSSL_FUNC_rand_generate_fn crng_test_generate;
static OSSL_FUNC_rand_reseed_fn crng_test_reseed;
static OSSL_FUNC_rand_gettable_ctx_params_fn crng_test_gettable_ctx_params;
static OSSL_FUNC_rand_get_ctx_params_fn crng_test_get_ctx_params;
static OSSL_FUNC_rand_verify_zeroization_fn crng_test_verify_zeroization;
static OSSL_FUNC_rand_enable_locking_fn crng_test_enable_locking;
static OSSL_FUNC_rand_lock_fn crng_test_lock;
static OSSL_FUNC_rand_unlock_fn crng_test_unlock;
static OSSL_FUNC_rand_get_seed_fn crng_test_get_seed;
static OSSL_FUNC_rand_clear_seed_fn crng_test_clear_seed;

#ifndef ENTROPY_H
# define ENTROPY_H 6    /* default to six bits per byte of entropy */
#endif
#ifndef ENTROPY_APT_W
# define ENTROPY_APT_W 512
#endif

typedef struct crng_testal_st {
    void *provctx;
    CRYPTO_RWLOCK *lock;
    int state;

    /* State for SP 800-90B 4.4.1 Repetition Count Test */
    struct {
        unsigned int b;
        uint8_t a;
    } rct;

    /* State for SP 800-90B 4.4.2 Adaptive Proportion Test */
    struct {
        unsigned int b;
        unsigned int i;
        uint8_t a;
    } apt;

    /* Parent PROV_RAND and its dispatch table functions */
    void *parent;
    OSSL_FUNC_rand_enable_locking_fn *parent_enable_locking;
    OSSL_FUNC_rand_lock_fn *parent_lock;
    OSSL_FUNC_rand_unlock_fn *parent_unlock;
    OSSL_FUNC_rand_get_ctx_params_fn *parent_get_ctx_params;
    OSSL_FUNC_rand_gettable_ctx_params_fn *parent_gettable_ctx_params;
    OSSL_FUNC_rand_get_seed_fn *parent_get_seed;
    OSSL_FUNC_rand_clear_seed_fn *parent_clear_seed;
} CRNG_TEST;

/*
 * Some helper functions
 */
static int lock_parent(CRNG_TEST *crngt)
{
    void *parent = crngt->parent;

    if (parent != NULL
            && crngt->parent_lock != NULL
            && !crngt->parent_lock(parent)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_PARENT_LOCKING_NOT_ENABLED);
        return 0;
    }
    return 1;
}

static void unlock_parent(CRNG_TEST *crngt)
{
    void *parent = crngt->parent;

    if (parent != NULL && crngt->parent_unlock != NULL)
        crngt->parent_unlock(parent);
}

/*
 * Implementation of SP 800-90B section 4.4.1: Repetition Count Test
 */
static int RCT_test(CRNG_TEST *crngt, uint8_t next)
{
    /*
     * Critical values for this test are computed using:
     *
     *      C = 1 + \left\lceil\frac{-log_2 \alpha}H\right\rceil
     *
     * where alpha = 2^-20 and H is the expected entropy per sample.
     */
    static const unsigned int rct_c[9] = {
        41,                                 /* H = 0.5 */
        21, 11, 8, 6, 5, 5, 4, 4            /* H = 1, ..., 8 */
    };

    if (ossl_likely(crngt->rct.b != 0)
            && ossl_unlikely(next == crngt->rct.a))
        return ossl_likely(++crngt->rct.b < rct_c[ENTROPY_H]);
    crngt->rct.a = next;
    crngt->rct.b = 1;
    return 1;
}

/*
 * Implementation of SP 800-90B section 4.4.2: Adaptive Proportion Test
 */
static int APT_test(CRNG_TEST *crngt, uint8_t next)
{
    /*
     * Critical values for this test are drawn from a binomial
     * distribution with n = 512, p = 2^-H at a critical threshold of
     * 2^-20.  H being the expected entropy per sample.  Refer SP 800-90B
     * section 4.4.2, table 2.
     */
    static const unsigned int apt_c[9] = {
        410,                                /* H = 0.5 */
        311, 177, 103, 62, 39, 25, 18, 13   /* H = 1, ..., 8 */
    };

    if (ossl_likely(crngt->apt.b != 0)) {
        if (ossl_unlikely(crngt->apt.a == next)
                && ossl_unlikely(++crngt->apt.b >= apt_c[ENTROPY_H])) {
            crngt->apt.b = 0;
            return 0;
        }
        if (ossl_unlikely(++crngt->apt.i >= ENTROPY_APT_W))
            crngt->apt.b = 0;
        return 1;
    }
    crngt->apt.a = next;
    crngt->apt.b = 1;
    crngt->apt.i = 1;
    return 1;
}

static int crng_test(CRNG_TEST *crngt, const unsigned char *buf, size_t n)
{
    size_t i;

    for (i = 0; i < n; i++)
        if (!RCT_test(crngt, buf[i]) || !APT_test(crngt, buf[i])) {
            crngt->state = EVP_RAND_STATE_ERROR;
            ERR_raise(ERR_LIB_PROV,
                      PROV_R_ENTROPY_SOURCE_FAILED_CONTINUOUS_TESTS);
            return 0;
        }
    return 1;
}

static const OSSL_DISPATCH *find_call(const OSSL_DISPATCH *dispatch,
                                      int function)
{
    if (dispatch != NULL)
        while (dispatch->function_id != 0) {
            if (dispatch->function_id == function)
                return dispatch;
            dispatch++;
        }
    return NULL;
}

static void *crng_test_new(void *provctx, void *parent,
                           const OSSL_DISPATCH *p_dispatch)
{
    CRNG_TEST *crngt = OPENSSL_zalloc(sizeof(*crngt));
    const OSSL_DISPATCH *pfunc;

    if (crngt == NULL)
        return NULL;

    crngt->provctx = provctx;
    crngt->state = EVP_RAND_STATE_UNINITIALISED;

    /* Extract parent's functions */
    if (parent != NULL) {
        crngt->parent = parent;
        if ((pfunc = find_call(p_dispatch, OSSL_FUNC_RAND_ENABLE_LOCKING)) != NULL)
            crngt->parent_enable_locking = OSSL_FUNC_rand_enable_locking(pfunc);
        if ((pfunc = find_call(p_dispatch, OSSL_FUNC_RAND_LOCK)) != NULL)
            crngt->parent_lock = OSSL_FUNC_rand_lock(pfunc);
        if ((pfunc = find_call(p_dispatch, OSSL_FUNC_RAND_UNLOCK)) != NULL)
            crngt->parent_unlock = OSSL_FUNC_rand_unlock(pfunc);
        if ((pfunc = find_call(p_dispatch, OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS)) != NULL)
            crngt->parent_gettable_ctx_params = OSSL_FUNC_rand_gettable_ctx_params(pfunc);
        if ((pfunc = find_call(p_dispatch, OSSL_FUNC_RAND_GET_CTX_PARAMS)) != NULL)
            crngt->parent_get_ctx_params = OSSL_FUNC_rand_get_ctx_params(pfunc);
        if ((pfunc = find_call(p_dispatch, OSSL_FUNC_RAND_GET_SEED)) != NULL)
            crngt->parent_get_seed = OSSL_FUNC_rand_get_seed(pfunc);
        if ((pfunc = find_call(p_dispatch, OSSL_FUNC_RAND_CLEAR_SEED)) != NULL)
            crngt->parent_clear_seed = OSSL_FUNC_rand_clear_seed(pfunc);
    }

    return crngt;
}

static void crng_test_free(void *vcrngt)
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;

    if (crngt != NULL) {
        CRYPTO_THREAD_lock_free(crngt->lock);
        OPENSSL_free(crngt);
    }
}

static int crng_test_instantiate(void *vcrngt, unsigned int strength,
                                 int prediction_resistance,
                                 const unsigned char *pstr,
                                 size_t pstr_len,
                                 ossl_unused const OSSL_PARAM params[])
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;

    /* Start up health tests should go here */
    crngt->state = EVP_RAND_STATE_READY;
    return 1;
}

static int crng_test_uninstantiate(void *vcrngt)
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;

    crngt->state = EVP_RAND_STATE_UNINITIALISED;
    return 1;
}

static int crng_test_generate(void *vcrngt, unsigned char *out, size_t outlen,
                              unsigned int strength, int prediction_resistance,
                              const unsigned char *adin, size_t adin_len)
{
    unsigned char *p;
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;

    if (!crng_test_get_seed(crngt, &p, 0, outlen, outlen, prediction_resistance,
                            adin, adin_len))
        return 0;
    memcpy(out, p, outlen);
    crng_test_clear_seed(crngt, p, outlen);
    return 1;
}

static int crng_test_reseed(ossl_unused void *vcrngt,
                            ossl_unused int prediction_resistance,
                            ossl_unused const unsigned char *ent,
                            ossl_unused size_t ent_len,
                            ossl_unused const unsigned char *adin,
                            ossl_unused size_t adin_len)
{
    return 1;
}

static int crng_test_verify_zeroization(ossl_unused void *vcrngt)
{
    return 1;
}

static size_t crng_test_get_seed(void *vcrngt, unsigned char **pout,
                                 int entropy, size_t min_len,
                                 size_t max_len,
                                 int prediction_resistance,
                                 const unsigned char *adin,
                                 size_t adin_len)
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;
    size_t n;
    int r = 0;

    /* Without a parent, we rely on the up calls */
    if (crngt->parent == NULL
            || crngt->parent_get_seed == NULL) {
        n = ossl_prov_get_entropy(crngt->provctx, pout, entropy,
                                  min_len, max_len);
        if (n == 0)
            return 0;
        r = crng_test(crngt, *pout, n);
        return r > 0 ? n : 0;
    }

    /* Grab seed from our parent */
    if (!lock_parent(crngt))
        return 0;

    n = crngt->parent_get_seed(crngt->parent, pout, entropy,
                               min_len, max_len, prediction_resistance,
                               adin, adin_len);
    if (n > 0 && crng_test(crngt, *pout, n) > 0)
        r = n;
    else if (crngt->parent_clear_seed != NULL)
        crngt->parent_clear_seed(crngt->parent, *pout, n);
    unlock_parent(crngt);
    return r;
}

static void crng_test_clear_seed(void *vcrngt,
                                 unsigned char *out, size_t outlen)
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;

    if (crngt->parent == NULL || crngt->parent_get_seed == NULL)
        ossl_prov_cleanup_entropy(crngt->provctx, out, outlen);
    else if (crngt->parent_clear_seed != NULL)
        crngt->parent_clear_seed(crngt->parent, out, outlen);
}

static int crng_test_enable_locking(void *vcrngt)
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;

    if (crngt != NULL && crngt->lock == NULL) {
        if (crngt->parent_enable_locking != NULL)
            if (!crngt->parent_enable_locking(crngt->parent)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_PARENT_LOCKING_NOT_ENABLED);
                return 0;
            }
        crngt->lock = CRYPTO_THREAD_lock_new();
        if (crngt->lock == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_CREATE_LOCK);
            return 0;
        }
    }
    return 1;
}

static int crng_test_lock(ossl_unused void *vcrngt)
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;

    return crngt->lock == NULL || CRYPTO_THREAD_write_lock(crngt->lock);
}

static void crng_test_unlock(ossl_unused void *vcrngt)
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;

    if (crngt->lock != NULL)
        CRYPTO_THREAD_unlock(crngt->lock);
}

static int crng_test_get_ctx_params(void *vcrngt, OSSL_PARAM params[])
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;
    OSSL_PARAM *p;

    if (crngt->parent != NULL && crngt->parent_get_ctx_params != NULL)
        return crngt->parent_get_ctx_params(crngt->parent, params);

    /* No parent means we are using call backs for entropy */
    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STATE);
    if (p != NULL && !OSSL_PARAM_set_int(p, crngt->state))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STRENGTH);
    if (p != NULL && !OSSL_PARAM_set_int(p, 1024))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_MAX_REQUEST);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, 128))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_FIPS_APPROVED_INDICATOR);
    if (p != NULL && !OSSL_PARAM_set_int(p, 0))
        return 0;
    return 1;
}

static const OSSL_PARAM *crng_test_gettable_ctx_params(void *vcrngt,
                                                       void *provctx)
{
    CRNG_TEST *crngt = (CRNG_TEST *)vcrngt;
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_int(OSSL_RAND_PARAM_STATE, NULL),
        OSSL_PARAM_uint(OSSL_RAND_PARAM_STRENGTH, NULL),
        OSSL_PARAM_size_t(OSSL_RAND_PARAM_MAX_REQUEST, NULL),
        OSSL_PARAM_int(OSSL_RAND_PARAM_FIPS_APPROVED_INDICATOR, NULL),
        OSSL_PARAM_END
    };

    if (crngt->parent != NULL && crngt->parent_gettable_ctx_params != NULL)
        return crngt->parent_gettable_ctx_params(crngt->parent, provctx);
    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_crng_test_functions[] = {
    { OSSL_FUNC_RAND_NEWCTX, (void(*)(void))crng_test_new },
    { OSSL_FUNC_RAND_FREECTX, (void(*)(void))crng_test_free },
    { OSSL_FUNC_RAND_INSTANTIATE,
      (void(*)(void))crng_test_instantiate },
    { OSSL_FUNC_RAND_UNINSTANTIATE,
      (void(*)(void))crng_test_uninstantiate },
    { OSSL_FUNC_RAND_GENERATE, (void(*)(void))crng_test_generate },
    { OSSL_FUNC_RAND_RESEED, (void(*)(void))crng_test_reseed },
    { OSSL_FUNC_RAND_ENABLE_LOCKING, (void(*)(void))crng_test_enable_locking },
    { OSSL_FUNC_RAND_LOCK, (void(*)(void))crng_test_lock },
    { OSSL_FUNC_RAND_UNLOCK, (void(*)(void))crng_test_unlock },
    { OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS,
      (void(*)(void))crng_test_gettable_ctx_params },
    { OSSL_FUNC_RAND_GET_CTX_PARAMS, (void(*)(void))crng_test_get_ctx_params },
    { OSSL_FUNC_RAND_VERIFY_ZEROIZATION,
      (void(*)(void))crng_test_verify_zeroization },
    { OSSL_FUNC_RAND_GET_SEED, (void(*)(void))crng_test_get_seed },
    { OSSL_FUNC_RAND_CLEAR_SEED, (void(*)(void))crng_test_clear_seed },
    OSSL_DISPATCH_END
};
