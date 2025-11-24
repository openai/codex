/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <time.h>
#include "internal/cryptlib.h"
#include <openssl/opensslconf.h>
#include "crypto/rand.h"
#include <openssl/engine.h>
#include "internal/thread_once.h"
#include "crypto/rand_pool.h"

/*
 * Allocate memory and initialize a new random pool
 */
RAND_POOL *ossl_rand_pool_new(int entropy_requested, int secure,
                              size_t min_len, size_t max_len)
{
    RAND_POOL *pool = OPENSSL_zalloc(sizeof(*pool));
    size_t min_alloc_size = RAND_POOL_MIN_ALLOCATION(secure);

    if (pool == NULL)
        return NULL;

    pool->min_len = min_len;
    pool->max_len = (max_len > RAND_POOL_MAX_LENGTH) ?
        RAND_POOL_MAX_LENGTH : max_len;
    pool->alloc_len = min_len < min_alloc_size ? min_alloc_size : min_len;
    if (pool->alloc_len > pool->max_len)
        pool->alloc_len = pool->max_len;

    if (secure)
        pool->buffer = OPENSSL_secure_zalloc(pool->alloc_len);
    else
        pool->buffer = OPENSSL_zalloc(pool->alloc_len);

    if (pool->buffer == NULL)
        goto err;

    pool->entropy_requested = entropy_requested;
    pool->secure = secure;
    return pool;

err:
    OPENSSL_free(pool);
    return NULL;
}

/*
 * Attach new random pool to the given buffer
 *
 * This function is intended to be used only for feeding random data
 * provided by RAND_add() and RAND_seed() into the <master> DRBG.
 */
RAND_POOL *ossl_rand_pool_attach(const unsigned char *buffer, size_t len,
                                 size_t entropy)
{
    RAND_POOL *pool = OPENSSL_zalloc(sizeof(*pool));

    if (pool == NULL)
        return NULL;

    /*
     * The const needs to be cast away, but attached buffers will not be
     * modified (in contrary to allocated buffers which are zeroed and
     * freed in the end).
     */
    pool->buffer = (unsigned char *) buffer;
    pool->len = len;

    pool->attached = 1;

    pool->min_len = pool->max_len = pool->alloc_len = pool->len;
    pool->entropy = entropy;

    return pool;
}

/*
 * Free |pool|, securely erasing its buffer.
 */
void ossl_rand_pool_free(RAND_POOL *pool)
{
    if (pool == NULL)
        return;

    /*
     * Although it would be advisable from a cryptographical viewpoint,
     * we are not allowed to clear attached buffers, since they are passed
     * to ossl_rand_pool_attach() as `const unsigned char*`.
     * (see corresponding comment in ossl_rand_pool_attach()).
     */
    if (!pool->attached) {
        if (pool->secure)
            OPENSSL_secure_clear_free(pool->buffer, pool->alloc_len);
        else
            OPENSSL_clear_free(pool->buffer, pool->alloc_len);
    }

    OPENSSL_free(pool);
}

/*
 * Return the |pool|'s buffer to the caller (readonly).
 */
const unsigned char *ossl_rand_pool_buffer(RAND_POOL *pool)
{
    return pool->buffer;
}

/*
 * Return the |pool|'s entropy to the caller.
 */
size_t ossl_rand_pool_entropy(RAND_POOL *pool)
{
    return pool->entropy;
}

/*
 * Return the |pool|'s buffer length to the caller.
 */
size_t ossl_rand_pool_length(RAND_POOL *pool)
{
    return pool->len;
}

/*
 * Detach the |pool| buffer and return it to the caller.
 * It's the responsibility of the caller to free the buffer
 * using OPENSSL_secure_clear_free() or to re-attach it
 * again to the pool using ossl_rand_pool_reattach().
 */
unsigned char *ossl_rand_pool_detach(RAND_POOL *pool)
{
    unsigned char *ret = pool->buffer;
    pool->buffer = NULL;
    pool->entropy = 0;
    return ret;
}

/*
 * Re-attach the |pool| buffer. It is only allowed to pass
 * the |buffer| which was previously detached from the same pool.
 */
void ossl_rand_pool_reattach(RAND_POOL *pool, unsigned char *buffer)
{
    pool->buffer = buffer;
    OPENSSL_cleanse(pool->buffer, pool->len);
    pool->len = 0;
}

/*
 * If |entropy_factor| bits contain 1 bit of entropy, how many bytes does one
 * need to obtain at least |bits| bits of entropy?
 */
#define ENTROPY_TO_BYTES(bits, entropy_factor) \
    (((bits) * (entropy_factor) + 7) / 8)


