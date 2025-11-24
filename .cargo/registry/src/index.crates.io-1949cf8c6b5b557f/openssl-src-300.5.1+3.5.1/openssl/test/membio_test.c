/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/bio.h>
#include "testutil.h"

#ifndef OPENSSL_NO_DGRAM
static int test_dgram(void)
{
    BIO *bio = BIO_new(BIO_s_dgram_mem()), *rbio = NULL;
    int testresult = 0;
    const char msg1[] = "12345656";
    const char msg2[] = "abcdefghijklmno";
    const char msg3[] = "ABCDEF";
    const char msg4[] = "FEDCBA";
    char buf[80];

    if (!TEST_ptr(bio))
        goto err;

    rbio = BIO_new_mem_buf(msg1, sizeof(msg1));
    if (!TEST_ptr(rbio))
        goto err;

    /* Setting the EOF return value on a non datagram mem BIO should be fine */
    if (!TEST_int_gt(BIO_set_mem_eof_return(rbio, 0), 0))
        goto err;

    /* Setting the EOF return value on a datagram mem BIO should fail */
    if (!TEST_int_le(BIO_set_mem_eof_return(bio, 0), 0))
        goto err;

    /* Write 4 dgrams */
    if (!TEST_int_eq(BIO_write(bio, msg1, sizeof(msg1)), sizeof(msg1)))
        goto err;
    if (!TEST_int_eq(BIO_write(bio, msg2, sizeof(msg2)), sizeof(msg2)))
        goto err;
    if (!TEST_int_eq(BIO_write(bio, msg3, sizeof(msg3)), sizeof(msg3)))
        goto err;
    if (!TEST_int_eq(BIO_write(bio, msg4, sizeof(msg4)), sizeof(msg4)))
        goto err;

    /* Reading all 4 dgrams out again should all be the correct size */
    if (!TEST_int_eq(BIO_read(bio, buf, sizeof(buf)), sizeof(msg1))
            || !TEST_mem_eq(buf, sizeof(msg1), msg1, sizeof(msg1))
            || !TEST_int_eq(BIO_read(bio, buf, sizeof(buf)), sizeof(msg2))
            || !TEST_mem_eq(buf, sizeof(msg2), msg2, sizeof(msg2))
            || !TEST_int_eq(BIO_read(bio, buf, sizeof(buf)), sizeof(msg3))
            || !TEST_mem_eq(buf, sizeof(msg3), msg3, sizeof(msg3))
            || !TEST_int_eq(BIO_read(bio, buf, sizeof(buf)), sizeof(msg4))
            || !TEST_mem_eq(buf, sizeof(msg4), msg4, sizeof(msg4)))
        goto err;

    /* Interleaving writes and reads should be fine */
    if (!TEST_int_eq(BIO_write(bio, msg1, sizeof(msg1)), sizeof(msg1)))
        goto err;
    if (!TEST_int_eq(BIO_write(bio, msg2, sizeof(msg2)), sizeof(msg2)))
        goto err;
    if (!TEST_int_eq(BIO_read(bio, buf, sizeof(buf)), sizeof(msg1))
            || !TEST_mem_eq(buf, sizeof(msg1), msg1, sizeof(msg1)))
        goto err;
    if (!TEST_int_eq(BIO_write(bio, msg3, sizeof(msg3)), sizeof(msg3)))
        goto err;
    if (!TEST_int_eq(BIO_read(bio, buf, sizeof(buf)), sizeof(msg2))
            || !TEST_mem_eq(buf, sizeof(msg2), msg2, sizeof(msg2))
            || !TEST_int_eq(BIO_read(bio, buf, sizeof(buf)), sizeof(msg3))
            || !TEST_mem_eq(buf, sizeof(msg3), msg3, sizeof(msg3)))
        goto err;

    /*
     * Requesting less than the available data in a dgram should not impact the
     * next packet.
     */
    if (!TEST_int_eq(BIO_write(bio, msg1, sizeof(msg1)), sizeof(msg1)))
        goto err;
    if (!TEST_int_eq(BIO_write(bio, msg2, sizeof(msg2)), sizeof(msg2)))
        goto err;
    if (!TEST_int_eq(BIO_read(bio, buf, /* Short buffer */ 2), 2)
            || !TEST_mem_eq(buf, 2, msg1, 2))
        goto err;
    if (!TEST_int_eq(BIO_read(bio, buf, sizeof(buf)), sizeof(msg2))
            || !TEST_mem_eq(buf, sizeof(msg2), msg2, sizeof(msg2)))
        goto err;

    /*
     * Writing a zero length datagram will return zero, but no datagrams will
     * be written. Attempting to read when there are no datagrams to read should
     * return a negative result, but not eof. Retry flags will be set.
     */
    if (!TEST_int_eq(BIO_write(bio, NULL, 0), 0)
            || !TEST_int_lt(BIO_read(bio, buf, sizeof(buf)), 0)
            || !TEST_false(BIO_eof(bio))
            || !TEST_true(BIO_should_retry(bio)))
        goto err;

    if (!TEST_int_eq(BIO_dgram_set_mtu(bio, 123456), 1)
            || !TEST_int_eq(BIO_dgram_get_mtu(bio), 123456))
        goto err;

    testresult = 1;
 err:
    BIO_free(rbio);
    BIO_free(bio);
    return testresult;
}
#endif

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

#ifndef OPENSSL_NO_DGRAM
    ADD_TEST(test_dgram);
#endif

    return 1;
}
