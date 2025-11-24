/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include "internal/provider.h"
#include "internal/core.h"
#include "crypto/evp.h"
#include "evp_local.h"

static int evp_mac_up_ref(void *vmac)
{
    EVP_MAC *mac = vmac;
    int ref = 0;

    CRYPTO_UP_REF(&mac->refcnt, &ref);
    return 1;
}

static void evp_mac_free(void *vmac)
{
    EVP_MAC *mac = vmac;
    int ref = 0;

    if (mac == NULL)
        return;

    CRYPTO_DOWN_REF(&mac->refcnt, &ref);
    if (ref > 0)
        return;
    OPENSSL_free(mac->type_name);
    ossl_provider_free(mac->prov);
    CRYPTO_FREE_REF(&mac->refcnt);
    OPENSSL_free(mac);
}

static void *evp_mac_new(void)
{
    EVP_MAC *mac = NULL;

    if ((mac = OPENSSL_zalloc(sizeof(*mac))) == NULL
        || !CRYPTO_NEW_REF(&mac->refcnt, 1)) {
        evp_mac_free(mac);
        return NULL;
    }
    return mac;
}

static void *evp_mac_from_algorithm(int name_id,
                                    const OSSL_ALGORITHM *algodef,
                                    OSSL_PROVIDER *prov)
{
    const OSSL_DISPATCH *fns = algodef->implementation;
    EVP_MAC *mac = NULL;
    int fnmaccnt = 0, fnctxcnt = 0, mac_init_found = 0;

    if ((mac = evp_mac_new()) == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        goto err;
    }
    mac->name_id = name_id;

    if ((mac->type_name = ossl_algorithm_get1_first_name(algodef)) == NULL)
        goto err;

    mac->description = algodef->algorithm_description;

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_MAC_NEWCTX:
            if (mac->newctx != NULL)
                break;
            mac->newctx = OSSL_FUNC_mac_newctx(fns);
            fnctxcnt++;
            break;
        case OSSL_FUNC_MAC_DUPCTX:
            if (mac->dupctx != NULL)
                break;
            mac->dupctx = OSSL_FUNC_mac_dupctx(fns);
            break;
        case OSSL_FUNC_MAC_FREECTX:
            if (mac->freectx != NULL)
                break;
            mac->freectx = OSSL_FUNC_mac_freectx(fns);
            fnctxcnt++;
            break;
        case OSSL_FUNC_MAC_INIT:
            if (mac->init != NULL)
                break;
            mac->init = OSSL_FUNC_mac_init(fns);
            mac_init_found = 1;
            break;
        case OSSL_FUNC_MAC_UPDATE:
            if (mac->update != NULL)
                break;
            mac->update = OSSL_FUNC_mac_update(fns);
            fnmaccnt++;
            break;
        case OSSL_FUNC_MAC_FINAL:
            if (mac->final != NULL)
                break;
            mac->final = OSSL_FUNC_mac_final(fns);
            fnmaccnt++;
            break;
        case OSSL_FUNC_MAC_GETTABLE_PARAMS:
            if (mac->gettable_params != NULL)
                break;
            mac->gettable_params =
                OSSL_FUNC_mac_gettable_params(fns);
            break;
        case OSSL_FUNC_MAC_GETTABLE_CTX_PARAMS:
            if (mac->gettable_ctx_params != NULL)
                break;
            mac->gettable_ctx_params =
                OSSL_FUNC_mac_gettable_ctx_params(fns);
            break;
        case OSSL_FUNC_MAC_SETTABLE_CTX_PARAMS:
            if (mac->settable_ctx_params != NULL)
                break;
            mac->settable_ctx_params =
                OSSL_FUNC_mac_settable_ctx_params(fns);
            break;
        case OSSL_FUNC_MAC_GET_PARAMS:
            if (mac->get_params != NULL)
                break;
            mac->get_params = OSSL_FUNC_mac_get_params(fns);
            break;
        case OSSL_FUNC_MAC_GET_CTX_PARAMS:
            if (mac->get_ctx_params != NULL)
                break;
            mac->get_ctx_params = OSSL_FUNC_mac_get_ctx_params(fns);
            break;
        case OSSL_FUNC_MAC_SET_CTX_PARAMS:
            if (mac->set_ctx_params != NULL)
                break;
            mac->set_ctx_params = OSSL_FUNC_mac_set_ctx_params(fns);
            break;
        case OSSL_FUNC_MAC_INIT_SKEY:
            if (mac->init_skey != NULL)
                break;
            mac->init_skey = OSSL_FUNC_mac_init_skey(fns);
            mac_init_found = 1;
            break;
        }
    }
    fnmaccnt += mac_init_found;
    if (fnmaccnt != 3
        || fnctxcnt != 2) {
        /*
         * In order to be a consistent set of functions we must have at least
         * a complete set of "mac" functions, and a complete set of context
         * management functions, as well as the size function.
         */
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_PROVIDER_FUNCTIONS);
        goto err;
    }

    if (prov != NULL && !ossl_provider_up_ref(prov))
        goto err;

    mac->prov = prov;

    return mac;

