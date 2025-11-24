/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include "internal/cryptlib.h"
#include "internal/refcount.h"
#include "internal/provider.h"
#include "internal/core.h"
#include "internal/numbers.h"   /* includes SIZE_MAX */
#include "crypto/evp.h"
#include "evp_local.h"

static void evp_keyexch_free(void *data)
{
    EVP_KEYEXCH_free(data);
}

static int evp_keyexch_up_ref(void *data)
{
    return EVP_KEYEXCH_up_ref(data);
}

static EVP_KEYEXCH *evp_keyexch_new(OSSL_PROVIDER *prov)
{
    EVP_KEYEXCH *exchange = OPENSSL_zalloc(sizeof(EVP_KEYEXCH));

    if (exchange == NULL)
        return NULL;

    if (!CRYPTO_NEW_REF(&exchange->refcnt, 1)
        || !ossl_provider_up_ref(prov)) {
        CRYPTO_FREE_REF(&exchange->refcnt);
        OPENSSL_free(exchange);
        return NULL;
    }
    exchange->prov = prov;

    return exchange;
}

static void *evp_keyexch_from_algorithm(int name_id,
                                        const OSSL_ALGORITHM *algodef,
                                        OSSL_PROVIDER *prov)
{
    const OSSL_DISPATCH *fns = algodef->implementation;
    EVP_KEYEXCH *exchange = NULL;
    int fncnt = 0, sparamfncnt = 0, gparamfncnt = 0;

    if ((exchange = evp_keyexch_new(prov)) == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        goto err;
    }

    exchange->name_id = name_id;
    if ((exchange->type_name = ossl_algorithm_get1_first_name(algodef)) == NULL)
        goto err;
    exchange->description = algodef->algorithm_description;

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_KEYEXCH_NEWCTX:
            if (exchange->newctx != NULL)
                break;
            exchange->newctx = OSSL_FUNC_keyexch_newctx(fns);
            fncnt++;
            break;
        case OSSL_FUNC_KEYEXCH_INIT:
            if (exchange->init != NULL)
                break;
            exchange->init = OSSL_FUNC_keyexch_init(fns);
            fncnt++;
            break;
        case OSSL_FUNC_KEYEXCH_SET_PEER:
            if (exchange->set_peer != NULL)
                break;
            exchange->set_peer = OSSL_FUNC_keyexch_set_peer(fns);
            break;
        case OSSL_FUNC_KEYEXCH_DERIVE:
            if (exchange->derive != NULL)
                break;
            exchange->derive = OSSL_FUNC_keyexch_derive(fns);
            fncnt++;
            break;
        case OSSL_FUNC_KEYEXCH_FREECTX:
            if (exchange->freectx != NULL)
                break;
            exchange->freectx = OSSL_FUNC_keyexch_freectx(fns);
            fncnt++;
            break;
        case OSSL_FUNC_KEYEXCH_DUPCTX:
            if (exchange->dupctx != NULL)
                break;
            exchange->dupctx = OSSL_FUNC_keyexch_dupctx(fns);
            break;
        case OSSL_FUNC_KEYEXCH_GET_CTX_PARAMS:
            if (exchange->get_ctx_params != NULL)
                break;
            exchange->get_ctx_params = OSSL_FUNC_keyexch_get_ctx_params(fns);
            gparamfncnt++;
            break;
        case OSSL_FUNC_KEYEXCH_GETTABLE_CTX_PARAMS:
            if (exchange->gettable_ctx_params != NULL)
                break;
            exchange->gettable_ctx_params
                = OSSL_FUNC_keyexch_gettable_ctx_params(fns);
            gparamfncnt++;
            break;
        case OSSL_FUNC_KEYEXCH_SET_CTX_PARAMS:
            if (exchange->set_ctx_params != NULL)
                break;
            exchange->set_ctx_params = OSSL_FUNC_keyexch_set_ctx_params(fns);
            sparamfncnt++;
            break;
        case OSSL_FUNC_KEYEXCH_SETTABLE_CTX_PARAMS:
            if (exchange->settable_ctx_params != NULL)
                break;
            exchange->settable_ctx_params
                = OSSL_FUNC_keyexch_settable_ctx_params(fns);
            sparamfncnt++;
            break;
        }
    }
    if (fncnt != 4
            || (gparamfncnt != 0 && gparamfncnt != 2)
            || (sparamfncnt != 0 && sparamfncnt != 2)) {
        /*
         * In order to be a consistent set of functions we must have at least
         * a complete set of "exchange" functions: init, derive, newctx,
         * and freectx. The set_ctx_params and settable_ctx_params functions are
         * optional, but if one of them is present then the other one must also
         * be present. Same goes for get_ctx_params and gettable_ctx_params.
         * The dupctx and set_peer functions are optional.
         */
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_PROVIDER_FUNCTIONS);
        goto err;
    }

    return exchange;

 err:
    EVP_KEYEXCH_free(exchange);
    return NULL;
}

