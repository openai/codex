/*
 * Copyright 2018-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#define OPENSSL_SUPPRESS_DEPRECATED
#include <stdio.h>
#include <string.h>
#include <openssl/bio.h>

#include "testutil.h"

#define MAXCOUNT 5
static int         my_param_count;
static BIO        *my_param_b[MAXCOUNT];
static int         my_param_oper[MAXCOUNT];
static const char *my_param_argp[MAXCOUNT];
static int         my_param_argi[MAXCOUNT];
static long        my_param_argl[MAXCOUNT];
static long        my_param_ret[MAXCOUNT];
static size_t      my_param_len[MAXCOUNT];
static size_t      my_param_processed[MAXCOUNT];

static long my_bio_cb_ex(BIO *b, int oper, const char *argp, size_t len,
                         int argi, long argl, int ret, size_t *processed)
{
    if (my_param_count >= MAXCOUNT)
        return -1;
    my_param_b[my_param_count]    = b;
    my_param_oper[my_param_count] = oper;
    my_param_argp[my_param_count] = argp;
    my_param_argi[my_param_count] = argi;
    my_param_argl[my_param_count] = argl;
    my_param_ret[my_param_count]  = ret;
    my_param_len[my_param_count]  = len;
    my_param_processed[my_param_count] = processed != NULL ? *processed : 0;

    my_param_count++;
    return ret;
}

static int test_bio_callback_ex(void)
{
    int ok = 0;
    BIO *bio;
    int i;
    char test1[] = "test";
    const size_t test1len = sizeof(test1) - 1;
    char test2[] = "hello";
    const size_t test2len = sizeof(test2) - 1;
    char buf[16];

    my_param_count = 0;

    bio = BIO_new(BIO_s_mem());
    if (bio == NULL)
        goto err;

    BIO_set_callback_ex(bio, my_bio_cb_ex);
    i = BIO_write(bio, test1, test1len);
    if (!TEST_int_eq(i, test1len)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_WRITE)
            || !TEST_ptr_eq(my_param_argp[0], test1)
            || !TEST_size_t_eq(my_param_len[0], test1len)
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_int_eq((int)my_param_ret[0], 1)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_WRITE | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], test1)
            || !TEST_size_t_eq(my_param_len[1], test1len)
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_size_t_eq(my_param_processed[1], test1len)
            || !TEST_int_eq((int)my_param_ret[1], 1))
        goto err;

    my_param_count = 0;
    i = BIO_read(bio, buf, sizeof(buf));
    if (!TEST_mem_eq(buf, i, test1, test1len)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_READ)
            || !TEST_ptr_eq(my_param_argp[0], buf)
            || !TEST_size_t_eq(my_param_len[0], sizeof(buf))
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_int_eq((int)my_param_ret[0], 1)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_READ | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], buf)
            || !TEST_size_t_eq(my_param_len[1], sizeof(buf))
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_size_t_eq(my_param_processed[1], test1len)
            || !TEST_int_eq((int)my_param_ret[1], 1))
        goto err;

    /* By default a mem bio returns -1 if it has run out of data */
    my_param_count = 0;
    i = BIO_read(bio, buf, sizeof(buf));
    if (!TEST_int_eq(i, -1)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_READ)
            || !TEST_ptr_eq(my_param_argp[0], buf)
            || !TEST_size_t_eq(my_param_len[0], sizeof(buf))
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_int_eq((int)my_param_ret[0], 1)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_READ | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], buf)
            || !TEST_size_t_eq(my_param_len[1], sizeof(buf))
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_size_t_eq(my_param_processed[1], 0)
            || !TEST_int_eq((int)my_param_ret[1], -1))
        goto err;

    /* Force the mem bio to return 0 if it has run out of data */
    my_param_count = 0;
    i = BIO_set_mem_eof_return(bio, 0);
    if (!TEST_int_eq(i, 1)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_CTRL)
            || !TEST_ptr_eq(my_param_argp[0], NULL)
            || !TEST_int_eq(my_param_argi[0], BIO_C_SET_BUF_MEM_EOF_RETURN)
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_int_eq((int)my_param_ret[0], 1)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_CTRL | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], NULL)
            || !TEST_int_eq(my_param_argi[1], BIO_C_SET_BUF_MEM_EOF_RETURN)
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_int_eq((int)my_param_ret[1], 1))
        goto err;
    my_param_count = 0;
    i = BIO_read(bio, buf, sizeof(buf));
    if (!TEST_int_eq(i, 0)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_READ)
            || !TEST_ptr_eq(my_param_argp[0], buf)
            || !TEST_size_t_eq(my_param_len[0], sizeof(buf))
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_int_eq((int)my_param_ret[0], 1)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_READ | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], buf)
            || !TEST_size_t_eq(my_param_len[1], sizeof(buf))
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_size_t_eq(my_param_processed[1], 0)
            || !TEST_int_eq((int)my_param_ret[1], 0))
        goto err;

    my_param_count = 0;
    i = BIO_puts(bio, test2);
    if (!TEST_int_eq(i, 5)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_PUTS)
            || !TEST_ptr_eq(my_param_argp[0], test2)
            || !TEST_int_eq(my_param_argi[0], 0)
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_int_eq((int)my_param_ret[0], 1)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_PUTS | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], test2)
            || !TEST_int_eq(my_param_argi[1], 0)
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_size_t_eq(my_param_processed[1], test2len)
            || !TEST_int_eq((int)my_param_ret[1], 1))
        goto err;

    my_param_count = 0;
    i = BIO_free(bio);
    if (!TEST_int_eq(i, 1)
            || !TEST_int_eq(my_param_count, 1)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_FREE)
            || !TEST_ptr_eq(my_param_argp[0], NULL)
            || !TEST_int_eq(my_param_argi[0], 0)
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_int_eq((int)my_param_ret[0], 1))
        goto finish;

    ok = 1;
    goto finish;

