/*
 * Copyright 1995-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/rsa.h>

int RSA_padding_add_none(unsigned char *to, int tlen,
                         const unsigned char *from, int flen)
{
    if (flen > tlen) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DATA_TOO_LARGE_FOR_KEY_SIZE);
        return 0;
    }

    if (flen < tlen) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DATA_TOO_SMALL_FOR_KEY_SIZE);
        return 0;
    }

    memcpy(to, from, (unsigned int)flen);
    return 1;
}

int RSA_padding_check_none(unsigned char *to, int tlen,
                           const unsigned char *from, int flen, int num)
{

    if (flen > tlen) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DATA_TOO_LARGE);
        return -1;
    }

    memset(to, 0, tlen - flen);
    memcpy(to + tlen - flen, from, flen);
    return tlen;
}
