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

void SEED_ecb_encrypt(const unsigned char *in, unsigned char *out,
                      const SEED_KEY_SCHEDULE *ks, int enc)
{
    if (enc)
        SEED_encrypt(in, out, ks);
    else
        SEED_decrypt(in, out, ks);
}