/*
 * Checks whether the |pool|'s entropy is available to the caller.
 * This is the case when entropy count and buffer length are high enough.
 * Returns
 *
 *  |entropy|  if the entropy count and buffer size is large enough
 *      0      otherwise
 */
size_t ossl_rand_pool_entropy_available(RAND_POOL *pool)
{
    if (pool->entropy < pool->entropy_requested)
        return 0;

    if (pool->len < pool->min_len)
        return 0;

    return pool->entropy;
}

/*
 * Returns the (remaining) amount of entropy needed to fill
 * the random pool.
 */

size_t ossl_rand_pool_entropy_needed(RAND_POOL *pool)
{
    if (pool->entropy < pool->entropy_requested)
        return pool->entropy_requested - pool->entropy;

    return 0;
}

/* Increase the allocation size -- not usable for an attached pool */
static int rand_pool_grow(RAND_POOL *pool, size_t len)
{
    if (len > pool->alloc_len - pool->len) {
        unsigned char *p;
        const size_t limit = pool->max_len / 2;
        size_t newlen = pool->alloc_len;

        if (pool->attached || len > pool->max_len - pool->len) {
            ERR_raise(ERR_LIB_RAND, ERR_R_INTERNAL_ERROR);
            return 0;
        }

        do
            newlen = newlen < limit ? newlen * 2 : pool->max_len;
        while (len > newlen - pool->len);

        if (pool->secure)
            p = OPENSSL_secure_zalloc(newlen);
        else
            p = OPENSSL_zalloc(newlen);
        if (p == NULL)
            return 0;
        memcpy(p, pool->buffer, pool->len);
        if (pool->secure)
            OPENSSL_secure_clear_free(pool->buffer, pool->alloc_len);
        else
            OPENSSL_clear_free(pool->buffer, pool->alloc_len);
        pool->buffer = p;
        pool->alloc_len = newlen;
    }
    return 1;
}

/*
 * Returns the number of bytes needed to fill the pool, assuming
 * the input has 1 / |entropy_factor| entropy bits per data bit.
 * In case of an error, 0 is returned.
 */

size_t ossl_rand_pool_bytes_needed(RAND_POOL *pool, unsigned int entropy_factor)
{
    size_t bytes_needed;
    size_t entropy_needed = ossl_rand_pool_entropy_needed(pool);

    if (entropy_factor < 1) {
        ERR_raise(ERR_LIB_RAND, RAND_R_ARGUMENT_OUT_OF_RANGE);
        return 0;
    }

    bytes_needed = ENTROPY_TO_BYTES(entropy_needed, entropy_factor);

    if (bytes_needed > pool->max_len - pool->len) {
        /* not enough space left */
        ERR_raise_data(ERR_LIB_RAND, RAND_R_RANDOM_POOL_OVERFLOW,
                       "entropy_factor=%u, entropy_needed=%zu, bytes_needed=%zu,"
                       "pool->max_len=%zu, pool->len=%zu",
                       entropy_factor, entropy_needed, bytes_needed,
                       pool->max_len, pool->len);
        return 0;
    }

    if (pool->len < pool->min_len &&
        bytes_needed < pool->min_len - pool->len)
        /* to meet the min_len requirement */
        bytes_needed = pool->min_len - pool->len;

    /*
     * Make sure the buffer is large enough for the requested amount
     * of data. This guarantees that existing code patterns where
     * ossl_rand_pool_add_begin, ossl_rand_pool_add_end or ossl_rand_pool_add
     * are used to collect entropy data without any error handling
     * whatsoever, continue to be valid.
     * Furthermore if the allocation here fails once, make sure that
     * we don't fall back to a less secure or even blocking random source,
     * as that could happen by the existing code patterns.
     * This is not a concern for additional data, therefore that
     * is not needed if rand_pool_grow fails in other places.
     */
    if (!rand_pool_grow(pool, bytes_needed)) {
        /* persistent error for this pool */
        pool->max_len = pool->len = 0;
        return 0;
    }

    return bytes_needed;
}

/* Returns the remaining number of bytes available */
size_t ossl_rand_pool_bytes_remaining(RAND_POOL *pool)
{
    return pool->max_len - pool->len;
}

/*
 * Add random bytes to the random pool.
 *
 * It is expected that the |buffer| contains |len| bytes of
 * random input which contains at least |entropy| bits of
 * randomness.
 *
 * Returns 1 if the added amount is adequate, otherwise 0
 */
