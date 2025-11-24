/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/evp.h"

/* NOTE: The underlying block cipher is CBC so we reuse most of the code */
#define IMPLEMENT_cts_cipher(alg, UCALG, lcmode, UCMODE, flags, kbits,         \
                             blkbits, ivbits, typ)                             \
static OSSL_FUNC_cipher_get_params_fn alg##_##kbits##_##lcmode##_get_params;   \
static int alg##_cts_##kbits##_##lcmode##_get_params(OSSL_PARAM params[])      \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                          flags, kbits, blkbits, ivbits);      \
}                                                                              \
const OSSL_DISPATCH ossl_##alg##kbits##lcmode##_cts_functions[] = {            \
    { OSSL_FUNC_CIPHER_NEWCTX,                                                 \
      (void (*)(void)) alg##_##kbits##_##lcmode##_newctx },                    \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void)) alg##_freectx },              \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void)) alg##_dupctx },                \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void)) alg##_cbc_cts_einit },   \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void)) alg##_cbc_cts_dinit },   \
    { OSSL_FUNC_CIPHER_UPDATE,                                                 \
      (void (*)(void)) ossl_cipher_cbc_cts_block_update },                     \
    { OSSL_FUNC_CIPHER_FINAL,                                                  \
      (void (*)(void)) ossl_cipher_cbc_cts_block_final },                      \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))ossl_cipher_generic_cipher },   \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void)) alg##_cts_##kbits##_##lcmode##_get_params },            \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void)) alg##_cbc_cts_get_ctx_params },                         \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void)) alg##_cbc_cts_set_ctx_params },                         \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void)) alg##_cbc_cts_gettable_ctx_params },                    \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void)) alg##_cbc_cts_settable_ctx_params },                     \
    OSSL_DISPATCH_END                                                          \
};

OSSL_FUNC_cipher_update_fn ossl_cipher_cbc_cts_block_update;
OSSL_FUNC_cipher_final_fn ossl_cipher_cbc_cts_block_final;

const char *ossl_cipher_cbc_cts_mode_id2name(unsigned int id);
int ossl_cipher_cbc_cts_mode_name2id(const char *name);