void EVP_KEYEXCH_free(EVP_KEYEXCH *exchange)
{
    int i;

    if (exchange == NULL)
        return;
    CRYPTO_DOWN_REF(&exchange->refcnt, &i);
    if (i > 0)
        return;
    OPENSSL_free(exchange->type_name);
    ossl_provider_free(exchange->prov);
    CRYPTO_FREE_REF(&exchange->refcnt);
    OPENSSL_free(exchange);
}

int EVP_KEYEXCH_up_ref(EVP_KEYEXCH *exchange)
{
    int ref = 0;

    CRYPTO_UP_REF(&exchange->refcnt, &ref);
    return 1;
}

OSSL_PROVIDER *EVP_KEYEXCH_get0_provider(const EVP_KEYEXCH *exchange)
{
    return exchange->prov;
}

EVP_KEYEXCH *EVP_KEYEXCH_fetch(OSSL_LIB_CTX *ctx, const char *algorithm,
                               const char *properties)
{
    return evp_generic_fetch(ctx, OSSL_OP_KEYEXCH, algorithm, properties,
                             evp_keyexch_from_algorithm,
                             evp_keyexch_up_ref,
                             evp_keyexch_free);
}

EVP_KEYEXCH *evp_keyexch_fetch_from_prov(OSSL_PROVIDER *prov,
                                         const char *algorithm,
                                         const char *properties)
{
    return evp_generic_fetch_from_prov(prov, OSSL_OP_KEYEXCH,
                                       algorithm, properties,
                                       evp_keyexch_from_algorithm,
                                       evp_keyexch_up_ref,
                                       evp_keyexch_free);
}

int EVP_PKEY_derive_init(EVP_PKEY_CTX *ctx)
{
    return EVP_PKEY_derive_init_ex(ctx, NULL);
}

