/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OPENSSL_HASHFUNC_H
# define OPENSSL_HASHFUNC_H

# include <openssl/e_os2.h>
# include <stddef.h>
/**
 * Generalized fnv1a 64 bit hash function
 */
ossl_unused uint64_t ossl_fnv1a_hash(uint8_t *key, size_t len);

#endif
