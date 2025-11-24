/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "prov/ciphercommon.h"
#include "crypto/aes_platform.h"

int ossl_cipher_capable_aes_cbc_hmac_sha1(void);
int ossl_cipher_capable_aes_cbc_hmac_sha256(void);

typedef struct prov_cipher_hw_aes_hmac_sha_ctx_st {
    PROV_CIPHER_HW base; /* must be first */
    void (*init_mac_key)(void *ctx, const unsigned char *inkey, size_t inlen);
    int (*set_tls1_aad)(void *ctx, unsigned char *aad_rec, int aad_len);
# if !defined(OPENSSL_NO_MULTIBLOCK)
    int (*tls1_multiblock_max_bufsize)(void *ctx);
    int (*tls1_multiblock_aad)(
        void *vctx, EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM *param);
    int (*tls1_multiblock_encrypt)(
        void *ctx, EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM *param);
# endif /* OPENSSL_NO_MULTIBLOCK) */
} PROV_CIPHER_HW_AES_HMAC_SHA;

const PROV_CIPHER_HW_AES_HMAC_SHA *ossl_prov_cipher_hw_aes_cbc_hmac_sha1(void);
const PROV_CIPHER_HW_AES_HMAC_SHA *ossl_prov_cipher_hw_aes_cbc_hmac_sha256(void);

#ifdef AES_CBC_HMAC_SHA_CAPABLE
# include <openssl/aes.h>
# include <openssl/sha.h>

typedef struct prov_aes_hmac_sha_ctx_st {
    PROV_CIPHER_CTX base;
    AES_KEY ks;
    size_t payload_length;      /* AAD length in decrypt case */
    union {
        unsigned int tls_ver;
        unsigned char tls_aad[16]; /* 13 used */
    } aux;
    const PROV_CIPHER_HW_AES_HMAC_SHA *hw;
    /* some value that are setup by set methods - that can be retrieved */
    unsigned int multiblock_interleave;
    unsigned int multiblock_aad_packlen;
    size_t multiblock_max_send_fragment;
    size_t multiblock_encrypt_len;
    size_t tls_aad_pad;
} PROV_AES_HMAC_SHA_CTX;

typedef struct prov_aes_hmac_sha1_ctx_st {
    PROV_AES_HMAC_SHA_CTX base_ctx;
    SHA_CTX head, tail, md;
} PROV_AES_HMAC_SHA1_CTX;

typedef struct prov_aes_hmac_sha256_ctx_st {
    PROV_AES_HMAC_SHA_CTX base_ctx;
    SHA256_CTX head, tail, md;
} PROV_AES_HMAC_SHA256_CTX;

# define NO_PAYLOAD_LENGTH ((size_t)-1)

#endif /* AES_CBC_HMAC_SHA_CAPABLE */
