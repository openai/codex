/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/trace.h>

#include "testutil.h"

static int test_trace_categories(void)
{
    int cat_num;

    for (cat_num = -1; cat_num <= OSSL_TRACE_CATEGORY_NUM + 1; ++cat_num) {
        const char *cat_name = OSSL_trace_get_category_name(cat_num);
        const char *expected_cat_name = NULL;
        int ret_cat_num;

#define SET_EXPECTED_CAT_NAME(name) expected_cat_name = #name; break
        switch (cat_num) {
        case OSSL_TRACE_CATEGORY_ALL:
            SET_EXPECTED_CAT_NAME(ALL);
        case OSSL_TRACE_CATEGORY_TRACE:
            SET_EXPECTED_CAT_NAME(TRACE);
        case OSSL_TRACE_CATEGORY_INIT:
            SET_EXPECTED_CAT_NAME(INIT);
        case OSSL_TRACE_CATEGORY_TLS:
            SET_EXPECTED_CAT_NAME(TLS);
        case OSSL_TRACE_CATEGORY_TLS_CIPHER:
            SET_EXPECTED_CAT_NAME(TLS_CIPHER);
        case OSSL_TRACE_CATEGORY_CONF:
            SET_EXPECTED_CAT_NAME(CONF);
        case OSSL_TRACE_CATEGORY_ENGINE_TABLE:
            SET_EXPECTED_CAT_NAME(ENGINE_TABLE);
        case OSSL_TRACE_CATEGORY_ENGINE_REF_COUNT:
            SET_EXPECTED_CAT_NAME(ENGINE_REF_COUNT);
        case OSSL_TRACE_CATEGORY_PKCS5V2:
            SET_EXPECTED_CAT_NAME(PKCS5V2);
        case OSSL_TRACE_CATEGORY_PKCS12_KEYGEN:
            SET_EXPECTED_CAT_NAME(PKCS12_KEYGEN);
        case OSSL_TRACE_CATEGORY_PKCS12_DECRYPT:
            SET_EXPECTED_CAT_NAME(PKCS12_DECRYPT);
        case OSSL_TRACE_CATEGORY_X509V3_POLICY:
            SET_EXPECTED_CAT_NAME(X509V3_POLICY);
        case OSSL_TRACE_CATEGORY_BN_CTX:
            SET_EXPECTED_CAT_NAME(BN_CTX);
        case OSSL_TRACE_CATEGORY_CMP:
            SET_EXPECTED_CAT_NAME(CMP);
        case OSSL_TRACE_CATEGORY_STORE:
            SET_EXPECTED_CAT_NAME(STORE);
        case OSSL_TRACE_CATEGORY_DECODER:
            SET_EXPECTED_CAT_NAME(DECODER);
        case OSSL_TRACE_CATEGORY_ENCODER:
            SET_EXPECTED_CAT_NAME(ENCODER);
        case OSSL_TRACE_CATEGORY_REF_COUNT:
            SET_EXPECTED_CAT_NAME(REF_COUNT);
        case OSSL_TRACE_CATEGORY_HTTP:
            SET_EXPECTED_CAT_NAME(HTTP);
        case OSSL_TRACE_CATEGORY_PROVIDER:
            SET_EXPECTED_CAT_NAME(PROVIDER);
        case OSSL_TRACE_CATEGORY_QUERY:
            SET_EXPECTED_CAT_NAME(QUERY);
        default:
            if (cat_num == -1 || cat_num >= OSSL_TRACE_CATEGORY_NUM)
                expected_cat_name = NULL;
            break;
        }
#undef SET_EXPECTED_CAT_NAME

        if (!TEST_str_eq(cat_name, expected_cat_name))
            return 0;
        ret_cat_num =
            OSSL_trace_get_category_num(cat_name);
        if (cat_num < OSSL_TRACE_CATEGORY_NUM)
            if (!TEST_int_eq(cat_num, ret_cat_num))
                return 0;
    }

    return 1;
}

#ifndef OPENSSL_NO_TRACE