err:
    evp_mac_free(mac);
    return NULL;
}

EVP_MAC *EVP_MAC_fetch(OSSL_LIB_CTX *libctx, const char *algorithm,
                       const char *properties)
{
    return evp_generic_fetch(libctx, OSSL_OP_MAC, algorithm, properties,
                             evp_mac_from_algorithm, evp_mac_up_ref,
                             evp_mac_free);
}

int EVP_MAC_up_ref(EVP_MAC *mac)
{
    return evp_mac_up_ref(mac);
}

void EVP_MAC_free(EVP_MAC *mac)
{
    evp_mac_free(mac);
}

const OSSL_PROVIDER *EVP_MAC_get0_provider(const EVP_MAC *mac)
{
    return mac->prov;
}

const OSSL_PARAM *EVP_MAC_gettable_params(const EVP_MAC *mac)
{
    if (mac->gettable_params == NULL)
        return NULL;
    return mac->gettable_params(ossl_provider_ctx(EVP_MAC_get0_provider(mac)));
}

const OSSL_PARAM *EVP_MAC_gettable_ctx_params(const EVP_MAC *mac)
{
    void *alg;

    if (mac->gettable_ctx_params == NULL)
        return NULL;
    alg = ossl_provider_ctx(EVP_MAC_get0_provider(mac));
    return mac->gettable_ctx_params(NULL, alg);
}

const OSSL_PARAM *EVP_MAC_settable_ctx_params(const EVP_MAC *mac)
{
    void *alg;

    if (mac->settable_ctx_params == NULL)
        return NULL;
    alg = ossl_provider_ctx(EVP_MAC_get0_provider(mac));
    return mac->settable_ctx_params(NULL, alg);
}

const OSSL_PARAM *EVP_MAC_CTX_gettable_params(EVP_MAC_CTX *ctx)
{
    void *alg;

    if (ctx->meth->gettable_ctx_params == NULL)
        return NULL;
    alg = ossl_provider_ctx(EVP_MAC_get0_provider(ctx->meth));
    return ctx->meth->gettable_ctx_params(ctx->algctx, alg);
}

const OSSL_PARAM *EVP_MAC_CTX_settable_params(EVP_MAC_CTX *ctx)
{
    void *alg;

    if (ctx->meth->settable_ctx_params == NULL)
        return NULL;
    alg = ossl_provider_ctx(EVP_MAC_get0_provider(ctx->meth));
    return ctx->meth->settable_ctx_params(ctx->algctx, alg);
}

void EVP_MAC_do_all_provided(OSSL_LIB_CTX *libctx,
                             void (*fn)(EVP_MAC *mac, void *arg),
                             void *arg)
{
    evp_generic_do_all(libctx, OSSL_OP_MAC,
                       (void (*)(void *, void *))fn, arg,
                       evp_mac_from_algorithm, evp_mac_up_ref, evp_mac_free);
}

EVP_MAC *evp_mac_fetch_from_prov(OSSL_PROVIDER *prov,
                                 const char *algorithm,
                                 const char *properties)
{
    return evp_generic_fetch_from_prov(prov, OSSL_OP_MAC,
                                       algorithm, properties,
                                       evp_mac_from_algorithm,
                                       evp_mac_up_ref,
                                       evp_mac_free);
}
