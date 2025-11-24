/*
 * Copyright 2018-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/proverr.h>

#include "crypto/siphash.h"

#include "prov/implementations.h"
#include "prov/providercommon.h"

/*
 * Forward declaration of everything implemented here.  This is not strictly
 * necessary for the compiler, but provides an assurance that the signatures
 * of the functions in the dispatch table are correct.
 */
static OSSL_FUNC_mac_newctx_fn siphash_new;
static OSSL_FUNC_mac_dupctx_fn siphash_dup;
static OSSL_FUNC_mac_freectx_fn siphash_free;
static OSSL_FUNC_mac_gettable_ctx_params_fn siphash_gettable_ctx_params;
static OSSL_FUNC_mac_get_ctx_params_fn siphash_get_ctx_params;
static OSSL_FUNC_mac_settable_ctx_params_fn siphash_settable_ctx_params;
static OSSL_FUNC_mac_set_ctx_params_fn siphash_set_params;
static OSSL_FUNC_mac_init_fn siphash_init;
static OSSL_FUNC_mac_update_fn siphash_update;
static OSSL_FUNC_mac_final_fn siphash_final;

struct siphash_data_st {
    void *provctx;
    SIPHASH siphash;             /* Siphash data */
    SIPHASH sipcopy;             /* Siphash data copy for reinitialization */
    unsigned int crounds, drounds;
};

static unsigned int crounds(struct siphash_data_st *ctx)
{
    return ctx->crounds != 0 ? ctx->crounds : SIPHASH_C_ROUNDS;
}

static unsigned int drounds(struct siphash_data_st *ctx)
{
    return ctx->drounds != 0 ? ctx->drounds : SIPHASH_D_ROUNDS;
}

static void *siphash_new(void *provctx)
{
    struct siphash_data_st *ctx;

    if (!ossl_prov_is_running())
        return NULL;
    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL)
        ctx->provctx = provctx;
    return ctx;
}

static void siphash_free(void *vmacctx)
{
    OPENSSL_free(vmacctx);
}

static void *siphash_dup(void *vsrc)
{
    struct siphash_data_st *ssrc = vsrc;
    struct siphash_data_st *sdst;

    if (!ossl_prov_is_running())
        return NULL;
    sdst = OPENSSL_malloc(sizeof(*sdst));
    if (sdst == NULL)
        return NULL;

    *sdst = *ssrc;
    return sdst;
}

static size_t siphash_size(void *vmacctx)
{
    struct siphash_data_st *ctx = vmacctx;

    return SipHash_hash_size(&ctx->siphash);
}

static int siphash_setkey(struct siphash_data_st *ctx,
                          const unsigned char *key, size_t keylen)
{
    int ret;

    if (keylen != SIPHASH_KEY_SIZE)
        return 0;
    ret = SipHash_Init(&ctx->siphash, key, crounds(ctx), drounds(ctx));
    if (ret)
        ctx->sipcopy = ctx->siphash;
    return ret;
}

static int siphash_init(void *vmacctx, const unsigned char *key, size_t keylen,
                        const OSSL_PARAM params[])
{
    struct siphash_data_st *ctx = vmacctx;

    if (!ossl_prov_is_running() || !siphash_set_params(ctx, params))
        return 0;
    /*
     * Without a key, there is not much to do here,
     * The actual initialization happens through controls.
     */
    if (key == NULL) {
        ctx->siphash = ctx->sipcopy;
        return 1;
    }
    return siphash_setkey(ctx, key, keylen);
}

static int siphash_update(void *vmacctx, const unsigned char *data,
                          size_t datalen)
{
    struct siphash_data_st *ctx = vmacctx;

    if (datalen == 0)
        return 1;

    SipHash_Update(&ctx->siphash, data, datalen);
    return 1;
}

