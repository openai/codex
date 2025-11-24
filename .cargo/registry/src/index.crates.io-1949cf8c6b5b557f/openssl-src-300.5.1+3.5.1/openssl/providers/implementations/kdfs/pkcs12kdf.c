/*
 * Copyright 1999-2023 The OpenSSL Project Authors. All Rights Reserved.
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

static OSSL_FUNC_kdf_newctx_fn kdf_pkcs12_new;
static OSSL_FUNC_kdf_dupctx_fn kdf_pkcs12_dup;
static OSSL_FUNC_kdf_freectx_fn kdf_pkcs12_free;
static OSSL_FUNC_kdf_reset_fn kdf_pkcs12_reset;
static OSSL_FUNC_kdf_derive_fn kdf_pkcs12_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn kdf_pkcs12_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn kdf_pkcs12_set_ctx_params;
static OSSL_FUNC_kdf_gettable_ctx_params_fn kdf_pkcs12_gettable_ctx_params;
static OSSL_FUNC_kdf_get_ctx_params_fn kdf_pkcs12_get_ctx_params;

typedef struct {
    void *provctx;
    PROV_DIGEST digest;
    unsigned char *pass;
    size_t pass_len;
    unsigned char *salt;
    size_t salt_len;
    uint64_t iter;
    int id;
} KDF_PKCS12;

/* PKCS12 compatible key/IV generation */

static int pkcs12kdf_derive(const unsigned char *pass, size_t passlen,
                            const unsigned char *salt, size_t saltlen,
                            int id, uint64_t iter, const EVP_MD *md_type,
                            unsigned char *out, size_t n)
{
    unsigned char *B = NULL, *D = NULL, *I = NULL, *p = NULL, *Ai = NULL;
    size_t Slen, Plen, Ilen;
    size_t i, j, k, u, v;
    uint64_t iter_cnt;
    int ret = 0, ui, vi;
    EVP_MD_CTX *ctx = NULL;

    ctx = EVP_MD_CTX_new();
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EVP_LIB);
        goto end;
    }
    vi = EVP_MD_get_block_size(md_type);
    ui = EVP_MD_get_size(md_type);
    if (ui <= 0 || vi <= 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_SIZE);
        goto end;
    }
    u = (size_t)ui;
    v = (size_t)vi;
    D = OPENSSL_malloc(v);
    Ai = OPENSSL_malloc(u);
    B = OPENSSL_malloc(v + 1);
    Slen = v * ((saltlen + v - 1) / v);
    if (passlen != 0)
        Plen = v * ((passlen + v - 1) / v);
    else
        Plen = 0;
    Ilen = Slen + Plen;
    I = OPENSSL_malloc(Ilen);
    if (D == NULL || Ai == NULL || B == NULL || I == NULL)
        goto end;
    for (i = 0; i < v; i++)
        D[i] = id;
    p = I;
    for (i = 0; i < Slen; i++)
        *p++ = salt[i % saltlen];
    for (i = 0; i < Plen; i++)
        *p++ = pass[i % passlen];
    for (;;) {
        if (!EVP_DigestInit_ex(ctx, md_type, NULL)
            || !EVP_DigestUpdate(ctx, D, v)
            || !EVP_DigestUpdate(ctx, I, Ilen)
            || !EVP_DigestFinal_ex(ctx, Ai, NULL))
            goto end;
        for (iter_cnt = 1; iter_cnt < iter; iter_cnt++) {
            if (!EVP_DigestInit_ex(ctx, md_type, NULL)
                || !EVP_DigestUpdate(ctx, Ai, u)
                || !EVP_DigestFinal_ex(ctx, Ai, NULL))
                goto end;
        }
        memcpy(out, Ai, n < u ? n : u);
        if (u >= n) {
            ret = 1;
            break;
        }
        n -= u;
        out += u;
        for (j = 0; j < v; j++)
            B[j] = Ai[j % u];
        for (j = 0; j < Ilen; j += v) {
            unsigned char *Ij = I + j;
            uint16_t c = 1;

            /* Work out Ij = Ij + B + 1 */
            for (k = v; k > 0;) {
                k--;
                c += Ij[k] + B[k];
                Ij[k] = (unsigned char)c;
                c >>= 8;
            }
        }
    }

 end:
    OPENSSL_free(Ai);
    OPENSSL_free(B);
    OPENSSL_free(D);
    OPENSSL_free(I);
    EVP_MD_CTX_free(ctx);
    return ret;
}

