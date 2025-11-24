/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for AES CBC CTS ciphers */

#include <openssl/proverr.h>
#include "cipher_cts.h"

#define CTS_FLAGS PROV_CIPHER_FLAG_CTS

static OSSL_FUNC_cipher_encrypt_init_fn aes_cbc_cts_einit;
static OSSL_FUNC_cipher_decrypt_init_fn aes_cbc_cts_dinit;
static OSSL_FUNC_cipher_get_ctx_params_fn aes_cbc_cts_get_ctx_params;
static OSSL_FUNC_cipher_set_ctx_params_fn aes_cbc_cts_set_ctx_params;
static OSSL_FUNC_cipher_gettable_ctx_params_fn aes_cbc_cts_gettable_ctx_params;
static OSSL_FUNC_cipher_settable_ctx_params_fn aes_cbc_cts_settable_ctx_params;

CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_START(aes_cbc_cts)
OSSL_PARAM_utf8_string(OSSL_CIPHER_PARAM_CTS_MODE, NULL, 0),
CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_END(aes_cbc_cts)

static int aes_cbc_cts_einit(void *ctx, const unsigned char *key, size_t keylen,
                             const unsigned char *iv, size_t ivlen,
                             const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_einit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return aes_cbc_cts_set_ctx_params(ctx, params);
}

static int aes_cbc_cts_dinit(void *ctx, const unsigned char *key, size_t keylen,
                             const unsigned char *iv, size_t ivlen,
                             const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_dinit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return aes_cbc_cts_set_ctx_params(ctx, params);
}

static int aes_cbc_cts_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_CTS_MODE);
    if (p != NULL) {
        const char *name = ossl_cipher_cbc_cts_mode_id2name(ctx->cts_mode);

        if (name == NULL || !OSSL_PARAM_set_utf8_string(p, name)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }
    return ossl_cipher_generic_get_ctx_params(vctx, params);
}

CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_START(aes_cbc_cts)
OSSL_PARAM_utf8_string(OSSL_CIPHER_PARAM_CTS_MODE, NULL, 0),
CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_END(aes_cbc_cts)

static int aes_cbc_cts_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    const OSSL_PARAM *p;
    int id;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_CTS_MODE);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            goto err;
        id = ossl_cipher_cbc_cts_mode_name2id(p->data);
        if (id < 0)
            goto err;

        ctx->cts_mode = (unsigned int)id;
    }
    return ossl_cipher_generic_set_ctx_params(vctx, params);
err:
    ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
    return 0;
}

/* ossl_aes256cbc_cts_functions */
IMPLEMENT_cts_cipher(aes, AES, cbc, CBC, CTS_FLAGS, 256, 128, 128, block)
/* ossl_aes192cbc_cts_functions */
IMPLEMENT_cts_cipher(aes, AES, cbc, CBC, CTS_FLAGS, 192, 128, 128, block)
/* ossl_aes128cbc_cts_functions */
IMPLEMENT_cts_cipher(aes, AES, cbc, CBC, CTS_FLAGS, 128, 128, 128, block)