int ossl_rand_pool_add(RAND_POOL *pool,
                  const unsigned char *buffer, size_t len, size_t entropy)
{
    if (len > pool->max_len - pool->len) {
        ERR_raise(ERR_LIB_RAND, RAND_R_ENTROPY_INPUT_TOO_LONG);
        return 0;
    }

    if (pool->buffer == NULL) {
        ERR_raise(ERR_LIB_RAND, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (len > 0) {
        /*
         * This is to protect us from accidentally passing the buffer
         * returned from ossl_rand_pool_add_begin.
         * The check for alloc_len makes sure we do not compare the
         * address of the end of the allocated memory to something
         * different, since that comparison would have an
         * indeterminate result.
         */
        if (pool->alloc_len > pool->len && pool->buffer + pool->len == buffer) {
            ERR_raise(ERR_LIB_RAND, ERR_R_INTERNAL_ERROR);
            return 0;
        }
        /*
         * We have that only for cases when a pool is used to collect
         * additional data.
         * For entropy data, as long as the allocation request stays within
         * the limits given by ossl_rand_pool_bytes_needed this rand_pool_grow
         * below is guaranteed to succeed, thus no allocation happens.
         */
        if (!rand_pool_grow(pool, len))
            return 0;
        memcpy(pool->buffer + pool->len, buffer, len);
        pool->len += len;
        pool->entropy += entropy;
    }

    return 1;
}

/*
 * Start to add random bytes to the random pool in-place.
 *
 * Reserves the next |len| bytes for adding random bytes in-place
 * and returns a pointer to the buffer.
 * The caller is allowed to copy up to |len| bytes into the buffer.
 * If |len| == 0 this is considered a no-op and a NULL pointer
 * is returned without producing an error message.
 *
 * After updating the buffer, ossl_rand_pool_add_end() needs to be called
 * to finish the update operation (see next comment).
 */
unsigned char *ossl_rand_pool_add_begin(RAND_POOL *pool, size_t len)
{
    if (len == 0)
        return NULL;

    if (len > pool->max_len - pool->len) {
        ERR_raise(ERR_LIB_RAND, RAND_R_RANDOM_POOL_OVERFLOW);
        return NULL;
    }

    if (pool->buffer == NULL) {
        ERR_raise(ERR_LIB_RAND, ERR_R_INTERNAL_ERROR);
        return NULL;
    }

    /*
     * As long as the allocation request stays within the limits given
     * by ossl_rand_pool_bytes_needed this rand_pool_grow below is guaranteed
     * to succeed, thus no allocation happens.
     * We have that only for cases when a pool is used to collect
     * additional data. Then the buffer might need to grow here,
     * and of course the caller is responsible to check the return
     * value of this function.
     */
    if (!rand_pool_grow(pool, len))
        return NULL;

    return pool->buffer + pool->len;
}

/*
 * Finish to add random bytes to the random pool in-place.
 *
 * Finishes an in-place update of the random pool started by
 * ossl_rand_pool_add_begin() (see previous comment).
 * It is expected that |len| bytes of random input have been added
 * to the buffer which contain at least |entropy| bits of randomness.
 * It is allowed to add less bytes than originally reserved.
 */
int ossl_rand_pool_add_end(RAND_POOL *pool, size_t len, size_t entropy)
{
    if (len > pool->alloc_len - pool->len) {
        ERR_raise(ERR_LIB_RAND, RAND_R_RANDOM_POOL_OVERFLOW);
        return 0;
    }

    if (len > 0) {
        pool->len += len;
        pool->entropy += entropy;
    }

    return 1;
}

/**
 * @brief Mix in the additional input into an existing entropy in the pool
 *
 * @param pool     A RAND_POOL to mix the additional input in
 * @param adin     A buffer with the additional input
 * @param adin_len A length of the additional input
 *
 * @return 1 if there is any existing entropy in the pool so the additional input
 *         can be mixed in, 0 otherwise.
 */

int ossl_rand_pool_adin_mix_in(RAND_POOL *pool, const unsigned char *adin,
                               size_t adin_len)
{
    if (adin == NULL || adin_len == 0)
        /* Nothing to mix in -> success */
        return 1;

    if (pool->buffer == NULL) {
        ERR_raise(ERR_LIB_RAND, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (pool->len == 0) {
        ERR_raise(ERR_LIB_RAND, RAND_R_RANDOM_POOL_IS_EMPTY);
        return 0;
    }

    if (adin != NULL && adin_len > 0) {
        size_t i;

        /* xor the additional data into the pool */
        for (i = 0; i < adin_len; ++i)
            pool->buffer[i % pool->len] ^= adin[i];
    }

    return 1;
}
