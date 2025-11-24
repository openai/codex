/*
 * Copyright 2007-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * SEED low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/seed.h>
#include <openssl/modes.h>

void SEED_cfb128_encrypt(const unsigned char *in, unsigned char *out,
                         size_t len, const SEED_KEY_SCHEDULE *ks,
                         unsigned char ivec[SEED_BLOCK_SIZE], int *num,
                         int enc)
{
    CRYPTO_cfb128_encrypt(in, out, len, ks, ivec, num, enc,
                          (block128_f) SEED_encrypt);
}
