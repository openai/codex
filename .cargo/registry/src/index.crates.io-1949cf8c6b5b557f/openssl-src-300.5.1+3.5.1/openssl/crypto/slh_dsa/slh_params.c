/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <stddef.h>
#include <string.h>
#include "slh_params.h"
#include <openssl/obj_mac.h>

/* H(), T() use this to calculate the number of zeros for security cat 3 & 5 */
#define OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND2 128

/* See FIPS 205 Section 11 Table 2 (n h d h` a k m sc pk sig*/
#define OSSL_SLH_DSA_128S_N 16
#define OSSL_SLH_DSA_128S_D 7
#define OSSL_SLH_DSA_128S_H_DASH 9
#define OSSL_SLH_DSA_128S_H (OSSL_SLH_DSA_128S_D * OSSL_SLH_DSA_128S_H_DASH)
#define OSSL_SLH_DSA_128S_A 12
#define OSSL_SLH_DSA_128S_K 14
#define OSSL_SLH_DSA_128S_M 30
#define OSSL_SLH_DSA_128S_SECURITY_CATEGORY 1
#define OSSL_SLH_DSA_128S_PUB_BYTES 32
#define OSSL_SLH_DSA_128S_SIG_BYTES 7856

#define OSSL_SLH_DSA_128F_N 16
#define OSSL_SLH_DSA_128F_D 22
#define OSSL_SLH_DSA_128F_H_DASH 3
#define OSSL_SLH_DSA_128F_H (OSSL_SLH_DSA_128F_D * OSSL_SLH_DSA_128F_H_DASH)
#define OSSL_SLH_DSA_128F_A 6
#define OSSL_SLH_DSA_128F_K 33
#define OSSL_SLH_DSA_128F_M 34
#define OSSL_SLH_DSA_128F_SECURITY_CATEGORY 1
#define OSSL_SLH_DSA_128F_PUB_BYTES 32
#define OSSL_SLH_DSA_128F_SIG_BYTES 17088

#define OSSL_SLH_DSA_192S_N 24
#define OSSL_SLH_DSA_192S_D 7
#define OSSL_SLH_DSA_192S_H_DASH 9
#define OSSL_SLH_DSA_192S_H (OSSL_SLH_DSA_192S_D * OSSL_SLH_DSA_192S_H_DASH)
#define OSSL_SLH_DSA_192S_A 14
#define OSSL_SLH_DSA_192S_K 17
#define OSSL_SLH_DSA_192S_M 39
#define OSSL_SLH_DSA_192S_SECURITY_CATEGORY 3
#define OSSL_SLH_DSA_192S_PUB_BYTES 48
#define OSSL_SLH_DSA_192S_SIG_BYTES 16224

#define OSSL_SLH_DSA_192F_N 24
#define OSSL_SLH_DSA_192F_D 22
#define OSSL_SLH_DSA_192F_H_DASH 3
#define OSSL_SLH_DSA_192F_H (OSSL_SLH_DSA_192F_D * OSSL_SLH_DSA_192F_H_DASH)
#define OSSL_SLH_DSA_192F_A 8
#define OSSL_SLH_DSA_192F_K 33
#define OSSL_SLH_DSA_192F_M 42
#define OSSL_SLH_DSA_192F_SECURITY_CATEGORY 3
#define OSSL_SLH_DSA_192F_PUB_BYTES 48
#define OSSL_SLH_DSA_192F_SIG_BYTES 35664

#define OSSL_SLH_DSA_256S_N 32
#define OSSL_SLH_DSA_256S_D 8
#define OSSL_SLH_DSA_256S_H_DASH 8
#define OSSL_SLH_DSA_256S_H (OSSL_SLH_DSA_256S_D * OSSL_SLH_DSA_256S_H_DASH)
#define OSSL_SLH_DSA_256S_A 14
#define OSSL_SLH_DSA_256S_K 22
#define OSSL_SLH_DSA_256S_M 47
#define OSSL_SLH_DSA_256S_SECURITY_CATEGORY 5
#define OSSL_SLH_DSA_256S_PUB_BYTES 64
#define OSSL_SLH_DSA_256S_SIG_BYTES 29792

