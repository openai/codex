/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CMLL_PLATFORM_H
# define OSSL_CMLL_PLATFORM_H
# pragma once

# if defined(CMLL_ASM) && (defined(__sparc) || defined(__sparc__))

/* Fujitsu SPARC64 X support */
#  include "crypto/sparc_arch.h"

#  ifndef OPENSSL_NO_CAMELLIA
#   define SPARC_CMLL_CAPABLE      (OPENSSL_sparcv9cap_P[1] & CFR_CAMELLIA)
#   include <openssl/camellia.h>

void cmll_t4_set_key(const unsigned char *key, int bits, CAMELLIA_KEY *ks);
void cmll_t4_encrypt(const unsigned char *in, unsigned char *out,
                     const CAMELLIA_KEY *key);
void cmll_t4_decrypt(const unsigned char *in, unsigned char *out,
                     const CAMELLIA_KEY *key);

void cmll128_t4_cbc_encrypt(const unsigned char *in, unsigned char *out,
                            size_t len, const CAMELLIA_KEY *key,
                            unsigned char *ivec, int /*unused*/);
void cmll128_t4_cbc_decrypt(const unsigned char *in, unsigned char *out,
                            size_t len, const CAMELLIA_KEY *key,
                            unsigned char *ivec, int /*unused*/);
void cmll256_t4_cbc_encrypt(const unsigned char *in, unsigned char *out,
                            size_t len, const CAMELLIA_KEY *key,
                            unsigned char *ivec, int /*unused*/);
void cmll256_t4_cbc_decrypt(const unsigned char *in, unsigned char *out,
                            size_t len, const CAMELLIA_KEY *key,
                            unsigned char *ivec, int /*unused*/);
void cmll128_t4_ctr32_encrypt(const unsigned char *in, unsigned char *out,
                              size_t blocks, const CAMELLIA_KEY *key,
                              unsigned char *ivec);
void cmll256_t4_ctr32_encrypt(const unsigned char *in, unsigned char *out,
                              size_t blocks, const CAMELLIA_KEY *key,
                              unsigned char *ivec);
#  endif /* OPENSSL_NO_CAMELLIA */

# endif /* CMLL_ASM && sparc */

#endif /* OSSL_CRYPTO_CIPHERMODE_PLATFORM_H */
