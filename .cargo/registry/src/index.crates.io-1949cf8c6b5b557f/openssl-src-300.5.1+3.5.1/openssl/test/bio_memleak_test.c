/*
 * Copyright 2018-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <stdio.h>
#include <string.h>
#include <openssl/buffer.h>
#include <openssl/bio.h>

#include "testutil.h"

static int test_bio_memleak(void)
{
    int ok = 0;
    BIO *bio;
    BUF_MEM bufmem;
    static const char str[] = "BIO test\n";
    char buf[100];

    bio = BIO_new(BIO_s_mem());
    if (!TEST_ptr(bio))
        goto finish;
    bufmem.length = sizeof(str);
    bufmem.data = (char *) str;
    bufmem.max = bufmem.length;
    BIO_set_mem_buf(bio, &bufmem, BIO_NOCLOSE);
    BIO_set_flags(bio, BIO_FLAGS_MEM_RDONLY);
    if (!TEST_int_eq(BIO_read(bio, buf, sizeof(buf)), sizeof(str)))
        goto finish;
    if (!TEST_mem_eq(buf, sizeof(str), str, sizeof(str)))
        goto finish;
    ok = 1;

 finish:
    BIO_free(bio);
    return ok;
}

static int test_bio_get_mem(void)
{
    int ok = 0;
    BIO *bio = NULL;
    BUF_MEM *bufmem = NULL;

    bio = BIO_new(BIO_s_mem());
    if (!TEST_ptr(bio))
        goto finish;
    if (!TEST_int_eq(BIO_puts(bio, "Hello World\n"), 12))
        goto finish;
    BIO_get_mem_ptr(bio, &bufmem);
    if (!TEST_ptr(bufmem))
        goto finish;
    if (!TEST_int_gt(BIO_set_close(bio, BIO_NOCLOSE), 0))
        goto finish;
    BIO_free(bio);
    bio = NULL;
    if (!TEST_mem_eq(bufmem->data, bufmem->length, "Hello World\n", 12))
        goto finish;
    ok = 1;

 finish:
    BIO_free(bio);
    BUF_MEM_free(bufmem);
    return ok;
}

static int test_bio_new_mem_buf(void)
{
    int ok = 0;
    BIO *bio;
    BUF_MEM *bufmem;
    char data[16];

    bio = BIO_new_mem_buf("Hello World\n", 12);
    if (!TEST_ptr(bio))
        goto finish;
    if (!TEST_int_eq(BIO_read(bio, data, 5), 5))
        goto finish;
    if (!TEST_mem_eq(data, 5, "Hello", 5))
        goto finish;
    if (!TEST_int_gt(BIO_get_mem_ptr(bio, &bufmem), 0))
        goto finish;
    if (!TEST_int_lt(BIO_write(bio, "test", 4), 0))
        goto finish;
    if (!TEST_int_eq(BIO_read(bio, data, 16), 7))
        goto finish;
    if (!TEST_mem_eq(data, 7, " World\n", 7))
        goto finish;
    if (!TEST_int_gt(BIO_reset(bio), 0))
        goto finish;
    if (!TEST_int_eq(BIO_read(bio, data, 16), 12))
        goto finish;
    if (!TEST_mem_eq(data, 12, "Hello World\n", 12))
        goto finish;
    ok = 1;

 finish:
    BIO_free(bio);
    return ok;
}

static int test_bio_rdonly_mem_buf(void)
{
    int ok = 0;
    BIO *bio, *bio2 = NULL;
    BUF_MEM *bufmem;
    char data[16];

    bio = BIO_new_mem_buf("Hello World\n", 12);
    if (!TEST_ptr(bio))
        goto finish;
    if (!TEST_int_eq(BIO_read(bio, data, 5), 5))
        goto finish;
    if (!TEST_mem_eq(data, 5, "Hello", 5))
        goto finish;
    if (!TEST_int_gt(BIO_get_mem_ptr(bio, &bufmem), 0))
        goto finish;
    (void)BIO_set_close(bio, BIO_NOCLOSE);

    bio2 = BIO_new(BIO_s_mem());
    if (!TEST_ptr(bio2))
        goto finish;
    BIO_set_mem_buf(bio2, bufmem, BIO_CLOSE);
    BIO_set_flags(bio2, BIO_FLAGS_MEM_RDONLY);

    if (!TEST_int_eq(BIO_read(bio2, data, 16), 7))
        goto finish;
    if (!TEST_mem_eq(data, 7, " World\n", 7))
        goto finish;
    if (!TEST_int_gt(BIO_reset(bio2), 0))
        goto finish;
    if (!TEST_int_eq(BIO_read(bio2, data, 16), 7))
        goto finish;
    if (!TEST_mem_eq(data, 7, " World\n", 7))
        goto finish;
    ok = 1;

 finish:
    BIO_free(bio);
    BIO_free(bio2);
    return ok;
}

static int test_bio_rdwr_rdonly(void)
{
    int ok = 0;
    BIO *bio = NULL;
    char data[16];

    bio = BIO_new(BIO_s_mem());
    if (!TEST_ptr(bio))
        goto finish;
    if (!TEST_int_eq(BIO_puts(bio, "Hello World\n"), 12))
        goto finish;

    BIO_set_flags(bio, BIO_FLAGS_MEM_RDONLY);
    if (!TEST_int_eq(BIO_read(bio, data, 16), 12))
        goto finish;
    if (!TEST_mem_eq(data, 12, "Hello World\n", 12))
        goto finish;
    if (!TEST_int_gt(BIO_reset(bio), 0))
        goto finish;

    BIO_clear_flags(bio, BIO_FLAGS_MEM_RDONLY);
    if (!TEST_int_eq(BIO_puts(bio, "Hi!\n"), 4))
        goto finish;
    if (!TEST_int_eq(BIO_read(bio, data, 16), 16))
        goto finish;

    if (!TEST_mem_eq(data, 16, "Hello World\nHi!\n", 16))
        goto finish;

    ok = 1;

 finish:
    BIO_free(bio);
    return ok;
}

static int test_bio_nonclear_rst(void)
{
    int ok = 0;
    BIO *bio = NULL;
    char data[16];

    bio = BIO_new(BIO_s_mem());
    if (!TEST_ptr(bio))
        goto finish;
    if (!TEST_int_eq(BIO_puts(bio, "Hello World\n"), 12))
        goto finish;

    BIO_set_flags(bio, BIO_FLAGS_NONCLEAR_RST);

    if (!TEST_int_eq(BIO_read(bio, data, 16), 12))
        goto finish;
    if (!TEST_mem_eq(data, 12, "Hello World\n", 12))
        goto finish;
    if (!TEST_int_gt(BIO_reset(bio), 0))
        goto finish;

    if (!TEST_int_eq(BIO_read(bio, data, 16), 12))
        goto finish;
    if (!TEST_mem_eq(data, 12, "Hello World\n", 12))
        goto finish;

    BIO_clear_flags(bio, BIO_FLAGS_NONCLEAR_RST);
    if (!TEST_int_gt(BIO_reset(bio), 0))
        goto finish;

    if (!TEST_int_lt(BIO_read(bio, data, 16), 1))
        goto finish;

    ok = 1;

 finish:
    BIO_free(bio);
    return ok;
}

static int error_callback_fired;
static long BIO_error_callback(BIO *bio, int cmd, const char *argp,
                               size_t len, int argi,
                               long argl, int ret, size_t *processed)
{
    if ((cmd & (BIO_CB_READ | BIO_CB_RETURN)) != 0) {
        error_callback_fired = 1;
        ret = 0;  /* fail for read operations to simulate error in input BIO */
    }
    return ret;
}

