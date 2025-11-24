/*
 * Copyright 2021 The OpenSSL Project Authors. All Rights Reserved.
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
#include "crypto/arm_arch.h"

size_t OPENSSL_rndrrs_bytes(unsigned char *buf, size_t len);

static size_t get_hardware_random_value(unsigned char *buf, size_t len);

/*
 * Acquire entropy using Arm-specific cpu instructions
 *
 * Uses the RNDRRS instruction. RNDR is never needed since
 * RNDRRS will always be available if RNDR is an available
 * instruction.
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
            if (get_hardware_random_value(buffer, bytes_needed) == bytes_needed)
                ossl_rand_pool_add_end(pool, bytes_needed, 8 * bytes_needed);
            else
                ossl_rand_pool_add_end(pool, 0, 0);
        }
    }

    return ossl_rand_pool_entropy_available(pool);
}

static size_t get_hardware_random_value(unsigned char *buf, size_t len)
{
    /* Always use RNDRRS or nothing */
    if (OPENSSL_armcap_P & ARMV8_RNG)  {
        if (OPENSSL_rndrrs_bytes(buf, len) != len)
            return 0;
    } else {
        return 0;
    }
    return len;
}

#else
NON_EMPTY_TRANSLATION_UNIT
#endif /* OPENSSL_RAND_SEED_RDCPU */
