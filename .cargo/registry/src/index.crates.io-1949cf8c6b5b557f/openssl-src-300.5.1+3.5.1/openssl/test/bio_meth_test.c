/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/bio.h>
#include "testutil.h"

static int test_bio_meth(void)
{
    int i, ret = 0, id;
    BIO_METHOD *meth1 = NULL, *meth2 = NULL, *meth3 = NULL;
    BIO *membio = NULL, *bio1 = NULL, *bio2 = NULL, *bio3 = NULL;

    id = BIO_get_new_index();
    if (!TEST_int_eq(id, BIO_TYPE_START + 1))
        goto err;

    if (!TEST_ptr(meth1 = BIO_meth_new(id, "Method1"))
        || !TEST_ptr(meth2 = BIO_meth_new(BIO_TYPE_NONE, "Method2"))
        || !TEST_ptr(meth3 = BIO_meth_new(BIO_TYPE_NONE|BIO_TYPE_FILTER, "Method3"))
        || !TEST_ptr(bio1 = BIO_new(meth1))
        || !TEST_ptr(bio2 = BIO_new(meth2))
        || !TEST_ptr(bio3 = BIO_new(meth3))
        || !TEST_ptr(membio = BIO_new(BIO_s_mem())))
        goto err;

    BIO_set_next(bio3, bio2);
    BIO_set_next(bio2, bio1);
    BIO_set_next(bio1, membio);

    /* Check that we get an error if we exhaust BIO_get_new_index() */
    for (i = id + 1; i <= BIO_TYPE_MASK; ++i) {
        if (!TEST_int_eq(BIO_get_new_index(), i))
            goto err;
    }
    if (!TEST_int_eq(BIO_get_new_index(), -1))
        goto err;

    /* test searching works */
    if (!TEST_ptr_eq(BIO_find_type(bio3, BIO_TYPE_MEM), membio)
        || !TEST_ptr_eq(BIO_find_type(bio3, id), bio1))
        goto err;

    /* Check searching for BIO_TYPE_NONE returns NULL */
    if (!TEST_ptr_null(BIO_find_type(bio3, BIO_TYPE_NONE)))
        goto err;
    /* Check searching for BIO_TYPE_NONE + BIO_TYPE_FILTER works */
    if (!TEST_ptr_eq(BIO_find_type(bio3, BIO_TYPE_FILTER), bio3))
        goto err;
    ret = 1;
err:
    BIO_free(membio);
    BIO_free(bio3);
    BIO_free(bio2);
    BIO_free(bio1);
    BIO_meth_free(meth3);
    BIO_meth_free(meth2);
    BIO_meth_free(meth1);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_bio_meth);
    return 1;
}
