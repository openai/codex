/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_PROV_CIPHERCOMMON_AEAD_H
# define OSSL_PROV_CIPHERCOMMON_AEAD_H
# pragma once

# define UNINITIALISED_SIZET ((size_t)-1)

# define AEAD_FLAGS (PROV_CIPHER_FLAG_AEAD | PROV_CIPHER_FLAG_CUSTOM_IV)

# define IMPLEMENT_aead_cipher(alg, lc, UCMODE, flags, kbits, blkbits, ivbits)  \
static OSSL_FUNC_cipher_get_params_fn alg##_##kbits##_##lc##_get_params;       \
static int alg##_##kbits##_##lc##_get_params(OSSL_PARAM params[])              \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                          flags, kbits, blkbits, ivbits);      \
}                                                                              \
static OSSL_FUNC_cipher_newctx_fn alg##kbits##lc##_newctx;                     \
static void * alg##kbits##lc##_newctx(void *provctx)                           \
{                                                                              \
    return alg##_##lc##_newctx(provctx, kbits);                                \
}                                                                              \
static void * alg##kbits##lc##_dupctx(void *src)                               \
{                                                                              \
    return alg##_##lc##_dupctx(src);                                           \
}                                                                              \
const OSSL_DISPATCH ossl_##alg##kbits##lc##_functions[] = {                    \
    { OSSL_FUNC_CIPHER_NEWCTX, (void (*)(void))alg##kbits##lc##_newctx },      \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))alg##_##lc##_freectx },        \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))alg##kbits##lc##_dupctx },      \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))ossl_##lc##_einit },      \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))ossl_##lc##_dinit },      \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))ossl_##lc##_stream_update },    \
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))ossl_##lc##_stream_final },      \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))ossl_##lc##_cipher },           \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void)) alg##_##kbits##_##lc##_get_params },                    \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void)) ossl_##lc##_get_ctx_params },                           \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void)) ossl_##lc##_set_ctx_params },                           \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_cipher_aead_gettable_ctx_params },                  \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_cipher_aead_settable_ctx_params },                  \
    OSSL_DISPATCH_END                                                          \
}

#endif
