/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/core_names.h>
#include <openssl/proverr.h>
#include <openssl/err.h>
#include "prov/blake2.h"
#include "prov/digestcommon.h"
#include "prov/implementations.h"

#define IMPLEMENT_BLAKE_functions(variant, VARIANT, variantsize) \
static const OSSL_PARAM known_blake##variant##_ctx_params[] = { \
    {OSSL_DIGEST_PARAM_SIZE, OSSL_PARAM_UNSIGNED_INTEGER, NULL, 0, 0}, \
    OSSL_PARAM_END \
}; \
 \
const OSSL_PARAM *ossl_blake##variant##_gettable_ctx_params(ossl_unused void *ctx, \
                                                   ossl_unused void *pctx) \
{ \
    return known_blake##variant##_ctx_params; \
} \
 \
const OSSL_PARAM *ossl_blake##variant##_settable_ctx_params(ossl_unused void *ctx, \
                                                   ossl_unused void *pctx) \
{ \
    return known_blake##variant##_ctx_params; \
} \
 \
int ossl_blake##variant##_get_ctx_params(void *vctx, OSSL_PARAM params[]) \
{ \
    struct blake##variant##_md_data_st *mdctx = vctx; \
    OSSL_PARAM *p; \
 \
    BLAKE##VARIANT##_CTX *ctx = &mdctx->ctx; \
 \
    if (ctx == NULL) \
        return 0; \
    if (ossl_param_is_empty(params)) \
        return 1; \
 \
    p = OSSL_PARAM_locate(params, OSSL_DIGEST_PARAM_SIZE); \
    if (p != NULL \
        && !OSSL_PARAM_set_uint(p, (unsigned int)mdctx->params.digest_length)) { \
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER); \
        return 0; \
    } \
 \
    return 1; \
} \
 \
int ossl_blake##variant##_set_ctx_params(void *vctx, const OSSL_PARAM params[]) \
{ \
    size_t size; \
    struct blake##variant##_md_data_st *mdctx = vctx; \
    const OSSL_PARAM *p; \
 \
    BLAKE##VARIANT##_CTX *ctx = &mdctx->ctx; \
 \
    if (ctx == NULL) \
        return 0; \
    if (ossl_param_is_empty(params)) \
        return 1; \
 \
    p = OSSL_PARAM_locate_const(params, OSSL_DIGEST_PARAM_SIZE); \
    if (p != NULL) { \
        if (!OSSL_PARAM_get_size_t(p, &size)) { \
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER); \
            return 0; \
        } \
        if (size < 1 || size > BLAKE##VARIANT##_OUTBYTES) { \
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_SIZE); \
            return 0; \
        } \
        ossl_blake##variant##_param_set_digest_length(&mdctx->params, (uint8_t)size); \
    } \
 \
    return 1; \
} \
 \
static int ossl_blake##variantsize##_init(void *ctx) \
{ \
    struct blake##variant##_md_data_st *mdctx = ctx; \
    uint8_t digest_length = mdctx->params.digest_length; \
 \
    ossl_blake##variant##_param_init(&mdctx->params); \
    if (digest_length != 0) \
        mdctx->params.digest_length = digest_length; \
    return ossl_blake##variant##_init(&mdctx->ctx, &mdctx->params); \
} \
 \
static OSSL_FUNC_digest_init_fn blake##variantsize##_internal_init; \
static OSSL_FUNC_digest_newctx_fn blake##variantsize##_newctx; \
static OSSL_FUNC_digest_freectx_fn blake##variantsize##_freectx; \
static OSSL_FUNC_digest_dupctx_fn blake##variantsize##_dupctx; \
static OSSL_FUNC_digest_final_fn blake##variantsize##_internal_final; \
static OSSL_FUNC_digest_get_params_fn blake##variantsize##_get_params; \
 \
static int blake##variantsize##_internal_init(void *ctx, const OSSL_PARAM params[]) \
{ \
    return ossl_prov_is_running() && ossl_blake##variant##_set_ctx_params(ctx, params) \
        && ossl_blake##variantsize##_init(ctx); \
} \
 \
