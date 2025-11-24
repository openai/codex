/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for chacha20_poly1305 cipher */

#include <openssl/proverr.h>
#include "cipher_chacha20_poly1305.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define CHACHA20_POLY1305_KEYLEN CHACHA_KEY_SIZE
#define CHACHA20_POLY1305_BLKLEN 1
#define CHACHA20_POLY1305_MAX_IVLEN 12
#define CHACHA20_POLY1305_MODE 0
#define CHACHA20_POLY1305_FLAGS (PROV_CIPHER_FLAG_AEAD                         \
                                 | PROV_CIPHER_FLAG_CUSTOM_IV)

static OSSL_FUNC_cipher_newctx_fn chacha20_poly1305_newctx;
static OSSL_FUNC_cipher_freectx_fn chacha20_poly1305_freectx;
static OSSL_FUNC_cipher_dupctx_fn chacha20_poly1305_dupctx;
static OSSL_FUNC_cipher_encrypt_init_fn chacha20_poly1305_einit;
static OSSL_FUNC_cipher_decrypt_init_fn chacha20_poly1305_dinit;
static OSSL_FUNC_cipher_get_params_fn chacha20_poly1305_get_params;
static OSSL_FUNC_cipher_get_ctx_params_fn chacha20_poly1305_get_ctx_params;
static OSSL_FUNC_cipher_set_ctx_params_fn chacha20_poly1305_set_ctx_params;
static OSSL_FUNC_cipher_cipher_fn chacha20_poly1305_cipher;
static OSSL_FUNC_cipher_final_fn chacha20_poly1305_final;
static OSSL_FUNC_cipher_gettable_ctx_params_fn chacha20_poly1305_gettable_ctx_params;
static OSSL_FUNC_cipher_settable_ctx_params_fn chacha20_poly1305_settable_ctx_params;
#define chacha20_poly1305_gettable_params ossl_cipher_generic_gettable_params
#define chacha20_poly1305_update chacha20_poly1305_cipher

static void *chacha20_poly1305_newctx(void *provctx)
{
    PROV_CHACHA20_POLY1305_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL) {
        ossl_cipher_generic_initkey(&ctx->base, CHACHA20_POLY1305_KEYLEN * 8,
                                    CHACHA20_POLY1305_BLKLEN * 8,
                                    CHACHA20_POLY1305_IVLEN * 8,
                                    CHACHA20_POLY1305_MODE,
                                    CHACHA20_POLY1305_FLAGS,
                                    ossl_prov_cipher_hw_chacha20_poly1305(
                                        CHACHA20_POLY1305_KEYLEN * 8),
                                    NULL);
        ctx->tls_payload_length = NO_TLS_PAYLOAD_LENGTH;
        ossl_chacha20_initctx(&ctx->chacha);
    }
    return ctx;
}

static void *chacha20_poly1305_dupctx(void *provctx)
{
    PROV_CHACHA20_POLY1305_CTX *ctx = provctx;
    PROV_CHACHA20_POLY1305_CTX *dctx = NULL;

    if (ctx == NULL)
        return NULL;
    dctx = OPENSSL_memdup(ctx, sizeof(*ctx));
    if (dctx != NULL && dctx->base.tlsmac != NULL && dctx->base.alloced) {
        dctx->base.tlsmac = OPENSSL_memdup(dctx->base.tlsmac,
                                           dctx->base.tlsmacsize);
        if (dctx->base.tlsmac == NULL) {
            OPENSSL_free(dctx);
            dctx = NULL;
        }
    }
    return dctx;
}

static void chacha20_poly1305_freectx(void *vctx)
{
    PROV_CHACHA20_POLY1305_CTX *ctx = (PROV_CHACHA20_POLY1305_CTX *)vctx;

    if (ctx != NULL) {
        ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
        OPENSSL_clear_free(ctx, sizeof(*ctx));
    }
}

static int chacha20_poly1305_get_params(OSSL_PARAM params[])
{
    return ossl_cipher_generic_get_params(params, 0, CHACHA20_POLY1305_FLAGS,
                                          CHACHA20_POLY1305_KEYLEN * 8,
                                          CHACHA20_POLY1305_BLKLEN * 8,
                                          CHACHA20_POLY1305_IVLEN * 8);
}

static int chacha20_poly1305_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_CHACHA20_POLY1305_CTX *ctx = (PROV_CHACHA20_POLY1305_CTX *)vctx;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_set_size_t(p, CHACHA20_POLY1305_IVLEN)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, CHACHA20_POLY1305_KEYLEN)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAGLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->tag_len)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TLS1_AAD_PAD);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->tls_aad_pad_sz)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAG);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
        if (!ctx->base.enc) {
            ERR_raise(ERR_LIB_PROV, PROV_R_TAG_NOT_SET);
            return 0;
        }
        if (p->data_size == 0 || p->data_size > POLY1305_BLOCK_SIZE) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_TAG_LENGTH);
            return 0;
        }
        memcpy(p->data, ctx->tag, p->data_size);
    }

    return 1;
}

static const OSSL_PARAM chacha20_poly1305_known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TAGLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD_PAD, NULL),
    OSSL_PARAM_END
};
static const OSSL_PARAM *chacha20_poly1305_gettable_ctx_params
    (ossl_unused void *cctx, ossl_unused void *provctx)
{
    return chacha20_poly1305_known_gettable_ctx_params;
}

