/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/core.h>
#include <openssl/core_names.h>
#include <openssl/crypto.h>
#include "internal/cryptlib.h"
#include "internal/numbers.h"
#include "internal/provider.h"
#include "internal/core.h"
#include "crypto/evp.h"
#include "evp_local.h"

struct evp_rand_st {
    OSSL_PROVIDER *prov;
    int name_id;
    char *type_name;
    const char *description;
    CRYPTO_REF_COUNT refcnt;

    const OSSL_DISPATCH *dispatch;
    OSSL_FUNC_rand_newctx_fn *newctx;
    OSSL_FUNC_rand_freectx_fn *freectx;
    OSSL_FUNC_rand_instantiate_fn *instantiate;
    OSSL_FUNC_rand_uninstantiate_fn *uninstantiate;
    OSSL_FUNC_rand_generate_fn *generate;
    OSSL_FUNC_rand_reseed_fn *reseed;
    OSSL_FUNC_rand_nonce_fn *nonce;
    OSSL_FUNC_rand_enable_locking_fn *enable_locking;
    OSSL_FUNC_rand_lock_fn *lock;
    OSSL_FUNC_rand_unlock_fn *unlock;
    OSSL_FUNC_rand_gettable_params_fn *gettable_params;
    OSSL_FUNC_rand_gettable_ctx_params_fn *gettable_ctx_params;
    OSSL_FUNC_rand_settable_ctx_params_fn *settable_ctx_params;
    OSSL_FUNC_rand_get_params_fn *get_params;
    OSSL_FUNC_rand_get_ctx_params_fn *get_ctx_params;
    OSSL_FUNC_rand_set_ctx_params_fn *set_ctx_params;
    OSSL_FUNC_rand_verify_zeroization_fn *verify_zeroization;
    OSSL_FUNC_rand_get_seed_fn *get_seed;
    OSSL_FUNC_rand_clear_seed_fn *clear_seed;
} /* EVP_RAND */ ;

static int evp_rand_up_ref(void *vrand)
{
    EVP_RAND *rand = (EVP_RAND *)vrand;
    int ref = 0;

    if (rand != NULL)
        return CRYPTO_UP_REF(&rand->refcnt, &ref);
    return 1;
}

static void evp_rand_free(void *vrand)
{
    EVP_RAND *rand = (EVP_RAND *)vrand;
    int ref = 0;

    if (rand == NULL)
        return;
    CRYPTO_DOWN_REF(&rand->refcnt, &ref);
    if (ref > 0)
        return;
    OPENSSL_free(rand->type_name);
    ossl_provider_free(rand->prov);
    CRYPTO_FREE_REF(&rand->refcnt);
    OPENSSL_free(rand);
}

static void *evp_rand_new(void)
{
    EVP_RAND *rand = OPENSSL_zalloc(sizeof(*rand));

    if (rand == NULL)
        return NULL;

    if (!CRYPTO_NEW_REF(&rand->refcnt, 1)) {
        OPENSSL_free(rand);
        return NULL;
    }
    return rand;
}

/* Enable locking of the underlying DRBG/RAND if available */
int EVP_RAND_enable_locking(EVP_RAND_CTX *rand)
{
    if (rand->meth->enable_locking != NULL)
        return rand->meth->enable_locking(rand->algctx);
    ERR_raise(ERR_LIB_EVP, EVP_R_LOCKING_NOT_SUPPORTED);
    return 0;
}

/* Lock the underlying DRBG/RAND if available */
static int evp_rand_lock(EVP_RAND_CTX *rand)
{
    if (rand->meth->lock != NULL)
        return rand->meth->lock(rand->algctx);
    return 1;
}

/* Unlock the underlying DRBG/RAND if available */
static void evp_rand_unlock(EVP_RAND_CTX *rand)
{
    if (rand->meth->unlock != NULL)
        rand->meth->unlock(rand->algctx);
}

