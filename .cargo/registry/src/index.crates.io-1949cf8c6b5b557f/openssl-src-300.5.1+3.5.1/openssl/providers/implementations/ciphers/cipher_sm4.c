/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for cast cipher modes ecb, cbc, ofb, cfb */

#include "cipher_sm4.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

static OSSL_FUNC_cipher_freectx_fn sm4_freectx;
static OSSL_FUNC_cipher_dupctx_fn sm4_dupctx;

static void sm4_freectx(void *vctx)
{
    PROV_SM4_CTX *ctx = (PROV_SM4_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *sm4_dupctx(void *ctx)
{
    PROV_SM4_CTX *in = (PROV_SM4_CTX *)ctx;
    PROV_SM4_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    in->base.hw->copyctx(&ret->base, &in->base);

    return ret;
}

/* ossl_sm4128ecb_functions */
IMPLEMENT_generic_cipher(sm4, SM4, ecb, ECB, 0, 128, 128, 0, block)
/* ossl_sm4128cbc_functions */
IMPLEMENT_generic_cipher(sm4, SM4, cbc, CBC, 0, 128, 128, 128, block)
/* ossl_sm4128ctr_functions */
IMPLEMENT_generic_cipher(sm4, SM4, ctr, CTR, 0, 128, 8, 128, stream)
/* ossl_sm4128ofb128_functions */
IMPLEMENT_generic_cipher(sm4, SM4, ofb128, OFB, 0, 128, 8, 128, stream)
/* ossl_sm4128cfb128_functions */
IMPLEMENT_generic_cipher(sm4, SM4, cfb128,  CFB, 0, 128, 8, 128, stream)
