
/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for SM4 XTS mode */

#include <openssl/proverr.h>
#include "cipher_sm4_xts.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define SM4_XTS_FLAGS PROV_CIPHER_FLAG_CUSTOM_IV
#define SM4_XTS_IV_BITS 128
#define SM4_XTS_BLOCK_BITS 8

/* forward declarations */
static OSSL_FUNC_cipher_encrypt_init_fn sm4_xts_einit;
static OSSL_FUNC_cipher_decrypt_init_fn sm4_xts_dinit;
static OSSL_FUNC_cipher_update_fn sm4_xts_stream_update;
static OSSL_FUNC_cipher_final_fn sm4_xts_stream_final;
static OSSL_FUNC_cipher_cipher_fn sm4_xts_cipher;
static OSSL_FUNC_cipher_freectx_fn sm4_xts_freectx;
static OSSL_FUNC_cipher_dupctx_fn sm4_xts_dupctx;
static OSSL_FUNC_cipher_set_ctx_params_fn sm4_xts_set_ctx_params;
static OSSL_FUNC_cipher_settable_ctx_params_fn sm4_xts_settable_ctx_params;

/*-
 * Provider dispatch functions
 */
static int sm4_xts_init(void *vctx, const unsigned char *key, size_t keylen,
                        const unsigned char *iv, size_t ivlen,
                        const OSSL_PARAM params[], int enc)
{
    PROV_SM4_XTS_CTX *xctx = (PROV_SM4_XTS_CTX *)vctx;
    PROV_CIPHER_CTX *ctx = &xctx->base;

    if (!ossl_prov_is_running())
        return 0;

    ctx->enc = enc;

    if (iv != NULL) {
        if (!ossl_cipher_generic_initiv(vctx, iv, ivlen))
            return 0;
    }
    if (key != NULL) {
        if (keylen != ctx->keylen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
        if (!ctx->hw->init(ctx, key, keylen))
            return 0;
    }
    return sm4_xts_set_ctx_params(xctx, params);
}

static int sm4_xts_einit(void *vctx, const unsigned char *key, size_t keylen,
                         const unsigned char *iv, size_t ivlen,
                         const OSSL_PARAM params[])
{
    return sm4_xts_init(vctx, key, keylen, iv, ivlen, params, 1);
}

static int sm4_xts_dinit(void *vctx, const unsigned char *key, size_t keylen,
                         const unsigned char *iv, size_t ivlen,
                         const OSSL_PARAM params[])
{
    return sm4_xts_init(vctx, key, keylen, iv, ivlen, params, 0);
}

static void *sm4_xts_newctx(void *provctx, unsigned int mode, uint64_t flags,
                            size_t kbits, size_t blkbits, size_t ivbits)
{
    PROV_SM4_XTS_CTX *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL) {
        ossl_cipher_generic_initkey(&ctx->base, kbits, blkbits, ivbits, mode,
                                    flags, ossl_prov_cipher_hw_sm4_xts(kbits),
                                    NULL);
    }
    return ctx;
}

static void sm4_xts_freectx(void *vctx)
{
    PROV_SM4_XTS_CTX *ctx = (PROV_SM4_XTS_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx, sizeof(*ctx));
}

static void *sm4_xts_dupctx(void *vctx)
{
    PROV_SM4_XTS_CTX *in = (PROV_SM4_XTS_CTX *)vctx;
    PROV_SM4_XTS_CTX *ret = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if (in->xts.key1 != NULL) {
        if (in->xts.key1 != &in->ks1)
            return NULL;
    }
    if (in->xts.key2 != NULL) {
        if (in->xts.key2 != &in->ks2)
            return NULL;
    }
    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    in->base.hw->copyctx(&ret->base, &in->base);
    return ret;
}

static int sm4_xts_cipher(void *vctx, unsigned char *out, size_t *outl,
                          size_t outsize, const unsigned char *in, size_t inl)
{
    PROV_SM4_XTS_CTX *ctx = (PROV_SM4_XTS_CTX *)vctx;

    if (!ossl_prov_is_running()
            || ctx->xts.key1 == NULL
            || ctx->xts.key2 == NULL
            || !ctx->base.iv_set
            || out == NULL
            || in == NULL
            || inl < SM4_BLOCK_SIZE)
        return 0;

    /*
     * Impose a limit of 2^20 blocks per data unit as specified by
     * IEEE Std 1619-2018.  The earlier and obsolete IEEE Std 1619-2007
     * indicated that this was a SHOULD NOT rather than a MUST NOT.
     * NIST SP 800-38E mandates the same limit.
     */
    if (inl > XTS_MAX_BLOCKS_PER_DATA_UNIT * SM4_BLOCK_SIZE) {
        ERR_raise(ERR_LIB_PROV, PROV_R_XTS_DATA_UNIT_IS_TOO_LARGE);
        return 0;
    }
    if (ctx->xts_standard) {
        if (ctx->stream != NULL)
            (*ctx->stream)(in, out, inl, ctx->xts.key1, ctx->xts.key2,
                           ctx->base.iv, ctx->base.enc);
        else if (CRYPTO_xts128_encrypt(&ctx->xts, ctx->base.iv, in, out, inl,
                                       ctx->base.enc))
            return 0;
    } else {
        if (ctx->stream_gb != NULL)
            (*ctx->stream_gb)(in, out, inl, ctx->xts.key1, ctx->xts.key2,
                              ctx->base.iv, ctx->base.enc);
        else if (ossl_crypto_xts128gb_encrypt(&ctx->xts, ctx->base.iv, in, out,
                                              inl, ctx->base.enc))
            return 0;
    }
    *outl = inl;
    return 1;
}

