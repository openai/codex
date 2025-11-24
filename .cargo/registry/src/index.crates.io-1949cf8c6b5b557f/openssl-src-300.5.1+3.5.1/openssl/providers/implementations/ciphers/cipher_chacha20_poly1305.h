/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for chacha20_poly1305 cipher */

#include "include/crypto/poly1305.h"
#include "cipher_chacha20.h"

#define NO_TLS_PAYLOAD_LENGTH ((size_t)-1)
#define CHACHA20_POLY1305_IVLEN 12

typedef struct {
    PROV_CIPHER_CTX base;       /* must be first */
    PROV_CHACHA20_CTX chacha;
    POLY1305 poly1305;
    unsigned int nonce[12 / 4];
    unsigned char tag[POLY1305_BLOCK_SIZE];
    unsigned char tls_aad[POLY1305_BLOCK_SIZE];
    struct { uint64_t aad, text; } len;
    unsigned int aad : 1;
    unsigned int mac_inited : 1;
    size_t tag_len;
    size_t tls_payload_length;
    size_t tls_aad_pad_sz;
} PROV_CHACHA20_POLY1305_CTX;

typedef struct prov_cipher_hw_chacha_aead_st {
    PROV_CIPHER_HW base; /* must be first */
    int (*aead_cipher)(PROV_CIPHER_CTX *dat, unsigned char *out, size_t *outl,
                       const unsigned char *in, size_t len);
    int (*initiv)(PROV_CIPHER_CTX *ctx);
    int (*tls_init)(PROV_CIPHER_CTX *ctx, unsigned char *aad, size_t alen);
    int (*tls_iv_set_fixed)(PROV_CIPHER_CTX *ctx, unsigned char *fixed,
                            size_t flen);
} PROV_CIPHER_HW_CHACHA20_POLY1305;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_chacha20_poly1305(size_t keybits);