static void *evp_rand_from_algorithm(int name_id,
                                     const OSSL_ALGORITHM *algodef,
                                     OSSL_PROVIDER *prov)
{
    const OSSL_DISPATCH *fns = algodef->implementation;
    EVP_RAND *rand = NULL;
    int fnrandcnt = 0, fnctxcnt = 0, fnlockcnt = 0, fnenablelockcnt = 0;
#ifdef FIPS_MODULE
    int fnzeroizecnt = 0;
#endif

    if ((rand = evp_rand_new()) == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        return NULL;
    }
    rand->name_id = name_id;
    if ((rand->type_name = ossl_algorithm_get1_first_name(algodef)) == NULL) {
        evp_rand_free(rand);
        return NULL;
    }
    rand->description = algodef->algorithm_description;
    rand->dispatch = fns;
    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_RAND_NEWCTX:
            if (rand->newctx != NULL)
                break;
            rand->newctx = OSSL_FUNC_rand_newctx(fns);
            fnctxcnt++;
            break;
        case OSSL_FUNC_RAND_FREECTX:
            if (rand->freectx != NULL)
                break;
            rand->freectx = OSSL_FUNC_rand_freectx(fns);
            fnctxcnt++;
            break;
        case OSSL_FUNC_RAND_INSTANTIATE:
            if (rand->instantiate != NULL)
                break;
            rand->instantiate = OSSL_FUNC_rand_instantiate(fns);
            fnrandcnt++;
            break;
        case OSSL_FUNC_RAND_UNINSTANTIATE:
             if (rand->uninstantiate != NULL)
                break;
            rand->uninstantiate = OSSL_FUNC_rand_uninstantiate(fns);
            fnrandcnt++;
            break;
        case OSSL_FUNC_RAND_GENERATE:
            if (rand->generate != NULL)
                break;
            rand->generate = OSSL_FUNC_rand_generate(fns);
            fnrandcnt++;
            break;
        case OSSL_FUNC_RAND_RESEED:
            if (rand->reseed != NULL)
                break;
            rand->reseed = OSSL_FUNC_rand_reseed(fns);
            break;
        case OSSL_FUNC_RAND_NONCE:
            if (rand->nonce != NULL)
                break;
            rand->nonce = OSSL_FUNC_rand_nonce(fns);
            break;
        case OSSL_FUNC_RAND_ENABLE_LOCKING:
            if (rand->enable_locking != NULL)
                break;
            rand->enable_locking = OSSL_FUNC_rand_enable_locking(fns);
            fnenablelockcnt++;
            break;
        case OSSL_FUNC_RAND_LOCK:
            if (rand->lock != NULL)
                break;
            rand->lock = OSSL_FUNC_rand_lock(fns);
            fnlockcnt++;
            break;
        case OSSL_FUNC_RAND_UNLOCK:
            if (rand->unlock != NULL)
                break;
            rand->unlock = OSSL_FUNC_rand_unlock(fns);
            fnlockcnt++;
            break;
        case OSSL_FUNC_RAND_GETTABLE_PARAMS:
            if (rand->gettable_params != NULL)
                break;
            rand->gettable_params =
                OSSL_FUNC_rand_gettable_params(fns);
            break;
        case OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS:
            if (rand->gettable_ctx_params != NULL)
                break;
            rand->gettable_ctx_params =
                OSSL_FUNC_rand_gettable_ctx_params(fns);
            break;
        case OSSL_FUNC_RAND_SETTABLE_CTX_PARAMS:
            if (rand->settable_ctx_params != NULL)
                break;
            rand->settable_ctx_params =
                OSSL_FUNC_rand_settable_ctx_params(fns);
            break;
        case OSSL_FUNC_RAND_GET_PARAMS:
            if (rand->get_params != NULL)
                break;
            rand->get_params = OSSL_FUNC_rand_get_params(fns);
            break;
        case OSSL_FUNC_RAND_GET_CTX_PARAMS:
            if (rand->get_ctx_params != NULL)
                break;
            rand->get_ctx_params = OSSL_FUNC_rand_get_ctx_params(fns);
            fnctxcnt++;
            break;
        case OSSL_FUNC_RAND_SET_CTX_PARAMS:
            if (rand->set_ctx_params != NULL)
                break;
            rand->set_ctx_params = OSSL_FUNC_rand_set_ctx_params(fns);
            break;
        case OSSL_FUNC_RAND_VERIFY_ZEROIZATION:
            if (rand->verify_zeroization != NULL)
                break;
            rand->verify_zeroization = OSSL_FUNC_rand_verify_zeroization(fns);
#ifdef FIPS_MODULE
            fnzeroizecnt++;
#endif
            break;
        case OSSL_FUNC_RAND_GET_SEED:
            if (rand->get_seed != NULL)
                break;
            rand->get_seed = OSSL_FUNC_rand_get_seed(fns);
            break;
        case OSSL_FUNC_RAND_CLEAR_SEED:
            if (rand->clear_seed != NULL)
                break;
            rand->clear_seed = OSSL_FUNC_rand_clear_seed(fns);
            break;
        }
    }
    /*
     * In order to be a consistent set of functions we must have at least
     * a complete set of "rand" functions and a complete set of context
     * management functions.  In FIPS mode, we also require the zeroization
     * verification function.
     *
     * In addition, if locking can be enabled, we need a complete set of
     * locking functions.
     */
    if (fnrandcnt != 3
            || fnctxcnt != 3
            || (fnenablelockcnt != 0 && fnenablelockcnt != 1)
            || (fnlockcnt != 0 && fnlockcnt != 2)
#ifdef FIPS_MODULE
            || fnzeroizecnt != 1
#endif
       ) {
        evp_rand_free(rand);
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_PROVIDER_FUNCTIONS);
        return NULL;
    }

    if (prov != NULL && !ossl_provider_up_ref(prov)) {
        evp_rand_free(rand);
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        return NULL;
    }
    rand->prov = prov;

    return rand;
}

