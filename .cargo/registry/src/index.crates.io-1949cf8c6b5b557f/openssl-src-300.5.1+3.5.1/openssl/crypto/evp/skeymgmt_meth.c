/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/core_dispatch.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include "internal/core.h"
#include "internal/provider.h"
#include "internal/refcount.h"
#include "crypto/evp.h"
#include "evp_local.h"

void *evp_skeymgmt_generate(const EVP_SKEYMGMT *skeymgmt, const OSSL_PARAM params[])
{
    void *provctx = ossl_provider_ctx(EVP_SKEYMGMT_get0_provider(skeymgmt));

    return (skeymgmt->generate != NULL) ? skeymgmt->generate(provctx, params) : NULL;
}

void *evp_skeymgmt_import(const EVP_SKEYMGMT *skeymgmt, int selection, const OSSL_PARAM params[])
{
    void *provctx = ossl_provider_ctx(EVP_SKEYMGMT_get0_provider(skeymgmt));

    /* This is mandatory, no need to check for its presence */
    return skeymgmt->import(provctx, selection, params);
}

int evp_skeymgmt_export(const EVP_SKEYMGMT *skeymgmt, void *keydata,
                        int selection, OSSL_CALLBACK *param_cb, void *cbarg)
{
    /* This is mandatory, no need to check for its presence */
    return skeymgmt->export(keydata, selection, param_cb, cbarg);
}

void evp_skeymgmt_freedata(const EVP_SKEYMGMT *skeymgmt, void *keydata)
{
    /* This is mandatory, no need to check for its presence */
    skeymgmt->free(keydata);
}

static void *skeymgmt_new(void)
{
    EVP_SKEYMGMT *skeymgmt = NULL;

    if ((skeymgmt = OPENSSL_zalloc(sizeof(*skeymgmt))) == NULL)
        return NULL;
    if (!CRYPTO_NEW_REF(&skeymgmt->refcnt, 1)) {
        EVP_SKEYMGMT_free(skeymgmt);
        return NULL;
    }
    return skeymgmt;
}

static void *skeymgmt_from_algorithm(int name_id,
                                     const OSSL_ALGORITHM *algodef,
                                     OSSL_PROVIDER *prov)
{
    const OSSL_DISPATCH *fns = algodef->implementation;
    EVP_SKEYMGMT *skeymgmt = NULL;

    if ((skeymgmt = skeymgmt_new()) == NULL)
        return NULL;

    skeymgmt->name_id = name_id;
    if ((skeymgmt->type_name = ossl_algorithm_get1_first_name(algodef)) == NULL) {
        EVP_SKEYMGMT_free(skeymgmt);
        return NULL;
    }
    skeymgmt->description = algodef->algorithm_description;

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_SKEYMGMT_FREE:
            if (skeymgmt->free == NULL)
                skeymgmt->free = OSSL_FUNC_skeymgmt_free(fns);
            break;
        case OSSL_FUNC_SKEYMGMT_IMPORT:
            if (skeymgmt->import == NULL)
                skeymgmt->import = OSSL_FUNC_skeymgmt_import(fns);
            break;
        case OSSL_FUNC_SKEYMGMT_EXPORT:
            if (skeymgmt->export == NULL)
                skeymgmt->export = OSSL_FUNC_skeymgmt_export(fns);
            break;
        case OSSL_FUNC_SKEYMGMT_GENERATE:
            if (skeymgmt->generate == NULL)
                skeymgmt->generate = OSSL_FUNC_skeymgmt_generate(fns);
            break;
        case OSSL_FUNC_SKEYMGMT_GET_KEY_ID:
            if (skeymgmt->get_key_id == NULL)
                skeymgmt->get_key_id = OSSL_FUNC_skeymgmt_get_key_id(fns);
            break;
        case OSSL_FUNC_SKEYMGMT_IMP_SETTABLE_PARAMS:
            if (skeymgmt->imp_params == NULL)
                skeymgmt->imp_params = OSSL_FUNC_skeymgmt_imp_settable_params(fns);
            break;
        case OSSL_FUNC_SKEYMGMT_GEN_SETTABLE_PARAMS:
            if (skeymgmt->gen_params == NULL)
                skeymgmt->gen_params = OSSL_FUNC_skeymgmt_gen_settable_params(fns);
            break;
        }
    }

    /* Check that the provider is sensible */
    if (skeymgmt->free == NULL
        || skeymgmt->import == NULL
        || skeymgmt->export == NULL) {
        EVP_SKEYMGMT_free(skeymgmt);
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_PROVIDER_FUNCTIONS);
        return NULL;
    }

    if (!ossl_provider_up_ref(prov)) {
        EVP_SKEYMGMT_free(skeymgmt);
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return NULL;
    }
    skeymgmt->prov = prov;

    return skeymgmt;
}

