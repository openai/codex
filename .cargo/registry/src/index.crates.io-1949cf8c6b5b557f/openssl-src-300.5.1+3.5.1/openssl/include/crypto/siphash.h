/*
 * Copyright 2017-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_SIPHASH_H
# define OSSL_CRYPTO_SIPHASH_H
# pragma once

# include <stddef.h>

# define SIPHASH_BLOCK_SIZE        8
# define SIPHASH_KEY_SIZE         16
# define SIPHASH_MIN_DIGEST_SIZE   8
# define SIPHASH_MAX_DIGEST_SIZE  16

typedef struct siphash_st SIPHASH;

size_t SipHash_ctx_size(void);
size_t SipHash_hash_size(SIPHASH *ctx);
int SipHash_set_hash_size(SIPHASH *ctx, size_t hash_size);
int SipHash_Init(SIPHASH *ctx, const unsigned char *k,
                 int crounds, int drounds);
void SipHash_Update(SIPHASH *ctx, const unsigned char *in, size_t inlen);
int SipHash_Final(SIPHASH *ctx, unsigned char *out, size_t outlen);

/* Based on https://131002.net/siphash C reference implementation */

struct siphash_st {
    uint64_t total_inlen;
    uint64_t v0;
    uint64_t v1;
    uint64_t v2;
    uint64_t v3;
    unsigned int len;
    unsigned int hash_size;
    unsigned int crounds;
    unsigned int drounds;
    unsigned char leavings[SIPHASH_BLOCK_SIZE];
};

/* default: SipHash-2-4 */
# define SIPHASH_C_ROUNDS 2
# define SIPHASH_D_ROUNDS 4

#endif
