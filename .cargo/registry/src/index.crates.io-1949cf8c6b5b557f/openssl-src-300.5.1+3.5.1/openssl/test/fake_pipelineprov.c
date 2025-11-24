/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This file uses the low level AES functions (which are deprecated for
 * non-internal use) in order to implement provider AES ciphers.
 */
#include "internal/deprecated.h"

#include <openssl/core.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/proverr.h>
#include "prov/providercommon.h"
#include "prov/ciphercommon.h"
#include "prov/ciphercommon_aead.h"
#include "testutil.h"
#include "fake_pipelineprov.h"

/*
 * This file provides a fake provider that implements a pipeline cipher
 * for AES GCM.
 */

typedef struct fake_pipeline_ctx_st {
    size_t keylen;
    size_t ivlen;
    size_t numpipes;
    EVP_CIPHER *cipher;
    EVP_CIPHER_CTX *cipher_ctxs[EVP_MAX_PIPES];
} CIPHER_PIPELINE_CTX;

static void *fake_pipeline_newctx(void *provctx, char *ciphername,
                                  size_t kbits, size_t ivbits)
{
    CIPHER_PIPELINE_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;

    ctx->keylen = kbits / 8;
    ctx->ivlen = ivbits / 8;
    ctx->numpipes = 0;
    ctx->cipher = EVP_CIPHER_fetch(provctx, ciphername, "provider=default");

    return ctx;
}

static OSSL_FUNC_cipher_freectx_fn fake_pipeline_freectx;
static void fake_pipeline_freectx(void *vctx)
{
    CIPHER_PIPELINE_CTX *ctx = (CIPHER_PIPELINE_CTX *)vctx;
    size_t i;

    EVP_CIPHER_free(ctx->cipher);
    for (i = 0; i < ctx->numpipes; i++)
        EVP_CIPHER_CTX_free(ctx->cipher_ctxs[i]);
    OPENSSL_clear_free(ctx, sizeof(*ctx));
}

OSSL_FUNC_cipher_pipeline_encrypt_init_fn fake_pipeline_einit;
OSSL_FUNC_cipher_pipeline_decrypt_init_fn fake_pipeline_dinit;
OSSL_FUNC_cipher_pipeline_update_fn fake_pipeline_update;
OSSL_FUNC_cipher_pipeline_final_fn fake_pipeline_final;
OSSL_FUNC_cipher_gettable_ctx_params_fn fake_pipeline_aead_gettable_ctx_params;
OSSL_FUNC_cipher_get_ctx_params_fn fake_pipeline_aead_get_ctx_params;
OSSL_FUNC_cipher_settable_ctx_params_fn fake_pipeline_aead_settable_ctx_params;
OSSL_FUNC_cipher_set_ctx_params_fn fake_pipeline_aead_set_ctx_params;

static int fake_pipeline_init(void *vctx,
                              const unsigned char *key, size_t keylen,
                              size_t numpipes, const unsigned char **iv,
                              size_t ivlen, int enc)
{
    CIPHER_PIPELINE_CTX *ctx = (CIPHER_PIPELINE_CTX *)vctx;
    size_t i = 0;

    ctx->numpipes = numpipes;
    for (i = 0; i < numpipes; i++) {
        ctx->cipher_ctxs[i] = EVP_CIPHER_CTX_new();
        if (ctx->cipher_ctxs[i] == NULL)
            return 0;
        if (!EVP_CipherInit(ctx->cipher_ctxs[i], ctx->cipher, key, iv[i], enc))
            return 0;
    }

    return 1;
}

int fake_pipeline_einit(void *vctx,
                        const unsigned char *key, size_t keylen,
                        size_t numpipes, const unsigned char **iv,
                        size_t ivlen, const OSSL_PARAM params[])
{
    return fake_pipeline_init(vctx, key, keylen, numpipes, iv, ivlen, 1);
}

int fake_pipeline_dinit(void *vctx,
                        const unsigned char *key, size_t keylen,
                        size_t numpipes, const unsigned char **iv,
                        size_t ivlen, const OSSL_PARAM params[])
{
    return fake_pipeline_init(vctx, key, keylen, numpipes, iv, ivlen, 0);
}

