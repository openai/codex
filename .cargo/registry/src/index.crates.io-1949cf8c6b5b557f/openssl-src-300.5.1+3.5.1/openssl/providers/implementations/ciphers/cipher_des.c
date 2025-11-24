/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DES low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <openssl/rand.h>
#include <openssl/proverr.h>
#include "prov/ciphercommon.h"
#include "cipher_des.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define DES_FLAGS PROV_CIPHER_FLAG_RAND_KEY

static OSSL_FUNC_cipher_freectx_fn des_freectx;
static OSSL_FUNC_cipher_encrypt_init_fn des_einit;
static OSSL_FUNC_cipher_decrypt_init_fn des_dinit;
static OSSL_FUNC_cipher_get_ctx_params_fn des_get_ctx_params;
static OSSL_FUNC_cipher_gettable_ctx_params_fn des_gettable_ctx_params;

static void *des_newctx(void *provctx, size_t kbits, size_t blkbits,
                        size_t ivbits, unsigned int mode, uint64_t flags,
                        const PROV_CIPHER_HW *hw)
{
    PROV_DES_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL)
        ossl_cipher_generic_initkey(ctx, kbits, blkbits, ivbits, mode, flags,
                                    hw, provctx);
    return ctx;
}

static void *des_dupctx(void *ctx)
{
    PROV_DES_CTX *in = (PROV_DES_CTX *)ctx;
    PROV_DES_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    in->base.hw->copyctx(&ret->base, &in->base);

    return ret;
}

static void des_freectx(void *vctx)
{
    PROV_DES_CTX *ctx = (PROV_DES_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static int des_init(void *vctx, const unsigned char *key, size_t keylen,
                    const unsigned char *iv, size_t ivlen,
                    const OSSL_PARAM params[], int enc)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    ctx->num = 0;
    ctx->bufsz = 0;
    ctx->enc = enc;

    if (iv != NULL) {
        if (!ossl_cipher_generic_initiv(ctx, iv, ivlen))
            return 0;
    } else if (ctx->iv_set) {
        /* reset IV to keep compatibility with 1.1.1 */
        memcpy(ctx->iv, ctx->oiv, ctx->ivlen);
    }

    if (key != NULL) {
        if (keylen != ctx->keylen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
        if (!ctx->hw->init(ctx, key, keylen))
            return 0;
        ctx->key_set = 1;
    }
    return ossl_cipher_generic_set_ctx_params(ctx, params);
}

static int des_einit(void *vctx, const unsigned char *key, size_t keylen,
                     const unsigned char *iv, size_t ivlen,
                     const OSSL_PARAM params[])
{
    return des_init(vctx, key, keylen, iv, ivlen, params, 1);
}

static int des_dinit(void *vctx, const unsigned char *key, size_t keylen,
                     const unsigned char *iv, size_t ivlen,
                     const OSSL_PARAM params[])
{
    return des_init(vctx, key, keylen, iv, ivlen, params, 0);
}

static int des_generatekey(PROV_CIPHER_CTX *ctx, void *ptr)
{

    DES_cblock *deskey = ptr;
    size_t kl = ctx->keylen;

    if (kl == 0 || RAND_priv_bytes_ex(ctx->libctx, ptr, kl, 0) <= 0)
        return 0;
    DES_set_odd_parity(deskey);
    return 1;
}

CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_START(des)
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_RANDOM_KEY, NULL, 0),
CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_END(des)

static int des_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_CIPHER_CTX  *ctx = (PROV_CIPHER_CTX *)vctx;
    OSSL_PARAM *p;

    if (!ossl_cipher_generic_get_ctx_params(vctx, params))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_RANDOM_KEY);
    if (p != NULL && !des_generatekey(ctx, p->data)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GENERATE_KEY);
        return 0;
    }
    return 1;
}

#define IMPLEMENT_des_cipher(type, lcmode, UCMODE, flags,                      \
                             kbits, blkbits, ivbits, block)                    \
static OSSL_FUNC_cipher_newctx_fn type##_##lcmode##_newctx;                    \
static void *des_##lcmode##_newctx(void *provctx)                              \
{                                                                              \
    return des_newctx(provctx, kbits, blkbits, ivbits,                         \
                      EVP_CIPH_##UCMODE##_MODE, flags,                         \
                      ossl_prov_cipher_hw_des_##lcmode());                          \
}                                                                              \
static OSSL_FUNC_cipher_get_params_fn des_##lcmode##_get_params;               \
static int des_##lcmode##_get_params(OSSL_PARAM params[])                      \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                          flags, kbits, blkbits, ivbits);      \
}                                                                              \
const OSSL_DISPATCH ossl_##des_##lcmode##_functions[] = {                      \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))des_einit },              \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))des_dinit },              \
    { OSSL_FUNC_CIPHER_UPDATE,                                                 \
      (void (*)(void))ossl_cipher_generic_##block##_update },                  \
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))ossl_cipher_generic_##block##_final },\
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))ossl_cipher_generic_cipher },   \
    { OSSL_FUNC_CIPHER_NEWCTX,                                                 \
      (void (*)(void))des_##lcmode##_newctx },                                 \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))des_dupctx },                   \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))des_freectx },                 \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void))des_##lcmode##_get_params },                             \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS, (void (*)(void))des_get_ctx_params },   \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))des_gettable_ctx_params },                               \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
     (void (*)(void))ossl_cipher_generic_set_ctx_params },                     \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))ossl_cipher_generic_settable_ctx_params },                \
    OSSL_DISPATCH_END                                                          \
}

/* ossl_des_ecb_functions */
IMPLEMENT_des_cipher(des, ecb, ECB, DES_FLAGS, 64, 64, 0, block);
/* ossl_des_cbc_functions */
IMPLEMENT_des_cipher(des, cbc, CBC, DES_FLAGS, 64, 64, 64, block);
/* ossl_des_ofb64_functions */
IMPLEMENT_des_cipher(des, ofb64, OFB, DES_FLAGS, 64, 8, 64, stream);
/* ossl_des_cfb64_functions */
IMPLEMENT_des_cipher(des, cfb64, CFB, DES_FLAGS, 64, 8, 64, stream);
/* ossl_des_cfb1_functions */
IMPLEMENT_des_cipher(des, cfb1, CFB, DES_FLAGS, 64, 8, 64, stream);
/* ossl_des_cfb8_functions */
IMPLEMENT_des_cipher(des, cfb8, CFB, DES_FLAGS, 64, 8, 64, stream);
