/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for AES SIV mode */

/*
 * This file uses the low level AES functions (which are deprecated for
 * non-internal use) in order to implement provider AES ciphers.
 */
#include "internal/deprecated.h"

#include <openssl/proverr.h>
#include "cipher_aes_siv.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/ciphercommon_aead.h"
#include "prov/provider_ctx.h"

#define siv_stream_update siv_cipher
#define SIV_FLAGS AEAD_FLAGS

static OSSL_FUNC_cipher_set_ctx_params_fn aes_siv_set_ctx_params;

static void *aes_siv_newctx(void *provctx, size_t keybits, unsigned int mode,
                            uint64_t flags)
{
    PROV_AES_SIV_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL) {
        ctx->taglen = SIV_LEN;
        ctx->mode = mode;
        ctx->keylen = keybits / 8;
        ctx->hw = ossl_prov_cipher_hw_aes_siv(keybits);
        ctx->libctx = PROV_LIBCTX_OF(provctx);
    }
    return ctx;
}

static void aes_siv_freectx(void *vctx)
{
    PROV_AES_SIV_CTX *ctx = (PROV_AES_SIV_CTX *)vctx;

    if (ctx != NULL) {
        ctx->hw->cleanup(ctx);
        OPENSSL_clear_free(ctx,  sizeof(*ctx));
    }
}

static void *siv_dupctx(void *vctx)
{
    PROV_AES_SIV_CTX *in = (PROV_AES_SIV_CTX *)vctx;
    PROV_AES_SIV_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    if (!in->hw->dupctx(in, ret)) {
        OPENSSL_free(ret);
        ret = NULL;
    }
    return ret;
}

