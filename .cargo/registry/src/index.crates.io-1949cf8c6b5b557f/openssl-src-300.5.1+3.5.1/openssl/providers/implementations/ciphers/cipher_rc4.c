/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for RC4 ciphers */

/*
 * RC4 low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include "cipher_rc4.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define RC4_FLAGS PROV_CIPHER_FLAG_VARIABLE_LENGTH

static OSSL_FUNC_cipher_encrypt_init_fn rc4_einit;
static OSSL_FUNC_cipher_decrypt_init_fn rc4_dinit;
static OSSL_FUNC_cipher_freectx_fn rc4_freectx;
static OSSL_FUNC_cipher_dupctx_fn rc4_dupctx;

static void rc4_freectx(void *vctx)
{
    PROV_RC4_CTX *ctx = (PROV_RC4_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *rc4_dupctx(void *ctx)
{
    PROV_RC4_CTX *in = (PROV_RC4_CTX *)ctx;
    PROV_RC4_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    *ret = *in;

    return ret;
}

static int rc4_einit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_einit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return ossl_cipher_var_keylen_set_ctx_params(ctx, params);
}

static int rc4_dinit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_dinit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return ossl_cipher_var_keylen_set_ctx_params(ctx, params);
}

#define IMPLEMENT_cipher(alg, UCALG, flags, kbits, blkbits, ivbits, typ)       \
static OSSL_FUNC_cipher_get_params_fn alg##_##kbits##_get_params;              \
static int alg##_##kbits##_get_params(OSSL_PARAM params[])                     \
{                                                                              \
    return ossl_cipher_generic_get_params(params, 0, flags,                    \
                                     kbits, blkbits, ivbits);                  \
}                                                                              \
static OSSL_FUNC_cipher_newctx_fn alg##_##kbits##_newctx;                      \
static void *alg##_##kbits##_newctx(void *provctx)                             \
{                                                                              \
     PROV_##UCALG##_CTX *ctx;                                                  \
     if (!ossl_prov_is_running())                                              \
        return NULL;                                                           \
     ctx = OPENSSL_zalloc(sizeof(*ctx));                                       \
     if (ctx != NULL) {                                                        \
         ossl_cipher_generic_initkey(ctx, kbits, blkbits, ivbits, 0, flags,    \
                                     ossl_prov_cipher_hw_##alg(kbits), NULL);  \
     }                                                                         \
     return ctx;                                                               \
}                                                                              \
const OSSL_DISPATCH ossl_##alg##kbits##_functions[] = {                        \
    { OSSL_FUNC_CIPHER_NEWCTX,                                                 \
      (void (*)(void)) alg##_##kbits##_newctx },                               \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void)) alg##_freectx },              \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void)) alg##_dupctx },                \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))rc4_einit },              \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))rc4_dinit },              \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))ossl_cipher_generic_##typ##_update },\
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))ossl_cipher_generic_##typ##_final },  \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))ossl_cipher_generic_cipher },   \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void)) alg##_##kbits##_get_params },                           \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_cipher_generic_get_ctx_params },                    \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_cipher_var_keylen_set_ctx_params },                 \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_cipher_generic_gettable_ctx_params },               \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))ossl_cipher_var_keylen_settable_ctx_params },             \
    OSSL_DISPATCH_END                                                          \
};

/* ossl_rc440_functions */
IMPLEMENT_cipher(rc4, RC4, RC4_FLAGS, 40, 8, 0, stream)
/* ossl_rc4128_functions */
IMPLEMENT_cipher(rc4, RC4, RC4_FLAGS, 128, 8, 0, stream)
