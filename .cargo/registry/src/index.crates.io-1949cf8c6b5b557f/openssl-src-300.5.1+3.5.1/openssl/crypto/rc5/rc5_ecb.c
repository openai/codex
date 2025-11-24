/*
 * Copyright 1995-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RC5 low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <openssl/rc5.h>
#include "rc5_local.h"
#include <openssl/opensslv.h>

void RC5_32_ecb_encrypt(const unsigned char *in, unsigned char *out,
                        RC5_32_KEY *ks, int encrypt)
{
    unsigned long l, d[2];

    c2l(in, l);
    d[0] = l;
    c2l(in, l);
    d[1] = l;
    if (encrypt)
        RC5_32_encrypt(d, ks);
    else
        RC5_32_decrypt(d, ks);
    l = d[0];
    l2c(l, out);
    l = d[1];
    l2c(l, out);
    l = d[0] = d[1] = 0;
}
