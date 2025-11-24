/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_HMAC_LOCAL_H
# define OSSL_CRYPTO_HMAC_LOCAL_H

# include "internal/common.h"
# include "internal/numbers.h"
# include "openssl/sha.h"

/* The current largest case is for SHA3-224 */
#define HMAC_MAX_MD_CBLOCK_SIZE     144

struct hmac_ctx_st {
    const EVP_MD *md;
    EVP_MD_CTX *md_ctx;
    EVP_MD_CTX *i_ctx;
    EVP_MD_CTX *o_ctx;

    /* Platform specific data */
    union {
        int dummy;
# ifdef OPENSSL_HMAC_S390X
        struct {
            unsigned int fc; /* 0 if not supported by kmac instruction */
            int blk_size;
            int ikp;
            int iimp;
            unsigned char *buf;
            size_t size; /* must be multiple of digest block size */
            size_t num;
            union {
                OSSL_UNION_ALIGN;
                struct {
                    uint32_t h[8];
                    uint64_t imbl;
                    unsigned char key[64];
                } hmac_224_256;
                struct {
                    uint64_t h[8];
                    uint128_t imbl;
                    unsigned char key[128];
                } hmac_384_512;
            } param;
        } s390x;
# endif /* OPENSSL_HMAC_S390X */
    } plat;
};

# ifdef OPENSSL_HMAC_S390X
#  define HMAC_S390X_BUF_NUM_BLOCKS 64

int s390x_HMAC_init(HMAC_CTX *ctx, const void *key, int key_len, ENGINE *impl);
int s390x_HMAC_update(HMAC_CTX *ctx, const unsigned char *data, size_t len);
int s390x_HMAC_final(HMAC_CTX *ctx, unsigned char *md, unsigned int *len);
int s390x_HMAC_CTX_copy(HMAC_CTX *dctx, HMAC_CTX *sctx);
int s390x_HMAC_CTX_cleanup(HMAC_CTX *ctx);
# endif /* OPENSSL_HMAC_S390X */

#endif