EVP_RAND *EVP_RAND_fetch(OSSL_LIB_CTX *libctx, const char *algorithm,
                         const char *properties)
{
    return evp_generic_fetch(libctx, OSSL_OP_RAND, algorithm, properties,
                             evp_rand_from_algorithm, evp_rand_up_ref,
                             evp_rand_free);
}

int EVP_RAND_up_ref(EVP_RAND *rand)
{
    return evp_rand_up_ref(rand);
}

void EVP_RAND_free(EVP_RAND *rand)
{
    evp_rand_free(rand);
}

int evp_rand_get_number(const EVP_RAND *rand)
{
    return rand->name_id;
}

const char *EVP_RAND_get0_name(const EVP_RAND *rand)
{
    return rand->type_name;
}

const char *EVP_RAND_get0_description(const EVP_RAND *rand)
{
    return rand->description;
}

int EVP_RAND_is_a(const EVP_RAND *rand, const char *name)
{
    return rand != NULL && evp_is_a(rand->prov, rand->name_id, NULL, name);
}

const OSSL_PROVIDER *EVP_RAND_get0_provider(const EVP_RAND *rand)
{
    return rand->prov;
}

int EVP_RAND_get_params(EVP_RAND *rand, OSSL_PARAM params[])
{
    if (rand->get_params != NULL)
        return rand->get_params(params);
    return 1;
}

int EVP_RAND_CTX_up_ref(EVP_RAND_CTX *ctx)
{
    int ref = 0;

    return CRYPTO_UP_REF(&ctx->refcnt, &ref);
}

EVP_RAND_CTX *EVP_RAND_CTX_new(EVP_RAND *rand, EVP_RAND_CTX *parent)
{
    EVP_RAND_CTX *ctx;
    void *parent_ctx = NULL;
    const OSSL_DISPATCH *parent_dispatch = NULL;

    if (rand == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_NULL_ALGORITHM);
        return NULL;
    }

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;
    if (!CRYPTO_NEW_REF(&ctx->refcnt, 1)) {
        OPENSSL_free(ctx);
        return NULL;
    }
    if (parent != NULL) {
        if (!EVP_RAND_CTX_up_ref(parent)) {
            ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
            CRYPTO_FREE_REF(&ctx->refcnt);
            OPENSSL_free(ctx);
            return NULL;
        }
        parent_ctx = parent->algctx;
        parent_dispatch = parent->meth->dispatch;
    }
    if ((ctx->algctx = rand->newctx(ossl_provider_ctx(rand->prov), parent_ctx,
                                    parent_dispatch)) == NULL
            || !EVP_RAND_up_ref(rand)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        rand->freectx(ctx->algctx);
        CRYPTO_FREE_REF(&ctx->refcnt);
        OPENSSL_free(ctx);
        EVP_RAND_CTX_free(parent);
        return NULL;
    }
    ctx->meth = rand;
    ctx->parent = parent;
    return ctx;
}

