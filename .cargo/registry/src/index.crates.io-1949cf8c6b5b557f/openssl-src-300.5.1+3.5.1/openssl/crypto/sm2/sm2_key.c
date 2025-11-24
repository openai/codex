/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h" /* to be able to use EC_KEY and EC_GROUP */

#include <openssl/err.h>
#include "crypto/sm2err.h"
#include "crypto/sm2.h"
#include <openssl/ec.h> /* EC_KEY and EC_GROUP functions */

/*
 * SM2 key generation is implemented within ec_generate_key() in
 * crypto/ec/ec_key.c
 */

int ossl_sm2_key_private_check(const EC_KEY *eckey)
{
    int ret = 0;
    BIGNUM *max = NULL;
    const EC_GROUP *group = NULL;
    const BIGNUM *priv_key = NULL, *order = NULL;

    if (eckey == NULL
            || (group = EC_KEY_get0_group(eckey)) == NULL
            || (priv_key = EC_KEY_get0_private_key(eckey)) == NULL
            || (order = EC_GROUP_get0_order(group)) == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /* range of SM2 private key is [1, n-1) */
    max = BN_dup(order);
    if (max == NULL || !BN_sub_word(max, 1))
        goto end;
    if (BN_cmp(priv_key, BN_value_one()) < 0
        || BN_cmp(priv_key, max) >= 0) {
        ERR_raise(ERR_LIB_SM2, SM2_R_INVALID_PRIVATE_KEY);
        goto end;
    }
    ret = 1;

 end:
    BN_free(max);
    return ret;
}
