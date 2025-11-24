/*
 * Copyright 2018-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/core_names.h>
#include <openssl/proverr.h>
#include <openssl/err.h>
#include "internal/numbers.h"    /* SIZE_MAX */
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_util.h"

static OSSL_FUNC_kdf_newctx_fn kdf_pvk_new;
static OSSL_FUNC_kdf_dupctx_fn kdf_pvk_dup;
static OSSL_FUNC_kdf_freectx_fn kdf_pvk_free;
static OSSL_FUNC_kdf_reset_fn kdf_pvk_reset;
static OSSL_FUNC_kdf_derive_fn kdf_pvk_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn kdf_pvk_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn kdf_pvk_set_ctx_params;
static OSSL_FUNC_kdf_gettable_ctx_params_fn kdf_pvk_gettable_ctx_params;
static OSSL_FUNC_kdf_get_ctx_params_fn kdf_pvk_get_ctx_params;

typedef struct {
    void *provctx;
    unsigned char *pass;
    size_t pass_len;
    unsigned char *salt;
    size_t salt_len;
    PROV_DIGEST digest;
} KDF_PVK;

static void kdf_pvk_init(KDF_PVK *ctx);

static void *kdf_pvk_new(void *provctx)
{
    KDF_PVK *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;
    ctx->provctx = provctx;
    kdf_pvk_init(ctx);
    return ctx;
}

static void kdf_pvk_cleanup(KDF_PVK *ctx)
{
    ossl_prov_digest_reset(&ctx->digest);
    OPENSSL_free(ctx->salt);
    OPENSSL_clear_free(ctx->pass, ctx->pass_len);
    OPENSSL_cleanse(ctx, sizeof(*ctx));
}

static void kdf_pvk_free(void *vctx)
{
    KDF_PVK *ctx = (KDF_PVK *)vctx;

    if (ctx != NULL) {
        kdf_pvk_cleanup(ctx);
        OPENSSL_free(ctx);
    }
}

static void *kdf_pvk_dup(void *vctx)
{
    const KDF_PVK *src = (const KDF_PVK *)vctx;
    KDF_PVK *dest;

    dest = kdf_pvk_new(src->provctx);
    if (dest != NULL)
        if (!ossl_prov_memdup(src->salt, src->salt_len,
                              &dest->salt, &dest->salt_len)
                || !ossl_prov_memdup(src->pass, src->pass_len,
                                     &dest->pass , &dest->pass_len)
                || !ossl_prov_digest_copy(&dest->digest, &src->digest))
            goto err;
    return dest;

 err:
    kdf_pvk_free(dest);
    return NULL;
}

static void kdf_pvk_reset(void *vctx)
{
    KDF_PVK *ctx = (KDF_PVK *)vctx;
    void *provctx = ctx->provctx;

    kdf_pvk_cleanup(ctx);
    ctx->provctx = provctx;
    kdf_pvk_init(ctx);
}

static void kdf_pvk_init(KDF_PVK *ctx)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    OSSL_LIB_CTX *provctx = PROV_LIBCTX_OF(ctx->provctx);

    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                                 SN_sha1, 0);
    if (!ossl_prov_digest_load_from_params(&ctx->digest, params, provctx))
        /* This is an error, but there is no way to indicate such directly */
        ossl_prov_digest_reset(&ctx->digest);
}

static int pvk_set_membuf(unsigned char **buffer, size_t *buflen,
                             const OSSL_PARAM *p)
{
    OPENSSL_clear_free(*buffer, *buflen);
    *buffer = NULL;
    *buflen = 0;

    if (p->data_size == 0) {
        if ((*buffer = OPENSSL_malloc(1)) == NULL)
            return 0;
    } else if (p->data != NULL) {
        if (!OSSL_PARAM_get_octet_string(p, (void **)buffer, 0, buflen))
            return 0;
    }
    return 1;
}

static int kdf_pvk_derive(void *vctx, unsigned char *key, size_t keylen,
                             const OSSL_PARAM params[])
{
    KDF_PVK *ctx = (KDF_PVK *)vctx;
    const EVP_MD *md;
    EVP_MD_CTX *mctx;
    int res;

    if (!ossl_prov_is_running() || !kdf_pvk_set_ctx_params(ctx, params))
        return 0;

    if (ctx->pass == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_PASS);
        return 0;
    }

    if (ctx->salt == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_SALT);
        return 0;
    }

    md = ossl_prov_digest_md(&ctx->digest);
    if (md == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST);
        return 0;
    }
    res = EVP_MD_get_size(md);
    if (res <= 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_BAD_LENGTH);
        return 0;
    }
    if ((size_t)res > keylen) {
        ERR_raise(ERR_LIB_PROV, PROV_R_LENGTH_TOO_LARGE);
        return 0;
    }

    mctx = EVP_MD_CTX_new();
    res = mctx != NULL
          && EVP_DigestInit_ex(mctx, md, NULL)
          && EVP_DigestUpdate(mctx, ctx->salt, ctx->salt_len)
          && EVP_DigestUpdate(mctx, ctx->pass, ctx->pass_len)
          && EVP_DigestFinal_ex(mctx, key, NULL);
    EVP_MD_CTX_free(mctx);
    return res;
}

static int kdf_pvk_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    KDF_PVK *ctx = vctx;
    OSSL_LIB_CTX *provctx = PROV_LIBCTX_OF(ctx->provctx);

    if (ossl_param_is_empty(params))
        return 1;

    if (!ossl_prov_digest_load_from_params(&ctx->digest, params, provctx))
        return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_PASSWORD)) != NULL)
        if (!pvk_set_membuf(&ctx->pass, &ctx->pass_len, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SALT)) != NULL) {
        if (!pvk_set_membuf(&ctx->salt, &ctx->salt_len, p))
            return 0;
    }

    return 1;
}

static const OSSL_PARAM *kdf_pvk_settable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_DIGEST, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_PASSWORD, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SALT, NULL, 0),
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int kdf_pvk_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_SIZE)) != NULL)
        return OSSL_PARAM_set_size_t(p, SIZE_MAX);
    return -2;
}

static const OSSL_PARAM *kdf_pvk_gettable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_KDF_PARAM_SIZE, NULL),
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_kdf_pvk_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))kdf_pvk_new },
    { OSSL_FUNC_KDF_DUPCTX, (void(*)(void))kdf_pvk_dup },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))kdf_pvk_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))kdf_pvk_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))kdf_pvk_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_pvk_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))kdf_pvk_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_pvk_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))kdf_pvk_get_ctx_params },
    OSSL_DISPATCH_END
};