# define OSSL_START "xyz-"
# define OSSL_HELLO "Hello World\n"
/* OSSL_STR80 must have length OSSL_TRACE_STRING_MAX */
# define OSSL_STR80 "1234567890123456789012345678901234567890123456789012345678901234567890123456789\n"
# define OSSL_STR81 (OSSL_STR80"x")
# define OSSL_CTRL "A\xfe\nB"
# define OSSL_MASKED "A \nB"
# define OSSL_BYE "Good Bye Universe\n"
# define OSSL_END "-abc"

# define trace_string(text, full, str) \
    OSSL_trace_string(trc_out, text, full, (unsigned char *)(str), strlen(str))

static int put_trace_output(void)
{
    int res = 1;

    OSSL_TRACE_BEGIN(HTTP) {
        res = TEST_int_eq(BIO_printf(trc_out, OSSL_HELLO), strlen(OSSL_HELLO));
        res += TEST_int_eq(trace_string(0, 0, OSSL_STR80), strlen(OSSL_STR80));
        res += TEST_int_eq(trace_string(0, 0, OSSL_STR81), strlen(OSSL_STR80));
        res += TEST_int_eq(trace_string(1, 1, OSSL_CTRL), strlen(OSSL_CTRL));
        res += TEST_int_eq(trace_string(0, 1, OSSL_MASKED), strlen(OSSL_MASKED)
                           + 1); /* newline added */
        res += TEST_int_eq(BIO_printf(trc_out, OSSL_BYE), strlen(OSSL_BYE));
        res = res == 6;
        /* not using '&&' but '+' to catch potentially multiple test failures */
    } OSSL_TRACE_END(HTTP);
    return res;
}

static int test_trace_channel(void)
{
    static const char expected[] =
        OSSL_START"\n" OSSL_HELLO
        OSSL_STR80 "[len 81 limited to 80]: "OSSL_STR80
        OSSL_CTRL OSSL_MASKED"\n" OSSL_BYE OSSL_END"\n";
    static const size_t expected_len = sizeof(expected) - 1;
    BIO *bio = NULL;
    char *p_buf = NULL;
    long len = 0;
    int ret = 0;

    bio = BIO_new(BIO_s_mem());
    if (!TEST_ptr(bio))
        goto end;

    if (!TEST_int_eq(OSSL_trace_set_channel(OSSL_TRACE_CATEGORY_HTTP, bio), 1)) {
        BIO_free(bio);
        goto end;
    }

    if (!TEST_true(OSSL_trace_enabled(OSSL_TRACE_CATEGORY_HTTP)))
        goto end;

    if (!TEST_int_eq(OSSL_trace_set_prefix(OSSL_TRACE_CATEGORY_HTTP,
                                           OSSL_START), 1))
        goto end;
    if (!TEST_int_eq(OSSL_trace_set_suffix(OSSL_TRACE_CATEGORY_HTTP,
                                           OSSL_END), 1))
        goto end;

    ret = put_trace_output();
    len = BIO_get_mem_data(bio, &p_buf);
    if (!TEST_strn2_eq(p_buf, len, expected, expected_len))
        ret = 0;
    ret = TEST_int_eq(OSSL_trace_set_channel(OSSL_TRACE_CATEGORY_HTTP, NULL), 1)
        && ret;

 end:
    return ret;
}

static int trace_cb_failure;
static int trace_cb_called;

static size_t trace_cb(const char *buffer, size_t count,
                       int category, int cmd, void *data)
{
    trace_cb_called = 1;
    if (!TEST_true(category == OSSL_TRACE_CATEGORY_TRACE))
        trace_cb_failure = 1;
    return count;
}

static int test_trace_callback(void)
{
    int ret = 0;

    if (!TEST_true(OSSL_trace_set_callback(OSSL_TRACE_CATEGORY_TRACE, trace_cb,
                                           NULL)))
        goto end;

    put_trace_output();

    if (!TEST_false(trace_cb_failure) || !TEST_true(trace_cb_called))
        goto end;

    ret = 1;
 end:
    return ret;
}
#endif

OPT_TEST_DECLARE_USAGE("\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    ADD_TEST(test_trace_categories);
#ifndef OPENSSL_NO_TRACE
    ADD_TEST(test_trace_channel);
    ADD_TEST(test_trace_callback);
#endif
    return 1;
}

void cleanup_tests(void)
{
}
