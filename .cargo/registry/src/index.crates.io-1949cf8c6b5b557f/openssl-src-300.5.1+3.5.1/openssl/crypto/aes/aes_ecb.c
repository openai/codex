/*
 * Copyright 2002-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>

/*
 * AES_encrypt/AES_decrypt are deprecated - but we need to use them to implement
 * AES_ecb_encrypt
 */
#include "internal/deprecated.h"

#include <openssl/aes.h>
#include "aes_local.h"

void AES_ecb_encrypt(const unsigned char *in, unsigned char *out,
                     const AES_KEY *key, const int enc)
{

    assert(in && out && key);
    assert((AES_ENCRYPT == enc) || (AES_DECRYPT == enc));

    if (AES_ENCRYPT == enc)
        AES_encrypt(in, out, key);
    else
        AES_decrypt(in, out, key);
}