int fake_pipeline_update(void *vctx, size_t numpipes,
                         unsigned char **out, size_t *outl,
                         const size_t *outsize,
                         const unsigned char **in, const size_t *inl)
{
    CIPHER_PIPELINE_CTX *ctx = (CIPHER_PIPELINE_CTX *)vctx;
    int ioutl, inl_;
    size_t i = 0;

    for (i = 0; i < numpipes; i++) {
        inl_ = (int)inl[i];
        if (!EVP_CipherUpdate(ctx->cipher_ctxs[i],
                              (out != NULL) ? out[i] : NULL,
                              &ioutl,
                              in[i], inl_))
            return 0;
        outl[i] = (size_t)ioutl;
    }
    return 1;
}

int fake_pipeline_final(void *vctx, size_t numpipes,
                        unsigned char **out, size_t *outl,
                        const size_t *outsize)
{
    CIPHER_PIPELINE_CTX *ctx = (CIPHER_PIPELINE_CTX *)vctx;
    int ioutl;
    size_t i = 0;

    for (i = 0; i < numpipes; i++) {
        if (!EVP_CipherFinal(ctx->cipher_ctxs[i], out[i], &ioutl))
            return 0;
        outl[i] = (size_t)ioutl;
    }
    return 1;
}

static const OSSL_PARAM fake_pipeline_aead_known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TAGLEN, NULL),
    OSSL_PARAM_octet_ptr(OSSL_CIPHER_PARAM_PIPELINE_AEAD_TAG, NULL, 0),
    OSSL_PARAM_END
};
const OSSL_PARAM *fake_pipeline_aead_gettable_ctx_params(ossl_unused void *cctx,
                                                         ossl_unused void *provctx)
{
    return fake_pipeline_aead_known_gettable_ctx_params;
}

static const OSSL_PARAM fake_pipeline_aead_known_settable_ctx_params[] = {
    OSSL_PARAM_octet_ptr(OSSL_CIPHER_PARAM_PIPELINE_AEAD_TAG, NULL, 0),
    OSSL_PARAM_END
};
const OSSL_PARAM *fake_pipeline_aead_settable_ctx_params(ossl_unused void *cctx,
                                                         ossl_unused void *provctx)
{
    return fake_pipeline_aead_known_settable_ctx_params;
}

int fake_pipeline_aead_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    CIPHER_PIPELINE_CTX *ctx = (CIPHER_PIPELINE_CTX *)vctx;
    OSSL_PARAM *p;
    size_t taglen, i;
    unsigned char **aead_tags = NULL;
    OSSL_PARAM aead_params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_set_size_t(p, ctx->ivlen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_set_size_t(p, ctx->keylen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_PIPELINE_AEAD_TAG);
    if (p != NULL) {
        if (!OSSL_PARAM_get_octet_ptr(p, (const void **)&aead_tags, &taglen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
        for (i = 0; i < ctx->numpipes; i++) {
            aead_params[0] = OSSL_PARAM_construct_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG,
                                                               (void *)aead_tags[i],
                                                               taglen);
            if (!EVP_CIPHER_CTX_get_params(ctx->cipher_ctxs[i], aead_params)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
                return 0;
            }
        }
    }

    return 1;
}

int fake_pipeline_aead_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    CIPHER_PIPELINE_CTX *ctx = (CIPHER_PIPELINE_CTX *)vctx;
    const OSSL_PARAM *p;
    size_t taglen, i;
    unsigned char **aead_tags = NULL;
    OSSL_PARAM aead_params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_PIPELINE_AEAD_TAG);
    if (p != NULL) {
        if (!OSSL_PARAM_get_octet_ptr(p, (const void **)&aead_tags, &taglen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
        for (i = 0; i < ctx->numpipes; i++) {
            aead_params[0] = OSSL_PARAM_construct_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG,
                                                               (void *)aead_tags[i],
                                                               taglen);
            if (!EVP_CIPHER_CTX_set_params(ctx->cipher_ctxs[i], aead_params)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
                return 0;
            }
        }
    }

    /* No other settable ctx param */
    return 1;
}

