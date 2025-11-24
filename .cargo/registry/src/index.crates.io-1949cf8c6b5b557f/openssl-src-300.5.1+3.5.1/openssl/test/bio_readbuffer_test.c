/*
 * Copyright 2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/bio.h>
#include "testutil.h"

static const char *filename = NULL;

/*
 * Test that a BIO_f_readbuffer() with a BIO_new_file() behaves nicely if
 * BIO_gets() and BIO_read_ex() are both called.
 * Since the BIO_gets() calls buffer the reads, the BIO_read_ex() should
 * still be able to read the buffered data if we seek back to the start.
 *
 * The following cases are tested using tstid:
 * 0 : Just use BIO_read_ex().
 * 1 : Try a few reads using BIO_gets() before using BIO_read_ex()
 * 2 : Read the entire file using BIO_gets() before using BIO_read_ex().
 */
static int test_readbuffer_file_bio(int tstid)
{
    int ret = 0, len, partial;
    BIO *in = NULL, *in_bio = NULL, *readbuf_bio = NULL;
    char buf[255];
    char expected[4096];
    size_t readbytes = 0, bytes = 0, count = 0;

    /* Open a file BIO and read all the data */
    if (!TEST_ptr(in = BIO_new_file(filename, "r"))
        || !TEST_int_eq(BIO_read_ex(in, expected, sizeof(expected),
                                    &readbytes), 1)
        || !TEST_int_lt(readbytes, sizeof(expected)))
        goto err;
    BIO_free(in);
    in = NULL;

    /* Create a new file bio that sits under a readbuffer BIO */
    if (!TEST_ptr(readbuf_bio = BIO_new(BIO_f_readbuffer()))
        || !TEST_ptr(in_bio = BIO_new_file(filename, "r")))
        goto err;

    in_bio = BIO_push(readbuf_bio, in_bio);
    readbuf_bio = NULL;

    if (!TEST_int_eq(BIO_tell(in_bio), 0))
        goto err;

    if (tstid != 0) {
        partial = 4;
        while (!BIO_eof(in_bio)) {
            len = BIO_gets(in_bio, buf, sizeof(buf));
            if (len == 0) {
                if (!TEST_true(BIO_eof(in_bio)))
                    goto err;
            } else {
                if (!TEST_int_gt(len, 0)
                    || !TEST_int_le(len, (int)sizeof(buf) - 1))
                    goto err;
                if (!TEST_true(buf[len] == 0))
                    goto err;
                if (len > 1
                    && !BIO_eof(in_bio)
                    && len != ((int)sizeof(buf) - 1)
                    && !TEST_true(buf[len - 1] == '\n'))
                    goto err;
            }
            if (tstid == 1 && --partial == 0)
                break;
        }
    }
    if (!TEST_int_eq(BIO_seek(in_bio, 0), 1))
        goto err;

    len = 8; /* Do a small partial read to start with */
    while (!BIO_eof(in_bio)) {
        if (!TEST_int_eq(BIO_read_ex(in_bio, buf, len, &bytes), 1))
            break;
        if (!TEST_mem_eq(buf, bytes, expected + count, bytes))
            goto err;
        count += bytes;
        len = sizeof(buf); /* fill the buffer on subsequent reads */
    }
    if (!TEST_int_eq(count, readbytes))
        goto err;
    ret = 1;
err:
    BIO_free(in);
    BIO_free_all(in_bio);
    BIO_free(readbuf_bio);
    return ret;
}

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_WITH_EXTRA_USAGE("file\n"),
        { OPT_HELP_STR, 1, '-', "file\tFile to run tests on.\n" },
        { NULL }
    };
    return test_options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_TEST_CASES:
            break;
        default:
            return 0;
        }
    }
    filename = test_get_argument(0);

    ADD_ALL_TESTS(test_readbuffer_file_bio, 3);
    return 1;
}
