/*
 * Copyright 2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_UNICODE_H
# define OSSL_INTERNAL_UNICODE_H
# pragma once

typedef enum {
    SURROGATE_MIN = 0xd800UL,
    SURROGATE_MAX = 0xdfffUL,
    UNICODE_MAX = 0x10ffffUL,
    UNICODE_LIMIT
} UNICODE_CONSTANTS;

static ossl_unused ossl_inline int is_unicode_surrogate(unsigned long value)
{
    return value >= SURROGATE_MIN && value <= SURROGATE_MAX;
}

static ossl_unused ossl_inline int is_unicode_valid(unsigned long value)
{
    return value <= UNICODE_MAX && !is_unicode_surrogate(value);
}

#endif
