/*
 * Copyright 2018-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/proverr.h>

#include "crypto/poly1305.h"

#include "prov/implementations.h"
#include "prov/providercommon.h"

/*
 * Forward declaration of everything implemented here.  This is not strictly
 * necessary for the compiler, but provides an assurance that the signatures
 * of the functions in the dispatch table are correct.
 */
static OSSL_FUNC_mac_newctx_fn poly1305_new;
static OSSL_FUNC_mac_dupctx_fn poly1305_dup;
static OSSL_FUNC_mac_freectx_fn poly1305_free;
static OSSL_FUNC_mac_gettable_params_fn poly1305_gettable_params;
static OSSL_FUNC_mac_get_params_fn poly1305_get_params;
static OSSL_FUNC_mac_settable_ctx_params_fn poly1305_settable_ctx_params;
static OSSL_FUNC_mac_set_ctx_params_fn poly1305_set_ctx_params;
static OSSL_FUNC_mac_init_fn poly1305_init;
static OSSL_FUNC_mac_update_fn poly1305_update;
static OSSL_FUNC_mac_final_fn poly1305_final;

struct poly1305_data_st {
    void *provctx;
    int updated;
    POLY1305 poly1305;           /* Poly1305 data */
};

static void *poly1305_new(void *provctx)
{
    struct poly1305_data_st *ctx;

    if (!ossl_prov_is_running())
        return NULL;
    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL)
        ctx->provctx = provctx;
    return ctx;
}

static void poly1305_free(void *vmacctx)
{
    OPENSSL_free(vmacctx);
}

static void *poly1305_dup(void *vsrc)
{
    struct poly1305_data_st *src = vsrc;
    struct poly1305_data_st *dst;

    if (!ossl_prov_is_running())
        return NULL;
    dst = OPENSSL_malloc(sizeof(*dst));
    if (dst == NULL)
        return NULL;

    *dst = *src;
    return dst;
}

static size_t poly1305_size(void)
{
    return POLY1305_DIGEST_SIZE;
}

static int poly1305_setkey(struct poly1305_data_st *ctx,
                           const unsigned char *key, size_t keylen)
{
    if (keylen != POLY1305_KEY_SIZE) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
        return 0;
    }
    Poly1305_Init(&ctx->poly1305, key);
    ctx->updated = 0;
    return 1;
}

static int poly1305_init(void *vmacctx, const unsigned char *key,
                         size_t keylen, const OSSL_PARAM params[])
{
    struct poly1305_data_st *ctx = vmacctx;

    /* initialize the context in MAC_ctrl function */
    if (!ossl_prov_is_running() || !poly1305_set_ctx_params(ctx, params))
        return 0;
    if (key != NULL)
        return poly1305_setkey(ctx, key, keylen);
    /* no reinitialization of context with the same key is allowed */
    return ctx->updated == 0;
}

static int poly1305_update(void *vmacctx, const unsigned char *data,
                       size_t datalen)
{
    struct poly1305_data_st *ctx = vmacctx;

    ctx->updated = 1;
    if (datalen == 0)
        return 1;

    /* poly1305 has nothing to return in its update function */
    Poly1305_Update(&ctx->poly1305, data, datalen);
    return 1;
}

static int poly1305_final(void *vmacctx, unsigned char *out, size_t *outl,
                          size_t outsize)
{
    struct poly1305_data_st *ctx = vmacctx;

    if (!ossl_prov_is_running())
        return 0;
    ctx->updated = 1;
    Poly1305_Final(&ctx->poly1305, out);
    *outl = poly1305_size();
    return 1;
}

static const OSSL_PARAM known_gettable_params[] = {
    OSSL_PARAM_size_t(OSSL_MAC_PARAM_SIZE, NULL),
    OSSL_PARAM_END
};
static const OSSL_PARAM *poly1305_gettable_params(void *provctx)
{
    return known_gettable_params;
}

static int poly1305_get_params(OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_SIZE)) != NULL)
        return OSSL_PARAM_set_size_t(p, poly1305_size());

    return 1;
}

static const OSSL_PARAM known_settable_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_MAC_PARAM_KEY, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *poly1305_settable_ctx_params(ossl_unused void *ctx,
                                                      ossl_unused void *provctx)
{
    return known_settable_ctx_params;
}

static int poly1305_set_ctx_params(void *vmacctx, const OSSL_PARAM *params)
{
    struct poly1305_data_st *ctx = vmacctx;
    const OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_KEY)) != NULL
            && !poly1305_setkey(ctx, p->data, p->data_size))
        return 0;
    return 1;
}

const OSSL_DISPATCH ossl_poly1305_functions[] = {
    { OSSL_FUNC_MAC_NEWCTX, (void (*)(void))poly1305_new },
    { OSSL_FUNC_MAC_DUPCTX, (void (*)(void))poly1305_dup },
    { OSSL_FUNC_MAC_FREECTX, (void (*)(void))poly1305_free },
    { OSSL_FUNC_MAC_INIT, (void (*)(void))poly1305_init },
    { OSSL_FUNC_MAC_UPDATE, (void (*)(void))poly1305_update },
    { OSSL_FUNC_MAC_FINAL, (void (*)(void))poly1305_final },
    { OSSL_FUNC_MAC_GETTABLE_PARAMS, (void (*)(void))poly1305_gettable_params },
    { OSSL_FUNC_MAC_GET_PARAMS, (void (*)(void))poly1305_get_params },
    { OSSL_FUNC_MAC_SETTABLE_CTX_PARAMS,
      (void (*)(void))poly1305_settable_ctx_params },
    { OSSL_FUNC_MAC_SET_CTX_PARAMS, (void (*)(void))poly1305_set_ctx_params },
    OSSL_DISPATCH_END
};
