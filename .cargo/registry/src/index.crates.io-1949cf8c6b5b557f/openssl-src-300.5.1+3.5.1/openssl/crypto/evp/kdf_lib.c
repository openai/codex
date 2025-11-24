/*
 * Copyright 2018-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2018-2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include "internal/cryptlib.h"
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/core.h>
#include <openssl/core_names.h>
#include "crypto/evp.h"
#include "internal/numbers.h"
#include "internal/provider.h"
#include "evp_local.h"

EVP_KDF_CTX *EVP_KDF_CTX_new(EVP_KDF *kdf)
{
    EVP_KDF_CTX *ctx = NULL;

    if (kdf == NULL)
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(EVP_KDF_CTX));
    if (ctx == NULL
        || (ctx->algctx = kdf->newctx(ossl_provider_ctx(kdf->prov))) == NULL
        || !EVP_KDF_up_ref(kdf)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        if (ctx != NULL)
            kdf->freectx(ctx->algctx);
        OPENSSL_free(ctx);
        ctx = NULL;
    } else {
        ctx->meth = kdf;
    }
    return ctx;
}

void EVP_KDF_CTX_free(EVP_KDF_CTX *ctx)
{
    if (ctx == NULL)
        return;
    ctx->meth->freectx(ctx->algctx);
    ctx->algctx = NULL;
    EVP_KDF_free(ctx->meth);
    OPENSSL_free(ctx);
}

EVP_KDF_CTX *EVP_KDF_CTX_dup(const EVP_KDF_CTX *src)
{
    EVP_KDF_CTX *dst;

    if (src == NULL || src->algctx == NULL || src->meth->dupctx == NULL)
        return NULL;

    dst = OPENSSL_malloc(sizeof(*dst));
    if (dst == NULL)
        return NULL;

    memcpy(dst, src, sizeof(*dst));
    if (!EVP_KDF_up_ref(dst->meth)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        OPENSSL_free(dst);
        return NULL;
    }

    dst->algctx = src->meth->dupctx(src->algctx);
    if (dst->algctx == NULL) {
        EVP_KDF_CTX_free(dst);
        return NULL;
    }
    return dst;
}

int evp_kdf_get_number(const EVP_KDF *kdf)
{
    return kdf->name_id;
}

const char *EVP_KDF_get0_name(const EVP_KDF *kdf)
{
    return kdf->type_name;
}

const char *EVP_KDF_get0_description(const EVP_KDF *kdf)
{
    return kdf->description;
}

int EVP_KDF_is_a(const EVP_KDF *kdf, const char *name)
{
    return kdf != NULL && evp_is_a(kdf->prov, kdf->name_id, NULL, name);
}

const OSSL_PROVIDER *EVP_KDF_get0_provider(const EVP_KDF *kdf)
{
    return kdf->prov;
}

const EVP_KDF *EVP_KDF_CTX_kdf(EVP_KDF_CTX *ctx)
{
    return ctx->meth;
}

void EVP_KDF_CTX_reset(EVP_KDF_CTX *ctx)
{
    if (ctx == NULL)
        return;

    if (ctx->meth->reset != NULL)
        ctx->meth->reset(ctx->algctx);
}

size_t EVP_KDF_CTX_get_kdf_size(EVP_KDF_CTX *ctx)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    size_t s = 0;

    if (ctx == NULL)
        return 0;

    *params = OSSL_PARAM_construct_size_t(OSSL_KDF_PARAM_SIZE, &s);
    if (ctx->meth->get_ctx_params != NULL
        && ctx->meth->get_ctx_params(ctx->algctx, params))
            return s;
    if (ctx->meth->get_params != NULL
        && ctx->meth->get_params(params))
            return s;
    return 0;
}

int EVP_KDF_derive(EVP_KDF_CTX *ctx, unsigned char *key, size_t keylen,
                   const OSSL_PARAM params[])
{
    if (ctx == NULL)
        return 0;

    return ctx->meth->derive(ctx->algctx, key, keylen, params);
}

/*
 * The {get,set}_params functions return 1 if there is no corresponding
 * function in the implementation.  This is the same as if there was one,
 * but it didn't recognise any of the given params, i.e. nothing in the
 * bag of parameters was useful.
 */
int EVP_KDF_get_params(EVP_KDF *kdf, OSSL_PARAM params[])
{
    if (kdf->get_params != NULL)
        return kdf->get_params(params);
    return 1;
}

int EVP_KDF_CTX_get_params(EVP_KDF_CTX *ctx, OSSL_PARAM params[])
{
    if (ctx->meth->get_ctx_params != NULL)
        return ctx->meth->get_ctx_params(ctx->algctx, params);
    return 1;
}

int EVP_KDF_CTX_set_params(EVP_KDF_CTX *ctx, const OSSL_PARAM params[])
{
    if (ctx->meth->set_ctx_params != NULL)
        return ctx->meth->set_ctx_params(ctx->algctx, params);
    return 1;
}

int EVP_KDF_names_do_all(const EVP_KDF *kdf,
                         void (*fn)(const char *name, void *data),
                         void *data)
{
    if (kdf->prov != NULL)
        return evp_names_do_all(kdf->prov, kdf->name_id, fn, data);

    return 1;
}
