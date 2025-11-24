/*
 * Copyright 2018-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/opensslconf.h>
#include <openssl/err.h>
#include <openssl/macros.h>

#include "testutil.h"

#if defined(OPENSSL_SYS_WINDOWS)
# include <windows.h>
#else
# include <errno.h>
#endif

#ifndef OPENSSL_NO_DEPRECATED_3_0
# define IS_HEX(ch) ((ch >= '0' && ch <='9') || (ch >= 'A' && ch <='F'))

static int test_print_error_format(void)
{
    /* Variables used to construct an error line */
    char *lib;
    const char *func = OPENSSL_FUNC;
    char *reason;
# ifdef OPENSSL_NO_ERR
    char reasonbuf[255];
# endif
# ifndef OPENSSL_NO_FILENAMES
    const char *file = OPENSSL_FILE;
    const int line = OPENSSL_LINE;
# else
    const char *file = "";
    const int line = 0;
# endif
    /* The format for OpenSSL error lines */
    const char *expected_format = ":error:%08lX:%s:%s:%s:%s:%d";
    /*-
     *                                          ^^ ^^ ^^ ^^ ^^
     * "library" name --------------------------++ || || || ||
     * function name ------------------------------++ || || ||
     * reason string (system error string) -----------++ || ||
     * file name ----------------------------------------++ ||
     * line number -----------------------------------------++
     */
    char expected[512];

    char *out = NULL, *p = NULL;
    int ret = 0, len;
    BIO *bio = NULL;
    const int syserr = EPERM;
    unsigned long errorcode;
    unsigned long reasoncode;

    /*
     * We set a mark here so we can clear the system error that we generate
     * with ERR_PUT_error().  That is, after all, just a simulation to verify
     * ERR_print_errors() output, not a real error.
     */
    ERR_set_mark();

    ERR_PUT_error(ERR_LIB_SYS, 0, syserr, file, line);
    errorcode = ERR_peek_error();
    reasoncode = ERR_GET_REASON(errorcode);

    if (!TEST_int_eq(reasoncode, syserr)) {
        ERR_pop_to_mark();
        goto err;
    }

# if !defined(OPENSSL_NO_ERR)
#  if defined(OPENSSL_NO_AUTOERRINIT)
    lib = "lib(2)";
#  else
    lib = "system library";
#  endif
    reason = strerror(syserr);
# else
    lib = "lib(2)";
    BIO_snprintf(reasonbuf, sizeof(reasonbuf), "reason(%lu)", reasoncode);
    reason = reasonbuf;
# endif

    BIO_snprintf(expected, sizeof(expected), expected_format,
                 errorcode, lib, func, reason, file, line);

    if (!TEST_ptr(bio = BIO_new(BIO_s_mem())))
        goto err;

    ERR_print_errors(bio);

    if (!TEST_int_gt(len = BIO_get_mem_data(bio, &out), 0))
        goto err;
    /* Skip over the variable thread id at the start of the string */
    for (p = out; *p != ':' && *p != 0; ++p) {
        if (!TEST_true(IS_HEX(*p)))
            goto err;
    }
    if (!TEST_true(*p != 0)
        || !TEST_strn_eq(expected, p, strlen(expected)))
        goto err;

    ret = 1;
err:
    BIO_free(bio);
    return ret;
}
#endif

/* Test that querying the error queue preserves the OS error. */
static int preserves_system_error(void)
{
#if defined(OPENSSL_SYS_WINDOWS)
    SetLastError(ERROR_INVALID_FUNCTION);
    ERR_get_error();
    return TEST_int_eq(GetLastError(), ERROR_INVALID_FUNCTION);
#else
    errno = EINVAL;
    ERR_get_error();
    return TEST_int_eq(errno, EINVAL);
#endif
}

/* Test that calls to ERR_add_error_[v]data append */
static int vdata_appends(void)
{
    const char *data;

    ERR_raise(ERR_LIB_CRYPTO, ERR_R_MALLOC_FAILURE);
    ERR_add_error_data(1, "hello ");
    ERR_add_error_data(1, "world");
    ERR_peek_error_data(&data, NULL);
    return TEST_str_eq(data, "hello world");
}

static int raised_error(void)
{
    const char *f, *data;
    int l;
    unsigned long e;

    /*
     * When OPENSSL_NO_ERR or OPENSSL_NO_FILENAMES, no file name or line
     * number is saved, so no point checking them.
     */
#if !defined(OPENSSL_NO_FILENAMES) && !defined(OPENSSL_NO_ERR)
    const char *file;
    int line;

    file = __FILE__;
    line = __LINE__ + 2; /* The error is generated on the ERR_raise_data line */
#endif
    ERR_raise_data(ERR_LIB_NONE, ERR_R_INTERNAL_ERROR,
                   "calling exit()");
    if (!TEST_ulong_ne(e = ERR_get_error_all(&f, &l, NULL, &data, NULL), 0)
            || !TEST_int_eq(ERR_GET_REASON(e), ERR_R_INTERNAL_ERROR)
#if !defined(OPENSSL_NO_FILENAMES) && !defined(OPENSSL_NO_ERR)
            || !TEST_int_eq(l, line)
            || !TEST_str_eq(f, file)
#endif
            || !TEST_str_eq(data, "calling exit()"))
        return 0;
    return 1;
}