EVP_SKEYMGMT *evp_skeymgmt_fetch_from_prov(OSSL_PROVIDER *prov,
                                           const char *name,
                                           const char *properties)
{
    return evp_generic_fetch_from_prov(prov,
                                       OSSL_OP_SKEYMGMT,
                                       name, properties,
                                       skeymgmt_from_algorithm,
                                       (int (*)(void *))EVP_SKEYMGMT_up_ref,
                                       (void (*)(void *))EVP_SKEYMGMT_free);
}

EVP_SKEYMGMT *EVP_SKEYMGMT_fetch(OSSL_LIB_CTX *ctx, const char *algorithm,
                                 const char *properties)
{
    return evp_generic_fetch(ctx, OSSL_OP_SKEYMGMT, algorithm, properties,
                             skeymgmt_from_algorithm,
                             (int (*)(void *))EVP_SKEYMGMT_up_ref,
                             (void (*)(void *))EVP_SKEYMGMT_free);
}

int EVP_SKEYMGMT_up_ref(EVP_SKEYMGMT *skeymgmt)
{
    int ref = 0;

    CRYPTO_UP_REF(&skeymgmt->refcnt, &ref);
    return 1;
}

void EVP_SKEYMGMT_free(EVP_SKEYMGMT *skeymgmt)
{
    int ref = 0;

    if (skeymgmt == NULL)
        return;

    CRYPTO_DOWN_REF(&skeymgmt->refcnt, &ref);
    if (ref > 0)
        return;
    OPENSSL_free(skeymgmt->type_name);
    ossl_provider_free(skeymgmt->prov);
    CRYPTO_FREE_REF(&skeymgmt->refcnt);
    OPENSSL_free(skeymgmt);
}

const OSSL_PROVIDER *EVP_SKEYMGMT_get0_provider(const EVP_SKEYMGMT *skeymgmt)
{
    return (skeymgmt != NULL) ? skeymgmt->prov : NULL;
}

const char *EVP_SKEYMGMT_get0_description(const EVP_SKEYMGMT *skeymgmt)
{
    return (skeymgmt != NULL) ? skeymgmt->description : NULL;
}

const char *EVP_SKEYMGMT_get0_name(const EVP_SKEYMGMT *skeymgmt)
{
    return (skeymgmt != NULL) ? skeymgmt->type_name : NULL;
}

int EVP_SKEYMGMT_is_a(const EVP_SKEYMGMT *skeymgmt, const char *name)
{
    return skeymgmt != NULL
        && evp_is_a(skeymgmt->prov, skeymgmt->name_id, NULL, name);
}

void EVP_SKEYMGMT_do_all_provided(OSSL_LIB_CTX *libctx,
                                  void (*fn)(EVP_SKEYMGMT *skeymgmt, void *arg),
                                  void *arg)
{
    evp_generic_do_all(libctx, OSSL_OP_KEYMGMT,
                       (void (*)(void *, void *))fn, arg,
                       skeymgmt_from_algorithm,
                       (int (*)(void *))EVP_SKEYMGMT_up_ref,
                       (void (*)(void *))EVP_SKEYMGMT_free);
}

int EVP_SKEYMGMT_names_do_all(const EVP_SKEYMGMT *skeymgmt,
                              void (*fn)(const char *name, void *data),
                              void *data)
{
    if (skeymgmt == NULL)
        return 0;

    if (skeymgmt->prov != NULL)
        return evp_names_do_all(skeymgmt->prov, skeymgmt->name_id, fn, data);

    return 1;
}

const OSSL_PARAM *EVP_SKEYMGMT_get0_gen_settable_params(const EVP_SKEYMGMT *skeymgmt)
{
    void *provctx = NULL;

    if (skeymgmt == NULL)
        return 0;

    provctx = ossl_provider_ctx(EVP_SKEYMGMT_get0_provider(skeymgmt));

    return (skeymgmt->gen_params != NULL) ? skeymgmt->gen_params(provctx) : NULL;
}

const OSSL_PARAM *EVP_SKEYMGMT_get0_imp_settable_params(const EVP_SKEYMGMT *skeymgmt)
{
    void *provctx = NULL;

    if (skeymgmt == NULL)
        return 0;

    provctx = ossl_provider_ctx(EVP_SKEYMGMT_get0_provider(skeymgmt));

    return (skeymgmt->imp_params != NULL) ? skeymgmt->imp_params(provctx) : NULL;
}
