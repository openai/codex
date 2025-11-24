
/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * AES low level APIs are deprecated for public use, but still ok for internal
 * use where we're using them to implement the higher level EVP interface, as is
 * the case here.
 */
#include "internal/deprecated.h"

#include <openssl/proverr.h>
#include "cipher_aes_xts.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define AES_XTS_FLAGS PROV_CIPHER_FLAG_CUSTOM_IV
#define AES_XTS_IV_BITS 128
#define AES_XTS_BLOCK_BITS 8

/* forward declarations */
static OSSL_FUNC_cipher_encrypt_init_fn aes_xts_einit;
static OSSL_FUNC_cipher_decrypt_init_fn aes_xts_dinit;
static OSSL_FUNC_cipher_update_fn aes_xts_stream_update;
static OSSL_FUNC_cipher_final_fn aes_xts_stream_final;
static OSSL_FUNC_cipher_cipher_fn aes_xts_cipher;
static OSSL_FUNC_cipher_freectx_fn aes_xts_freectx;
static OSSL_FUNC_cipher_dupctx_fn aes_xts_dupctx;
static OSSL_FUNC_cipher_set_ctx_params_fn aes_xts_set_ctx_params;
static OSSL_FUNC_cipher_settable_ctx_params_fn aes_xts_settable_ctx_params;

/*
 * Verify that the two keys are different.
 *
 * This addresses the vulnerability described in Rogaway's
 * September 2004 paper:
 *
 *      "Efficient Instantiations of Tweakable Blockciphers and
 *       Refinements to Modes OCB and PMAC".
 *      (http://web.cs.ucdavis.edu/~rogaway/papers/offsets.pdf)
 *
 * FIPS 140-2 IG A.9 XTS-AES Key Generation Requirements states
 * that:
 *      "The check for Key_1 != Key_2 shall be done at any place
 *       BEFORE using the keys in the XTS-AES algorithm to process
 *       data with them."
 */
static int aes_xts_check_keys_differ(const unsigned char *key, size_t bytes,
                                     int enc)
{
    if ((!ossl_aes_xts_allow_insecure_decrypt || enc)
            && CRYPTO_memcmp(key, key + bytes, bytes) == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_XTS_DUPLICATED_KEYS);
        return 0;
    }
    return 1;
}

#ifdef AES_XTS_S390X
# include "cipher_aes_xts_s390x.inc"
#endif

/*-
 * Provider dispatch functions
 */
static int aes_xts_init(void *vctx, const unsigned char *key, size_t keylen,
                        const unsigned char *iv, size_t ivlen,
                        const OSSL_PARAM params[], int enc)
{
    PROV_AES_XTS_CTX *xctx = (PROV_AES_XTS_CTX *)vctx;
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
        if (!aes_xts_check_keys_differ(key, keylen / 2, enc))
            return 0;
        if (!ctx->hw->init(ctx, key, keylen))
            return 0;
    }
    return aes_xts_set_ctx_params(ctx, params);
}

static int aes_xts_einit(void *vctx, const unsigned char *key, size_t keylen,
                         const unsigned char *iv, size_t ivlen,
                         const OSSL_PARAM params[])
{
#ifdef AES_XTS_S390X
    if (s390x_aes_xts_einit(vctx, key, keylen, iv, ivlen, params) == 1)
        return 1;
#endif
    return aes_xts_init(vctx, key, keylen, iv, ivlen, params, 1);
}

static int aes_xts_dinit(void *vctx, const unsigned char *key, size_t keylen,
                         const unsigned char *iv, size_t ivlen,
                         const OSSL_PARAM params[])
{
#ifdef AES_XTS_S390X
    if (s390x_aes_xts_dinit(vctx, key, keylen, iv, ivlen, params) == 1)
        return 1;
#endif
    return aes_xts_init(vctx, key, keylen, iv, ivlen, params, 0);
}

static void *aes_xts_newctx(void *provctx, unsigned int mode, uint64_t flags,
                            size_t kbits, size_t blkbits, size_t ivbits)
{
    PROV_AES_XTS_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL) {
        ossl_cipher_generic_initkey(&ctx->base, kbits, blkbits, ivbits, mode,
                                    flags, ossl_prov_cipher_hw_aes_xts(kbits),
                                    NULL);
    }
    return ctx;
}