void EVP_RAND_CTX_free(EVP_RAND_CTX *ctx)
{
    int ref = 0;
    EVP_RAND_CTX *parent;

    if (ctx == NULL)
        return;

    CRYPTO_DOWN_REF(&ctx->refcnt, &ref);
    if (ref > 0)
        return;
    parent = ctx->parent;
    ctx->meth->freectx(ctx->algctx);
    ctx->algctx = NULL;
    EVP_RAND_free(ctx->meth);
    CRYPTO_FREE_REF(&ctx->refcnt);
    OPENSSL_free(ctx);
    EVP_RAND_CTX_free(parent);
}

EVP_RAND *EVP_RAND_CTX_get0_rand(EVP_RAND_CTX *ctx)
{
    return ctx->meth;
}

static int evp_rand_get_ctx_params_locked(EVP_RAND_CTX *ctx,
                                          OSSL_PARAM params[])
{
    return ctx->meth->get_ctx_params(ctx->algctx, params);
}

int EVP_RAND_CTX_get_params(EVP_RAND_CTX *ctx, OSSL_PARAM params[])
{
    int res;

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_get_ctx_params_locked(ctx, params);
    evp_rand_unlock(ctx);
    return res;
}

static int evp_rand_set_ctx_params_locked(EVP_RAND_CTX *ctx,
                                          const OSSL_PARAM params[])
{
    if (ctx->meth->set_ctx_params != NULL)
        return ctx->meth->set_ctx_params(ctx->algctx, params);
    return 1;
}

int EVP_RAND_CTX_set_params(EVP_RAND_CTX *ctx, const OSSL_PARAM params[])
{
    int res;

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_set_ctx_params_locked(ctx, params);
    evp_rand_unlock(ctx);
    return res;
}

const OSSL_PARAM *EVP_RAND_gettable_params(const EVP_RAND *rand)
{
    if (rand->gettable_params == NULL)
        return NULL;
    return rand->gettable_params(ossl_provider_ctx(EVP_RAND_get0_provider(rand)));
}

const OSSL_PARAM *EVP_RAND_gettable_ctx_params(const EVP_RAND *rand)
{
    void *provctx;

    if (rand->gettable_ctx_params == NULL)
        return NULL;
    provctx = ossl_provider_ctx(EVP_RAND_get0_provider(rand));
    return rand->gettable_ctx_params(NULL, provctx);
}

const OSSL_PARAM *EVP_RAND_settable_ctx_params(const EVP_RAND *rand)
{
    void *provctx;

    if (rand->settable_ctx_params == NULL)
        return NULL;
    provctx = ossl_provider_ctx(EVP_RAND_get0_provider(rand));
    return rand->settable_ctx_params(NULL, provctx);
}

const OSSL_PARAM *EVP_RAND_CTX_gettable_params(EVP_RAND_CTX *ctx)
{
    void *provctx;

    if (ctx->meth->gettable_ctx_params == NULL)
        return NULL;
    provctx = ossl_provider_ctx(EVP_RAND_get0_provider(ctx->meth));
    return ctx->meth->gettable_ctx_params(ctx->algctx, provctx);
}

const OSSL_PARAM *EVP_RAND_CTX_settable_params(EVP_RAND_CTX *ctx)
{
    void *provctx;

    if (ctx->meth->settable_ctx_params == NULL)
        return NULL;
    provctx = ossl_provider_ctx(EVP_RAND_get0_provider(ctx->meth));
    return ctx->meth->settable_ctx_params(ctx->algctx, provctx);
}

