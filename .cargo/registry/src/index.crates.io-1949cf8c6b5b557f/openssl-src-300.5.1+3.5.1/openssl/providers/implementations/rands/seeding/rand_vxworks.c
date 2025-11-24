/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/opensslconf.h>

#include <openssl/rand.h>
#include "crypto/rand_pool.h"
#include "crypto/rand.h"
#include "internal/cryptlib.h"
#include "prov/seeding.h"
#include <version.h>
#include <taskLib.h>

#if defined(OPENSSL_RAND_SEED_NONE)
/* none means none */
# undef OPENSSL_RAND_SEED_OS
#endif

#if defined(OPENSSL_RAND_SEED_OS)
# if _WRS_VXWORKS_MAJOR >= 7
#   define RAND_SEED_VXRANDLIB
# else
#   error "VxWorks <7 only support RAND_SEED_NONE"
# endif
#endif

#if defined(RAND_SEED_VXRANDLIB)
# include <randomNumGen.h>
#endif

/* Macro to convert two thirty two bit values into a sixty four bit one */
#define TWO32TO64(a, b) ((((uint64_t)(a)) << 32) + (b))

static uint64_t get_time_stamp(void)
{
    struct timespec ts;

    if (clock_gettime(CLOCK_REALTIME, &ts) == 0)
        return TWO32TO64(ts.tv_sec, ts.tv_nsec);
    return time(NULL);
}

static uint64_t get_timer_bits(void)
{
    uint64_t res = OPENSSL_rdtsc();
    struct timespec ts;

    if (res != 0)
        return res;

    if (clock_gettime(CLOCK_MONOTONIC, &ts) == 0)
        return TWO32TO64(ts.tv_sec, ts.tv_nsec);
    return time(NULL);
}

/*
 * empty implementation
 * vxworks does not need to init/cleanup or keep open the random lib
 */
int ossl_rand_pool_init(void)
{
    return 1;
}

void ossl_rand_pool_cleanup(void)
{
}

void ossl_rand_pool_keep_random_devices_open(int keep)
{
}

int ossl_pool_add_nonce_data(RAND_POOL *pool)
{
    struct {
        pid_t pid;
        CRYPTO_THREAD_ID tid;
        uint64_t time;
    } data;

    memset(&data, 0, sizeof(data));

    /*
     * Add process id, thread id, and a high resolution timestamp to
     * ensure that the nonce is unique with high probability for
     * different process instances.
     */
    data.pid = getpid();
    data.tid = CRYPTO_THREAD_get_current_id();
    data.time = get_time_stamp();

    return ossl_rand_pool_add(pool, (unsigned char *)&data, sizeof(data), 0);
}

size_t ossl_pool_acquire_entropy(RAND_POOL *pool)
{
#if defined(RAND_SEED_VXRANDLIB)
    /* vxRandLib based entropy method */
    size_t bytes_needed;

    bytes_needed = ossl_rand_pool_bytes_needed(pool, 1 /*entropy_factor*/);
    if (bytes_needed > 0) {
        int retryCount = 0;
        STATUS result = ERROR;
        unsigned char *buffer;

        buffer = ossl_rand_pool_add_begin(pool, bytes_needed);
        while ((result != OK) && (retryCount < 10)) {
            RANDOM_NUM_GEN_STATUS status = randStatus();

            if ((status == RANDOM_NUM_GEN_ENOUGH_ENTROPY)
                    || (status == RANDOM_NUM_GEN_MAX_ENTROPY)) {
                result = randBytes(buffer, bytes_needed);
                if (result == OK)
                    ossl_rand_pool_add_end(pool, bytes_needed, 8 * bytes_needed);
                /*
                 * no else here: randStatus said ok, if randBytes failed
                 * it will result in another loop or no entropy
                 */
            } else {
                /*
                 * give a minimum delay here to allow OS to collect more
                 * entropy. taskDelay duration will depend on the system tick,
                 * this is by design as the sw-random lib uses interrupts
                 * which will at least happen during ticks
                 */
                taskDelay(5);
            }
            retryCount++;
        }
    }
    return ossl_rand_pool_entropy_available(pool);
#else
    /*
     * SEED_NONE means none, without randlib we dont have entropy and
     * rely on it being added externally
     */
    return ossl_rand_pool_entropy_available(pool);
#endif /* defined(RAND_SEED_VXRANDLIB) */
}
