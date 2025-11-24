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

#ifdef OPENSSL_RAND_SEED_RDTSC
/*
 * IMPORTANT NOTE:  It is not currently possible to use this code
 * because we are not sure about the amount of randomness it provides.
 * Some SP800-90B tests have been run, but there is internal skepticism.
 * So for now this code is not used.
 */
# error "RDTSC enabled?  Should not be possible!"

/*
 * Acquire entropy from high-speed clock
 *
 * Since we get some randomness from the low-order bits of the
 * high-speed clock, it can help.
 *
 * Returns the total entropy count, if it exceeds the requested
 * entropy count. Otherwise, returns an entropy count of 0.
 */
size_t ossl_prov_acquire_entropy_from_tsc(RAND_POOL *pool)
{
    unsigned char c;
    int i;

    if ((OPENSSL_ia32cap_P[0] & (1 << 4)) != 0) {
        for (i = 0; i < TSC_READ_COUNT; i++) {
            c = (unsigned char)(OPENSSL_rdtsc() & 0xFF);
            ossl_rand_pool_add(pool, &c, 1, 4);
        }
    }
    return ossl_rand_pool_entropy_available(pool);
}
#else
NON_EMPTY_TRANSLATION_UNIT
#endif
