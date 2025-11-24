/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <stdio.h>
#include <string.h>
#include <openssl/evp.h>
#include <openssl/bio.h>
#include <openssl/rand.h>
#include <openssl/comp.h>

#include "testutil.h"
#include "testutil/output.h"
#include "testutil/tu_local.h"

#define COMPRESS  1
#define EXPAND    0

#define BUFFER_SIZE    32 * 1024
#define NUM_SIZES      4
static int sizes[NUM_SIZES] = { 64, 512, 2048, 16 * 1024 };

/* using global buffers */
static unsigned char *original = NULL;
static unsigned char *result = NULL;

/*
 * For compression:
 *   the write operation compresses
 *   the read operation decompresses
 */

static int do_bio_comp_test(const BIO_METHOD *meth, size_t size)
{
    BIO *bcomp = NULL;
    BIO *bmem = NULL;
    BIO *bexp = NULL;
    int osize;
    int rsize;
    int ret = 0;

    /* Compress */
    if (!TEST_ptr(meth))
        goto err;
    if (!TEST_ptr(bcomp = BIO_new(meth)))
        goto err;
    if (!TEST_ptr(bmem = BIO_new(BIO_s_mem())))
        goto err;
    BIO_push(bcomp, bmem);
    osize = BIO_write(bcomp, original, size);
    if (!TEST_int_eq(osize, size)
        || !TEST_true(BIO_flush(bcomp)))
        goto err;
    BIO_free(bcomp);
    bcomp = NULL;

    /* decompress */
    if (!TEST_ptr(bexp = BIO_new(meth)))
        goto err;
    BIO_push(bexp, bmem);
    rsize = BIO_read(bexp, result, size);

    if (!TEST_int_eq(size, rsize)
        || !TEST_mem_eq(original, osize, result, rsize))
        goto err;

    ret = 1;
 err:
    BIO_free(bexp);
    BIO_free(bcomp);
    BIO_free(bmem);
    return ret;
}

static int do_bio_comp(const BIO_METHOD *meth, int n)
{
    int i;
    int success = 0;
    int size = sizes[n % 4];
    int type = n / 4;

    original = OPENSSL_malloc(BUFFER_SIZE);
    result = OPENSSL_malloc(BUFFER_SIZE);

    if (!TEST_ptr(original) || !TEST_ptr(result))
        goto err;

    switch (type) {
    case 0:
        TEST_info("zeros of size %d\n", size);
        memset(original, 0, BUFFER_SIZE);
        break;
    case 1:
        TEST_info("ones of size %d\n", size);
        memset(original, 1, BUFFER_SIZE);
        break;
    case 2:
        TEST_info("sequential of size %d\n", size);
        for (i = 0; i < BUFFER_SIZE; i++)
            original[i] = i & 0xFF;
        break;
    case 3:
        TEST_info("random of size %d\n", size);
        if (!TEST_int_gt(RAND_bytes(original, BUFFER_SIZE), 0))
            goto err;
        break;
    default:
        goto err;
    }

    if (!TEST_true(do_bio_comp_test(meth, size)))
        goto err;
    success = 1;
 err:
    OPENSSL_free(original);
    OPENSSL_free(result);
    return success;
}

#ifndef OPENSSL_NO_ZSTD
static int test_zstd(int n)
{
    return do_bio_comp(BIO_f_zstd(), n);
}
#endif
#ifndef OPENSSL_NO_BROTLI
static int test_brotli(int n)
{
    return do_bio_comp(BIO_f_brotli(), n);
}
#endif
#ifndef OPENSSL_NO_ZLIB
static int test_zlib(int n)
{
    return do_bio_comp(BIO_f_zlib(), n);
}
#endif

int setup_tests(void)
{
#ifndef OPENSSL_NO_ZLIB
    ADD_ALL_TESTS(test_zlib, NUM_SIZES * 4);
#endif
#ifndef OPENSSL_NO_BROTLI
    ADD_ALL_TESTS(test_brotli, NUM_SIZES * 4);
#endif
#ifndef OPENSSL_NO_ZSTD
    ADD_ALL_TESTS(test_zstd, NUM_SIZES * 4);
#endif
    return 1;
}
