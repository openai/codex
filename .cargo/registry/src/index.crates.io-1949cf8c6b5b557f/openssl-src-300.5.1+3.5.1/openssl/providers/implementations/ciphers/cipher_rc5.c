/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for RC5 cipher modes ecb, cbc, ofb, cfb */

/*
 * RC5 low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <openssl/proverr.h>
#include "cipher_rc5.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define RC5_FLAGS PROV_CIPHER_FLAG_VARIABLE_LENGTH

static OSSL_FUNC_cipher_encrypt_init_fn rc5_einit;
static OSSL_FUNC_cipher_decrypt_init_fn rc5_dinit;
static OSSL_FUNC_cipher_freectx_fn rc5_freectx;
static OSSL_FUNC_cipher_dupctx_fn rc5_dupctx;
OSSL_FUNC_cipher_gettable_ctx_params_fn rc5_gettable_ctx_params;
OSSL_FUNC_cipher_settable_ctx_params_fn rc5_settable_ctx_params;
static OSSL_FUNC_cipher_set_ctx_params_fn rc5_set_ctx_params;

static void rc5_freectx(void *vctx)
{
    PROV_RC5_CTX *ctx = (PROV_RC5_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *rc5_dupctx(void *ctx)
{
    PROV_RC5_CTX *in = (PROV_RC5_CTX *)ctx;
    PROV_RC5_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    *ret = *in;

    return ret;
}

static int rc5_einit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_einit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return rc5_set_ctx_params(ctx, params);
}

static int rc5_dinit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_dinit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return rc5_set_ctx_params(ctx, params);
}

static int rc5_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_RC5_CTX *ctx = (PROV_RC5_CTX *)vctx;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    if (!ossl_cipher_var_keylen_set_ctx_params(vctx, params))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_ROUNDS);
    if (p != NULL) {
        unsigned int rounds;

        if (!OSSL_PARAM_get_uint(p, &rounds)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (rounds != RC5_8_ROUNDS
            && rounds != RC5_12_ROUNDS
            && rounds != RC5_16_ROUNDS) {
            ERR_raise(ERR_LIB_PROV, PROV_R_UNSUPPORTED_NUMBER_OF_ROUNDS);
            return 0;
        }
        ctx->rounds = rounds;
    }
    return 1;
}

CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_START(rc5)
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_ROUNDS, NULL),
CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_END(rc5)

CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_START(rc5)
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_ROUNDS, NULL),
CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_END(rc5)


static int rc5_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_RC5_CTX *ctx = (PROV_RC5_CTX *)vctx;
    OSSL_PARAM *p;

    if (!ossl_cipher_generic_get_ctx_params(vctx, params))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_ROUNDS);
    if (p != NULL && !OSSL_PARAM_set_uint(p, ctx->rounds)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    return 1;
}

#define IMPLEMENT_cipher(alg, UCALG, lcmode, UCMODE, flags, kbits,             \
                         blkbits, ivbits, typ)                                 \
static OSSL_FUNC_cipher_get_params_fn alg##_##kbits##_##lcmode##_get_params;   \
static int alg##_##kbits##_##lcmode##_get_params(OSSL_PARAM params[])          \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                          flags, kbits, blkbits, ivbits);      \
}                                                                              \
static OSSL_FUNC_cipher_newctx_fn alg##_##kbits##_##lcmode##_newctx;           \
static void *alg##_##kbits##_##lcmode##_newctx(void *provctx)                  \
{                                                                              \
     PROV_##UCALG##_CTX *ctx;                                                  \
     if (!ossl_prov_is_running())                                              \
        return NULL;                                                           \
     ctx = OPENSSL_zalloc(sizeof(*ctx));                                       \
     if (ctx != NULL) {                                                        \
         ossl_cipher_generic_initkey(ctx, kbits, blkbits, ivbits,              \
                                     EVP_CIPH_##UCMODE##_MODE, flags,          \
                                     ossl_prov_cipher_hw_##alg##_##lcmode(kbits),\
                                NULL);                                         \
         ctx->rounds = RC5_12_ROUNDS;                                          \
     }                                                                         \
     return ctx;                                                               \
}                                                                              \
const OSSL_DISPATCH ossl_##alg##kbits##lcmode##_functions[] = {                \
    { OSSL_FUNC_CIPHER_NEWCTX,                                                 \
      (void (*)(void)) alg##_##kbits##_##lcmode##_newctx },                    \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void)) alg##_freectx },              \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void)) alg##_dupctx },                \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))rc5_einit },              \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))rc5_dinit },              \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))ossl_cipher_generic_##typ##_update },\
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))ossl_cipher_generic_##typ##_final },  \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))ossl_cipher_generic_cipher },   \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void)) alg##_##kbits##_##lcmode##_get_params },                \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void))rc5_get_ctx_params },                                    \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))rc5_gettable_ctx_params },                               \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void))rc5_set_ctx_params },                                    \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))rc5_settable_ctx_params },                                \
    OSSL_DISPATCH_END                                                          \
};

/* ossl_rc5128ecb_functions */
IMPLEMENT_cipher(rc5, RC5, ecb, ECB, RC5_FLAGS, 128, 64, 0, block)
/* ossl_rc5128cbc_functions */
IMPLEMENT_cipher(rc5, RC5, cbc, CBC, RC5_FLAGS, 128, 64, 64, block)
/* ossl_rc5128ofb64_functions */
IMPLEMENT_cipher(rc5, RC5, ofb64, OFB, RC5_FLAGS, 128, 8, 64, stream)
/* ossl_rc5128cfb64_functions */
IMPLEMENT_cipher(rc5, RC5, cfb64,  CFB, RC5_FLAGS, 128, 8, 64, stream)