void EVP_RAND_do_all_provided(OSSL_LIB_CTX *libctx,
                              void (*fn)(EVP_RAND *rand, void *arg),
                              void *arg)
{
    evp_generic_do_all(libctx, OSSL_OP_RAND,
                       (void (*)(void *, void *))fn, arg,
                       evp_rand_from_algorithm, evp_rand_up_ref,
                       evp_rand_free);
}

int EVP_RAND_names_do_all(const EVP_RAND *rand,
                          void (*fn)(const char *name, void *data),
                          void *data)
{
    if (rand->prov != NULL)
        return evp_names_do_all(rand->prov, rand->name_id, fn, data);

    return 1;
}

static int evp_rand_instantiate_locked
    (EVP_RAND_CTX *ctx, unsigned int strength, int prediction_resistance,
     const unsigned char *pstr, size_t pstr_len, const OSSL_PARAM params[])
{
    return ctx->meth->instantiate(ctx->algctx, strength, prediction_resistance,
                                  pstr, pstr_len, params);
}

int EVP_RAND_instantiate(EVP_RAND_CTX *ctx, unsigned int strength,
                         int prediction_resistance,
                         const unsigned char *pstr, size_t pstr_len,
                         const OSSL_PARAM params[])
{
    int res;

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_instantiate_locked(ctx, strength, prediction_resistance,
                                      pstr, pstr_len, params);
    evp_rand_unlock(ctx);
    return res;
}

static int evp_rand_uninstantiate_locked(EVP_RAND_CTX *ctx)
{
    return ctx->meth->uninstantiate(ctx->algctx);
}

int EVP_RAND_uninstantiate(EVP_RAND_CTX *ctx)
{
    int res;

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_uninstantiate_locked(ctx);
    evp_rand_unlock(ctx);
    return res;
}

static int evp_rand_generate_locked(EVP_RAND_CTX *ctx, unsigned char *out,
                                    size_t outlen, unsigned int strength,
                                    int prediction_resistance,
                                    const unsigned char *addin,
                                    size_t addin_len)
{
    size_t chunk, max_request = 0;
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };

    params[0] = OSSL_PARAM_construct_size_t(OSSL_RAND_PARAM_MAX_REQUEST,
                                            &max_request);
    if (!evp_rand_get_ctx_params_locked(ctx, params)
            || max_request == 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UNABLE_TO_GET_MAXIMUM_REQUEST_SIZE);
        return 0;
    }
    for (; outlen > 0; outlen -= chunk, out += chunk) {
        chunk = outlen > max_request ? max_request : outlen;
        if (!ctx->meth->generate(ctx->algctx, out, chunk, strength,
                                 prediction_resistance, addin, addin_len)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_GENERATE_ERROR);
            return 0;
        }
        /*
         * Prediction resistance is only relevant the first time around,
         * subsequently, the DRBG has already been properly reseeded.
         */
        prediction_resistance = 0;
    }
    return 1;
}

int EVP_RAND_generate(EVP_RAND_CTX *ctx, unsigned char *out, size_t outlen,
                      unsigned int strength, int prediction_resistance,
                      const unsigned char *addin, size_t addin_len)
{
    int res;

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_generate_locked(ctx, out, outlen, strength,
                                   prediction_resistance, addin, addin_len);
    evp_rand_unlock(ctx);
    return res;
}

static int evp_rand_reseed_locked(EVP_RAND_CTX *ctx, int prediction_resistance,
                                  const unsigned char *ent, size_t ent_len,
                                  const unsigned char *addin, size_t addin_len)
{
    if (ctx->meth->reseed != NULL)
        return ctx->meth->reseed(ctx->algctx, prediction_resistance,
                                 ent, ent_len, addin, addin_len);
    return 1;
}

int EVP_RAND_reseed(EVP_RAND_CTX *ctx, int prediction_resistance,
                    const unsigned char *ent, size_t ent_len,
                    const unsigned char *addin, size_t addin_len)
{
    int res;

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_reseed_locked(ctx, prediction_resistance,
                                 ent, ent_len, addin, addin_len);
    evp_rand_unlock(ctx);
    return res;
}