static int sm4_xts_stream_update(void *vctx, unsigned char *out, size_t *outl,
                                 size_t outsize, const unsigned char *in,
                                 size_t inl)
{
    PROV_SM4_XTS_CTX *ctx = (PROV_SM4_XTS_CTX *)vctx;

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (!sm4_xts_cipher(ctx, out, outl, outsize, in, inl)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }

    return 1;
}

static int sm4_xts_stream_final(void *vctx, unsigned char *out, size_t *outl,
                                size_t outsize)
{
    if (!ossl_prov_is_running())
        return 0;
    *outl = 0;
    return 1;
}

static const OSSL_PARAM sm4_xts_known_settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_CIPHER_PARAM_XTS_STANDARD, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *sm4_xts_settable_ctx_params(ossl_unused void *cctx,
                                                     ossl_unused void *provctx)
{
    return sm4_xts_known_settable_ctx_params;
}

static int sm4_xts_set_ctx_params(void *vxctx, const OSSL_PARAM params[])
{
    PROV_SM4_XTS_CTX *xctx = (PROV_SM4_XTS_CTX *)vxctx;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    /*-
     * Sets the XTS standard to use with SM4-XTS algorithm.
     *
     * Must be utf8 string "GB" or "IEEE",
     * "GB" means the GB/T 17964-2021 standard
     * "IEEE" means the IEEE Std 1619-2007 standard
     */
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_XTS_STANDARD);

    if (p != NULL) {
        const char *xts_standard = NULL;

        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;

        if (!OSSL_PARAM_get_utf8_string_ptr(p, &xts_standard)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (OPENSSL_strcasecmp(xts_standard, "GB") == 0) {
            xctx->xts_standard = 0;
        } else if (OPENSSL_strcasecmp(xts_standard, "IEEE") == 0) {
            xctx->xts_standard = 1;
        } else {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }

    return 1;
}

#define IMPLEMENT_cipher(lcmode, UCMODE, kbits, flags)                         \
static OSSL_FUNC_cipher_get_params_fn sm4_##kbits##_##lcmode##_get_params;     \
static int sm4_##kbits##_##lcmode##_get_params(OSSL_PARAM params[])            \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                          flags, 2 * kbits, SM4_XTS_BLOCK_BITS,\
                                          SM4_XTS_IV_BITS);                    \
}                                                                              \
static OSSL_FUNC_cipher_newctx_fn sm4_##kbits##_xts_newctx;                    \
static void *sm4_##kbits##_xts_newctx(void *provctx)                           \
{                                                                              \
    return sm4_xts_newctx(provctx, EVP_CIPH_##UCMODE##_MODE, flags, 2 * kbits, \
                          SM4_XTS_BLOCK_BITS, SM4_XTS_IV_BITS);                \
}                                                                              \
const OSSL_DISPATCH ossl_sm4##kbits##xts_functions[] = {                       \
    { OSSL_FUNC_CIPHER_NEWCTX, (void (*)(void))sm4_##kbits##_xts_newctx },     \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))sm4_xts_einit },          \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))sm4_xts_dinit },          \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))sm4_xts_stream_update },        \
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))sm4_xts_stream_final },          \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))sm4_xts_cipher },               \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))sm4_xts_freectx },             \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))sm4_xts_dupctx },               \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void))sm4_##kbits##_##lcmode##_get_params },                   \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_cipher_generic_get_ctx_params },                    \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_cipher_generic_gettable_ctx_params },               \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void))sm4_xts_set_ctx_params },                                \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))sm4_xts_settable_ctx_params },                            \
    OSSL_DISPATCH_END                                                          \
}
/* ossl_sm4128xts_functions */
IMPLEMENT_cipher(xts, XTS, 128, SM4_XTS_FLAGS);