static void *kdf_pkcs12_new(void *provctx)
{
    KDF_PKCS12 *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;
    ctx->provctx = provctx;
    return ctx;
}

static void kdf_pkcs12_cleanup(KDF_PKCS12 *ctx)
{
    ossl_prov_digest_reset(&ctx->digest);
    OPENSSL_free(ctx->salt);
    OPENSSL_clear_free(ctx->pass, ctx->pass_len);
    memset(ctx, 0, sizeof(*ctx));
}

static void kdf_pkcs12_free(void *vctx)
{
    KDF_PKCS12 *ctx = (KDF_PKCS12 *)vctx;

    if (ctx != NULL) {
        kdf_pkcs12_cleanup(ctx);
        OPENSSL_free(ctx);
    }
}

static void kdf_pkcs12_reset(void *vctx)
{
    KDF_PKCS12 *ctx = (KDF_PKCS12 *)vctx;
    void *provctx = ctx->provctx;

    kdf_pkcs12_cleanup(ctx);
    ctx->provctx = provctx;
}

static void *kdf_pkcs12_dup(void *vctx)
{
    const KDF_PKCS12 *src = (const KDF_PKCS12 *)vctx;
    KDF_PKCS12 *dest;

    dest = kdf_pkcs12_new(src->provctx);
    if (dest != NULL) {
        if (!ossl_prov_memdup(src->salt, src->salt_len,
                              &dest->salt, &dest->salt_len)
                || !ossl_prov_memdup(src->pass, src->pass_len,
                                     &dest->pass , &dest->pass_len)
                || !ossl_prov_digest_copy(&dest->digest, &src->digest))
            goto err;
        dest->iter = src->iter;
        dest->id = src->id;
    }
    return dest;

 err:
    kdf_pkcs12_free(dest);
    return NULL;
}

static int pkcs12kdf_set_membuf(unsigned char **buffer, size_t *buflen,
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

static int kdf_pkcs12_derive(void *vctx, unsigned char *key, size_t keylen,
                             const OSSL_PARAM params[])
{
    KDF_PKCS12 *ctx = (KDF_PKCS12 *)vctx;
    const EVP_MD *md;

    if (!ossl_prov_is_running() || !kdf_pkcs12_set_ctx_params(ctx, params))
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
    return pkcs12kdf_derive(ctx->pass, ctx->pass_len, ctx->salt, ctx->salt_len,
                            ctx->id, ctx->iter, md, key, keylen);
}

static int kdf_pkcs12_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    KDF_PKCS12 *ctx = vctx;
    OSSL_LIB_CTX *provctx = PROV_LIBCTX_OF(ctx->provctx);

    if (ossl_param_is_empty(params))
        return 1;

    if (!ossl_prov_digest_load_from_params(&ctx->digest, params, provctx))
        return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_PASSWORD)) != NULL)
        if (!pkcs12kdf_set_membuf(&ctx->pass, &ctx->pass_len, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SALT)) != NULL)
        if (!pkcs12kdf_set_membuf(&ctx->salt, &ctx->salt_len, p))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_PKCS12_ID)) != NULL)
        if (!OSSL_PARAM_get_int(p, &ctx->id))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_ITER)) != NULL)
        if (!OSSL_PARAM_get_uint64(p, &ctx->iter))
            return 0;
    return 1;
}

static const OSSL_PARAM *kdf_pkcs12_settable_ctx_params(
        ossl_unused void *ctx, ossl_unused void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_DIGEST, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_PASSWORD, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SALT, NULL, 0),
        OSSL_PARAM_uint64(OSSL_KDF_PARAM_ITER, NULL),
        OSSL_PARAM_int(OSSL_KDF_PARAM_PKCS12_ID, NULL),
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int kdf_pkcs12_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_SIZE)) != NULL)
        return OSSL_PARAM_set_size_t(p, SIZE_MAX);
    return -2;
}

static const OSSL_PARAM *kdf_pkcs12_gettable_ctx_params(
        ossl_unused void *ctx, ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_KDF_PARAM_SIZE, NULL),
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_kdf_pkcs12_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))kdf_pkcs12_new },
    { OSSL_FUNC_KDF_DUPCTX, (void(*)(void))kdf_pkcs12_dup },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))kdf_pkcs12_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))kdf_pkcs12_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))kdf_pkcs12_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_pkcs12_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))kdf_pkcs12_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))kdf_pkcs12_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))kdf_pkcs12_get_ctx_params },
    OSSL_DISPATCH_END
};
