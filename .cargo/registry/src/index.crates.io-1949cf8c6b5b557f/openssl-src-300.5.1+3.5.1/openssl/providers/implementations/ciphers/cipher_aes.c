/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
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

/* Dispatch functions for AES cipher modes ecb, cbc, ofb, cfb, ctr */

#include "cipher_aes.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

static OSSL_FUNC_cipher_freectx_fn aes_freectx;
static OSSL_FUNC_cipher_dupctx_fn aes_dupctx;

static void aes_freectx(void *vctx)
{
    PROV_AES_CTX *ctx = (PROV_AES_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *aes_dupctx(void *ctx)
{
    PROV_AES_CTX *in = (PROV_AES_CTX *)ctx;
    PROV_AES_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    in->base.hw->copyctx(&ret->base, &in->base);

    return ret;
}

/* ossl_aes256ecb_functions */
IMPLEMENT_generic_cipher(aes, AES, ecb, ECB, 0, 256, 128, 0, block)
/* ossl_aes192ecb_functions */
IMPLEMENT_generic_cipher(aes, AES, ecb, ECB, 0, 192, 128, 0, block)
/* ossl_aes128ecb_functions */
IMPLEMENT_generic_cipher(aes, AES, ecb, ECB, 0, 128, 128, 0, block)
/* ossl_aes256cbc_functions */
IMPLEMENT_generic_cipher(aes, AES, cbc, CBC, 0, 256, 128, 128, block)
/* ossl_aes192cbc_functions */
IMPLEMENT_generic_cipher(aes, AES, cbc, CBC, 0, 192, 128, 128, block)
/* ossl_aes128cbc_functions */
IMPLEMENT_generic_cipher(aes, AES, cbc, CBC, 0, 128, 128, 128, block)
/* ossl_aes256ofb_functions */
IMPLEMENT_generic_cipher(aes, AES, ofb, OFB, 0, 256, 8, 128, stream)
/* ossl_aes192ofb_functions */
IMPLEMENT_generic_cipher(aes, AES, ofb, OFB, 0, 192, 8, 128, stream)
/* ossl_aes128ofb_functions */
IMPLEMENT_generic_cipher(aes, AES, ofb, OFB, 0, 128, 8, 128, stream)
/* ossl_aes256cfb_functions */
IMPLEMENT_generic_cipher(aes, AES, cfb,  CFB, 0, 256, 8, 128, stream)
/* ossl_aes192cfb_functions */
IMPLEMENT_generic_cipher(aes, AES, cfb,  CFB, 0, 192, 8, 128, stream)
/* ossl_aes128cfb_functions */
IMPLEMENT_generic_cipher(aes, AES, cfb,  CFB, 0, 128, 8, 128, stream)
/* ossl_aes256cfb1_functions */
IMPLEMENT_generic_cipher(aes, AES, cfb1, CFB, 0, 256, 8, 128, stream)
/* ossl_aes192cfb1_functions */
IMPLEMENT_generic_cipher(aes, AES, cfb1, CFB, 0, 192, 8, 128, stream)
/* ossl_aes128cfb1_functions */
IMPLEMENT_generic_cipher(aes, AES, cfb1, CFB, 0, 128, 8, 128, stream)
/* ossl_aes256cfb8_functions */
IMPLEMENT_generic_cipher(aes, AES, cfb8, CFB, 0, 256, 8, 128, stream)
/* ossl_aes192cfb8_functions */
IMPLEMENT_generic_cipher(aes, AES, cfb8, CFB, 0, 192, 8, 128, stream)
/* ossl_aes128cfb8_functions */
IMPLEMENT_generic_cipher(aes, AES, cfb8, CFB, 0, 128, 8, 128, stream)
/* ossl_aes256ctr_functions */
IMPLEMENT_generic_cipher(aes, AES, ctr, CTR, 0, 256, 8, 128, stream)
/* ossl_aes192ctr_functions */
IMPLEMENT_generic_cipher(aes, AES, ctr, CTR, 0, 192, 8, 128, stream)
/* ossl_aes128ctr_functions */
IMPLEMENT_generic_cipher(aes, AES, ctr, CTR, 0, 128, 8, 128, stream)

#include "cipher_aes_cts.inc"