err:
    BIO_free(bio);

finish:
    /* This helps finding memory leaks with ASAN */
    memset(my_param_b, 0, sizeof(my_param_b));
    memset(my_param_argp, 0, sizeof(my_param_argp));
    return ok;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
static long my_bio_callback(BIO *b, int oper, const char *argp, int argi,
                            long argl, long ret)
{
    if (my_param_count >= MAXCOUNT)
        return -1;
    my_param_b[my_param_count]    = b;
    my_param_oper[my_param_count] = oper;
    my_param_argp[my_param_count] = argp;
    my_param_argi[my_param_count] = argi;
    my_param_argl[my_param_count] = argl;
    my_param_ret[my_param_count]  = ret;
    my_param_count++;
    return ret;
}

static int test_bio_callback(void)
{
    int ok = 0;
    BIO *bio;
    int i;
    char test1[] = "test";
    const int test1len = sizeof(test1) - 1;
    char test2[] = "hello";
    const int test2len = sizeof(test2) - 1;
    char buf[16];

    my_param_count = 0;

    bio = BIO_new(BIO_s_mem());
    if (bio == NULL)
        goto err;

    BIO_set_callback(bio, my_bio_callback);
    i = BIO_write(bio, test1, test1len);
    if (!TEST_int_eq(i, test1len)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_WRITE)
            || !TEST_ptr_eq(my_param_argp[0], test1)
            || !TEST_int_eq(my_param_argi[0], test1len)
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_long_eq(my_param_ret[0], 1L)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_WRITE | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], test1)
            || !TEST_int_eq(my_param_argi[1], test1len)
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_long_eq(my_param_ret[1], (long)test1len))
        goto err;

    my_param_count = 0;
    i = BIO_read(bio, buf, sizeof(buf));
    if (!TEST_mem_eq(buf, i, test1, test1len)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_READ)
            || !TEST_ptr_eq(my_param_argp[0], buf)
            || !TEST_int_eq(my_param_argi[0], sizeof(buf))
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_long_eq(my_param_ret[0], 1L)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_READ | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], buf)
            || !TEST_int_eq(my_param_argi[1], sizeof(buf))
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_long_eq(my_param_ret[1], (long)test1len))
        goto err;

    /* By default a mem bio returns -1 if it has run out of data */
    my_param_count = 0;
    i = BIO_read(bio, buf, sizeof(buf));
    if (!TEST_int_eq(i, -1)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_READ)
            || !TEST_ptr_eq(my_param_argp[0], buf)
            || !TEST_int_eq(my_param_argi[0], sizeof(buf))
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_long_eq(my_param_ret[0], 1L)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_READ | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], buf)
            || !TEST_int_eq(my_param_argi[1], sizeof(buf))
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_long_eq(my_param_ret[1], -1L))
        goto err;

    /* Force the mem bio to return 0 if it has run out of data */
    BIO_set_mem_eof_return(bio, 0);
    my_param_count = 0;
    i = BIO_read(bio, buf, sizeof(buf));
    if (!TEST_int_eq(i, 0)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_READ)
            || !TEST_ptr_eq(my_param_argp[0], buf)
            || !TEST_int_eq(my_param_argi[0], sizeof(buf))
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_long_eq(my_param_ret[0], 1L)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_READ | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], buf)
            || !TEST_int_eq(my_param_argi[1], sizeof(buf))
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_long_eq(my_param_ret[1], 0L))
        goto err;

    my_param_count = 0;
    i = BIO_puts(bio, test2);
    if (!TEST_int_eq(i, 5)
            || !TEST_int_eq(my_param_count, 2)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_PUTS)
            || !TEST_ptr_eq(my_param_argp[0], test2)
            || !TEST_int_eq(my_param_argi[0], 0)
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_long_eq(my_param_ret[0], 1L)
            || !TEST_ptr_eq(my_param_b[1], bio)
            || !TEST_int_eq(my_param_oper[1], BIO_CB_PUTS | BIO_CB_RETURN)
            || !TEST_ptr_eq(my_param_argp[1], test2)
            || !TEST_int_eq(my_param_argi[1], 0)
            || !TEST_long_eq(my_param_argl[1], 0L)
            || !TEST_long_eq(my_param_ret[1], (long)test2len))
        goto err;

    my_param_count = 0;
    i = BIO_free(bio);
    if (!TEST_int_eq(i, 1)
            || !TEST_int_eq(my_param_count, 1)
            || !TEST_ptr_eq(my_param_b[0], bio)
            || !TEST_int_eq(my_param_oper[0], BIO_CB_FREE)
            || !TEST_ptr_eq(my_param_argp[0], NULL)
            || !TEST_int_eq(my_param_argi[0], 0)
            || !TEST_long_eq(my_param_argl[0], 0L)
            || !TEST_long_eq(my_param_ret[0], 1L))
        goto finish;

    ok = 1;
    goto finish;

err:
    BIO_free(bio);

finish:
    /* This helps finding memory leaks with ASAN */
    memset(my_param_b, 0, sizeof(my_param_b));
    memset(my_param_argp, 0, sizeof(my_param_argp));
    return ok;
}
#endif

int setup_tests(void)
{
    ADD_TEST(test_bio_callback_ex);
#ifndef OPENSSL_NO_DEPRECATED_3_0
    ADD_TEST(test_bio_callback);
#endif
    return 1;
}
