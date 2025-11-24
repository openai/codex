/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for RC4_HMAC_MD5 cipher */

/*
 * MD5 and RC4 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/proverr.h>
#include "cipher_rc4_hmac_md5.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define RC4_HMAC_MD5_FLAGS (PROV_CIPHER_FLAG_VARIABLE_LENGTH                   \
                            | PROV_CIPHER_FLAG_AEAD)

#define RC4_HMAC_MD5_KEY_BITS (16 * 8)
#define RC4_HMAC_MD5_BLOCK_BITS (1 * 8)
#define RC4_HMAC_MD5_IV_BITS 0
#define RC4_HMAC_MD5_MODE 0

#define GET_HW(ctx) ((PROV_CIPHER_HW_RC4_HMAC_MD5 *)ctx->base.hw)

static OSSL_FUNC_cipher_encrypt_init_fn rc4_hmac_md5_einit;
static OSSL_FUNC_cipher_decrypt_init_fn rc4_hmac_md5_dinit;
static OSSL_FUNC_cipher_newctx_fn rc4_hmac_md5_newctx;
static OSSL_FUNC_cipher_freectx_fn rc4_hmac_md5_freectx;
static OSSL_FUNC_cipher_dupctx_fn rc4_hmac_md5_dupctx;
static OSSL_FUNC_cipher_get_ctx_params_fn rc4_hmac_md5_get_ctx_params;
static OSSL_FUNC_cipher_gettable_ctx_params_fn rc4_hmac_md5_gettable_ctx_params;
static OSSL_FUNC_cipher_set_ctx_params_fn rc4_hmac_md5_set_ctx_params;
static OSSL_FUNC_cipher_settable_ctx_params_fn rc4_hmac_md5_settable_ctx_params;
static OSSL_FUNC_cipher_get_params_fn rc4_hmac_md5_get_params;
#define rc4_hmac_md5_gettable_params ossl_cipher_generic_gettable_params
#define rc4_hmac_md5_update ossl_cipher_generic_stream_update
#define rc4_hmac_md5_final ossl_cipher_generic_stream_final
#define rc4_hmac_md5_cipher ossl_cipher_generic_cipher

static void *rc4_hmac_md5_newctx(void *provctx)
{
    PROV_RC4_HMAC_MD5_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL)
        ossl_cipher_generic_initkey(ctx, RC4_HMAC_MD5_KEY_BITS,
                                    RC4_HMAC_MD5_BLOCK_BITS,
                                    RC4_HMAC_MD5_IV_BITS,
                                    RC4_HMAC_MD5_MODE, RC4_HMAC_MD5_FLAGS,
                                    ossl_prov_cipher_hw_rc4_hmac_md5(
                                        RC4_HMAC_MD5_KEY_BITS
                                    ), NULL);
     return ctx;
}

static void rc4_hmac_md5_freectx(void *vctx)
{
    PROV_RC4_HMAC_MD5_CTX *ctx = (PROV_RC4_HMAC_MD5_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *rc4_hmac_md5_dupctx(void *vctx)
{
    PROV_RC4_HMAC_MD5_CTX *ctx = vctx;

    if (ctx == NULL)
        return NULL;
    return OPENSSL_memdup(ctx, sizeof(*ctx));
}

static int rc4_hmac_md5_einit(void *ctx, const unsigned char *key,
                              size_t keylen, const unsigned char *iv,
                              size_t ivlen, const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_einit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return rc4_hmac_md5_set_ctx_params(ctx, params);
}

static int rc4_hmac_md5_dinit(void *ctx, const unsigned char *key,
                              size_t keylen, const unsigned char *iv,
                              size_t ivlen, const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_dinit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return rc4_hmac_md5_set_ctx_params(ctx, params);
}

static const OSSL_PARAM rc4_hmac_md5_known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD_PAD, NULL),
    OSSL_PARAM_END
};
const OSSL_PARAM *rc4_hmac_md5_gettable_ctx_params(ossl_unused void *cctx,
                                                   ossl_unused void *provctx)
{
    return rc4_hmac_md5_known_gettable_ctx_params;
}

static int rc4_hmac_md5_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_RC4_HMAC_MD5_CTX *ctx = (PROV_RC4_HMAC_MD5_CTX *)vctx;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->base.keylen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->base.ivlen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TLS1_AAD_PAD);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->tls_aad_pad_sz)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    return 1;
}

static const OSSL_PARAM rc4_hmac_md5_known_settable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD, NULL, 0),
    OSSL_PARAM_END
};
const OSSL_PARAM *rc4_hmac_md5_settable_ctx_params(ossl_unused void *cctx,
                                                   ossl_unused void *provctx)
{
    return rc4_hmac_md5_known_settable_ctx_params;
}

static int rc4_hmac_md5_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_RC4_HMAC_MD5_CTX *ctx = (PROV_RC4_HMAC_MD5_CTX *)vctx;
    const OSSL_PARAM *p;
    size_t sz;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &sz)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (ctx->base.keylen != sz) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &sz)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (ctx->base.ivlen != sz) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TLS1_AAD);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        sz = GET_HW(ctx)->tls_init(&ctx->base, p->data, p->data_size);
        if (sz == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DATA);
            return 0;
        }
        ctx->tls_aad_pad_sz = sz;
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_MAC_KEY);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        GET_HW(ctx)->init_mackey(&ctx->base, p->data, p->data_size);
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_TLS_VERSION);
    if (p != NULL) {
        if (!OSSL_PARAM_get_uint(p, &ctx->base.tlsversion)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
    }

    return 1;
}

static int rc4_hmac_md5_get_params(OSSL_PARAM params[])
{
    return ossl_cipher_generic_get_params(params, RC4_HMAC_MD5_MODE,
                                          RC4_HMAC_MD5_FLAGS,
                                          RC4_HMAC_MD5_KEY_BITS,
                                          RC4_HMAC_MD5_BLOCK_BITS,
                                          RC4_HMAC_MD5_IV_BITS);
}

const OSSL_DISPATCH ossl_rc4_hmac_ossl_md5_functions[] = {
    { OSSL_FUNC_CIPHER_NEWCTX, (void (*)(void))rc4_hmac_md5_newctx },
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))rc4_hmac_md5_freectx },
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))rc4_hmac_md5_dupctx },
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))rc4_hmac_md5_einit },
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))rc4_hmac_md5_dinit },
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))rc4_hmac_md5_update },
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))rc4_hmac_md5_final },
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))rc4_hmac_md5_cipher },
    { OSSL_FUNC_CIPHER_GET_PARAMS, (void (*)(void))rc4_hmac_md5_get_params },
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,
        (void (*)(void))rc4_hmac_md5_gettable_params },
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,
        (void (*)(void))rc4_hmac_md5_get_ctx_params },
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,
        (void (*)(void))rc4_hmac_md5_gettable_ctx_params },
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,
        (void (*)(void))rc4_hmac_md5_set_ctx_params },
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,
        (void (*)(void))rc4_hmac_md5_settable_ctx_params },
    OSSL_DISPATCH_END
};