/* Checks i2d_ASN1_bio_stream() is freeing all memory when input BIO ends unexpectedly. */
static int test_bio_i2d_ASN1_mime(void)
{
    int ok = 0;
    BIO *bio = NULL, *out = NULL;
    BUF_MEM bufmem;
    static const char str[] = "BIO mime test\n";
    PKCS7 *p7 = NULL;

    if (!TEST_ptr(bio = BIO_new(BIO_s_mem())))
        goto finish;

    bufmem.length = sizeof(str);
    bufmem.data = (char *) str;
    bufmem.max = bufmem.length;
    BIO_set_mem_buf(bio, &bufmem, BIO_NOCLOSE);
    BIO_set_flags(bio, BIO_FLAGS_MEM_RDONLY);
    BIO_set_callback_ex(bio, BIO_error_callback);

    if (!TEST_ptr(out = BIO_new(BIO_s_mem())))
        goto finish;
    if (!TEST_ptr(p7 = PKCS7_new()))
        goto finish;
    if (!TEST_true(PKCS7_set_type(p7, NID_pkcs7_data)))
        goto finish;

    error_callback_fired = 0;

    if (!TEST_false(i2d_ASN1_bio_stream(out, (ASN1_VALUE*) p7, bio,
                                        SMIME_STREAM | SMIME_BINARY,
                                        ASN1_ITEM_rptr(PKCS7))))
        goto finish;

    if (!TEST_int_eq(error_callback_fired, 1))
        goto finish;

    ok = 1;

 finish:
    BIO_free(bio);
    BIO_free(out);
    PKCS7_free(p7);
    return ok;
}

int setup_tests(void)
{
    ADD_TEST(test_bio_memleak);
    ADD_TEST(test_bio_get_mem);
    ADD_TEST(test_bio_new_mem_buf);
    ADD_TEST(test_bio_rdonly_mem_buf);
    ADD_TEST(test_bio_rdwr_rdonly);
    ADD_TEST(test_bio_nonclear_rst);
    ADD_TEST(test_bio_i2d_ASN1_mime);
    return 1;
}
