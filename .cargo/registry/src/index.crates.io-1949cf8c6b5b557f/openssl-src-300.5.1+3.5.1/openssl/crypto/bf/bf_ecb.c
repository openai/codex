/*
 * Copyright 1995-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * BF low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <openssl/blowfish.h>
#include "bf_local.h"
#include <openssl/opensslv.h>

/*
 * Blowfish as implemented from 'Blowfish: Springer-Verlag paper' (From
 * LECTURE NOTES IN COMPUTER SCIENCE 809, FAST SOFTWARE ENCRYPTION, CAMBRIDGE
 * SECURITY WORKSHOP, CAMBRIDGE, U.K., DECEMBER 9-11, 1993)
 */

const char *BF_options(void)
{
    return "blowfish(ptr)";
}

void BF_ecb_encrypt(const unsigned char *in, unsigned char *out,
                    const BF_KEY *key, int encrypt)
{
    BF_LONG l, d[2];

    n2l(in, l);
    d[0] = l;
    n2l(in, l);
    d[1] = l;
    if (encrypt)
        BF_encrypt(d, key);
    else
        BF_decrypt(d, key);
    l = d[0];
    l2n(l, out);
    l = d[1];
    l2n(l, out);
    l = d[0] = d[1] = 0;
}