int EVP_PKEY_derive_init_ex(EVP_PKEY_CTX *ctx, const OSSL_PARAM params[])
{
    int ret;
    void *provkey = NULL;
    EVP_KEYEXCH *exchange = NULL;
    EVP_KEYMGMT *tmp_keymgmt = NULL;
    const OSSL_PROVIDER *tmp_prov = NULL;
    const char *supported_exch = NULL;
    int iter;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return -2;
    }

    evp_pkey_ctx_free_old_ops(ctx);
    ctx->operation = EVP_PKEY_OP_DERIVE;

    ERR_set_mark();

    if (evp_pkey_ctx_is_legacy(ctx))
        goto legacy;

    /*
     * Some algorithms (e.g. legacy KDFs) don't have a pkey - so we create
     * a blank one.
     */
    if (ctx->pkey == NULL) {
        EVP_PKEY *pkey = EVP_PKEY_new();

        if (pkey == NULL
            || !EVP_PKEY_set_type_by_keymgmt(pkey, ctx->keymgmt)
            || (pkey->keydata = evp_keymgmt_newdata(ctx->keymgmt)) == NULL) {
            ERR_clear_last_mark();
            EVP_PKEY_free(pkey);
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            goto err;
        }
        ctx->pkey = pkey;
    }

    /*
     * Try to derive the supported exch from |ctx->keymgmt|.
     */
    if (!ossl_assert(ctx->pkey->keymgmt == NULL
                     || ctx->pkey->keymgmt == ctx->keymgmt)) {
        ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    supported_exch = evp_keymgmt_util_query_operation_name(ctx->keymgmt,
                                                           OSSL_OP_KEYEXCH);
    if (supported_exch == NULL) {
        ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        goto err;
    }


    /*
     * We perform two iterations:
     *
     * 1.  Do the normal exchange fetch, using the fetching data given by
     *     the EVP_PKEY_CTX.
     * 2.  Do the provider specific exchange fetch, from the same provider
     *     as |ctx->keymgmt|
     *
     * We then try to fetch the keymgmt from the same provider as the
     * exchange, and try to export |ctx->pkey| to that keymgmt (when
     * this keymgmt happens to be the same as |ctx->keymgmt|, the export
     * is a no-op, but we call it anyway to not complicate the code even
     * more).
     * If the export call succeeds (returns a non-NULL provider key pointer),
     * we're done and can perform the operation itself.  If not, we perform
     * the second iteration, or jump to legacy.
     */
    for (iter = 1, provkey = NULL; iter < 3 && provkey == NULL; iter++) {
        EVP_KEYMGMT *tmp_keymgmt_tofree = NULL;

        /*
         * If we're on the second iteration, free the results from the first.
         * They are NULL on the first iteration, so no need to check what
         * iteration we're on.
         */
        EVP_KEYEXCH_free(exchange);
        EVP_KEYMGMT_free(tmp_keymgmt);

        switch (iter) {
        case 1:
            exchange =
                EVP_KEYEXCH_fetch(ctx->libctx, supported_exch, ctx->propquery);
            if (exchange != NULL)
                tmp_prov = EVP_KEYEXCH_get0_provider(exchange);
            break;
        case 2:
            tmp_prov = EVP_KEYMGMT_get0_provider(ctx->keymgmt);
            exchange =
                evp_keyexch_fetch_from_prov((OSSL_PROVIDER *)tmp_prov,
                                              supported_exch, ctx->propquery);
            if (exchange == NULL)
                goto legacy;
            break;
        }
        if (exchange == NULL)
            continue;

        /*
         * Ensure that the key is provided, either natively, or as a cached
         * export.  We start by fetching the keymgmt with the same name as
         * |ctx->keymgmt|, but from the provider of the exchange method, using
         * the same property query as when fetching the exchange method.
         * With the keymgmt we found (if we did), we try to export |ctx->pkey|
         * to it (evp_pkey_export_to_provider() is smart enough to only actually
         * export it if |tmp_keymgmt| is different from |ctx->pkey|'s keymgmt)
         */
        tmp_keymgmt_tofree = tmp_keymgmt =
            evp_keymgmt_fetch_from_prov((OSSL_PROVIDER *)tmp_prov,
                                        EVP_KEYMGMT_get0_name(ctx->keymgmt),
                                        ctx->propquery);
        if (tmp_keymgmt != NULL)
            provkey = evp_pkey_export_to_provider(ctx->pkey, ctx->libctx,
                                                  &tmp_keymgmt, ctx->propquery);
        if (tmp_keymgmt == NULL)
            EVP_KEYMGMT_free(tmp_keymgmt_tofree);
    }

    if (provkey == NULL) {
        EVP_KEYEXCH_free(exchange);
        goto legacy;
    }

    ERR_pop_to_mark();

    /* No more legacy from here down to legacy: */

    /* A Coverity false positive with up_ref/down_ref and free */
    /* coverity[use_after_free] */
    ctx->op.kex.exchange = exchange;
    /* A Coverity false positive with up_ref/down_ref and free */
    /* coverity[deref_arg] */
    ctx->op.kex.algctx = exchange->newctx(ossl_provider_ctx(exchange->prov));
    if (ctx->op.kex.algctx == NULL) {
        /* The provider key can stay in the cache */
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        goto err;
    }
    ret = exchange->init(ctx->op.kex.algctx, provkey, params);

    EVP_KEYMGMT_free(tmp_keymgmt);
    return ret ? 1 : 0;
 err:
    evp_pkey_ctx_free_old_ops(ctx);
    ctx->operation = EVP_PKEY_OP_UNDEFINED;
    EVP_KEYMGMT_free(tmp_keymgmt);
    return 0;

 legacy:
    /*
     * If we don't have the full support we need with provided methods,
     * let's go see if legacy does.
     */
    ERR_pop_to_mark();

#ifdef FIPS_MODULE
    return 0;
#else
    if (ctx->pmeth == NULL || ctx->pmeth->derive == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }

    if (ctx->pmeth->derive_init == NULL)
        return 1;
    ret = ctx->pmeth->derive_init(ctx);
    if (ret <= 0)
        ctx->operation = EVP_PKEY_OP_UNDEFINED;
    EVP_KEYMGMT_free(tmp_keymgmt);
    return ret;
#endif
}

