/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_PROV_HMAC_DRBG_H
# define OSSL_PROV_HMAC_DRBG_H
# pragma once

#include <openssl/evp.h>
#include "prov/provider_util.h"

typedef struct drbg_hmac_st {
    EVP_MAC_CTX *ctx;            /* H(x) = HMAC_hash OR H(x) = KMAC */
    PROV_DIGEST digest;          /* H(x) = hash(x) */
    size_t blocklen;
    unsigned char K[EVP_MAX_MD_SIZE];
    unsigned char V[EVP_MAX_MD_SIZE];
} PROV_DRBG_HMAC;

int ossl_drbg_hmac_init(PROV_DRBG_HMAC *drbg,
                        const unsigned char *ent, size_t ent_len,
                        const unsigned char *nonce, size_t nonce_len,
                        const unsigned char *pstr, size_t pstr_len);
int ossl_drbg_hmac_generate(PROV_DRBG_HMAC *hmac,
                            unsigned char *out, size_t outlen,
                            const unsigned char *adin, size_t adin_len);

#endif /* OSSL_PROV_HMAC_DRBG_H */