static int test_marks(void)
{
    unsigned long mallocfail, shouldnot;

    /* Set an initial error */
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_MALLOC_FAILURE);
    mallocfail = ERR_peek_last_error();
    if (!TEST_ulong_gt(mallocfail, 0))
        return 0;

    /* Setting and clearing a mark should not affect the error */
    if (!TEST_true(ERR_set_mark())
            || !TEST_true(ERR_pop_to_mark())
            || !TEST_ulong_eq(mallocfail, ERR_peek_last_error())
            || !TEST_true(ERR_set_mark())
            || !TEST_true(ERR_clear_last_mark())
            || !TEST_ulong_eq(mallocfail, ERR_peek_last_error()))
        return 0;

    /* Test popping errors */
    if (!TEST_true(ERR_set_mark()))
        return 0;
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
    if (!TEST_ulong_ne(mallocfail, ERR_peek_last_error())
            || !TEST_true(ERR_pop_to_mark())
            || !TEST_ulong_eq(mallocfail, ERR_peek_last_error()))
        return 0;

    /* Nested marks should also work */
    if (!TEST_true(ERR_set_mark())
            || !TEST_true(ERR_set_mark()))
        return 0;
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
    if (!TEST_ulong_ne(mallocfail, ERR_peek_last_error())
            || !TEST_true(ERR_pop_to_mark())
            || !TEST_true(ERR_pop_to_mark())
            || !TEST_ulong_eq(mallocfail, ERR_peek_last_error()))
        return 0;

    if (!TEST_true(ERR_set_mark()))
        return 0;
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    shouldnot = ERR_peek_last_error();
    if (!TEST_ulong_ne(mallocfail, shouldnot)
            || !TEST_true(ERR_set_mark()))
        return 0;
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
    if (!TEST_ulong_ne(shouldnot, ERR_peek_last_error())
            || !TEST_true(ERR_pop_to_mark())
            || !TEST_ulong_eq(shouldnot, ERR_peek_last_error())
            || !TEST_true(ERR_pop_to_mark())
            || !TEST_ulong_eq(mallocfail, ERR_peek_last_error()))
        return 0;

    /* Setting and clearing a mark should not affect the errors on the stack */
    if (!TEST_true(ERR_set_mark()))
        return 0;
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    if (!TEST_true(ERR_clear_last_mark())
            || !TEST_ulong_eq(shouldnot, ERR_peek_last_error()))
        return 0;

    /*
     * Popping where no mark has been set should pop everything - but return
     * a failure result
     */
    if (!TEST_false(ERR_pop_to_mark())
            || !TEST_ulong_eq(0, ERR_peek_last_error()))
        return 0;

    /* Clearing where there is no mark should fail */
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_MALLOC_FAILURE);
    if (!TEST_false(ERR_clear_last_mark())
                /* "get" the last error to remove it */
            || !TEST_ulong_eq(mallocfail, ERR_get_error())
            || !TEST_ulong_eq(0, ERR_peek_last_error()))
        return 0;

    /*
     * Setting a mark where there are no errors in the stack should fail.
     * NOTE: This is somewhat surprising behaviour but is historically how this
     * function behaves. In practice we typically set marks without first
     * checking whether there is anything on the stack - but we also don't
     * tend to check the success of this function. It turns out to work anyway
     * because although setting a mark with no errors fails, a subsequent call
     * to ERR_pop_to_mark() or ERR_clear_last_mark() will do the right thing
     * anyway (even though they will report a failure result).
     */
    if (!TEST_false(ERR_set_mark()))
        return 0;

    ERR_raise(ERR_LIB_CRYPTO, ERR_R_MALLOC_FAILURE);
    if (!TEST_true(ERR_set_mark()))
        return 0;
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);

    /* Should be able to "pop" past 2 errors */
    if (!TEST_true(ERR_pop_to_mark())
            || !TEST_ulong_eq(mallocfail, ERR_peek_last_error()))
        return 0;

    if (!TEST_true(ERR_set_mark()))
        return 0;
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
    ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);

    /* Should be able to "clear" past 2 errors */
    if (!TEST_true(ERR_clear_last_mark())
            || !TEST_ulong_eq(shouldnot, ERR_peek_last_error()))
        return 0;

    /* Clear remaining errors from last test */
    ERR_clear_error();

    return 1;
}

