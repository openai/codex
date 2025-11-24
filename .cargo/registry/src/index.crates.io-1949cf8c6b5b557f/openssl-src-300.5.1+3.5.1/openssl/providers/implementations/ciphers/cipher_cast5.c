/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * CAST low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

/* Dispatch functions for cast cipher modes ecb, cbc, ofb, cfb */

#include <openssl/proverr.h>
#include "cipher_cast.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define CAST5_FLAGS PROV_CIPHER_FLAG_VARIABLE_LENGTH

static OSSL_FUNC_cipher_freectx_fn cast5_freectx;
static OSSL_FUNC_cipher_dupctx_fn cast5_dupctx;

static void cast5_freectx(void *vctx)
{
    PROV_CAST_CTX *ctx = (PROV_CAST_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *cast5_dupctx(void *ctx)
{
    PROV_CAST_CTX *in = (PROV_CAST_CTX *)ctx;
    PROV_CAST_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    *ret = *in;

    return ret;
}

/* ossl_cast5128ecb_functions */
IMPLEMENT_var_keylen_cipher(cast5, CAST, ecb, ECB, CAST5_FLAGS, 128, 64, 0, block)
/* ossl_cast5128cbc_functions */
IMPLEMENT_var_keylen_cipher(cast5, CAST, cbc, CBC, CAST5_FLAGS, 128, 64, 64, block)
/* ossl_cast5128ofb64_functions */
IMPLEMENT_var_keylen_cipher(cast5, CAST, ofb64, OFB, CAST5_FLAGS, 128, 8, 64, stream)
/* ossl_cast5128cfb64_functions */
IMPLEMENT_var_keylen_cipher(cast5, CAST, cfb64,  CFB, CAST5_FLAGS, 128, 8, 64, stream)