int EVP_PKEY_derive_set_peer_ex(EVP_PKEY_CTX *ctx, EVP_PKEY *peer,
                                int validate_peer)
{
    int ret = 0, check;
    void *provkey = NULL;
    EVP_PKEY_CTX *check_ctx = NULL;
    EVP_KEYMGMT *tmp_keymgmt = NULL, *tmp_keymgmt_tofree = NULL;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }

    if (!EVP_PKEY_CTX_IS_DERIVE_OP(ctx) || ctx->op.kex.algctx == NULL)
        goto legacy;

    if (ctx->op.kex.exchange->set_peer == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }

    if (validate_peer) {
        check_ctx = EVP_PKEY_CTX_new_from_pkey(ctx->libctx, peer, ctx->propquery);
        if (check_ctx == NULL)
            return -1;
        check = EVP_PKEY_public_check(check_ctx);
        EVP_PKEY_CTX_free(check_ctx);
        if (check <= 0)
            return -1;
    }

    /*
     * Ensure that the |peer| is provided, either natively, or as a cached
     * export.  We start by fetching the keymgmt with the same name as
     * |ctx->keymgmt|, but from the provider of the exchange method, using
     * the same property query as when fetching the exchange method.
     * With the keymgmt we found (if we did), we try to export |peer|
     * to it (evp_pkey_export_to_provider() is smart enough to only actually
     * export it if |tmp_keymgmt| is different from |peer|'s keymgmt)
     */
    tmp_keymgmt_tofree = tmp_keymgmt =
        evp_keymgmt_fetch_from_prov((OSSL_PROVIDER *)
                                    EVP_KEYEXCH_get0_provider(ctx->op.kex.exchange),
                                    EVP_KEYMGMT_get0_name(ctx->keymgmt),
                                    ctx->propquery);
    if (tmp_keymgmt != NULL)
        /* A Coverity issue with up_ref/down_ref and free */
        /* coverity[pass_freed_arg] */
        provkey = evp_pkey_export_to_provider(peer, ctx->libctx,
                                              &tmp_keymgmt, ctx->propquery);
    EVP_KEYMGMT_free(tmp_keymgmt_tofree);

    /*
     * If making the key provided wasn't possible, legacy may be able to pick
     * it up
     */
    if (provkey == NULL)
        goto legacy;
    ret = ctx->op.kex.exchange->set_peer(ctx->op.kex.algctx, provkey);
    if (ret <= 0)
        return ret;
    goto common;

 legacy:
#ifdef FIPS_MODULE
    return ret;
#else
    if (ctx->pmeth == NULL
        || !(ctx->pmeth->derive != NULL
             || ctx->pmeth->encrypt != NULL
             || ctx->pmeth->decrypt != NULL)
        || ctx->pmeth->ctrl == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }
    if (ctx->operation != EVP_PKEY_OP_DERIVE
        && ctx->operation != EVP_PKEY_OP_ENCRYPT
        && ctx->operation != EVP_PKEY_OP_DECRYPT) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_INITIALIZED);
        return -1;
    }

    ret = ctx->pmeth->ctrl(ctx, EVP_PKEY_CTRL_PEER_KEY, 0, peer);

    if (ret <= 0)
        return ret;

    if (ret == 2)
        return 1;

    if (ctx->pkey == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_KEY_SET);
        return -1;
    }

    if (ctx->pkey->type != peer->type) {
        ERR_raise(ERR_LIB_EVP, EVP_R_DIFFERENT_KEY_TYPES);
        return -1;
    }

    /*
     * For clarity.  The error is if parameters in peer are
     * present (!missing) but don't match.  EVP_PKEY_parameters_eq may return
     * 1 (match), 0 (don't match) and -2 (comparison is not defined).  -1
     * (different key types) is impossible here because it is checked earlier.
     * -2 is OK for us here, as well as 1, so we can check for 0 only.
     */
    if (!EVP_PKEY_missing_parameters(peer) &&
        !EVP_PKEY_parameters_eq(ctx->pkey, peer)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_DIFFERENT_PARAMETERS);
        return -1;
    }

    ret = ctx->pmeth->ctrl(ctx, EVP_PKEY_CTRL_PEER_KEY, 1, peer);
    if (ret <= 0)
        return ret;