static void *blake##variantsize##_newctx(void *prov_ctx) \
{ \
    struct blake##variant##_md_data_st *ctx; \
 \
    ctx = ossl_prov_is_running() ? OPENSSL_zalloc(sizeof(*ctx)) : NULL; \
    return ctx; \
} \
 \
static void blake##variantsize##_freectx(void *vctx) \
{ \
    struct blake##variant##_md_data_st *ctx; \
 \
    ctx = (struct blake##variant##_md_data_st *)vctx; \
    OPENSSL_clear_free(ctx, sizeof(*ctx)); \
} \
 \
static void *blake##variantsize##_dupctx(void *ctx) \
{ \
    struct blake##variant##_md_data_st *in, *ret; \
 \
    in = (struct blake##variant##_md_data_st *)ctx; \
    ret = ossl_prov_is_running()? OPENSSL_malloc(sizeof(*ret)) : NULL; \
    if (ret != NULL) \
        *ret = *in; \
    return ret; \
} \
\
static void blake##variantsize##_copyctx(void *voutctx, void *vinctx) \
{ \
    struct blake##variant##_md_data_st *inctx, *outctx; \
 \
    outctx = (struct blake##variant##_md_data_st *)voutctx; \
    inctx = (struct blake##variant##_md_data_st *)vinctx; \
    *outctx = *inctx; \
} \
 \
static int blake##variantsize##_internal_final(void *ctx, unsigned char *out, \
                                     size_t *outl, size_t outsz) \
{ \
    struct blake##variant##_md_data_st *b_ctx; \
 \
    b_ctx = (struct blake##variant##_md_data_st *)ctx; \
 \
    if (!ossl_prov_is_running()) \
        return 0; \
 \
    *outl = b_ctx->ctx.outlen; \
 \
    if (outsz == 0) \
       return 1; \
 \
    if (outsz < *outl) { \
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_SIZE); \
        return 0; \
    } \
 \
    return ossl_blake##variant##_final(out, ctx); \
} \
 \
static int blake##variantsize##_get_params(OSSL_PARAM params[]) \
{ \
    return ossl_digest_default_get_params(params, BLAKE##VARIANT##_BLOCKBYTES, BLAKE##VARIANT##_OUTBYTES, 0); \
} \
 \
const OSSL_DISPATCH ossl_blake##variantsize##_functions[] = { \
    {OSSL_FUNC_DIGEST_NEWCTX, (void (*)(void))blake##variantsize##_newctx}, \
    {OSSL_FUNC_DIGEST_UPDATE, (void (*)(void))ossl_blake##variant##_update}, \
    {OSSL_FUNC_DIGEST_FINAL, (void (*)(void))blake##variantsize##_internal_final}, \
    {OSSL_FUNC_DIGEST_FREECTX, (void (*)(void))blake##variantsize##_freectx}, \
    {OSSL_FUNC_DIGEST_DUPCTX, (void (*)(void))blake##variantsize##_dupctx}, \
    {OSSL_FUNC_DIGEST_COPYCTX, (void (*)(void))blake##variantsize##_copyctx}, \
    {OSSL_FUNC_DIGEST_GET_PARAMS, (void (*)(void))blake##variantsize##_get_params}, \
    {OSSL_FUNC_DIGEST_GETTABLE_PARAMS, \
     (void (*)(void))ossl_digest_default_gettable_params}, \
    {OSSL_FUNC_DIGEST_INIT, (void (*)(void))blake##variantsize##_internal_init}, \
    {OSSL_FUNC_DIGEST_GETTABLE_CTX_PARAMS, \
     (void (*)(void))ossl_blake##variant##_gettable_ctx_params}, \
    {OSSL_FUNC_DIGEST_SETTABLE_CTX_PARAMS, \
     (void (*)(void))ossl_blake##variant##_settable_ctx_params}, \
    {OSSL_FUNC_DIGEST_GET_CTX_PARAMS, \
     (void (*)(void))ossl_blake##variant##_get_ctx_params}, \
    {OSSL_FUNC_DIGEST_SET_CTX_PARAMS, \
     (void (*)(void))ossl_blake##variant##_set_ctx_params}, \
    {0, NULL} \
};

IMPLEMENT_BLAKE_functions(2s, 2S, 2s256)
IMPLEMENT_BLAKE_functions(2b, 2B, 2b512)