static int siv_init(void *vctx, const unsigned char *key, size_t keylen,
                    const unsigned char *iv, size_t ivlen,
                    const OSSL_PARAM params[], int enc)
{
    PROV_AES_SIV_CTX *ctx = (PROV_AES_SIV_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    ctx->enc = enc;

    if (key != NULL) {
        if (keylen != ctx->keylen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
        if (!ctx->hw->initkey(ctx, key, ctx->keylen))
            return 0;
    }
    return aes_siv_set_ctx_params(ctx, params);
}

static int siv_einit(void *vctx, const unsigned char *key, size_t keylen,
                     const unsigned char *iv, size_t ivlen,
                     const OSSL_PARAM params[])
{
    return siv_init(vctx, key, keylen, iv, ivlen, params, 1);
}

static int siv_dinit(void *vctx, const unsigned char *key, size_t keylen,
                     const unsigned char *iv, size_t ivlen,
                     const OSSL_PARAM params[])
{
    return siv_init(vctx, key, keylen, iv, ivlen, params, 0);
}

static int siv_cipher(void *vctx, unsigned char *out, size_t *outl,
                      size_t outsize, const unsigned char *in, size_t inl)
{
    PROV_AES_SIV_CTX *ctx = (PROV_AES_SIV_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    if (out != NULL && outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (ctx->hw->cipher(ctx, out, in, inl) <= 0)
        return 0;

    if (outl != NULL)
        *outl = inl;
    return 1;
}

static int siv_stream_final(void *vctx, unsigned char *out, size_t *outl,
                            size_t outsize)
{
    PROV_AES_SIV_CTX *ctx = (PROV_AES_SIV_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    if (!ctx->hw->cipher(vctx, out, NULL, 0))
        return 0;

    if (outl != NULL)
        *outl = 0;
    return 1;
}

static int aes_siv_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_AES_SIV_CTX *ctx = (PROV_AES_SIV_CTX *)vctx;
    SIV128_CONTEXT *sctx = &ctx->siv;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAG);
    if (p != NULL && p->data_type == OSSL_PARAM_OCTET_STRING) {
        if (!ctx->enc
            || p->data_size != ctx->taglen
            || !OSSL_PARAM_set_octet_string(p, &sctx->tag.byte, ctx->taglen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAGLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->taglen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->keylen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    return 1;
}

static const OSSL_PARAM aes_siv_known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TAGLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *aes_siv_gettable_ctx_params(ossl_unused void *cctx,
                                                     ossl_unused void *provctx)
{
    return aes_siv_known_gettable_ctx_params;
}

static int aes_siv_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_AES_SIV_CTX *ctx = (PROV_AES_SIV_CTX *)vctx;
    const OSSL_PARAM *p;
    unsigned int speed = 0;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TAG);
    if (p != NULL) {
        if (ctx->enc)
            return 1;
        if (p->data_type != OSSL_PARAM_OCTET_STRING
            || !ctx->hw->settag(ctx, p->data, p->data_size)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_SPEED);
    if (p != NULL) {
        if (!OSSL_PARAM_get_uint(p, &speed)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        ctx->hw->setspeed(ctx, (int)speed);
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        size_t keylen;

        if (!OSSL_PARAM_get_size_t(p, &keylen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        /* The key length can not be modified */
        if (keylen != ctx->keylen)
            return 0;
    }
    return 1;
}

static const OSSL_PARAM aes_siv_known_settable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_SPEED, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *aes_siv_settable_ctx_params(ossl_unused void *cctx,
                                                     ossl_unused void *provctx)
{
    return aes_siv_known_settable_ctx_params;
}

#define IMPLEMENT_cipher(alg, lc, UCMODE, flags, kbits, blkbits, ivbits)       \
static OSSL_FUNC_cipher_newctx_fn alg##kbits##lc##_newctx;                     \
static OSSL_FUNC_cipher_freectx_fn alg##_##lc##_freectx;                       \
static OSSL_FUNC_cipher_dupctx_fn lc##_dupctx;                                 \
static OSSL_FUNC_cipher_encrypt_init_fn lc##_einit;                            \
static OSSL_FUNC_cipher_decrypt_init_fn lc##_dinit;                            \
static OSSL_FUNC_cipher_update_fn lc##_stream_update;                          \
static OSSL_FUNC_cipher_final_fn lc##_stream_final;                            \
static OSSL_FUNC_cipher_cipher_fn lc##_cipher;                                 \
static OSSL_FUNC_cipher_get_params_fn alg##_##kbits##_##lc##_get_params;       \
static OSSL_FUNC_cipher_get_ctx_params_fn alg##_##lc##_get_ctx_params;         \
static OSSL_FUNC_cipher_gettable_ctx_params_fn                                 \
            alg##_##lc##_gettable_ctx_params;                                  \
static OSSL_FUNC_cipher_set_ctx_params_fn alg##_##lc##_set_ctx_params;         \
static OSSL_FUNC_cipher_settable_ctx_params_fn                                 \
            alg##_##lc##_settable_ctx_params;                                  \
static int alg##_##kbits##_##lc##_get_params(OSSL_PARAM params[])              \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                          flags, 2*kbits, blkbits, ivbits);    \
}                                                                              \
static void *alg##kbits##lc##_newctx(void *provctx)                            \
{                                                                              \
    return alg##_##lc##_newctx(provctx, 2*kbits, EVP_CIPH_##UCMODE##_MODE,     \
                               flags);                                         \
}                                                                              \
const OSSL_DISPATCH ossl_##alg##kbits##lc##_functions[] = {                    \
    { OSSL_FUNC_CIPHER_NEWCTX, (void (*)(void))alg##kbits##lc##_newctx },      \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))alg##_##lc##_freectx },        \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void)) lc##_dupctx },                 \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void)) lc##_einit },            \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void)) lc##_dinit },            \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void)) lc##_stream_update },          \
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void)) lc##_stream_final },            \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void)) lc##_cipher },                 \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void)) alg##_##kbits##_##lc##_get_params },                    \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void)) alg##_##lc##_get_ctx_params },                          \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void)) alg##_##lc##_gettable_ctx_params },                     \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void)) alg##_##lc##_set_ctx_params },                          \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
      (void (*)(void)) alg##_##lc##_settable_ctx_params },                     \
    OSSL_DISPATCH_END                                                          \
};

IMPLEMENT_cipher(aes, siv, SIV, SIV_FLAGS, 128, 8, 0)
IMPLEMENT_cipher(aes, siv, SIV, SIV_FLAGS, 192, 8, 0)
IMPLEMENT_cipher(aes, siv, SIV, SIV_FLAGS, 256, 8, 0)