#endif

 common:
    if (!EVP_PKEY_up_ref(peer))
        return -1;

    EVP_PKEY_free(ctx->peerkey);
    ctx->peerkey = peer;

    return 1;
}

int EVP_PKEY_derive_set_peer(EVP_PKEY_CTX *ctx, EVP_PKEY *peer)
{
    return EVP_PKEY_derive_set_peer_ex(ctx, peer, 1);
}

int EVP_PKEY_derive(EVP_PKEY_CTX *ctx, unsigned char *key, size_t *pkeylen)
{
    int ret;

    if (ctx == NULL || pkeylen == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }

    if (!EVP_PKEY_CTX_IS_DERIVE_OP(ctx)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_INITIALIZED);
        return -1;
    }

    if (ctx->op.kex.algctx == NULL)
        goto legacy;

    ret = ctx->op.kex.exchange->derive(ctx->op.kex.algctx, key, pkeylen,
                                       key != NULL ? *pkeylen : 0);

    return ret;
 legacy:
    if (ctx->pmeth == NULL || ctx->pmeth->derive == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }

    M_check_autoarg(ctx, key, pkeylen, EVP_F_EVP_PKEY_DERIVE)
        return ctx->pmeth->derive(ctx, key, pkeylen);
}

int evp_keyexch_get_number(const EVP_KEYEXCH *keyexch)
{
    return keyexch->name_id;
}

const char *EVP_KEYEXCH_get0_name(const EVP_KEYEXCH *keyexch)
{
    return keyexch->type_name;
}

const char *EVP_KEYEXCH_get0_description(const EVP_KEYEXCH *keyexch)
{
    return keyexch->description;
}

int EVP_KEYEXCH_is_a(const EVP_KEYEXCH *keyexch, const char *name)
{
    return keyexch != NULL
           && evp_is_a(keyexch->prov, keyexch->name_id, NULL, name);
}

void EVP_KEYEXCH_do_all_provided(OSSL_LIB_CTX *libctx,
                                 void (*fn)(EVP_KEYEXCH *keyexch, void *arg),
                                 void *arg)
{
    evp_generic_do_all(libctx, OSSL_OP_KEYEXCH,
                       (void (*)(void *, void *))fn, arg,
                       evp_keyexch_from_algorithm,
                       evp_keyexch_up_ref,
                       evp_keyexch_free);
}

int EVP_KEYEXCH_names_do_all(const EVP_KEYEXCH *keyexch,
                             void (*fn)(const char *name, void *data),
                             void *data)
{
    if (keyexch->prov != NULL)
        return evp_names_do_all(keyexch->prov, keyexch->name_id, fn, data);

    return 1;
}

const OSSL_PARAM *EVP_KEYEXCH_gettable_ctx_params(const EVP_KEYEXCH *keyexch)
{
    void *provctx;

    if (keyexch == NULL || keyexch->gettable_ctx_params == NULL)
        return NULL;

    provctx = ossl_provider_ctx(EVP_KEYEXCH_get0_provider(keyexch));
    return keyexch->gettable_ctx_params(NULL, provctx);
}

const OSSL_PARAM *EVP_KEYEXCH_settable_ctx_params(const EVP_KEYEXCH *keyexch)
{
    void *provctx;

    if (keyexch == NULL || keyexch->settable_ctx_params == NULL)
        return NULL;
    provctx = ossl_provider_ctx(EVP_KEYEXCH_get0_provider(keyexch));
    return keyexch->settable_ctx_params(NULL, provctx);
}