#define OSSL_SLH_DSA_256F_N 32
#define OSSL_SLH_DSA_256F_D 17
#define OSSL_SLH_DSA_256F_H_DASH 4
#define OSSL_SLH_DSA_256F_H (OSSL_SLH_DSA_256F_D * OSSL_SLH_DSA_256F_H_DASH)
#define OSSL_SLH_DSA_256F_A 9
#define OSSL_SLH_DSA_256F_K 35
#define OSSL_SLH_DSA_256F_M 49
#define OSSL_SLH_DSA_256F_SECURITY_CATEGORY 5
#define OSSL_SLH_DSA_256F_PUB_BYTES 64
#define OSSL_SLH_DSA_256F_SIG_BYTES 49856

#define OSSL_SLH_PARAMS(name)                   \
    OSSL_SLH_DSA_##name##_N,                    \
    OSSL_SLH_DSA_##name##_H,                    \
    OSSL_SLH_DSA_##name##_D,                    \
    OSSL_SLH_DSA_##name##_H_DASH,               \
    OSSL_SLH_DSA_##name##_A,                    \
    OSSL_SLH_DSA_##name##_K,                    \
    OSSL_SLH_DSA_##name##_M,                    \
    OSSL_SLH_DSA_##name##_SECURITY_CATEGORY,    \
    OSSL_SLH_DSA_##name##_PUB_BYTES,            \
    OSSL_SLH_DSA_##name##_SIG_BYTES             \


static const SLH_DSA_PARAMS slh_dsa_params[] = {
    {"SLH-DSA-SHA2-128s",  NID_SLH_DSA_SHA2_128s,  0, OSSL_SLH_PARAMS(128S), OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND1},
    {"SLH-DSA-SHAKE-128s", NID_SLH_DSA_SHAKE_128s, 1, OSSL_SLH_PARAMS(128S)},
    {"SLH-DSA-SHA2-128f",  NID_SLH_DSA_SHA2_128f,  0, OSSL_SLH_PARAMS(128F), OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND1},
    {"SLH-DSA-SHAKE-128f", NID_SLH_DSA_SHAKE_128f, 1, OSSL_SLH_PARAMS(128F)},
    {"SLH-DSA-SHA2-192s",  NID_SLH_DSA_SHA2_192s,  0, OSSL_SLH_PARAMS(192S), OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND2},
    {"SLH-DSA-SHAKE-192s", NID_SLH_DSA_SHAKE_192s, 1, OSSL_SLH_PARAMS(192S)},
    {"SLH-DSA-SHA2-192f",  NID_SLH_DSA_SHA2_192f,  0, OSSL_SLH_PARAMS(192F), OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND2},
    {"SLH-DSA-SHAKE-192f", NID_SLH_DSA_SHAKE_192f, 1, OSSL_SLH_PARAMS(192F)},
    {"SLH-DSA-SHA2-256s",  NID_SLH_DSA_SHA2_256s,  0, OSSL_SLH_PARAMS(256S), OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND2},
    {"SLH-DSA-SHAKE-256s", NID_SLH_DSA_SHAKE_256s, 1, OSSL_SLH_PARAMS(256S)},
    {"SLH-DSA-SHA2-256f",  NID_SLH_DSA_SHA2_256f,  0, OSSL_SLH_PARAMS(256F), OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND2},
    {"SLH-DSA-SHAKE-256f", NID_SLH_DSA_SHAKE_256f, 1, OSSL_SLH_PARAMS(256F)},
    {NULL},
};

/**
 * @brief A getter to convert an algorithm name into a SLH_DSA_PARAMS object
 */
const SLH_DSA_PARAMS *ossl_slh_dsa_params_get(const char *alg)
{
    const SLH_DSA_PARAMS *p;

    if (alg == NULL)
        return NULL;
    for (p = slh_dsa_params; p->alg != NULL; ++p) {
        if (strcmp(p->alg, alg) == 0)
            return p;
    }
    return NULL;
}
