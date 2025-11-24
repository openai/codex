/*
 * Copyright 1995-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * MD2 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/mdc2.h>

unsigned char *MDC2(const unsigned char *d, size_t n, unsigned char *md)
{
    MDC2_CTX c;
    static unsigned char m[MDC2_DIGEST_LENGTH];

    if (md == NULL)
        md = m;
    if (!MDC2_Init(&c))
        return NULL;
    MDC2_Update(&c, d, n);
    MDC2_Final(md, &c);
    OPENSSL_cleanse(&c, sizeof(c)); /* security consideration */
    return md;
}
