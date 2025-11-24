/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/err.h>
#include <openssl/cryptoerr.h>
#include <openssl/provider.h>
#include <openssl/core_names.h>
#include "internal/provider.h"
#include "provider_local.h"

OSSL_PROVIDER *OSSL_PROVIDER_try_load_ex(OSSL_LIB_CTX *libctx, const char *name,
                                         OSSL_PARAM *params, int retain_fallbacks)
{
    OSSL_PROVIDER *prov = NULL, *actual;
    int isnew = 0;

    /* Find it or create it */
    if ((prov = ossl_provider_find(libctx, name, 0)) == NULL) {
        if ((prov = ossl_provider_new(libctx, name, NULL, params, 0)) == NULL)
            return NULL;
        isnew = 1;
    }

    if (!ossl_provider_activate(prov, 1, 0)) {
        ossl_provider_free(prov);
        return NULL;
    }

    actual = prov;
    if (isnew && !ossl_provider_add_to_store(prov, &actual, retain_fallbacks)) {
        ossl_provider_deactivate(prov, 1);
        ossl_provider_free(prov);
        return NULL;
    }
    if (actual != prov) {
        if (!ossl_provider_activate(actual, 1, 0)) {
            ossl_provider_free(actual);
            return NULL;
        }
    }

    return actual;
}

OSSL_PROVIDER *OSSL_PROVIDER_try_load(OSSL_LIB_CTX *libctx, const char *name,
                                      int retain_fallbacks)
{
    return OSSL_PROVIDER_try_load_ex(libctx, name, NULL, retain_fallbacks);
}

OSSL_PROVIDER *OSSL_PROVIDER_load_ex(OSSL_LIB_CTX *libctx, const char *name, OSSL_PARAM *params)
{
    /* Any attempt to load a provider disables auto-loading of defaults */
    if (ossl_provider_disable_fallback_loading(libctx))
        return OSSL_PROVIDER_try_load_ex(libctx, name, params, 0);
    return NULL;
}

OSSL_PROVIDER *OSSL_PROVIDER_load(OSSL_LIB_CTX *libctx, const char *name)
{
    return OSSL_PROVIDER_load_ex(libctx, name, NULL);
}

int OSSL_PROVIDER_unload(OSSL_PROVIDER *prov)
{
    if (!ossl_provider_deactivate(prov, 1))
        return 0;
    ossl_provider_free(prov);
    return 1;
}

const OSSL_PARAM *OSSL_PROVIDER_gettable_params(const OSSL_PROVIDER *prov)
{
    return ossl_provider_gettable_params(prov);
}

int OSSL_PROVIDER_get_params(const OSSL_PROVIDER *prov, OSSL_PARAM params[])
{
    return ossl_provider_get_params(prov, params);
}

const OSSL_ALGORITHM *OSSL_PROVIDER_query_operation(const OSSL_PROVIDER *prov,
                                                    int operation_id,
                                                    int *no_cache)
{
    return ossl_provider_query_operation(prov, operation_id, no_cache);
}

void OSSL_PROVIDER_unquery_operation(const OSSL_PROVIDER *prov,
                                     int operation_id,
                                     const OSSL_ALGORITHM *algs)
{
    ossl_provider_unquery_operation(prov, operation_id, algs);
}

void *OSSL_PROVIDER_get0_provider_ctx(const OSSL_PROVIDER *prov)
{
    return ossl_provider_ctx(prov);
}

const OSSL_DISPATCH *OSSL_PROVIDER_get0_dispatch(const OSSL_PROVIDER *prov)
{
    return ossl_provider_get0_dispatch(prov);
}

int OSSL_PROVIDER_self_test(const OSSL_PROVIDER *prov)
{
    return ossl_provider_self_test(prov);
}

int OSSL_PROVIDER_get_capabilities(const OSSL_PROVIDER *prov,
                                   const char *capability,
                                   OSSL_CALLBACK *cb,
                                   void *arg)
{
    return ossl_provider_get_capabilities(prov, capability, cb, arg);
}

int OSSL_PROVIDER_add_builtin(OSSL_LIB_CTX *libctx, const char *name,
                              OSSL_provider_init_fn *init_fn)
{
    OSSL_PROVIDER_INFO entry;

    if (name == NULL || init_fn == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    memset(&entry, 0, sizeof(entry));
    entry.name = OPENSSL_strdup(name);
    if (entry.name == NULL)
        return 0;
    entry.init = init_fn;
    if (!ossl_provider_info_add_to_store(libctx, &entry)) {
        ossl_provider_info_clear(&entry);
        return 0;
    }
    return 1;
}

const char *OSSL_PROVIDER_get0_name(const OSSL_PROVIDER *prov)
{
    return ossl_provider_name(prov);
}

int OSSL_PROVIDER_do_all(OSSL_LIB_CTX *ctx,
                         int (*cb)(OSSL_PROVIDER *provider,
                                   void *cbdata),
                         void *cbdata)
{
    return ossl_provider_doall_activated(ctx, cb, cbdata);
}
