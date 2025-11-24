/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for ARIA cipher modes ecb, cbc, ofb, cfb, ctr */

#include "cipher_aria.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

static OSSL_FUNC_cipher_freectx_fn aria_freectx;
static OSSL_FUNC_cipher_dupctx_fn aria_dupctx;

static void aria_freectx(void *vctx)
{
    PROV_ARIA_CTX *ctx = (PROV_ARIA_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *aria_dupctx(void *ctx)
{
    PROV_ARIA_CTX *in = (PROV_ARIA_CTX *)ctx;
    PROV_ARIA_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    in->base.hw->copyctx(&ret->base, &in->base);

    return ret;
}

/* ossl_aria256ecb_functions */
IMPLEMENT_generic_cipher(aria, ARIA, ecb, ECB, 0, 256, 128, 0, block)
/* ossl_aria192ecb_functions */
IMPLEMENT_generic_cipher(aria, ARIA, ecb, ECB, 0, 192, 128, 0, block)
/* ossl_aria128ecb_functions */
IMPLEMENT_generic_cipher(aria, ARIA, ecb, ECB, 0, 128, 128, 0, block)
/* ossl_aria256cbc_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cbc, CBC, 0, 256, 128, 128, block)
/* ossl_aria192cbc_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cbc, CBC, 0, 192, 128, 128, block)
/* ossl_aria128cbc_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cbc, CBC, 0, 128, 128, 128, block)
/* ossl_aria256ofb_functions */
IMPLEMENT_generic_cipher(aria, ARIA, ofb, OFB, 0, 256, 8, 128, stream)
/* ossl_aria192ofb_functions */
IMPLEMENT_generic_cipher(aria, ARIA, ofb, OFB, 0, 192, 8, 128, stream)
/* ossl_aria128ofb_functions */
IMPLEMENT_generic_cipher(aria, ARIA, ofb, OFB, 0, 128, 8, 128, stream)
/* ossl_aria256cfb_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cfb,  CFB, 0, 256, 8, 128, stream)
/* ossl_aria192cfb_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cfb,  CFB, 0, 192, 8, 128, stream)
/* ossl_aria128cfb_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cfb,  CFB, 0, 128, 8, 128, stream)
/* ossl_aria256cfb1_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cfb1, CFB, 0, 256, 8, 128, stream)
/* ossl_aria192cfb1_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cfb1, CFB, 0, 192, 8, 128, stream)
/* ossl_aria128cfb1_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cfb1, CFB, 0, 128, 8, 128, stream)
/* ossl_aria256cfb8_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cfb8, CFB, 0, 256, 8, 128, stream)
/* ossl_aria192cfb8_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cfb8, CFB, 0, 192, 8, 128, stream)
/* ossl_aria128cfb8_functions */
IMPLEMENT_generic_cipher(aria, ARIA, cfb8, CFB, 0, 128, 8, 128, stream)
/* ossl_aria256ctr_functions */
IMPLEMENT_generic_cipher(aria, ARIA, ctr, CTR, 0, 256, 8, 128, stream)
/* ossl_aria192ctr_functions */
IMPLEMENT_generic_cipher(aria, ARIA, ctr, CTR, 0, 192, 8, 128, stream)
/* ossl_aria128ctr_functions */
IMPLEMENT_generic_cipher(aria, ARIA, ctr, CTR, 0, 128, 8, 128, stream)
