/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_PROVIDER_RAND_POOL_H
# define OSSL_PROVIDER_RAND_POOL_H
# pragma once

# include <stdio.h>
# include <openssl/rand.h>

/*
 * Maximum allocation size for RANDOM_POOL buffers
 *
 * The max_len value for the buffer provided to the rand_drbg_get_entropy()
 * callback is currently 2^31 bytes (2 gigabytes), if a derivation function
 * is used. Since this is much too large to be allocated, the ossl_rand_pool_new()
 * function chooses more modest values as default pool length, bounded
 * by RAND_POOL_MIN_LENGTH and RAND_POOL_MAX_LENGTH
 *
 * The choice of the RAND_POOL_FACTOR is large enough such that the
 * RAND_POOL can store a random input which has a lousy entropy rate of
 * 8/256 (= 0.03125) bits per byte. This input will be sent through the
 * derivation function which 'compresses' the low quality input into a
 * high quality output.
 *
 * The factor 1.5 below is the pessimistic estimate for the extra amount
 * of entropy required when no get_nonce() callback is defined.
 */
# define RAND_POOL_FACTOR        256
# define RAND_POOL_MAX_LENGTH    (RAND_POOL_FACTOR * \
                                  3 * (RAND_DRBG_STRENGTH / 16))
/*
 *                             = (RAND_POOL_FACTOR * \
 *                                1.5 * (RAND_DRBG_STRENGTH / 8))
 */

/*
 * Initial allocation minimum.
 *
 * There is a distinction between the secure and normal allocation minimums.
 * Ideally, the secure allocation size should be a power of two.  The normal
 * allocation size doesn't have any such restriction.
 *
 * The secure value is based on 128 bits of secure material, which is 16 bytes.
 * Typically, the DRBGs will set a minimum larger than this so optimal
 * allocation ought to take place (for full quality seed material).
 *
 * The normal value has been chosen by noticing that the rand_drbg_get_nonce
 * function is usually the largest of the built in allocation (twenty four
 * bytes and then appending another sixteen bytes).  This means the buffer ends
 * with 40 bytes.  The value of forty eight is comfortably above this which
 * allows some slack in the platform specific values used.
 */
# define RAND_POOL_MIN_ALLOCATION(secure) ((secure) ? 16 : 48)

/*
 * The 'random pool' acts as a dumb container for collecting random
 * input from various entropy sources. It is the callers duty to 1) initialize
 * the random pool, 2) pass it to the polling callbacks, 3) seed the RNG, and
 * 4) cleanup the random pool again.
 *
 * The random pool contains no locking mechanism because its scope and
 * lifetime is intended to be restricted to a single stack frame.
 */
typedef struct rand_pool_st {
    unsigned char *buffer;  /* points to the beginning of the random pool */
    size_t len; /* current number of random bytes contained in the pool */

    int attached;  /* true pool was attached to existing buffer */
    int secure;    /* 1: allocated on the secure heap, 0: otherwise */

    size_t min_len; /* minimum number of random bytes requested */
    size_t max_len; /* maximum number of random bytes (allocated buffer size) */
    size_t alloc_len; /* current number of bytes allocated */
    size_t entropy; /* current entropy count in bits */
    size_t entropy_requested; /* requested entropy count in bits */
} RAND_POOL;

RAND_POOL *ossl_rand_pool_new(int entropy_requested, int secure,
                              size_t min_len, size_t max_len);
RAND_POOL *ossl_rand_pool_attach(const unsigned char *buffer, size_t len,
                                 size_t entropy);
void ossl_rand_pool_free(RAND_POOL *pool);

const unsigned char *ossl_rand_pool_buffer(RAND_POOL *pool);
unsigned char *ossl_rand_pool_detach(RAND_POOL *pool);
void ossl_rand_pool_reattach(RAND_POOL *pool, unsigned char *buffer);

size_t ossl_rand_pool_entropy(RAND_POOL *pool);
size_t ossl_rand_pool_length(RAND_POOL *pool);

size_t ossl_rand_pool_entropy_available(RAND_POOL *pool);
size_t ossl_rand_pool_entropy_needed(RAND_POOL *pool);
/* |entropy_factor| expresses how many bits of data contain 1 bit of entropy */
size_t ossl_rand_pool_bytes_needed(RAND_POOL *pool, unsigned int entropy_factor);
size_t ossl_rand_pool_bytes_remaining(RAND_POOL *pool);

int ossl_rand_pool_add(RAND_POOL *pool,
                       const unsigned char *buffer, size_t len, size_t entropy);
unsigned char *ossl_rand_pool_add_begin(RAND_POOL *pool, size_t len);
int ossl_rand_pool_add_end(RAND_POOL *pool, size_t len, size_t entropy);
int ossl_rand_pool_adin_mix_in(RAND_POOL *pool, const unsigned char *adin,
                               size_t adin_len);

#endif /* OSSL_PROVIDER_RAND_POOL_H */