static const OSSL_PARAM chacha20_poly1305_known_settable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_IV_FIXED, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *chacha20_poly1305_settable_ctx_params(
        ossl_unused void *cctx, ossl_unused void *provctx
    )
{
    return chacha20_poly1305_known_settable_ctx_params;
}

static int chacha20_poly1305_set_ctx_params(void *vctx,
                                            const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    size_t len;
    PROV_CHACHA20_POLY1305_CTX *ctx = (PROV_CHACHA20_POLY1305_CTX *)vctx;
    PROV_CIPHER_HW_CHACHA20_POLY1305 *hw =
        (PROV_CIPHER_HW_CHACHA20_POLY1305 *)ctx->base.hw;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &len)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (len != CHACHA20_POLY1305_KEYLEN) {
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
        if (len != CHACHA20_POLY1305_MAX_IVLEN) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TAG);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (p->data_size == 0 || p->data_size > POLY1305_BLOCK_SIZE) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_TAG_LENGTH);
            return 0;
        }
        if (p->data != NULL) {
            if (ctx->base.enc) {
                ERR_raise(ERR_LIB_PROV, PROV_R_TAG_NOT_NEEDED);
                return 0;
            }
            memcpy(ctx->tag, p->data, p->data_size);
        }
        ctx->tag_len = p->data_size;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TLS1_AAD);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        len = hw->tls_init(&ctx->base, p->data, p->data_size);
        if (len == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DATA);
            return 0;
        }
        ctx->tls_aad_pad_sz = len;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TLS1_IV_FIXED);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (hw->tls_iv_set_fixed(&ctx->base, p->data, p->data_size) == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
    }
    return 1;
}

static int chacha20_poly1305_einit(void *vctx, const unsigned char *key,
                                  size_t keylen, const unsigned char *iv,
                                  size_t ivlen, const OSSL_PARAM params[])
{
    int ret;

    /* The generic function checks for ossl_prov_is_running() */
    ret = ossl_cipher_generic_einit(vctx, key, keylen, iv, ivlen, NULL);
    if (ret && iv != NULL) {
        PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
        PROV_CIPHER_HW_CHACHA20_POLY1305 *hw =
            (PROV_CIPHER_HW_CHACHA20_POLY1305 *)ctx->hw;

        hw->initiv(ctx);
    }
    if (ret && !chacha20_poly1305_set_ctx_params(vctx, params))
        ret = 0;
    return ret;
}

static int chacha20_poly1305_dinit(void *vctx, const unsigned char *key,
                                  size_t keylen, const unsigned char *iv,
                                  size_t ivlen, const OSSL_PARAM params[])
{
    int ret;

    /* The generic function checks for ossl_prov_is_running() */
    ret = ossl_cipher_generic_dinit(vctx, key, keylen, iv, ivlen, NULL);
    if (ret && iv != NULL) {
        PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
        PROV_CIPHER_HW_CHACHA20_POLY1305 *hw =
            (PROV_CIPHER_HW_CHACHA20_POLY1305 *)ctx->hw;

        hw->initiv(ctx);
    }
    if (ret && !chacha20_poly1305_set_ctx_params(vctx, params))
        ret = 0;
    return ret;
}

static int chacha20_poly1305_cipher(void *vctx, unsigned char *out,
                                    size_t *outl, size_t outsize,
                                    const unsigned char *in, size_t inl)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    PROV_CIPHER_HW_CHACHA20_POLY1305 *hw =
        (PROV_CIPHER_HW_CHACHA20_POLY1305 *)ctx->hw;

    if (!ossl_prov_is_running())
        return 0;

    if (inl == 0) {
        *outl = 0;
        return 1;
    }

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (!hw->aead_cipher(ctx, out, outl, in, inl))
        return 0;

    return 1;
}

static int chacha20_poly1305_final(void *vctx, unsigned char *out, size_t *outl,
                                   size_t outsize)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    PROV_CIPHER_HW_CHACHA20_POLY1305 *hw =
        (PROV_CIPHER_HW_CHACHA20_POLY1305 *)ctx->hw;

    if (!ossl_prov_is_running())
        return 0;

    if (hw->aead_cipher(ctx, out, outl, NULL, 0) <= 0)
        return 0;

    *outl = 0;
    return 1;
}

/* ossl_chacha20_ossl_poly1305_functions */
const OSSL_DISPATCH ossl_chacha20_ossl_poly1305_functions[] = {
    { OSSL_FUNC_CIPHER_NEWCTX, (void (*)(void))chacha20_poly1305_newctx },
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))chacha20_poly1305_freectx },
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))chacha20_poly1305_dupctx },
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))chacha20_poly1305_einit },
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))chacha20_poly1305_dinit },
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))chacha20_poly1305_update },
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))chacha20_poly1305_final },
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))chacha20_poly1305_cipher },
    { OSSL_FUNC_CIPHER_GET_PARAMS,
        (void (*)(void))chacha20_poly1305_get_params },
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,
        (void (*)(void))chacha20_poly1305_gettable_params },
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,
         (void (*)(void))chacha20_poly1305_get_ctx_params },
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,
        (void (*)(void))chacha20_poly1305_gettable_ctx_params },
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,
        (void (*)(void))chacha20_poly1305_set_ctx_params },
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,
        (void (*)(void))chacha20_poly1305_settable_ctx_params },
    OSSL_DISPATCH_END
};

