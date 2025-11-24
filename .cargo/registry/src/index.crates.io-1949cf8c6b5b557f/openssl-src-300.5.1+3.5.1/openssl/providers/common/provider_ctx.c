/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <string.h>
#include "prov/provider_ctx.h"
#include "prov/bio.h"

PROV_CTX *ossl_prov_ctx_new(void)
{
    return OPENSSL_zalloc(sizeof(PROV_CTX));
}

void ossl_prov_ctx_free(PROV_CTX *ctx)
{
    OPENSSL_free(ctx);
}

void ossl_prov_ctx_set0_libctx(PROV_CTX *ctx, OSSL_LIB_CTX *libctx)
{
    if (ctx != NULL)
        ctx->libctx = libctx;
}

void ossl_prov_ctx_set0_handle(PROV_CTX *ctx, const OSSL_CORE_HANDLE *handle)
{
    if (ctx != NULL)
        ctx->handle = handle;
}

void ossl_prov_ctx_set0_core_bio_method(PROV_CTX *ctx, BIO_METHOD *corebiometh)
{
    if (ctx != NULL)
        ctx->corebiometh = corebiometh;
}

void
ossl_prov_ctx_set0_core_get_params(PROV_CTX *ctx,
                                   OSSL_FUNC_core_get_params_fn *c_get_params)
{
    if (ctx != NULL)
        ctx->core_get_params = c_get_params;
}

OSSL_LIB_CTX *ossl_prov_ctx_get0_libctx(PROV_CTX *ctx)
{
    if (ctx == NULL)
        return NULL;
    return ctx->libctx;
}

const OSSL_CORE_HANDLE *ossl_prov_ctx_get0_handle(PROV_CTX *ctx)
{
    if (ctx == NULL)
        return NULL;
    return ctx->handle;
}

BIO_METHOD *ossl_prov_ctx_get0_core_bio_method(PROV_CTX *ctx)
{
    if (ctx == NULL)
        return NULL;
    return ctx->corebiometh;
}

OSSL_FUNC_core_get_params_fn *ossl_prov_ctx_get0_core_get_params(PROV_CTX *ctx)
{
    if (ctx == NULL)
        return NULL;
    return ctx->core_get_params;
}

const char *
ossl_prov_ctx_get_param(PROV_CTX *ctx, const char *name, const char *defval)
{
    char *val = NULL;
    OSSL_PARAM param[2] = { OSSL_PARAM_END, OSSL_PARAM_END };

    if (ctx == NULL
        || ctx->handle == NULL
        || ctx->core_get_params == NULL)
        return defval;

    param[0].key = (char *) name;
    param[0].data_type = OSSL_PARAM_UTF8_PTR;
    param[0].data = (void *) &val;
    param[0].data_size = sizeof(val);
    param[0].return_size = OSSL_PARAM_UNMODIFIED;

    /* Errors are ignored, returning the default value */
    if (ctx->core_get_params(ctx->handle, param)
        && OSSL_PARAM_modified(param)
        && val != NULL)
        return val;
    return defval;
}

int ossl_prov_ctx_get_bool_param(PROV_CTX *ctx, const char *name, int defval)
{
    const char *val = ossl_prov_ctx_get_param(ctx, name, NULL);

    if (val != NULL) {
        if ((strcmp(val, "1") == 0)
            || (OPENSSL_strcasecmp(val, "yes") == 0)
            || (OPENSSL_strcasecmp(val, "true") == 0)
            || (OPENSSL_strcasecmp(val, "on") == 0))
            return 1;
        else if ((strcmp(val, "0") == 0)
                 || (OPENSSL_strcasecmp(val, "no") == 0)
                 || (OPENSSL_strcasecmp(val, "false") == 0)
                 || (OPENSSL_strcasecmp(val, "off") == 0))
            return 0;
    }
    return defval;
}
