/*
 * Copyright 2018-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdarg.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/core.h>
#include <openssl/core_names.h>
#include <openssl/types.h>
#include "internal/nelem.h"
#include "crypto/evp.h"
#include "internal/provider.h"
#include "evp_local.h"

EVP_MAC_CTX *EVP_MAC_CTX_new(EVP_MAC *mac)
{
    EVP_MAC_CTX *ctx = OPENSSL_zalloc(sizeof(EVP_MAC_CTX));

    if (ctx != NULL) {
        ctx->meth = mac;
        if ((ctx->algctx = mac->newctx(ossl_provider_ctx(mac->prov))) == NULL
            || !EVP_MAC_up_ref(mac)) {
            mac->freectx(ctx->algctx);
            ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
            OPENSSL_free(ctx);
            ctx = NULL;
        }
    }
    return ctx;
}

void EVP_MAC_CTX_free(EVP_MAC_CTX *ctx)
{
    if (ctx == NULL)
        return;
    ctx->meth->freectx(ctx->algctx);
    ctx->algctx = NULL;
    /* refcnt-- */
    EVP_MAC_free(ctx->meth);
    OPENSSL_free(ctx);
}

EVP_MAC_CTX *EVP_MAC_CTX_dup(const EVP_MAC_CTX *src)
{
    EVP_MAC_CTX *dst;

    if (src->algctx == NULL)
        return NULL;

    dst = OPENSSL_malloc(sizeof(*dst));
    if (dst == NULL)
        return NULL;

    *dst = *src;
    if (!EVP_MAC_up_ref(dst->meth)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        OPENSSL_free(dst);
        return NULL;
    }

    dst->algctx = src->meth->dupctx(src->algctx);
    if (dst->algctx == NULL) {
        EVP_MAC_CTX_free(dst);
        return NULL;
    }

    return dst;
}

EVP_MAC *EVP_MAC_CTX_get0_mac(EVP_MAC_CTX *ctx)
{
    return ctx->meth;
}

static size_t get_size_t_ctx_param(EVP_MAC_CTX *ctx, const char *name)
{
    size_t sz = 0;

    if (ctx->algctx != NULL) {
        OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };

        params[0] = OSSL_PARAM_construct_size_t(name, &sz);
        if (ctx->meth->get_ctx_params != NULL) {
            if (ctx->meth->get_ctx_params(ctx->algctx, params))
                return sz;
        } else if (ctx->meth->get_params != NULL) {
            if (ctx->meth->get_params(params))
                return sz;
        }
    }
    /*
     * If the MAC hasn't been initialized yet, or there is no size to get,
     * we return zero
     */
    return 0;
}

size_t EVP_MAC_CTX_get_mac_size(EVP_MAC_CTX *ctx)
{
    return get_size_t_ctx_param(ctx, OSSL_MAC_PARAM_SIZE);
}

size_t EVP_MAC_CTX_get_block_size(EVP_MAC_CTX *ctx)
{
    return get_size_t_ctx_param(ctx, OSSL_MAC_PARAM_BLOCK_SIZE);
}

int EVP_MAC_init(EVP_MAC_CTX *ctx, const unsigned char *key, size_t keylen,
                 const OSSL_PARAM params[])
{
    if (ctx->meth->init == NULL) {
        ERR_raise(ERR_R_EVP_LIB, ERR_R_UNSUPPORTED);
        return 0;
    }
    return ctx->meth->init(ctx->algctx, key, keylen, params);
}

int EVP_MAC_init_SKEY(EVP_MAC_CTX *ctx, EVP_SKEY *skey, const OSSL_PARAM params[])
{
    if (ctx->meth->init_skey == NULL
        || skey->skeymgmt->prov != ctx->meth->prov
        || ctx->meth->init_skey == NULL) {
        ERR_raise(ERR_R_EVP_LIB, ERR_R_UNSUPPORTED);
        return 0;
    }
    return ctx->meth->init_skey(ctx->algctx, skey->keydata, params);
}

int EVP_MAC_update(EVP_MAC_CTX *ctx, const unsigned char *data, size_t datalen)
{
    return ctx->meth->update(ctx->algctx, data, datalen);
}

static int evp_mac_final(EVP_MAC_CTX *ctx, int xof,
                         unsigned char *out, size_t *outl, size_t outsize)
{
    size_t l;
    int res;
    OSSL_PARAM params[2];
    size_t macsize;

    if (ctx == NULL || ctx->meth == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_NULL_ALGORITHM);
        return 0;
    }
    if (ctx->meth->final == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    macsize = EVP_MAC_CTX_get_mac_size(ctx);
    if (out == NULL) {
        if (outl == NULL) {
            ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
            return 0;
        }
        *outl = macsize;
        return 1;
    }
    if (outsize < macsize) {
        ERR_raise(ERR_LIB_EVP, EVP_R_BUFFER_TOO_SMALL);
        return 0;
    }
    if (xof) {
        params[0] = OSSL_PARAM_construct_int(OSSL_MAC_PARAM_XOF, &xof);
        params[1] = OSSL_PARAM_construct_end();

        if (EVP_MAC_CTX_set_params(ctx, params) <= 0) {
            ERR_raise(ERR_LIB_EVP, EVP_R_SETTING_XOF_FAILED);
            return 0;
        }
    }
    res = ctx->meth->final(ctx->algctx, out, &l, outsize);
    if (outl != NULL)
        *outl = l;
    return res;
}

