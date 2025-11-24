/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for Blowfish cipher modes ecb, cbc, ofb, cfb */

/*
 * BF low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include "cipher_blowfish.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define BF_FLAGS PROV_CIPHER_FLAG_VARIABLE_LENGTH

static OSSL_FUNC_cipher_freectx_fn blowfish_freectx;
static OSSL_FUNC_cipher_dupctx_fn blowfish_dupctx;

static void blowfish_freectx(void *vctx)
{
    PROV_BLOWFISH_CTX *ctx = (PROV_BLOWFISH_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *blowfish_dupctx(void *ctx)
{
    PROV_BLOWFISH_CTX *in = (PROV_BLOWFISH_CTX *)ctx;
    PROV_BLOWFISH_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    *ret = *in;

    return ret;
}

/* bf_ecb_functions */
IMPLEMENT_var_keylen_cipher(blowfish, BLOWFISH, ecb, ECB, BF_FLAGS, 128, 64, 0, block)
/* bf_cbc_functions */
IMPLEMENT_var_keylen_cipher(blowfish, BLOWFISH, cbc, CBC, BF_FLAGS, 128, 64, 64, block)
/* bf_ofb_functions */
IMPLEMENT_var_keylen_cipher(blowfish, BLOWFISH, ofb64, OFB, BF_FLAGS, 128, 8, 64, stream)
/* bf_cfb_functions */
IMPLEMENT_var_keylen_cipher(blowfish, BLOWFISH, cfb64,  CFB, BF_FLAGS, 128, 8, 64, stream)
