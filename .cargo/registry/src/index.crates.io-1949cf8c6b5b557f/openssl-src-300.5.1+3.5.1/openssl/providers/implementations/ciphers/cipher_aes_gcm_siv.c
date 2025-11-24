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
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/ciphercommon_aead.h"
#include "prov/provider_ctx.h"
#include "cipher_aes_gcm_siv.h"

static int ossl_aes_gcm_siv_set_ctx_params(void *vctx, const OSSL_PARAM params[]);

static void *ossl_aes_gcm_siv_newctx(void *provctx, size_t keybits)
{
    PROV_AES_GCM_SIV_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL) {
        ctx->key_len = keybits / 8;
        ctx->hw = ossl_prov_cipher_hw_aes_gcm_siv(keybits);
        ctx->libctx = PROV_LIBCTX_OF(provctx);
        ctx->provctx = provctx;
    }
    return ctx;
}

static void ossl_aes_gcm_siv_freectx(void *vctx)
{
    PROV_AES_GCM_SIV_CTX *ctx = (PROV_AES_GCM_SIV_CTX *)vctx;

    if (ctx == NULL)
        return;

    OPENSSL_clear_free(ctx->aad, ctx->aad_len);
    ctx->hw->clean_ctx(ctx);
    OPENSSL_clear_free(ctx, sizeof(*ctx));
}

static void *ossl_aes_gcm_siv_dupctx(void *vctx)
{
    PROV_AES_GCM_SIV_CTX *in = (PROV_AES_GCM_SIV_CTX *)vctx;
    PROV_AES_GCM_SIV_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    if (in->hw == NULL)
        return NULL;

    ret = OPENSSL_memdup(in, sizeof(*in));
    if (ret == NULL)
        return NULL;
    /* NULL-out these things we create later */
    ret->aad = NULL;
    ret->ecb_ctx = NULL;

    if (in->aad != NULL) {
        if ((ret->aad = OPENSSL_memdup(in->aad, UP16(ret->aad_len))) == NULL)
            goto err;
    }

    if (!in->hw->dup_ctx(ret, in))
        goto err;

    return ret;
 err:
    if (ret != NULL) {
        OPENSSL_clear_free(ret->aad, ret->aad_len);
        OPENSSL_free(ret);
    }
    return NULL;
}

static int ossl_aes_gcm_siv_init(void *vctx, const unsigned char *key, size_t keylen,
                                 const unsigned char *iv, size_t ivlen,
                                 const OSSL_PARAM params[], int enc)
{
    PROV_AES_GCM_SIV_CTX *ctx = (PROV_AES_GCM_SIV_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    ctx->enc = enc;

    if (key != NULL) {
        if (keylen != ctx->key_len) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
        memcpy(ctx->key_gen_key, key, ctx->key_len);
    }
    if (iv != NULL) {
        if (ivlen != sizeof(ctx->nonce)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
        memcpy(ctx->nonce, iv, sizeof(ctx->nonce));
    }

    if (!ctx->hw->initkey(ctx))
        return 0;

    return ossl_aes_gcm_siv_set_ctx_params(ctx, params);
}

static int ossl_aes_gcm_siv_einit(void *vctx, const unsigned char *key, size_t keylen,
                                  const unsigned char *iv, size_t ivlen,
                                  const OSSL_PARAM params[])
{
    return ossl_aes_gcm_siv_init(vctx, key, keylen, iv, ivlen, params, 1);
}

static int ossl_aes_gcm_siv_dinit(void *vctx, const unsigned char *key, size_t keylen,
                                  const unsigned char *iv, size_t ivlen,
                                  const OSSL_PARAM params[])
{
    return ossl_aes_gcm_siv_init(vctx, key, keylen, iv, ivlen, params, 0);
}

#define ossl_aes_gcm_siv_stream_update ossl_aes_gcm_siv_cipher
static int ossl_aes_gcm_siv_cipher(void *vctx, unsigned char *out, size_t *outl,
                                   size_t outsize, const unsigned char *in, size_t inl)
{
    PROV_AES_GCM_SIV_CTX *ctx = (PROV_AES_GCM_SIV_CTX *)vctx;
    int error = 0;

    if (!ossl_prov_is_running())
        return 0;

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    error |= !ctx->hw->cipher(ctx, out, in, inl);

    if (outl != NULL && !error)
        *outl = inl;
    return !error;
}

static int ossl_aes_gcm_siv_stream_final(void *vctx, unsigned char *out, size_t *outl,
                                         size_t outsize)
{
    PROV_AES_GCM_SIV_CTX *ctx = (PROV_AES_GCM_SIV_CTX *)vctx;
    int error = 0;

    if (!ossl_prov_is_running())
        return 0;

    error |= !ctx->hw->cipher(vctx, out, NULL, 0);

    if (outl != NULL && !error)
        *outl = 0;
    return !error;
}

static int ossl_aes_gcm_siv_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_AES_GCM_SIV_CTX *ctx = (PROV_AES_GCM_SIV_CTX *)vctx;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAG);
    if (p != NULL && p->data_type == OSSL_PARAM_OCTET_STRING) {
        if (!ctx->enc || !ctx->generated_tag
                || p->data_size != sizeof(ctx->tag)
                || !OSSL_PARAM_set_octet_string(p, ctx->tag, sizeof(ctx->tag))) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAGLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, sizeof(ctx->tag))) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->key_len)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    return 1;
}

