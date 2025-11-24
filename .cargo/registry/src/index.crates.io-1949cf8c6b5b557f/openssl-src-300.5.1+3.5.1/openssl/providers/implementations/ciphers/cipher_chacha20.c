/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for chacha20 cipher */

#include <openssl/proverr.h>
#include "cipher_chacha20.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define CHACHA20_KEYLEN (CHACHA_KEY_SIZE)
#define CHACHA20_BLKLEN (1)
#define CHACHA20_IVLEN (CHACHA_CTR_SIZE)
#define CHACHA20_FLAGS (PROV_CIPHER_FLAG_CUSTOM_IV)

static OSSL_FUNC_cipher_newctx_fn chacha20_newctx;
static OSSL_FUNC_cipher_freectx_fn chacha20_freectx;
static OSSL_FUNC_cipher_dupctx_fn chacha20_dupctx;
static OSSL_FUNC_cipher_get_params_fn chacha20_get_params;
static OSSL_FUNC_cipher_get_ctx_params_fn chacha20_get_ctx_params;
static OSSL_FUNC_cipher_set_ctx_params_fn chacha20_set_ctx_params;
static OSSL_FUNC_cipher_gettable_ctx_params_fn chacha20_gettable_ctx_params;
static OSSL_FUNC_cipher_settable_ctx_params_fn chacha20_settable_ctx_params;
#define chacha20_cipher ossl_cipher_generic_cipher
#define chacha20_update ossl_cipher_generic_stream_update
#define chacha20_final ossl_cipher_generic_stream_final
#define chacha20_gettable_params ossl_cipher_generic_gettable_params

void ossl_chacha20_initctx(PROV_CHACHA20_CTX *ctx)
{
    ossl_cipher_generic_initkey(ctx, CHACHA20_KEYLEN * 8,
                                CHACHA20_BLKLEN * 8,
                                CHACHA20_IVLEN * 8,
                                0, CHACHA20_FLAGS,
                                ossl_prov_cipher_hw_chacha20(CHACHA20_KEYLEN * 8),
                                NULL);
}

static void *chacha20_newctx(void *provctx)
{
    PROV_CHACHA20_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL)
        ossl_chacha20_initctx(ctx);
    return ctx;
}

static void chacha20_freectx(void *vctx)
{
    PROV_CHACHA20_CTX *ctx = (PROV_CHACHA20_CTX *)vctx;

    if (ctx != NULL) {
        ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
        OPENSSL_clear_free(ctx, sizeof(*ctx));
    }
}

static void *chacha20_dupctx(void *vctx)
{
    PROV_CHACHA20_CTX *ctx = (PROV_CHACHA20_CTX *)vctx;
    PROV_CHACHA20_CTX *dupctx = NULL;

    if (ctx != NULL) {
        dupctx = OPENSSL_memdup(ctx, sizeof(*dupctx));
        if (dupctx != NULL && dupctx->base.tlsmac != NULL && dupctx->base.alloced) {
            dupctx->base.tlsmac = OPENSSL_memdup(dupctx->base.tlsmac,
                                                 dupctx->base.tlsmacsize);
            if (dupctx->base.tlsmac == NULL) {
                OPENSSL_free(dupctx);
                dupctx = NULL;
            }
        }
    }
    return dupctx;
}

static int chacha20_get_params(OSSL_PARAM params[])
{
    return ossl_cipher_generic_get_params(params, 0, CHACHA20_FLAGS,
                                          CHACHA20_KEYLEN * 8,
                                          CHACHA20_BLKLEN * 8,
                                          CHACHA20_IVLEN * 8);
}

static int chacha20_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, CHACHA20_IVLEN)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, CHACHA20_KEYLEN)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }

    return 1;
}

static const OSSL_PARAM chacha20_known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_END
};
const OSSL_PARAM *chacha20_gettable_ctx_params(ossl_unused void *cctx,
                                               ossl_unused void *provctx)
{
    return chacha20_known_gettable_ctx_params;
}

static int chacha20_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    size_t len;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &len)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (len != CHACHA20_KEYLEN) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &len)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (len != CHACHA20_IVLEN) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
    }
    return 1;
}

static const OSSL_PARAM chacha20_known_settable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_END
};
const OSSL_PARAM *chacha20_settable_ctx_params(ossl_unused void *cctx,
                                               ossl_unused void *provctx)
{
    return chacha20_known_settable_ctx_params;
}

int ossl_chacha20_einit(void *vctx, const unsigned char *key, size_t keylen,
                        const unsigned char *iv, size_t ivlen,
                        const OSSL_PARAM params[])
{
    int ret;

    /* The generic function checks for ossl_prov_is_running() */
    ret = ossl_cipher_generic_einit(vctx, key, keylen, iv, ivlen, NULL);
    if (ret && iv != NULL) {
        PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
        PROV_CIPHER_HW_CHACHA20 *hw = (PROV_CIPHER_HW_CHACHA20 *)ctx->hw;

        hw->initiv(ctx);
    }
    if (ret && !chacha20_set_ctx_params(vctx, params))
        ret = 0;
    return ret;
}

int ossl_chacha20_dinit(void *vctx, const unsigned char *key, size_t keylen,
                        const unsigned char *iv, size_t ivlen,
                        const OSSL_PARAM params[])
{
    int ret;

    /* The generic function checks for ossl_prov_is_running() */
    ret = ossl_cipher_generic_dinit(vctx, key, keylen, iv, ivlen, NULL);
    if (ret && iv != NULL) {
        PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
        PROV_CIPHER_HW_CHACHA20 *hw = (PROV_CIPHER_HW_CHACHA20 *)ctx->hw;

        hw->initiv(ctx);
    }
    if (ret && !chacha20_set_ctx_params(vctx, params))
        ret = 0;
    return ret;
}

/* ossl_chacha20_functions */
const OSSL_DISPATCH ossl_chacha20_functions[] = {
    { OSSL_FUNC_CIPHER_NEWCTX, (void (*)(void))chacha20_newctx },
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))chacha20_freectx },
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))chacha20_dupctx },
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))ossl_chacha20_einit },
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))ossl_chacha20_dinit },
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))chacha20_update },
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))chacha20_final },
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))chacha20_cipher},
    { OSSL_FUNC_CIPHER_GET_PARAMS, (void (*)(void))chacha20_get_params },
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS, (void (*)(void))chacha20_gettable_params },
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS, (void (*)(void))chacha20_get_ctx_params },
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,
        (void (*)(void))chacha20_gettable_ctx_params },
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS, (void (*)(void))chacha20_set_ctx_params },
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,
        (void (*)(void))chacha20_settable_ctx_params },
    OSSL_DISPATCH_END
};

