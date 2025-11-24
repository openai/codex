/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_DES_PLATFORM_H
# define OSSL_DES_PLATFORM_H
# pragma once

# if defined(DES_ASM) && (defined(__sparc) || defined(__sparc__))

/* Fujitsu SPARC64 X support */
#  include "crypto/sparc_arch.h"

#  ifndef OPENSSL_NO_DES
#   define SPARC_DES_CAPABLE      (OPENSSL_sparcv9cap_P[1] & CFR_DES)
#   include <openssl/des.h>
void des_t4_key_expand(const void *key, DES_key_schedule *ks);
void des_t4_ede3_cbc_encrypt(const void *inp, void *out, size_t len,
                             const DES_key_schedule ks[3], unsigned char iv[8]);
void des_t4_ede3_cbc_decrypt(const void *inp, void *out, size_t len,
                             const DES_key_schedule ks[3], unsigned char iv[8]);
void des_t4_cbc_encrypt(const void *inp, void *out, size_t len,
                        const DES_key_schedule *ks, unsigned char iv[8]);
void des_t4_cbc_decrypt(const void *inp, void *out, size_t len,
                        const DES_key_schedule *ks, unsigned char iv[8]);
#  endif /* OPENSSL_NO_DES */

# endif /* DES_ASM && sparc */

#endif /* OSSL_CRYPTO_CIPHERMODE_PLATFORM_H */
