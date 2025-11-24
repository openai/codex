/*
 * Copyright 2017-2018 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2016 Cryptography Research, Inc.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * Originally written by Mike Hamburg
 */

#ifndef OSSL_CRYPTO_EC_CURVE448_ARCH_32_INTRINSICS_H
# define OSSL_CRYPTO_EC_CURVE448_ARCH_32_INTRINSICS_H

#include "internal/constant_time.h"

# define ARCH_WORD_BITS 32

#define word_is_zero(a)     constant_time_is_zero_32(a)

static ossl_inline uint64_t widemul(uint32_t a, uint32_t b)
{
    return ((uint64_t)a) * b;
}

#endif                          /* OSSL_CRYPTO_EC_CURVE448_ARCH_32_INTRINSICS_H */
