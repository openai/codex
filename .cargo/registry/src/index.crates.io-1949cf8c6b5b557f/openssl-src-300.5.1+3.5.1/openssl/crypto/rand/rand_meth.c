/*
 * Copyright 2011-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/rand.h>
#include "rand_local.h"

/* Implements the default OpenSSL RAND_add() method */
static int drbg_add(const void *buf, int num, double randomness)
{
    EVP_RAND_CTX *drbg = RAND_get0_primary(NULL);

    if (drbg == NULL || num <= 0)
        return 0;

    return EVP_RAND_reseed(drbg, 0, NULL, 0, buf, num);
}

/* Implements the default OpenSSL RAND_seed() method */
static int drbg_seed(const void *buf, int num)
{
    return drbg_add(buf, num, num);
}

/* Implements the default OpenSSL RAND_status() method */
static int drbg_status(void)
{
    EVP_RAND_CTX *drbg = RAND_get0_primary(NULL);

    if (drbg == NULL)
        return 0;

    return  EVP_RAND_get_state(drbg) == EVP_RAND_STATE_READY ? 1 : 0;
}

/* Implements the default OpenSSL RAND_bytes() method */
static int drbg_bytes(unsigned char *out, int count)
{
    EVP_RAND_CTX *drbg = RAND_get0_public(NULL);

    if (drbg == NULL)
        return 0;

    return EVP_RAND_generate(drbg, out, count, 0, 0, NULL, 0);
}

RAND_METHOD ossl_rand_meth = {
    drbg_seed,
    drbg_bytes,
    NULL,
    drbg_add,
    drbg_bytes,
    drbg_status
};

RAND_METHOD *RAND_OpenSSL(void)
{
    return &ossl_rand_meth;
}