static int siphash_final(void *vmacctx, unsigned char *out, size_t *outl,
                         size_t outsize)
{
    struct siphash_data_st *ctx = vmacctx;
    size_t hlen = siphash_size(ctx);

    if (!ossl_prov_is_running() || outsize < hlen)
        return 0;

    *outl = hlen;
    return SipHash_Final(&ctx->siphash, out, hlen);
}

static const OSSL_PARAM *siphash_gettable_ctx_params(ossl_unused void *ctx,
                                                     ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_MAC_PARAM_SIZE, NULL),
        OSSL_PARAM_uint(OSSL_MAC_PARAM_C_ROUNDS, NULL),
        OSSL_PARAM_uint(OSSL_MAC_PARAM_D_ROUNDS, NULL),
        OSSL_PARAM_END
    };

    return known_gettable_ctx_params;
}

static int siphash_get_ctx_params(void *vmacctx, OSSL_PARAM params[])
{
    struct siphash_data_st *ctx = vmacctx;
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_SIZE)) != NULL
        && !OSSL_PARAM_set_size_t(p, siphash_size(vmacctx)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_C_ROUNDS)) != NULL
        && !OSSL_PARAM_set_uint(p, crounds(ctx)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_D_ROUNDS)) != NULL
        && !OSSL_PARAM_set_uint(p, drounds(ctx)))
        return 0;
    return 1;
}

static const OSSL_PARAM *siphash_settable_ctx_params(ossl_unused void *ctx,
                                                     void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_MAC_PARAM_SIZE, NULL),
        OSSL_PARAM_octet_string(OSSL_MAC_PARAM_KEY, NULL, 0),
        OSSL_PARAM_uint(OSSL_MAC_PARAM_C_ROUNDS, NULL),
        OSSL_PARAM_uint(OSSL_MAC_PARAM_D_ROUNDS, NULL),
        OSSL_PARAM_END
    };

    return known_settable_ctx_params;
}

static int siphash_set_params(void *vmacctx, const OSSL_PARAM *params)
{
    struct siphash_data_st *ctx = vmacctx;
    const OSSL_PARAM *p = NULL;
    size_t size;

    if (ossl_param_is_empty(params))
        return 1;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_SIZE)) != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &size)
            || !SipHash_set_hash_size(&ctx->siphash, size)
            || !SipHash_set_hash_size(&ctx->sipcopy, size))
            return 0;
    }
    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_C_ROUNDS)) != NULL
            && !OSSL_PARAM_get_uint(p, &ctx->crounds))
        return 0;
    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_D_ROUNDS)) != NULL
            && !OSSL_PARAM_get_uint(p, &ctx->drounds))
        return 0;
    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_KEY)) != NULL)
        if (p->data_type != OSSL_PARAM_OCTET_STRING
            || !siphash_setkey(ctx, p->data, p->data_size))
            return 0;
    return 1;
}

const OSSL_DISPATCH ossl_siphash_functions[] = {
    { OSSL_FUNC_MAC_NEWCTX, (void (*)(void))siphash_new },
    { OSSL_FUNC_MAC_DUPCTX, (void (*)(void))siphash_dup },
    { OSSL_FUNC_MAC_FREECTX, (void (*)(void))siphash_free },
    { OSSL_FUNC_MAC_INIT, (void (*)(void))siphash_init },
    { OSSL_FUNC_MAC_UPDATE, (void (*)(void))siphash_update },
    { OSSL_FUNC_MAC_FINAL, (void (*)(void))siphash_final },
    { OSSL_FUNC_MAC_GETTABLE_CTX_PARAMS,
      (void (*)(void))siphash_gettable_ctx_params },
    { OSSL_FUNC_MAC_GET_CTX_PARAMS, (void (*)(void))siphash_get_ctx_params },
    { OSSL_FUNC_MAC_SETTABLE_CTX_PARAMS,
      (void (*)(void))siphash_settable_ctx_params },
    { OSSL_FUNC_MAC_SET_CTX_PARAMS, (void (*)(void))siphash_set_params },
    OSSL_DISPATCH_END
};