static void aes_xts_freectx(void *vctx)
{
    PROV_AES_XTS_CTX *ctx = (PROV_AES_XTS_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *aes_xts_dupctx(void *vctx)
{
    PROV_AES_XTS_CTX *in = (PROV_AES_XTS_CTX *)vctx;
    PROV_AES_XTS_CTX *ret = NULL;

    if (!ossl_prov_is_running())
        return NULL;

#ifdef AES_XTS_S390X
    if (in->plat.s390x.fc)
        return s390x_aes_xts_dupctx(vctx);
#endif

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

static int aes_xts_cipher(void *vctx, unsigned char *out, size_t *outl,
                          size_t outsize, const unsigned char *in, size_t inl)
{
    PROV_AES_XTS_CTX *ctx = (PROV_AES_XTS_CTX *)vctx;

#ifdef AES_XTS_S390X
    if (ctx->plat.s390x.fc)
        return s390x_aes_xts_cipher(vctx, out, outl, outsize, in, inl);
#endif

    if (!ossl_prov_is_running()
            || ctx->xts.key1 == NULL
            || ctx->xts.key2 == NULL
            || !ctx->base.iv_set
            || out == NULL
            || in == NULL
            || inl < AES_BLOCK_SIZE)
        return 0;

    /*
     * Impose a limit of 2^20 blocks per data unit as specified by
     * IEEE Std 1619-2018.  The earlier and obsolete IEEE Std 1619-2007
     * indicated that this was a SHOULD NOT rather than a MUST NOT.
     * NIST SP 800-38E mandates the same limit.
     */
    if (inl > XTS_MAX_BLOCKS_PER_DATA_UNIT * AES_BLOCK_SIZE) {
        ERR_raise(ERR_LIB_PROV, PROV_R_XTS_DATA_UNIT_IS_TOO_LARGE);
        return 0;
    }

    if (ctx->stream != NULL)
        (*ctx->stream)(in, out, inl, ctx->xts.key1, ctx->xts.key2, ctx->base.iv);
    else if (CRYPTO_xts128_encrypt(&ctx->xts, ctx->base.iv, in, out, inl,
                                   ctx->base.enc))
        return 0;

    *outl = inl;
    return 1;
}

static int aes_xts_stream_update(void *vctx, unsigned char *out, size_t *outl,
                                 size_t outsize, const unsigned char *in,
                                 size_t inl)
{
    PROV_AES_XTS_CTX *ctx = (PROV_AES_XTS_CTX *)vctx;

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (!aes_xts_cipher(ctx, out, outl, outsize, in, inl)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }

    return 1;
}

static int aes_xts_stream_final(void *vctx, unsigned char *out, size_t *outl,
                                size_t outsize)
{
    if (!ossl_prov_is_running())
        return 0;
    *outl = 0;
    return 1;
}

static const OSSL_PARAM aes_xts_known_settable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_END
};

static const OSSL_PARAM *aes_xts_settable_ctx_params(ossl_unused void *cctx,
                                                     ossl_unused void *provctx)
{
    return aes_xts_known_settable_ctx_params;
}

static int aes_xts_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        size_t keylen;

        if (!OSSL_PARAM_get_size_t(p, &keylen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        /* The key length can not be modified for xts mode */
        if (keylen != ctx->keylen)
            return 0;
    }

    return 1;
}

#define IMPLEMENT_cipher(lcmode, UCMODE, kbits, flags)                         \
static OSSL_FUNC_cipher_get_params_fn aes_##kbits##_##lcmode##_get_params;     \
static int aes_##kbits##_##lcmode##_get_params(OSSL_PARAM params[])            \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                     flags, 2 * kbits, AES_XTS_BLOCK_BITS,     \
                                     AES_XTS_IV_BITS);                         \
}                                                                              \
static OSSL_FUNC_cipher_newctx_fn aes_##kbits##_xts_newctx;                    \
static void *aes_##kbits##_xts_newctx(void *provctx)                           \
{                                                                              \
    return aes_xts_newctx(provctx, EVP_CIPH_##UCMODE##_MODE, flags, 2 * kbits, \
                          AES_XTS_BLOCK_BITS, AES_XTS_IV_BITS);                \
}                                                                              \
const OSSL_DISPATCH ossl_aes##kbits##xts_functions[] = {                       \
    { OSSL_FUNC_CIPHER_NEWCTX, (void (*)(void))aes_##kbits##_xts_newctx },     \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))aes_xts_einit },          \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))aes_xts_dinit },          \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))aes_xts_stream_update },        \
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))aes_xts_stream_final },          \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))aes_xts_cipher },               \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))aes_xts_freectx },             \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))aes_xts_dupctx },               \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void))aes_##kbits##_##lcmode##_get_params },                   \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_cipher_generic_get_ctx_params },                    \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_cipher_generic_gettable_ctx_params },               \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void))aes_xts_set_ctx_params },                                \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))aes_xts_settable_ctx_params },                            \
    OSSL_DISPATCH_END                                                          \
}

IMPLEMENT_cipher(xts, XTS, 256, AES_XTS_FLAGS);
IMPLEMENT_cipher(xts, XTS, 128, AES_XTS_FLAGS);
