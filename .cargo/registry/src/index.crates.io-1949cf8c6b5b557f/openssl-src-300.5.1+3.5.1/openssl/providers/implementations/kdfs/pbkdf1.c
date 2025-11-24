/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/trace.h>
#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/core_names.h>
#include <openssl/proverr.h>
#include "internal/cryptlib.h"
#include "internal/numbers.h"
#include "crypto/evp.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_util.h"

static OSSL_FUNC_kdf_newctx_fn kdf_pbkdf1_new;
static OSSL_FUNC_kdf_dupctx_fn kdf_pbkdf1_dup;
static OSSL_FUNC_kdf_freectx_fn kdf_pbkdf1_free;
static OSSL_FUNC_kdf_reset_fn kdf_pbkdf1_reset;
static OSSL_FUNC_kdf_derive_fn kdf_pbkdf1_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn kdf_pbkdf1_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn kdf_pbkdf1_set_ctx_params;
static OSSL_FUNC_kdf_gettable_ctx_params_fn kdf_pbkdf1_gettable_ctx_params;
static OSSL_FUNC_kdf_get_ctx_params_fn kdf_pbkdf1_get_ctx_params;

typedef struct {
    void *provctx;
    PROV_DIGEST digest;
    unsigned char *pass;
    size_t pass_len;
    unsigned char *salt;
    size_t salt_len;
    uint64_t iter;
} KDF_PBKDF1;

/*
 * PKCS5 PBKDF1 compatible key/IV generation as specified in:
 * https://tools.ietf.org/html/rfc8018#page-10
 */

static int kdf_pbkdf1_do_derive(const unsigned char *pass, size_t passlen,
                                const unsigned char *salt, size_t saltlen,
                                uint64_t iter, const EVP_MD *md_type,
                                unsigned char *out, size_t n)
{
    uint64_t i;
    int mdsize, ret = 0;
    unsigned char md_tmp[EVP_MAX_MD_SIZE];
    EVP_MD_CTX *ctx = NULL;

    ctx = EVP_MD_CTX_new();
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EVP_LIB);
        goto err;
    }

    if (!EVP_DigestInit_ex(ctx, md_type, NULL)
        || !EVP_DigestUpdate(ctx, pass, passlen)
        || !EVP_DigestUpdate(ctx, salt, saltlen)
        || !EVP_DigestFinal_ex(ctx, md_tmp, NULL))
        goto err;
    mdsize = EVP_MD_size(md_type);
    if (mdsize <= 0)
        goto err;
    if (n > (size_t)mdsize) {
        ERR_raise(ERR_LIB_PROV, PROV_R_LENGTH_TOO_LARGE);
        goto err;
    }

    for (i = 1; i < iter; i++) {
        if (!EVP_DigestInit_ex(ctx, md_type, NULL))
            goto err;
        if (!EVP_DigestUpdate(ctx, md_tmp, mdsize))
            goto err;
        if (!EVP_DigestFinal_ex(ctx, md_tmp, NULL))
            goto err;
    }

    memcpy(out, md_tmp, n);
    ret = 1;
err:
    OPENSSL_cleanse(md_tmp, EVP_MAX_MD_SIZE);
    EVP_MD_CTX_free(ctx);
    return ret;
}

static void *kdf_pbkdf1_new(void *provctx)
{
    KDF_PBKDF1 *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;
    ctx->provctx = provctx;
    return ctx;
}

static void kdf_pbkdf1_cleanup(KDF_PBKDF1 *ctx)
{
    ossl_prov_digest_reset(&ctx->digest);
    OPENSSL_free(ctx->salt);
    OPENSSL_clear_free(ctx->pass, ctx->pass_len);
    memset(ctx, 0, sizeof(*ctx));
}

static void kdf_pbkdf1_free(void *vctx)
{
    KDF_PBKDF1 *ctx = (KDF_PBKDF1 *)vctx;

    if (ctx != NULL) {
        kdf_pbkdf1_cleanup(ctx);
        OPENSSL_free(ctx);
    }
}

static void kdf_pbkdf1_reset(void *vctx)
{
    KDF_PBKDF1 *ctx = (KDF_PBKDF1 *)vctx;
    void *provctx = ctx->provctx;

    kdf_pbkdf1_cleanup(ctx);
    ctx->provctx = provctx;
}

static void *kdf_pbkdf1_dup(void *vctx)
{
    const KDF_PBKDF1 *src = (const KDF_PBKDF1 *)vctx;
    KDF_PBKDF1 *dest;

    dest = kdf_pbkdf1_new(src->provctx);
    if (dest != NULL) {
        if (!ossl_prov_memdup(src->salt, src->salt_len,
                              &dest->salt, &dest->salt_len)
                || !ossl_prov_memdup(src->pass, src->pass_len,
                                     &dest->pass , &dest->pass_len)
                || !ossl_prov_digest_copy(&dest->digest, &src->digest))
            goto err;
        dest->iter = src->iter;
    }
    return dest;

 err:
    kdf_pbkdf1_free(dest);
    return NULL;
}

static int kdf_pbkdf1_set_membuf(unsigned char **buffer, size_t *buflen,
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

static int kdf_pbkdf1_derive(void *vctx, unsigned char *key, size_t keylen,
                             const OSSL_PARAM params[])
{
    KDF_PBKDF1 *ctx = (KDF_PBKDF1 *)vctx;
    const EVP_MD *md;

    if (!ossl_prov_is_running() || !kdf_pbkdf1_set_ctx_params(ctx, params))
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
    return kdf_pbkdf1_do_derive(ctx->pass, ctx->pass_len, ctx->salt, ctx->salt_len,
                                ctx->iter, md, key, keylen);
}

static int kdf_pbkdf1_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    KDF_PBKDF1 *ctx = vctx;
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);

    if (!ossl_prov_digest_load_from_params(&ctx->digest, params, libctx))
        return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_PASSWORD)) != NULL)
        if (!kdf_pbkdf1_set_membuf(&ctx->pass, &ctx->pass_len, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SALT)) != NULL)
        if (!kdf_pbkdf1_set_membuf(&ctx->salt, &ctx->salt_len, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_ITER)) != NULL)
        if (!OSSL_PARAM_get_uint64(p, &ctx->iter))
            return 0;
    return 1;
}

static const OSSL_PARAM *kdf_pbkdf1_settable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_DIGEST, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_PASSWORD, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SALT, NULL, 0),
        OSSL_PARAM_uint64(OSSL_KDF_PARAM_ITER, NULL),
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int kdf_pbkdf1_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_SIZE)) != NULL)
        return OSSL_PARAM_set_size_t(p, SIZE_MAX);
    return -2;
}

static const OSSL_PARAM *kdf_pbkdf1_gettable_ctx_params(ossl_unused void *ctx,
                                                        ossl_unused void *p_ctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_KDF_PARAM_SIZE, NULL),
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_kdf_pbkdf1_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))kdf_pbkdf1_new },
    { OSSL_FUNC_KDF_DUPCTX, (void(*)(void))kdf_pbkdf1_dup },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))kdf_pbkdf1_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))kdf_pbkdf1_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))kdf_pbkdf1_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_pbkdf1_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))kdf_pbkdf1_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_pbkdf1_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))kdf_pbkdf1_get_ctx_params },
    OSSL_DISPATCH_END
};
