/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
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

#include <stdio.h>
#include "internal/cryptlib.h"

#ifndef OPENSSL_NO_CAST
# include <openssl/evp.h>
# include <openssl/objects.h>
# include "crypto/evp.h"
# include <openssl/cast.h>
# include "evp_local.h"

static int cast_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                         const unsigned char *iv, int enc);

typedef struct {
    CAST_KEY ks;
} EVP_CAST_KEY;

# define data(ctx)       EVP_C_DATA(EVP_CAST_KEY,ctx)

IMPLEMENT_BLOCK_CIPHER(cast5, ks, CAST, EVP_CAST_KEY,
                       NID_cast5, 8, CAST_KEY_LENGTH, 8, 64,
                       EVP_CIPH_VARIABLE_LENGTH, cast_init_key, NULL,
                       EVP_CIPHER_set_asn1_iv, EVP_CIPHER_get_asn1_iv, NULL)

static int cast_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                         const unsigned char *iv, int enc)
{
    int keylen = EVP_CIPHER_CTX_get_key_length(ctx);

    if (keylen <= 0)
        return 0;
    CAST_set_key(&data(ctx)->ks, keylen, key);
    return 1;
}

#endif