static unsigned int evp_rand_strength_locked(EVP_RAND_CTX *ctx)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    unsigned int strength = 0;

    params[0] = OSSL_PARAM_construct_uint(OSSL_RAND_PARAM_STRENGTH, &strength);
    if (!evp_rand_get_ctx_params_locked(ctx, params))
        return 0;
    return strength;
}

unsigned int EVP_RAND_get_strength(EVP_RAND_CTX *ctx)
{
    unsigned int res;

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_strength_locked(ctx);
    evp_rand_unlock(ctx);
    return res;
}

static int evp_rand_nonce_locked(EVP_RAND_CTX *ctx, unsigned char *out,
                                 size_t outlen)
{
    unsigned int str = evp_rand_strength_locked(ctx);

    if (ctx->meth->nonce != NULL)
        return ctx->meth->nonce(ctx->algctx, out, str, outlen, outlen) > 0;
    return evp_rand_generate_locked(ctx, out, outlen, str, 0, NULL, 0);
}

int EVP_RAND_nonce(EVP_RAND_CTX *ctx, unsigned char *out, size_t outlen)
{
    int res;

    if (ctx == NULL || out == NULL || outlen == 0) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_nonce_locked(ctx, out, outlen);
    evp_rand_unlock(ctx);
    return res;
}

int EVP_RAND_get_state(EVP_RAND_CTX *ctx)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    int state;

    params[0] = OSSL_PARAM_construct_int(OSSL_RAND_PARAM_STATE, &state);
    if (!EVP_RAND_CTX_get_params(ctx, params))
        state = EVP_RAND_STATE_ERROR;
    return state;
}

static int evp_rand_verify_zeroization_locked(EVP_RAND_CTX *ctx)
{
    if (ctx->meth->verify_zeroization != NULL)
        return ctx->meth->verify_zeroization(ctx->algctx);
    return 0;
}

int EVP_RAND_verify_zeroization(EVP_RAND_CTX *ctx)
{
    int res;

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_verify_zeroization_locked(ctx);
    evp_rand_unlock(ctx);
    return res;
}

int evp_rand_can_seed(EVP_RAND_CTX *ctx)
{
    return ctx->meth->get_seed != NULL;
}

static size_t evp_rand_get_seed_locked(EVP_RAND_CTX *ctx,
                                       unsigned char **buffer,
                                       int entropy,
                                       size_t min_len, size_t max_len,
                                       int prediction_resistance,
                                       const unsigned char *adin,
                                       size_t adin_len)
{
    if (ctx->meth->get_seed != NULL)
        return ctx->meth->get_seed(ctx->algctx, buffer,
                                   entropy, min_len, max_len,
                                   prediction_resistance,
                                   adin, adin_len);
    return 0;
}

size_t evp_rand_get_seed(EVP_RAND_CTX *ctx,
                         unsigned char **buffer,
                         int entropy, size_t min_len, size_t max_len,
                         int prediction_resistance,
                         const unsigned char *adin, size_t adin_len)
{
    int res;

    if (!evp_rand_lock(ctx))
        return 0;
    res = evp_rand_get_seed_locked(ctx,
                                   buffer,
                                   entropy, min_len, max_len,
                                   prediction_resistance,
                                   adin, adin_len);
    evp_rand_unlock(ctx);
    return res;
}

static void evp_rand_clear_seed_locked(EVP_RAND_CTX *ctx,
                                       unsigned char *buffer, size_t b_len)
{
    if (ctx->meth->clear_seed != NULL)
        ctx->meth->clear_seed(ctx->algctx, buffer, b_len);
}

void evp_rand_clear_seed(EVP_RAND_CTX *ctx,
                         unsigned char *buffer, size_t b_len)
{
    if (!evp_rand_lock(ctx))
        return;
    evp_rand_clear_seed_locked(ctx, buffer, b_len);
    evp_rand_unlock(ctx);
}