static const OSSL_PARAM aes_gcm_siv_known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TAGLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *ossl_aes_gcm_siv_gettable_ctx_params(ossl_unused void *cctx,
                                                              ossl_unused void *provctx)
{
    return aes_gcm_siv_known_gettable_ctx_params;
}

static int ossl_aes_gcm_siv_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_AES_GCM_SIV_CTX *ctx = (PROV_AES_GCM_SIV_CTX *)vctx;
    const OSSL_PARAM *p;
    unsigned int speed = 0;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TAG);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING
                || p->data_size != sizeof(ctx->user_tag)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (!ctx->enc) {
            memcpy(ctx->user_tag, p->data, sizeof(ctx->tag));
            ctx->have_user_tag = 1;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_SPEED);
    if (p != NULL) {
        if (!OSSL_PARAM_get_uint(p, &speed)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        ctx->speed = !!speed;
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        size_t key_len;

        if (!OSSL_PARAM_get_size_t(p, &key_len)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        /* The key length can not be modified */
        if (key_len != ctx->key_len) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}

static const OSSL_PARAM aes_gcm_siv_known_settable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_SPEED, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *ossl_aes_gcm_siv_settable_ctx_params(ossl_unused void *cctx,
                                                              ossl_unused void *provctx)
{
    return aes_gcm_siv_known_settable_ctx_params;
}

#define IMPLEMENT_cipher(alg, lc, UCMODE, flags, kbits, blkbits, ivbits)                                \
static OSSL_FUNC_cipher_newctx_fn              ossl_##alg##kbits##_##lc##_newctx;                       \
static OSSL_FUNC_cipher_freectx_fn             ossl_##alg##_##lc##_freectx;                             \
static OSSL_FUNC_cipher_dupctx_fn              ossl_##alg##_##lc##_dupctx;                              \
static OSSL_FUNC_cipher_encrypt_init_fn        ossl_##alg##_##lc##_einit;                               \
static OSSL_FUNC_cipher_decrypt_init_fn        ossl_##alg##_##lc##_dinit;                               \
static OSSL_FUNC_cipher_update_fn              ossl_##alg##_##lc##_stream_update;                       \
static OSSL_FUNC_cipher_final_fn               ossl_##alg##_##lc##_stream_final;                        \
static OSSL_FUNC_cipher_cipher_fn              ossl_##alg##_##lc##_cipher;                              \
static OSSL_FUNC_cipher_get_params_fn          ossl_##alg##_##kbits##_##lc##_get_params;                \
static OSSL_FUNC_cipher_get_ctx_params_fn      ossl_##alg##_##lc##_get_ctx_params;                      \
static OSSL_FUNC_cipher_gettable_ctx_params_fn ossl_##alg##_##lc##_gettable_ctx_params;                 \
static OSSL_FUNC_cipher_set_ctx_params_fn      ossl_##alg##_##lc##_set_ctx_params;                      \
static OSSL_FUNC_cipher_settable_ctx_params_fn ossl_##alg##_##lc##_settable_ctx_params;                 \
static int ossl_##alg##_##kbits##_##lc##_get_params(OSSL_PARAM params[])                                \
{                                                                                                       \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,                             \
                                          flags, kbits, blkbits, ivbits);                               \
}                                                                                                       \
static void *ossl_##alg##kbits##_##lc##_newctx(void *provctx)                                          \
{                                                                                                       \
    return ossl_##alg##_##lc##_newctx(provctx, kbits);                                                  \
}                                                                                                       \
const OSSL_DISPATCH ossl_##alg##kbits##lc##_functions[] = {                                             \
    { OSSL_FUNC_CIPHER_NEWCTX,              (void (*)(void))ossl_##alg##kbits##_##lc##_newctx },        \
    { OSSL_FUNC_CIPHER_FREECTX,             (void (*)(void))ossl_##alg##_##lc##_freectx },              \
    { OSSL_FUNC_CIPHER_DUPCTX,              (void (*)(void))ossl_##alg##_##lc##_dupctx },               \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT,        (void (*)(void))ossl_##alg##_##lc##_einit },                \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT,        (void (*)(void))ossl_##alg##_##lc##_dinit },                \
    { OSSL_FUNC_CIPHER_UPDATE,              (void (*)(void))ossl_##alg##_##lc##_stream_update },        \
    { OSSL_FUNC_CIPHER_FINAL,               (void (*)(void))ossl_##alg##_##lc##_stream_final },         \
    { OSSL_FUNC_CIPHER_CIPHER,              (void (*)(void))ossl_##alg##_##lc##_cipher },               \
    { OSSL_FUNC_CIPHER_GET_PARAMS,          (void (*)(void))ossl_##alg##_##kbits##_##lc##_get_params }, \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,     (void (*)(void))ossl_cipher_generic_gettable_params },      \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,      (void (*)(void))ossl_##alg##_##lc##_get_ctx_params },       \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS, (void (*)(void))ossl_##alg##_##lc##_gettable_ctx_params },  \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,      (void (*)(void))ossl_##alg##_##lc##_set_ctx_params },       \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS, (void (*)(void))ossl_##alg##_##lc##_settable_ctx_params },  \
    OSSL_DISPATCH_END                                                                                   \
}

IMPLEMENT_cipher(aes, gcm_siv, GCM_SIV, AEAD_FLAGS, 128, 8, 96);
IMPLEMENT_cipher(aes, gcm_siv, GCM_SIV, AEAD_FLAGS, 192, 8, 96);
IMPLEMENT_cipher(aes, gcm_siv, GCM_SIV, AEAD_FLAGS, 256, 8, 96);
