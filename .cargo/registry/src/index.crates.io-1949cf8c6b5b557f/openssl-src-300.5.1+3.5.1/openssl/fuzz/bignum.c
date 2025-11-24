/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

/*
 * Confirm that a^b mod c agrees when calculated cleverly vs naively, for
 * random a, b and c.
 */

#include <stdio.h>
#include <openssl/bn.h>
#include <openssl/err.h>
#include "fuzzer.h"


int FuzzerInitialize(int *argc, char ***argv)
{
    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS, NULL);
    ERR_clear_error();

    return 1;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    int success = 0;
    size_t l1 = 0, l2 = 0, l3 = 0;
    int s1 = 0, s3 = 0;
    BN_CTX *ctx;
    BIGNUM *b1;
    BIGNUM *b2;
    BIGNUM *b3;
    BIGNUM *b4;
    BIGNUM *b5;

    b1 = BN_new();
    b2 = BN_new();
    b3 = BN_new();
    b4 = BN_new();
    b5 = BN_new();
    ctx = BN_CTX_new();

    /* Divide the input into three parts, using the values of the first two
     * bytes to choose lengths, which generate b1, b2 and b3. Use three bits
     * of the third byte to choose signs for the three numbers.
     */
    if (len > 2) {
        len -= 3;
        /* limit l1, l2, and l3 to be no more than 512 bytes */
        l1 = ((buf[0] * len) / 255) % 512;
        ++buf;
        l2 = ((buf[0] * (len - l1)) / 255) % 512;
        ++buf;
        l3 = (len - l1 - l2) % 512;

        s1 = buf[0] & 1;
        s3 = buf[0] & 4;
        ++buf;
    }
    OPENSSL_assert(BN_bin2bn(buf, l1, b1) == b1);
    BN_set_negative(b1, s1);
    OPENSSL_assert(BN_bin2bn(buf + l1, l2, b2) == b2);
    OPENSSL_assert(BN_bin2bn(buf + l1 + l2, l3, b3) == b3);
    BN_set_negative(b3, s3);

    /* mod 0 is undefined */
    if (BN_is_zero(b3)) {
        success = 1;
        goto done;
    }

    OPENSSL_assert(BN_mod_exp(b4, b1, b2, b3, ctx));
    OPENSSL_assert(BN_mod_exp_simple(b5, b1, b2, b3, ctx));

    success = BN_cmp(b4, b5) == 0;
    if (!success) {
        BN_print_fp(stdout, b1);
        putchar('\n');
        BN_print_fp(stdout, b2);
        putchar('\n');
        BN_print_fp(stdout, b3);
        putchar('\n');
        BN_print_fp(stdout, b4);
        putchar('\n');
        BN_print_fp(stdout, b5);
        putchar('\n');
    }

 done:
    OPENSSL_assert(success);
    BN_free(b1);
    BN_free(b2);
    BN_free(b3);
    BN_free(b4);
    BN_free(b5);
    BN_CTX_free(ctx);
    ERR_clear_error();

    return 0;
}

void FuzzerCleanup(void)
{
}
