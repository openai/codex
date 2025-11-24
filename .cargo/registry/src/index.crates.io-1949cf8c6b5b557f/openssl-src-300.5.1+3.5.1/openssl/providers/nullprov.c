/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdio.h>
#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include "prov/implementations.h"
#include "prov/providercommon.h"

OSSL_provider_init_fn ossl_null_provider_init;

/* Parameters we provide to the core */
static const OSSL_PARAM null_param_types[] = {
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_NAME, OSSL_PARAM_UTF8_PTR, NULL, 0),
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_VERSION, OSSL_PARAM_UTF8_PTR, NULL, 0),
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_BUILDINFO, OSSL_PARAM_UTF8_PTR, NULL, 0),
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_STATUS, OSSL_PARAM_INTEGER, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *null_gettable_params(const OSSL_PROVIDER *prov)
{
    return null_param_types;
}

static int null_get_params(const OSSL_PROVIDER *provctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_NAME);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, "OpenSSL Null Provider"))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_VERSION);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, OPENSSL_VERSION_STR))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_BUILDINFO);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, OPENSSL_FULL_VERSION_STR))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_STATUS);
    if (p != NULL && !OSSL_PARAM_set_int(p, ossl_prov_is_running()))
        return 0;
    return 1;
}

static const OSSL_ALGORITHM *null_query(OSSL_PROVIDER *prov,
                                          int operation_id,
                                          int *no_cache)
{
    *no_cache = 0;
    return NULL;
}

/* Functions we provide to the core */
static const OSSL_DISPATCH null_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_GETTABLE_PARAMS, (void (*)(void))null_gettable_params },
    { OSSL_FUNC_PROVIDER_GET_PARAMS, (void (*)(void))null_get_params },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))null_query },
    OSSL_DISPATCH_END
};

int ossl_null_provider_init(const OSSL_CORE_HANDLE *handle,
                            const OSSL_DISPATCH *in,
                            const OSSL_DISPATCH **out,
                            void **provctx)
{
    *out = null_dispatch_table;

    /* Could be anything - we don't use it */
    *provctx = (void *)handle;
    return 1;
}
