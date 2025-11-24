/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/e_os2.h>

/*
 * PRF(), F() use this value to calculate the number of zeros
 * H(), T() use this to calculate the number of zeros for security cat 1
 */
#define OSSL_SLH_DSA_SHA2_NUM_ZEROS_H_AND_T_BOUND1 64

/*
 * Refer to FIPS 205 Section 11 parameter sets.
 * lgw has been omitted since it is 4 for all algorithms i.e. log(16)
 */
typedef struct slh_dsa_params_st {
    const char *alg;
    int type;
    int is_shake;
    uint32_t n;  /* Security parameter (Hash output size in bytes) (16, 24, 32) */
    uint32_t h;  /* The total height of the tree (63, 64, 66, 68). #keypairs = 2^h */
    uint32_t d;  /* The number of tree layers (7, 8, 17, 22) */
    uint32_t hm; /* The height (h') of each merkle tree. (h = hm * d ) */
    uint32_t a;  /* Height of a FORS tree */
    uint32_t k;  /* The number of FORS trees */
    uint32_t m;  /* The size of H_MSG() output */
    uint32_t security_category;
    uint32_t pk_len;
    uint32_t sig_len;
    size_t sha2_h_and_t_bound;
} SLH_DSA_PARAMS;

const SLH_DSA_PARAMS *ossl_slh_dsa_params_get(const char *alg);
