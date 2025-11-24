/*
 * Copyright 2017-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2017 Ribose Inc. All Rights Reserved.
 * Ported from Ribose contributions from Botan.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_SM2_H
# define OSSL_CRYPTO_SM2_H
# pragma once

# include <openssl/opensslconf.h>

# if !defined(OPENSSL_NO_SM2) && !defined(FIPS_MODULE)

#  include <openssl/ec.h>
#  include "crypto/types.h"

int ossl_sm2_key_private_check(const EC_KEY *eckey);

/* The default user id as specified in GM/T 0009-2012 */
#  define SM2_DEFAULT_USERID "1234567812345678"

int ossl_sm2_compute_z_digest(uint8_t *out,
                              const EVP_MD *digest,
                              const uint8_t *id,
                              const size_t id_len,
                              const EC_KEY *key);

/*
 * SM2 signature operation. Computes Z and then signs H(Z || msg) using SM2
 */
ECDSA_SIG *ossl_sm2_do_sign(const EC_KEY *key,
                            const EVP_MD *digest,
                            const uint8_t *id,
                            const size_t id_len,
                            const uint8_t *msg, size_t msg_len);

int ossl_sm2_do_verify(const EC_KEY *key,
                       const EVP_MD *digest,
                       const ECDSA_SIG *signature,
                       const uint8_t *id,
                       const size_t id_len,
                       const uint8_t *msg, size_t msg_len);

/*
 * SM2 signature generation.
 */
int ossl_sm2_internal_sign(const unsigned char *dgst, int dgstlen,
                           unsigned char *sig, unsigned int *siglen,
                           EC_KEY *eckey);

/*
 * SM2 signature verification.
 */
int ossl_sm2_internal_verify(const unsigned char *dgst, int dgstlen,
                             const unsigned char *sig, int siglen,
                             EC_KEY *eckey);

/*
 * SM2 encryption
 */
int ossl_sm2_ciphertext_size(const EC_KEY *key, const EVP_MD *digest,
                             size_t msg_len, size_t *ct_size);

int ossl_sm2_plaintext_size(const unsigned char *ct, size_t ct_size,
                            size_t *pt_size);

int ossl_sm2_encrypt(const EC_KEY *key,
                     const EVP_MD *digest,
                     const uint8_t *msg, size_t msg_len,
                     uint8_t *ciphertext_buf, size_t *ciphertext_len);

int ossl_sm2_decrypt(const EC_KEY *key,
                     const EVP_MD *digest,
                     const uint8_t *ciphertext, size_t ciphertext_len,
                     uint8_t *ptext_buf, size_t *ptext_len);

const unsigned char *ossl_sm2_algorithmidentifier_encoding(int md_nid,
                                                           size_t *len);
# endif /* OPENSSL_NO_SM2 */
#endif
