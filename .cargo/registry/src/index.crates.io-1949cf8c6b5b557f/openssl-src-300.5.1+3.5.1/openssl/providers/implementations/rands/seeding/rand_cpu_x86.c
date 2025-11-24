/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include <openssl/opensslconf.h>
#include "crypto/rand_pool.h"
#include "prov/seeding.h"

#ifdef OPENSSL_RAND_SEED_RDCPU
# if defined(OPENSSL_SYS_TANDEM) && defined(_TNS_X_TARGET)
#  include <builtin.h> /* _rdrand64 */
#  include <string.h> /* memcpy */
# else
size_t OPENSSL_ia32_rdseed_bytes(unsigned char *buf, size_t len);
size_t OPENSSL_ia32_rdrand_bytes(unsigned char *buf, size_t len);
# endif

static size_t get_hardware_random_value(unsigned char *buf, size_t len);

/*
 * Acquire entropy using Intel-specific cpu instructions
 *
 * Uses the RDSEED instruction if available, otherwise uses
 * RDRAND if available.
 *
 * For the differences between RDSEED and RDRAND, and why RDSEED
 * is the preferred choice, see https://goo.gl/oK3KcN
 *
 * Returns the total entropy count, if it exceeds the requested
 * entropy count. Otherwise, returns an entropy count of 0.
 */
size_t ossl_prov_acquire_entropy_from_cpu(RAND_POOL *pool)
{
    size_t bytes_needed;
    unsigned char *buffer;

    bytes_needed = ossl_rand_pool_bytes_needed(pool, 1 /*entropy_factor*/);
    if (bytes_needed > 0) {
        buffer = ossl_rand_pool_add_begin(pool, bytes_needed);

        if (buffer != NULL) {
            if (get_hardware_random_value(buffer, bytes_needed) == bytes_needed) {
                ossl_rand_pool_add_end(pool, bytes_needed, 8 * bytes_needed);
            } else {
                ossl_rand_pool_add_end(pool, 0, 0);
            }
        }
    }

    return ossl_rand_pool_entropy_available(pool);
}

#if defined(OPENSSL_SYS_TANDEM) && defined(_TNS_X_TARGET)
/* Obtain random bytes from the x86 hardware random function in 64 bit chunks */
static size_t get_hardware_random_value(unsigned char *buf, size_t len)
{
    size_t bytes_remaining = len;

    while (bytes_remaining > 0) {
        /* Always use 64 bit fetch, then use the lower bytes as needed. */
        /* The platform is big-endian. */
        uint64_t random_value = 0;

        if (_rdrand64(&random_value) != 0) {
            unsigned char *random_buffer = (unsigned char *)&random_value;

            if (bytes_remaining >= sizeof(random_value)) {
                memcpy(buf, random_buffer, sizeof(random_value));
                bytes_remaining -= sizeof(random_value);
                buf += sizeof(random_value);
            } else {
                memcpy(buf,
                    random_buffer + (sizeof(random_value) - bytes_remaining),
                    bytes_remaining);
                bytes_remaining = 0; /* This will terminate the loop */
            }
        } else
            break;
    }
    if (bytes_remaining == 0)
        return len;
    return 0;
}
#else
static size_t get_hardware_random_value(unsigned char *buf, size_t len) {
    /* Whichever comes first, use RDSEED, RDRAND or nothing */
    if ((OPENSSL_ia32cap_P[2] & (1 << 18)) != 0) {
	if (OPENSSL_ia32_rdseed_bytes(buf, len) != len)
	    return 0;
    } else if ((OPENSSL_ia32cap_P[1] & (1 << (62 - 32))) != 0) {
	if (OPENSSL_ia32_rdrand_bytes(buf, len) != len)
	    return 0;
    } else
	return 0;
    return len;
}
#endif

#else
NON_EMPTY_TRANSLATION_UNIT
#endif