static int test_clear_error(void)
{
    int flags = -1;
    const char *data = NULL;
    int res = 0;

    /* Raise an error with data and clear it */
    ERR_raise_data(0, 0, "hello %s", "world");
    ERR_peek_error_data(&data, &flags);
    if (!TEST_str_eq(data, "hello world")
            || !TEST_int_eq(flags, ERR_TXT_STRING | ERR_TXT_MALLOCED))
        goto err;
    ERR_clear_error();

    /* Raise a new error without data */
    ERR_raise(0, 0);
    ERR_peek_error_data(&data, &flags);
    if (!TEST_str_eq(data, "")
            || !TEST_int_eq(flags, ERR_TXT_MALLOCED))
        goto err;
    ERR_clear_error();

    /* Raise a new error with data */
    ERR_raise_data(0, 0, "goodbye %s world", "cruel");
    ERR_peek_error_data(&data, &flags);
    if (!TEST_str_eq(data, "goodbye cruel world")
            || !TEST_int_eq(flags, ERR_TXT_STRING | ERR_TXT_MALLOCED))
        goto err;
    ERR_clear_error();

    /*
     * Raise a new error without data to check that the malloced storage
     * is freed properly
     */
    ERR_raise(0, 0);
    ERR_peek_error_data(&data, &flags);
    if (!TEST_str_eq(data, "")
            || !TEST_int_eq(flags, ERR_TXT_MALLOCED))
        goto err;
    ERR_clear_error();

    res = 1;
 err:
     ERR_clear_error();
    return res;
}

/*
 * Test saving and restoring error state.
 * Test 0: Save using OSSL_ERR_STATE_save()
 * Test 1: Save using OSSL_ERR_STATE_save_to_mark()
 */
static int test_save_restore(int idx)
{
    ERR_STATE *es;
    int res = 0, i, flags = -1;
    unsigned long mallocfail, interr;
    static const char testdata[] = "test data";
    const char *data = NULL;

    if (!TEST_ptr(es = OSSL_ERR_STATE_new()))
        goto err;

    ERR_raise(ERR_LIB_CRYPTO, ERR_R_MALLOC_FAILURE);
    mallocfail = ERR_peek_last_error();
    if (!TEST_ulong_gt(mallocfail, 0))
        goto err;

    if (idx == 1 && !TEST_int_eq(ERR_set_mark(), 1))
        goto err;

    ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR, testdata);
    interr = ERR_peek_last_error();
    if (!TEST_ulong_ne(mallocfail, ERR_peek_last_error()))
        goto err;

    if (idx == 0) {
        OSSL_ERR_STATE_save(es);

        if (!TEST_ulong_eq(ERR_peek_last_error(), 0))
            goto err;
    } else {
        OSSL_ERR_STATE_save_to_mark(es);

        if (!TEST_ulong_ne(ERR_peek_last_error(), 0))
            goto err;
    }

    for (i = 0; i < 2; i++) {
        OSSL_ERR_STATE_restore(es);

        if (!TEST_ulong_eq(ERR_peek_last_error(), interr))
            goto err;
        ERR_peek_last_error_data(&data, &flags);
        if (!TEST_str_eq(data, testdata)
                || !TEST_int_eq(flags, ERR_TXT_STRING | ERR_TXT_MALLOCED))
            goto err;

        /* restore again to duplicate the entries */
        OSSL_ERR_STATE_restore(es);

        /* verify them all */
        if (idx == 0 || i == 0) {
            if (!TEST_ulong_eq(ERR_get_error_all(NULL, NULL, NULL,
                                                 &data, &flags), mallocfail)
                || !TEST_int_ne(flags, ERR_TXT_STRING | ERR_TXT_MALLOCED))
                goto err;
        }

        if (!TEST_ulong_eq(ERR_get_error_all(NULL, NULL, NULL,
                                             &data, &flags), interr)
            || !TEST_str_eq(data, testdata)
            || !TEST_int_eq(flags, ERR_TXT_STRING | ERR_TXT_MALLOCED))
            goto err;

        if (idx == 0) {
            if (!TEST_ulong_eq(ERR_get_error_all(NULL, NULL, NULL,
                                                 &data, &flags), mallocfail)
                || !TEST_int_ne(flags, ERR_TXT_STRING | ERR_TXT_MALLOCED))
                goto err;
        }

        if (!TEST_ulong_eq(ERR_get_error_all(NULL, NULL, NULL,
                                             &data, &flags), interr)
            || !TEST_str_eq(data, testdata)
            || !TEST_int_eq(flags, ERR_TXT_STRING | ERR_TXT_MALLOCED))
            goto err;

        if (!TEST_ulong_eq(ERR_get_error(), 0))
            goto err;
    }

    res = 1;
 err:
    OSSL_ERR_STATE_free(es);
    return res;
}

int setup_tests(void)
{
    ADD_TEST(preserves_system_error);
    ADD_TEST(vdata_appends);
    ADD_TEST(raised_error);
#ifndef OPENSSL_NO_DEPRECATED_3_0
    ADD_TEST(test_print_error_format);
#endif
    ADD_TEST(test_marks);
    ADD_ALL_TESTS(test_save_restore, 2);
    ADD_TEST(test_clear_error);
    return 1;
}