#define IMPLEMENT_aead_cipher_pipeline(alg, lc, UCMODE, flags, kbits, blkbits,          \
                                       ivbits, ciphername)                              \
    static OSSL_FUNC_cipher_get_params_fn alg##_##kbits##_##lc##_get_params;            \
    static int alg##_##kbits##_##lc##_get_params(OSSL_PARAM params[])                   \
    {                                                                                   \
        return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,         \
                                              flags, kbits, blkbits, ivbits);           \
    }                                                                                   \
    static OSSL_FUNC_cipher_newctx_fn fake_pipeline_##alg##_##kbits##_##lc##_newctx;    \
    static void * fake_pipeline_##alg##_##kbits##_##lc##_newctx(void *provctx)          \
    {                                                                                   \
        return fake_pipeline_newctx(provctx, ciphername, kbits, ivbits);                \
    }                                                                                   \
    static const OSSL_DISPATCH fake_pipeline_##alg##kbits##lc##_functions[] = {         \
        { OSSL_FUNC_CIPHER_NEWCTX,                                                      \
          (void (*)(void))fake_pipeline_##alg##_##kbits##_##lc##_newctx },              \
        { OSSL_FUNC_CIPHER_FREECTX,                                                     \
          (void (*)(void))fake_pipeline_freectx },                                      \
        { OSSL_FUNC_CIPHER_PIPELINE_ENCRYPT_INIT,                                       \
          (void (*)(void))fake_pipeline_einit },                                        \
        { OSSL_FUNC_CIPHER_PIPELINE_DECRYPT_INIT,                                       \
          (void (*)(void))fake_pipeline_dinit },                                        \
        { OSSL_FUNC_CIPHER_PIPELINE_UPDATE,                                             \
          (void (*)(void))fake_pipeline_update },                                       \
        { OSSL_FUNC_CIPHER_PIPELINE_FINAL,                                              \
          (void (*)(void))fake_pipeline_final },                                        \
        { OSSL_FUNC_CIPHER_GET_PARAMS,                                                  \
          (void (*)(void)) alg##_##kbits##_##lc##_get_params },                         \
        { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                              \
          (void (*)(void)) fake_pipeline_aead_get_ctx_params },                         \
        { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                              \
          (void (*)(void)) fake_pipeline_aead_set_ctx_params },                         \
        { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                             \
          (void (*)(void)) ossl_cipher_generic_gettable_params },                       \
        { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                         \
          (void (*)(void)) fake_pipeline_aead_gettable_ctx_params },                    \
        { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                         \
          (void (*)(void)) fake_pipeline_aead_settable_ctx_params },                    \
        OSSL_DISPATCH_END                                                               \
    }

IMPLEMENT_aead_cipher_pipeline(aes, gcm, GCM, AEAD_FLAGS, 256, 8, 96, "AES-256-GCM");

static const OSSL_ALGORITHM fake_ciphers[] = {
    {"AES-256-GCM", "provider=fake-pipeline", fake_pipeline_aes256gcm_functions},
    {NULL, NULL, NULL}
};

static const OSSL_ALGORITHM *fake_pipeline_query(OSSL_PROVIDER *prov,
                                                 int operation_id,
                                                 int *no_cache)
{
    *no_cache = 0;
    switch (operation_id) {
    case OSSL_OP_CIPHER:
        return fake_ciphers;
    }
    return NULL;
}

/* Functions we provide to the core */
static const OSSL_DISPATCH fake_pipeline_method[] = {
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))OSSL_LIB_CTX_free },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))fake_pipeline_query },
    OSSL_DISPATCH_END
};

static int fake_pipeline_provider_init(const OSSL_CORE_HANDLE *handle,
                                       const OSSL_DISPATCH *in,
                                       const OSSL_DISPATCH **out, void **provctx)
{
    if (!TEST_ptr(*provctx = OSSL_LIB_CTX_new()))
        return 0;
    *out = fake_pipeline_method;
    return 1;
}

OSSL_PROVIDER *fake_pipeline_start(OSSL_LIB_CTX *libctx)
{
    OSSL_PROVIDER *p;

    if (!TEST_true(OSSL_PROVIDER_add_builtin(libctx, "fake-pipeline",
                                             fake_pipeline_provider_init))
            || !TEST_ptr(p = OSSL_PROVIDER_try_load(libctx, "fake-pipeline", 1)))
        return NULL;

    return p;
}

void fake_pipeline_finish(OSSL_PROVIDER *p)
{
    OSSL_PROVIDER_unload(p);
}