int EVP_MAC_final(EVP_MAC_CTX *ctx,
                  unsigned char *out, size_t *outl, size_t outsize)
{
    return evp_mac_final(ctx, 0, out, outl, outsize);
}

int EVP_MAC_finalXOF(EVP_MAC_CTX *ctx, unsigned char *out, size_t outsize)
{
    return evp_mac_final(ctx, 1, out, NULL, outsize);
}

/*
 * The {get,set}_params functions return 1 if there is no corresponding
 * function in the implementation.  This is the same as if there was one,
 * but it didn't recognise any of the given params, i.e. nothing in the
 * bag of parameters was useful.
 */
int EVP_MAC_get_params(EVP_MAC *mac, OSSL_PARAM params[])
{
    if (mac->get_params != NULL)
        return mac->get_params(params);
    return 1;
}

int EVP_MAC_CTX_get_params(EVP_MAC_CTX *ctx, OSSL_PARAM params[])
{
    if (ctx->meth->get_ctx_params != NULL)
        return ctx->meth->get_ctx_params(ctx->algctx, params);
    return 1;
}

int EVP_MAC_CTX_set_params(EVP_MAC_CTX *ctx, const OSSL_PARAM params[])
{
    if (ctx->meth->set_ctx_params != NULL)
        return ctx->meth->set_ctx_params(ctx->algctx, params);
    return 1;
}

int evp_mac_get_number(const EVP_MAC *mac)
{
    return mac->name_id;
}

const char *EVP_MAC_get0_name(const EVP_MAC *mac)
{
    return mac->type_name;
}

const char *EVP_MAC_get0_description(const EVP_MAC *mac)
{
    return mac->description;
}

int EVP_MAC_is_a(const EVP_MAC *mac, const char *name)
{
    return mac != NULL && evp_is_a(mac->prov, mac->name_id, NULL, name);
}

int EVP_MAC_names_do_all(const EVP_MAC *mac,
                         void (*fn)(const char *name, void *data),
                         void *data)
{
    if (mac->prov != NULL)
        return evp_names_do_all(mac->prov, mac->name_id, fn, data);

    return 1;
}

unsigned char *EVP_Q_mac(OSSL_LIB_CTX *libctx,
                         const char *name, const char *propq,
                         const char *subalg, const OSSL_PARAM *params,
                         const void *key, size_t keylen,
                         const unsigned char *data, size_t datalen,
                         unsigned char *out, size_t outsize, size_t *outlen)
{
    EVP_MAC *mac = EVP_MAC_fetch(libctx, name, propq);
    OSSL_PARAM subalg_param[] = { OSSL_PARAM_END, OSSL_PARAM_END };
    EVP_MAC_CTX *ctx  = NULL;
    size_t len = 0;
    unsigned char *res = NULL;

    if (outlen != NULL)
        *outlen = 0;
    if (mac == NULL)
        return NULL;
    if (subalg != NULL) {
        const OSSL_PARAM *defined_params = EVP_MAC_settable_ctx_params(mac);
        const char *param_name = OSSL_MAC_PARAM_DIGEST;

        /*
         * The underlying algorithm may be a cipher or a digest.
         * We don't know which it is, but we can ask the MAC what it
         * should be and bet on that.
         */
        if (OSSL_PARAM_locate_const(defined_params, param_name) == NULL) {
            param_name = OSSL_MAC_PARAM_CIPHER;
            if (OSSL_PARAM_locate_const(defined_params, param_name) == NULL) {
                ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_INVALID_ARGUMENT);
                goto err;
            }
        }
        subalg_param[0] =
            OSSL_PARAM_construct_utf8_string(param_name, (char *)subalg, 0);
    }
    /* Single-shot - on NULL key input, set dummy key value for EVP_MAC_Init. */
    if (key == NULL && keylen == 0)
        key = data;
    if ((ctx = EVP_MAC_CTX_new(mac)) != NULL
            && EVP_MAC_CTX_set_params(ctx, subalg_param)
            && EVP_MAC_CTX_set_params(ctx, params)
            && EVP_MAC_init(ctx, key, keylen, params)
            && EVP_MAC_update(ctx, data, datalen)
            && EVP_MAC_final(ctx, out, &len, outsize)) {
        if (out == NULL) {
            out = OPENSSL_malloc(len);
            if (out != NULL && !EVP_MAC_final(ctx, out, NULL, len)) {
                OPENSSL_free(out);
                out = NULL;
            }
        }
        res = out;
        if (res != NULL && outlen != NULL)
            *outlen = len;
    }

 err:
    EVP_MAC_CTX_free(ctx);
    EVP_MAC_free(mac);
    return res;
}
