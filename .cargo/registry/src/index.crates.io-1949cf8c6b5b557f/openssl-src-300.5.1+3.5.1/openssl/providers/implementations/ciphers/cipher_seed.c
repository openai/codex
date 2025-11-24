/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for Seed cipher modes ecb, cbc, ofb, cfb */

/*
 * SEED low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "cipher_seed.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

static OSSL_FUNC_cipher_freectx_fn seed_freectx;
static OSSL_FUNC_cipher_dupctx_fn seed_dupctx;

static void seed_freectx(void *vctx)
{
    PROV_SEED_CTX *ctx = (PROV_SEED_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *seed_dupctx(void *ctx)
{
    PROV_SEED_CTX *in = (PROV_SEED_CTX *)ctx;
    PROV_SEED_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    *ret = *in;

    return ret;
}

/* ossl_seed128ecb_functions */
IMPLEMENT_generic_cipher(seed, SEED, ecb, ECB, 0, 128, 128, 0, block)
/* ossl_seed128cbc_functions */
IMPLEMENT_generic_cipher(seed, SEED, cbc, CBC, 0, 128, 128, 128, block)
/* ossl_seed128ofb128_functions */
IMPLEMENT_generic_cipher(seed, SEED, ofb128, OFB, 0, 128, 8, 128, stream)
/* ossl_seed128cfb128_functions */
IMPLEMENT_generic_cipher(seed, SEED, cfb128,  CFB, 0, 128, 8, 128, stream)
