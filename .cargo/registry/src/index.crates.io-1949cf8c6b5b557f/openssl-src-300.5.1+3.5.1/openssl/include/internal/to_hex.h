/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_TO_HEX_H
# define OSSL_INTERNAL_TO_HEX_H
# pragma once

static ossl_inline size_t to_hex(char *buf, uint8_t n, const char hexdig[17])
{
    *buf++ = hexdig[(n >> 4) & 0xf];
    *buf = hexdig[n & 0xf];
    return 2;
}

static ossl_inline size_t ossl_to_lowerhex(char *buf, uint8_t n)
{
    static const char hexdig[] = "0123456789abcdef";

    return to_hex(buf, n, hexdig);
}
#endif
