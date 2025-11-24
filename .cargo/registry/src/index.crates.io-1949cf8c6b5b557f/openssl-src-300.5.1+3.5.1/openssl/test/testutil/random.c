/*
 * Copyright 2019 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "../testutil.h"

/*
 * This is an implementation of the algorithm used by the GNU C library's
 * random(3) pseudorandom number generator as described:
 *      https://www.mscs.dal.ca/~selinger/random/
 */
static uint32_t test_random_state[31];

uint32_t test_random(void) {
    static unsigned int pos = 3;

    if (pos == 31)
        pos = 0;
    test_random_state[pos] += test_random_state[(pos + 28) % 31];
    return test_random_state[pos++] / 2;
}

void test_random_seed(uint32_t sd) {
    int i;
    int32_t s;
    const unsigned int mod = (1u << 31) - 1;

    test_random_state[0] = sd;
    for (i = 1; i < 31; i++) {
        s = (int32_t)test_random_state[i - 1];
        test_random_state[i] = (uint32_t)((16807 * (int64_t)s) % mod);
    }
    for (i = 34; i < 344; i++)
        test_random();
}
