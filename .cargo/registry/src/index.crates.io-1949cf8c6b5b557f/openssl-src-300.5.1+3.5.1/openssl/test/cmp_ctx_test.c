/*
 * Copyright 2007-2023 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "helpers/cmp_testlib.h"

#include <openssl/x509_vfy.h>

typedef struct test_fixture {
    const char *test_case_name;
    OSSL_CMP_CTX *ctx;
} OSSL_CMP_CTX_TEST_FIXTURE;

static void tear_down(OSSL_CMP_CTX_TEST_FIXTURE *fixture)
{
    if (fixture != NULL)
        OSSL_CMP_CTX_free(fixture->ctx);
    OPENSSL_free(fixture);
}

static OSSL_CMP_CTX_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    OSSL_CMP_CTX_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    if (!TEST_ptr(fixture->ctx = OSSL_CMP_CTX_new(NULL, NULL))) {
        tear_down(fixture);
        return NULL;
    }
    fixture->test_case_name = test_case_name;
    return fixture;
}

static STACK_OF(X509) *sk_X509_new_1(void)
{
    STACK_OF(X509) *sk = sk_X509_new_null();
    X509 *x = X509_new();

    if (x == NULL || !sk_X509_push(sk, x)) {
        sk_X509_free(sk);
        X509_free(x);
        sk = NULL;
    }
    return sk;
}

static void sk_X509_pop_X509_free(STACK_OF(X509) *sk)
{
    OSSL_STACK_OF_X509_free(sk);
}

static int execute_CTX_reinit_test(OSSL_CMP_CTX_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX *ctx = fixture->ctx;
    ASN1_OCTET_STRING *bytes = NULL;
    STACK_OF(X509) *certs = NULL;
    X509 *cert = X509_new();
    int res = 0;

    /* set non-default values in all relevant fields */
    ctx->status = 1;
    ctx->failInfoCode = 1;
    if (!ossl_cmp_ctx_set0_statusString(ctx, sk_ASN1_UTF8STRING_new_null())
            || !ossl_cmp_ctx_set0_newCert(ctx, X509_new())
            || !TEST_ptr(certs = sk_X509_new_1())
            || !ossl_cmp_ctx_set1_newChain(ctx, certs)
            || !ossl_cmp_ctx_set1_caPubs(ctx, certs)
            || !ossl_cmp_ctx_set1_extraCertsIn(ctx, certs)
            || !ossl_cmp_ctx_set1_validatedSrvCert(ctx, cert)
            || !TEST_ptr(bytes = ASN1_OCTET_STRING_new())
            || !OSSL_CMP_CTX_set1_transactionID(ctx, bytes)
            || !OSSL_CMP_CTX_set1_senderNonce(ctx, bytes)
            || !ossl_cmp_ctx_set1_recipNonce(ctx, bytes))
        goto err;

    if (!TEST_true(OSSL_CMP_CTX_reinit(ctx)))
        goto err;

    /* check whether values have been reset to default in all relevant fields */
    if (!TEST_true(ctx->status == -1
                       && ctx->failInfoCode == -1
                       && ctx->statusString == NULL
                       && ctx->newCert == NULL
                       && ctx->newChain == NULL
                       && ctx->caPubs == NULL
                       && ctx->extraCertsIn == NULL
                       && ctx->validatedSrvCert == NULL
                       && ctx->transactionID == NULL
                       && ctx->senderNonce == NULL
                       && ctx->recipNonce == NULL))
        goto err;

    /* this does not check that all remaining fields are untouched */
    res = 1;

 err:
    X509_free(cert);
    sk_X509_pop_X509_free(certs);
    ASN1_OCTET_STRING_free(bytes);
    return res;
}

static int test_CTX_libctx_propq(void)
{
    OSSL_LIB_CTX *libctx = OSSL_LIB_CTX_new();
    const char *propq = "?provider=legacy";
    OSSL_CMP_CTX *cmpctx = OSSL_CMP_CTX_new(libctx, propq);
    int res = TEST_ptr(libctx)
        && TEST_ptr(cmpctx)
        && TEST_ptr_eq(libctx, OSSL_CMP_CTX_get0_libctx(cmpctx))
        && TEST_str_eq(propq, OSSL_CMP_CTX_get0_propq(cmpctx));

    OSSL_CMP_CTX_free(cmpctx);
    OSSL_LIB_CTX_free(libctx);
    return res;
}

static int test_CTX_reinit(void)
{
    SETUP_TEST_FIXTURE(OSSL_CMP_CTX_TEST_FIXTURE, set_up);
    EXECUTE_TEST(execute_CTX_reinit_test, tear_down);
    return result;
}

#if !defined(OPENSSL_NO_ERR) && !defined(OPENSSL_NO_AUTOERRINIT)

static int msg_total_size = 0;
static int msg_total_size_log_cb(const char *func, const char *file, int line,
                                 OSSL_CMP_severity level, const char *msg)
{
    msg_total_size += strlen(msg);
    TEST_note("total=%d len=%zu msg='%s'\n", msg_total_size, strlen(msg), msg);
    return 1;
}

# define STR64 "This is a 64 bytes looooooooooooooooooooooooooooooooong string.\n"
/* max string length ISO C90 compilers are required to support is 509. */
# define STR509 STR64 STR64 STR64 STR64 STR64 STR64 STR64 \
    "This is a 61 bytes loooooooooooooooooooooooooooooong string.\n"
static const char *const max_str_literal = STR509;
# define STR_SEP "<SEP>"

static int execute_CTX_print_errors_test(OSSL_CMP_CTX_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX *ctx = fixture->ctx;
    int base_err_msg_size, expected_size;
    int res = 1;

    if (!TEST_true(OSSL_CMP_CTX_set_log_cb(ctx, NULL)))
        res = 0;
    if (!TEST_true(ctx->log_cb == NULL))
        res = 0;

# ifndef OPENSSL_NO_STDIO
    ERR_raise(ERR_LIB_CMP, CMP_R_MULTIPLE_SAN_SOURCES);
    OSSL_CMP_CTX_print_errors(ctx); /* should print above error to STDERR */
# endif

    /* this should work regardless of OPENSSL_NO_STDIO and OPENSSL_NO_TRACE: */
    if (!TEST_true(OSSL_CMP_CTX_set_log_cb(ctx, msg_total_size_log_cb)))
        res = 0;
    if (!TEST_true(ctx->log_cb == msg_total_size_log_cb)) {
        res = 0;
    } else {
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ARGS);
        base_err_msg_size = strlen("INVALID_ARGS");
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        base_err_msg_size += strlen("NULL_ARGUMENT");
        expected_size = base_err_msg_size;
        ossl_cmp_add_error_data("data1"); /* should prepend separator ":" */
        expected_size += strlen(":" "data1");
        ossl_cmp_add_error_data("data2"); /* should prepend separator " : " */
        expected_size += strlen(" : " "data2");
        ossl_cmp_add_error_line("new line"); /* should prepend separator "\n" */
        expected_size += strlen("\n" "new line");
        OSSL_CMP_CTX_print_errors(ctx);
        if (!TEST_int_eq(msg_total_size, expected_size))
            res = 0;

        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ARGS);
        base_err_msg_size = strlen("INVALID_ARGS") + strlen(":");
        expected_size = base_err_msg_size;
        while (expected_size < 4096) { /* force split */
            ERR_add_error_txt(STR_SEP, max_str_literal);
            expected_size += strlen(STR_SEP) + strlen(max_str_literal);
        }
        expected_size += base_err_msg_size - 2 * strlen(STR_SEP);
        msg_total_size = 0;
        OSSL_CMP_CTX_print_errors(ctx);
        if (!TEST_int_eq(msg_total_size, expected_size))
            res = 0;
    }

    return res;
}

static int test_CTX_print_errors(void)
{
    SETUP_TEST_FIXTURE(OSSL_CMP_CTX_TEST_FIXTURE, set_up);
    EXECUTE_TEST(execute_CTX_print_errors_test, tear_down);
    return result;
}
#endif

static
int execute_CTX_reqExtensions_have_SAN_test(OSSL_CMP_CTX_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX *ctx = fixture->ctx;
    const int len = 16;
    unsigned char str[16 /* = len */];
    ASN1_OCTET_STRING *data = NULL;
    X509_EXTENSION *ext = NULL;
    X509_EXTENSIONS *exts = NULL;
    int res = 0;

    if (!TEST_false(OSSL_CMP_CTX_reqExtensions_have_SAN(ctx)))
        return 0;

    if (!TEST_int_eq(1, RAND_bytes(str, len))
            || !TEST_ptr(data = ASN1_OCTET_STRING_new())
            || !TEST_true(ASN1_OCTET_STRING_set(data, str, len)))
        goto err;
    ext = X509_EXTENSION_create_by_NID(NULL, NID_subject_alt_name, 0, data);
    if (!TEST_ptr(ext)
            || !TEST_ptr(exts = sk_X509_EXTENSION_new_null())
            || !TEST_true(sk_X509_EXTENSION_push(exts, ext))
            || !TEST_true(OSSL_CMP_CTX_set0_reqExtensions(ctx, exts))) {
        X509_EXTENSION_free(ext);
        sk_X509_EXTENSION_free(exts);
        goto err;
    }
    if (TEST_int_eq(OSSL_CMP_CTX_reqExtensions_have_SAN(ctx), 1)) {
        ext = sk_X509_EXTENSION_pop(exts);
        res = TEST_false(OSSL_CMP_CTX_reqExtensions_have_SAN(ctx));
        X509_EXTENSION_free(ext);
    }
 err:
    ASN1_OCTET_STRING_free(data);
    return res;
}

static int test_CTX_reqExtensions_have_SAN(void)
{
    SETUP_TEST_FIXTURE(OSSL_CMP_CTX_TEST_FIXTURE, set_up);
    EXECUTE_TEST(execute_CTX_reqExtensions_have_SAN_test, tear_down);
    return result;
}

static int test_log_line;
static int test_log_cb_res = 0;
static int test_log_cb(const char *func, const char *file, int line,
                       OSSL_CMP_severity level, const char *msg)
{
    test_log_cb_res =
#ifndef PEDANTIC
        (TEST_str_eq(func, "execute_cmp_ctx_log_cb_test")
         || TEST_str_eq(func, "(unknown function)")) &&
#endif
        (TEST_str_eq(file, OPENSSL_FILE)
         || TEST_str_eq(file, "(no file)"))
        && (TEST_int_eq(line, test_log_line) || TEST_int_eq(line, 0))
        && (TEST_int_eq(level, OSSL_CMP_LOG_INFO) || TEST_int_eq(level, -1))
        && TEST_str_eq(msg, "ok");
    return 1;
}

static int execute_cmp_ctx_log_cb_test(OSSL_CMP_CTX_TEST_FIXTURE *fixture)
{
    int res = 1;
    OSSL_CMP_CTX *ctx = fixture->ctx;

    OSSL_TRACE(ALL, "this general trace message is not shown by default\n");

    OSSL_CMP_log_open();
    OSSL_CMP_log_open(); /* multiple calls should be harmless */

    if (!TEST_true(OSSL_CMP_CTX_set_log_cb(ctx, NULL))) {
        res = 0;
    } else {
        ossl_cmp_err(ctx, "this should be printed as CMP error message");
        ossl_cmp_warn(ctx, "this should be printed as CMP warning message");
        ossl_cmp_debug(ctx, "this should not be printed");
        TEST_true(OSSL_CMP_CTX_set_log_verbosity(ctx, OSSL_CMP_LOG_DEBUG));
        ossl_cmp_debug(ctx, "this should be printed as CMP debug message");
        TEST_true(OSSL_CMP_CTX_set_log_verbosity(ctx, OSSL_CMP_LOG_INFO));
    }
    if (!TEST_true(OSSL_CMP_CTX_set_log_cb(ctx, test_log_cb))) {
        res = 0;
    } else {
        test_log_line = OPENSSL_LINE + 1;
        ossl_cmp_log2(INFO, ctx, "%s%c", "o", 'k');
        if (!TEST_int_eq(test_log_cb_res, 1))
            res = 0;
        OSSL_CMP_CTX_set_log_verbosity(ctx, OSSL_CMP_LOG_ERR);
        test_log_cb_res = -1; /* callback should not be called at all */
        test_log_line = OPENSSL_LINE + 1;
        ossl_cmp_log2(INFO, ctx, "%s%c", "o", 'k');
        if (!TEST_int_eq(test_log_cb_res, -1))
            res = 0;
    }
    OSSL_CMP_log_close();
    OSSL_CMP_log_close(); /* multiple calls should be harmless */
    return res;
}

static int test_cmp_ctx_log_cb(void)
{
    SETUP_TEST_FIXTURE(OSSL_CMP_CTX_TEST_FIXTURE, set_up);
    EXECUTE_TEST(execute_cmp_ctx_log_cb_test, tear_down);
    return result;
}

#ifndef OPENSSL_NO_HTTP
static BIO *test_http_cb(BIO *bio, void *arg, int use_ssl, int detail)
{
    return NULL;
}
#endif

static OSSL_CMP_MSG *test_transfer_cb(OSSL_CMP_CTX *ctx,
                                      const OSSL_CMP_MSG *req)
{
    return NULL;
}

static int test_certConf_cb(OSSL_CMP_CTX *ctx, X509 *cert, int fail_info,
                            const char **txt)
{
    return 0;
}

typedef OSSL_CMP_CTX CMP_CTX; /* prevents rewriting type name by below macro */
#define OSSL_CMP_CTX 1 /* name prefix for exported setter functions */
#define ossl_cmp_ctx 0 /* name prefix for internal setter functions */
#define set 0
#define set0 0
#define set1 1
#define get 0
#define get0 0
#define get1 1

#define DEFINE_SET_GET_BASE_TEST(PREFIX, SETN, GETN, DUP, FIELD, TYPE, ERR, \
                                 DEFAULT, NEW, FREE) \
static int \
execute_CTX_##SETN##_##GETN##_##FIELD(OSSL_CMP_CTX_TEST_FIXTURE *fixture) \
{ \
    CMP_CTX *ctx = fixture->ctx; \
    int (*set_fn)(CMP_CTX *ctx, TYPE) = \
        (int (*)(CMP_CTX *ctx, TYPE))PREFIX##_##SETN##_##FIELD; \
    /* need type cast in above assignment as TYPE arg sometimes is const */ \
    TYPE (*get_fn)(const CMP_CTX *ctx) = OSSL_CMP_CTX_##GETN##_##FIELD; \
    TYPE val1_to_free = NEW; \
    TYPE val1 = val1_to_free; \
    TYPE val1_read = 0; /* 0 works for any type */ \
    TYPE val2_to_free = NEW; \
    TYPE val2 = val2_to_free; \
    TYPE val2_read = 0; \
    TYPE val3_read = 0; \
    int res = 1; \
    \
    if (!TEST_int_eq(ERR_peek_error(), 0)) \
        res = 0; \
    if (PREFIX == 1) { /* exported setter functions must test ctx == NULL */ \
        if ((*set_fn)(NULL, val1) || ERR_peek_error() == 0) { \
            TEST_error("setter did not return error on ctx == NULL"); \
            res = 0; \
        } \
    } \
    ERR_clear_error(); \
    \
    if ((*get_fn)(NULL) != ERR || ERR_peek_error() == 0) { \
        TEST_error("getter did not return error on ctx == NULL"); \
        res = 0; \
    } \
    ERR_clear_error(); \
    \
    val1_read = (*get_fn)(ctx); \
    if (!DEFAULT(val1_read)) { \
        TEST_error("did not get default value"); \
        res = 0; \
    } \
    if (!(*set_fn)(ctx, val1)) { \
        TEST_error("setting first value failed"); \
        res = 0; \
    } \
    if (SETN == 0) \
        val1_to_free = 0; /* 0 works for any type */ \
    \
    if (GETN == 1) \
        FREE(val1_read); \
    val1_read = (*get_fn)(ctx); \
    if (SETN == 0) { \
        if (val1_read != val1) { \
            TEST_error("set/get first value did not match"); \
            res = 0; \
        } \
    } else { \
        if (DUP && val1_read == val1) { \
            TEST_error("first set did not dup the value"); \
            val1_read = 0; \
            res = 0; \
        } \
        if (DEFAULT(val1_read)) { \
            TEST_error("first set had no effect"); \
            res = 0; \
        } \
    } \
    \
    if (!(*set_fn)(ctx, val2)) { \
        TEST_error("setting second value failed"); \
        res = 0; \
    } \
    if (SETN == 0) \
        val2_to_free = 0; \
    \
    val2_read = (*get_fn)(ctx); \
    if (DEFAULT(val2_read)) { \
        TEST_error("second set reset the value"); \
        res = 0; \
    } \
    if (SETN == 0 && GETN == 0) { \
        if (val2_read != val2) { \
            TEST_error("set/get second value did not match"); \
            res = 0; \
        } \
    } else { \
        if (DUP && val2_read == val2) { \
            TEST_error("second set did not dup the value"); \
            val2_read = 0; \
            res = 0; \
        } \
        if (val2 == val1) { \
            TEST_error("second value is same as first value"); \
            res = 0; \
        } \
        if (GETN == 1 && val2_read == val1_read) { \
            /* \
             * Note that if GETN == 0 then possibly val2_read == val1_read \
             * because set1 may allocate the new copy at the same location. \
             */ \
            TEST_error("second get returned same as first get"); \
            res = 0; \
        } \
    } \
    \
    val3_read = (*get_fn)(ctx); \
    if (DEFAULT(val3_read)) { \
        TEST_error("third set reset the value"); \
        res = 0; \
    } \
    if (GETN == 0) { \
        if (val3_read != val2_read) { \
            TEST_error("third get gave different value"); \
            res = 0; \
        } \
    } else { \
        if (DUP && val3_read == val2_read) { \
            TEST_error("third get did not create a new dup"); \
            val3_read = 0; \
            res = 0; \
        } \
    } \
    /* this does not check that all remaining fields are untouched */ \
    \
    if (!TEST_int_eq(ERR_peek_error(), 0)) \
        res = 0; \
    \
    FREE(val1_to_free); \
    FREE(val2_to_free); \
    if (GETN == 1) { \
        FREE(val1_read); \
        FREE(val2_read); \
        FREE(val3_read); \
    } \
    return TEST_true(res); \
} \
\
static int test_CTX_##SETN##_##GETN##_##FIELD(void) \
{ \
    SETUP_TEST_FIXTURE(OSSL_CMP_CTX_TEST_FIXTURE, set_up); \
    EXECUTE_TEST(execute_CTX_##SETN##_##GETN##_##FIELD, tear_down); \
    return result; \
}

static char *char_new(void)
{
    return OPENSSL_strdup("test");
}

static void char_free(char *val)
{
    OPENSSL_free(val);
}

#define EMPTY_SK_X509(x) ((x) == NULL || sk_X509_num(x) == 0)

static X509_STORE *X509_STORE_new_1(void)
{
    X509_STORE *store = X509_STORE_new();

    if (store != NULL)
        X509_VERIFY_PARAM_set_flags(X509_STORE_get0_param(store), 1);
    return store;
}

#define DEFAULT_STORE(x) \
    ((x) == NULL || X509_VERIFY_PARAM_get_flags(X509_STORE_get0_param(x)) == 0)

#define IS_NEG(x) ((x) < 0)
#define IS_0(x) ((x) == 0) /* for any type */
#define DROP(x) (void)(x) /* dummy free() for non-pointer and function types */

#define RET_IF_NULL_ARG(ctx, ret) \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return ret; \
    }

/* cannot use PREFIX instead of OSSL_CMP and CTX due to #define OSSL_CMP_CTX */
#define DEFINE_SET_GET_TEST(OSSL_CMP, CTX, N, M, DUP, FIELD, TYPE) \
    DEFINE_SET_GET_BASE_TEST(OSSL_CMP##_##CTX, set##N, get##M, DUP, FIELD, \
                             TYPE *, NULL, IS_0, TYPE##_new(), TYPE##_free)

#define DEFINE_SET_GET_SK_TEST_DEFAULT(OSSL_CMP, CTX, N, M, FIELD, ELEM_TYPE, \
                                       DEFAULT, NEW, FREE) \
    DEFINE_SET_GET_BASE_TEST(OSSL_CMP##_##CTX, set##N, get##M, 1, FIELD, \
                             STACK_OF(ELEM_TYPE)*, NULL, DEFAULT, NEW, FREE)
#define DEFINE_SET_GET_SK_TEST(OSSL_CMP, CTX, N, M, FIELD, T) \
    DEFINE_SET_GET_SK_TEST_DEFAULT(OSSL_CMP, CTX, N, M, FIELD, T, \
                                   IS_0, sk_##T##_new_null(), sk_##T##_free)
#define DEFINE_SET_GET_SK_X509_TEST(OSSL_CMP, CTX, N, M, FNAME) \
    DEFINE_SET_GET_SK_TEST_DEFAULT(OSSL_CMP, CTX, N, M, FNAME, X509, \
                                   EMPTY_SK_X509, \
                                   sk_X509_new_1(), sk_X509_pop_X509_free)

#define DEFINE_SET_GET_TEST_DEFAULT(OSSL_CMP, CTX, N, M, DUP, FIELD, TYPE, \
                                    DEFAULT) \
    DEFINE_SET_GET_BASE_TEST(OSSL_CMP##_##CTX, set##N, get##M, DUP, FIELD, \
                             TYPE *, NULL, DEFAULT, TYPE##_new(), TYPE##_free)
#define DEFINE_SET_TEST_DEFAULT(OSSL_CMP, CTX, N, DUP, FIELD, TYPE, DEFAULT) \
    static TYPE *OSSL_CMP_CTX_get0_##FIELD(const CMP_CTX *ctx) \
    { \
        RET_IF_NULL_ARG(ctx, NULL); \
        return (TYPE *)ctx->FIELD; \
    } \
    DEFINE_SET_GET_TEST_DEFAULT(OSSL_CMP, CTX, N, 0, DUP, FIELD, TYPE, DEFAULT)
#define DEFINE_SET_TEST(OSSL_CMP, CTX, N, DUP, FIELD, TYPE) \
    DEFINE_SET_TEST_DEFAULT(OSSL_CMP, CTX, N, DUP, FIELD, TYPE, IS_0)

#define DEFINE_SET_SK_TEST(OSSL_CMP, CTX, N, FIELD, TYPE) \
    static STACK_OF(TYPE) *OSSL_CMP_CTX_get0_##FIELD(const CMP_CTX *ctx) \
    { \
        RET_IF_NULL_ARG(ctx, NULL); \
        return ctx->FIELD; \
    } \
    DEFINE_SET_GET_BASE_TEST(OSSL_CMP##_##CTX, set##N, get0, 1, FIELD, \
                             STACK_OF(TYPE)*, NULL, IS_0, \
                             sk_##TYPE##_new_null(), sk_##TYPE##_free)

#ifndef OPENSSL_NO_HTTP
typedef OSSL_HTTP_bio_cb_t OSSL_CMP_http_cb_t;
#endif
#define DEFINE_SET_CB_TEST(FIELD) \
    static OSSL_CMP_##FIELD##_t OSSL_CMP_CTX_get_##FIELD(const CMP_CTX *ctx) \
    { \
        RET_IF_NULL_ARG(ctx, NULL); \
        return ctx->FIELD; \
    } \
    DEFINE_SET_GET_BASE_TEST(OSSL_CMP_CTX, set, get, 0, FIELD, \
                             OSSL_CMP_##FIELD##_t, NULL, IS_0, \
                             test_##FIELD, DROP)
#define DEFINE_SET_GET_P_VOID_TEST(FIELD) \
    DEFINE_SET_GET_BASE_TEST(OSSL_CMP_CTX, set, get, 0, FIELD, void *, \
                             NULL, IS_0, ((void *)1), DROP)

#define DEFINE_SET_GET_INT_TEST_DEFAULT(OSSL_CMP, CTX, FIELD, DEFAULT) \
    DEFINE_SET_GET_BASE_TEST(OSSL_CMP##_##CTX, set, get, 0, FIELD, int, -1, \
                             DEFAULT, 1, DROP)
#define DEFINE_SET_GET_INT_TEST(OSSL_CMP, CTX, FIELD) \
    DEFINE_SET_GET_INT_TEST_DEFAULT(OSSL_CMP, CTX, FIELD, IS_NEG)
#define DEFINE_SET_INT_TEST(FIELD) \
    static int OSSL_CMP_CTX_get_##FIELD(const CMP_CTX *ctx) \
    { \
        RET_IF_NULL_ARG(ctx, -1); \
        return ctx->FIELD; \
    } \
    DEFINE_SET_GET_INT_TEST_DEFAULT(OSSL_CMP, CTX, FIELD, IS_0)

#define DEFINE_SET_GET_ARG_FN(SETN, GETN, FIELD, ARG, T) \
    static int OSSL_CMP_CTX_##SETN##_##FIELD##_##ARG(CMP_CTX *ctx, T val) \
    { \
        return OSSL_CMP_CTX_##SETN##_##FIELD(ctx, ARG, val); \
    } \
    \
    static T OSSL_CMP_CTX_##GETN##_##FIELD##_##ARG(const CMP_CTX *ctx) \
    { \
        return OSSL_CMP_CTX_##GETN##_##FIELD(ctx, ARG); \
    }

#define DEFINE_SET_GET1_STR_FN(SETN, FIELD) \
    static int OSSL_CMP_CTX_##SETN##_##FIELD##_str(CMP_CTX *ctx, char *val)\
    { \
        return OSSL_CMP_CTX_##SETN##_##FIELD(ctx, (unsigned char *)val, \
                                             strlen(val));              \
    } \
    \
    static char *OSSL_CMP_CTX_get1_##FIELD##_str(const CMP_CTX *ctx) \
    { \
        const ASN1_OCTET_STRING *bytes = NULL; \
        \
        RET_IF_NULL_ARG(ctx, NULL); \
        bytes = ctx->FIELD; \
        return bytes == NULL ? NULL : \
            OPENSSL_strndup((char *)bytes->data, bytes->length); \
    }

#define push 0
#define push0 0
#define push1 1
#define DEFINE_PUSH_BASE_TEST(PUSHN, DUP, FIELD, ELEM, TYPE, T, \
                              DEFAULT, NEW, FREE) \
static TYPE sk_top_##FIELD(const CMP_CTX *ctx) \
{ \
    return sk_##T##_value(ctx->FIELD, sk_##T##_num(ctx->FIELD) - 1); \
} \
\
static int execute_CTX_##PUSHN##_##ELEM(OSSL_CMP_CTX_TEST_FIXTURE *fixture) \
{ \
    CMP_CTX *ctx = fixture->ctx; \
    int (*push_fn)(CMP_CTX *ctx, TYPE) = \
        (int (*)(CMP_CTX *ctx, TYPE))OSSL_CMP_CTX_##PUSHN##_##ELEM; \
    /* \
     * need type cast in above assignment because TYPE arg sometimes is const \
     */ \
    int n_elem = sk_##T##_num(ctx->FIELD); \
    STACK_OF(TYPE) field_read; \
    TYPE val1_to_free = NEW; \
    TYPE val1 = val1_to_free; \
    TYPE val1_read = 0; /* 0 works for any type */ \
    TYPE val2_to_free = NEW; \
    TYPE val2 = val2_to_free; \
    TYPE val2_read = 0; \
    int res = 1; \
    \
    if (!TEST_int_eq(ERR_peek_error(), 0)) \
        res = 0; \
    if ((*push_fn)(NULL, val1) || ERR_peek_error() == 0) { \
        TEST_error("pusher did not return error on ctx == NULL"); \
        res = 0; \
    } \
    ERR_clear_error(); \
    \
    if (n_elem < 0) /* can happen for NULL stack */ \
        n_elem = 0; \
    field_read = ctx->FIELD; \
    if (!DEFAULT(field_read)) { \
        TEST_error("did not get default value for stack field"); \
        res = 0; \
    } \
    if (!(*push_fn)(ctx, val1)) { \
        TEST_error("pushing first value failed"); \
        res = 0; \
    } \
    if (PUSHN == 0) \
        val1_to_free = 0; /* 0 works for any type */ \
    \
    if (sk_##T##_num(ctx->FIELD) != ++n_elem) { \
        TEST_error("pushing first value did not increment number"); \
        res = 0; \
    } \
    val1_read = sk_top_##FIELD(ctx); \
    if (PUSHN == 0) { \
        if (val1_read != val1) { \
            TEST_error("push/sk_top first value did not match"); \
            res = 0; \
        } \
    } else { \
        if (DUP && val1_read == val1) { \
            TEST_error("first push did not dup the value"); \
            res = 0; \
        } \
    } \
    \
    if (!(*push_fn)(ctx, val2)) { \
        TEST_error("pushing second value failed"); \
        res = 0; \
    } \
    if (PUSHN == 0) \
        val2_to_free = 0; \
    \
    if (sk_##T##_num(ctx->FIELD) != ++n_elem) { \
        TEST_error("pushing second value did not increment number"); \
        res = 0; \
    } \
    val2_read = sk_top_##FIELD(ctx); \
    if (PUSHN == 0) { \
        if (val2_read != val2) { \
            TEST_error("push/sk_top second value did not match"); \
            res = 0; \
        } \
    } else { \
        if (DUP && val2_read == val2) { \
            TEST_error("second push did not dup the value"); \
            res = 0; \
        } \
        if (val2 == val1) { \
            TEST_error("second value is same as first value"); \
            res = 0; \
        } \
    } \
    /* this does not check if all remaining fields and elems are untouched */ \
    \
    if (!TEST_int_eq(ERR_peek_error(), 0)) \
        res = 0; \
    \
    FREE(val1_to_free); \
    FREE(val2_to_free); \
    return TEST_true(res); \
} \
\
static int test_CTX_##PUSHN##_##ELEM(void) \
{ \
    SETUP_TEST_FIXTURE(OSSL_CMP_CTX_TEST_FIXTURE, set_up); \
    EXECUTE_TEST(execute_CTX_##PUSHN##_##ELEM, tear_down); \
    return result; \
} \

#define DEFINE_PUSH_TEST(N, DUP, FIELD, ELEM, TYPE) \
    DEFINE_PUSH_BASE_TEST(push##N, DUP, FIELD, ELEM, TYPE *, TYPE, \
                          IS_0, TYPE##_new(), TYPE##_free)

void cleanup_tests(void)
{
    return;
}

DEFINE_SET_GET_ARG_FN(set, get, option, 35, int) /* OPT_IGNORE_KEYUSAGE */
DEFINE_SET_GET_BASE_TEST(OSSL_CMP_CTX, set, get, 0, option_35, int, -1, IS_0, \
                         1 /* true */, DROP)

DEFINE_SET_CB_TEST(log_cb)

DEFINE_SET_TEST_DEFAULT(OSSL_CMP, CTX, 1, 1, serverPath, char, IS_0)
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 1, server, char)
DEFINE_SET_INT_TEST(serverPort)
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 1, proxy, char)
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 1, no_proxy, char)
#ifndef OPENSSL_NO_HTTP
DEFINE_SET_CB_TEST(http_cb)
DEFINE_SET_GET_P_VOID_TEST(http_cb_arg)
#endif
DEFINE_SET_CB_TEST(transfer_cb)
DEFINE_SET_GET_P_VOID_TEST(transfer_cb_arg)

DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 0, srvCert, X509)
DEFINE_SET_GET_TEST(ossl_cmp, ctx, 1, 0, 0, validatedSrvCert, X509)
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 1, expected_sender, X509_NAME)
DEFINE_SET_GET_BASE_TEST(OSSL_CMP_CTX, set0, get0, 0, trusted,
                         X509_STORE *, NULL,
                         DEFAULT_STORE, X509_STORE_new_1(), X509_STORE_free)
DEFINE_SET_GET_SK_X509_TEST(OSSL_CMP, CTX, 1, 0, untrusted)

DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 0, cert, X509)
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 0, pkey, EVP_PKEY)

DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 1, recipient, X509_NAME)
DEFINE_PUSH_TEST(0, 0, geninfo_ITAVs, geninfo_ITAV, OSSL_CMP_ITAV)
DEFINE_SET_SK_TEST(OSSL_CMP, CTX, 1, extraCertsOut, X509)
DEFINE_SET_GET_ARG_FN(set0, get0, newPkey, 1, EVP_PKEY *) /* priv == 1 */
DEFINE_SET_GET_TEST(OSSL_CMP, CTX, 0, 0, 0, newPkey_1, EVP_PKEY)
DEFINE_SET_GET_ARG_FN(set0, get0, newPkey, 0, EVP_PKEY *) /* priv == 0 */
DEFINE_SET_GET_TEST(OSSL_CMP, CTX, 0, 0, 0, newPkey_0, EVP_PKEY)
DEFINE_SET_GET1_STR_FN(set1, referenceValue)
DEFINE_SET_GET_TEST_DEFAULT(OSSL_CMP, CTX, 1, 1, 1, referenceValue_str, char,
                            IS_0)
DEFINE_SET_GET1_STR_FN(set1, secretValue)
DEFINE_SET_GET_TEST_DEFAULT(OSSL_CMP, CTX, 1, 1, 1, secretValue_str, char, IS_0)
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 1, issuer, X509_NAME)
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 1, subjectName, X509_NAME)
#ifdef ISSUE_9504_RESOLVED
DEFINE_PUSH_TEST(1, 1, subjectAltNames, subjectAltName, GENERAL_NAME)
#endif
DEFINE_SET_SK_TEST(OSSL_CMP, CTX, 0, reqExtensions, X509_EXTENSION)
DEFINE_PUSH_TEST(0, 0, policies, policy, POLICYINFO)
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 0, oldCert, X509)
#ifdef ISSUE_9504_RESOLVED
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 1, p10CSR, X509_REQ)
#endif
DEFINE_PUSH_TEST(0, 0, genm_ITAVs, genm_ITAV, OSSL_CMP_ITAV)
DEFINE_SET_CB_TEST(certConf_cb)
DEFINE_SET_GET_P_VOID_TEST(certConf_cb_arg)

DEFINE_SET_GET_INT_TEST(ossl_cmp, ctx, status)
DEFINE_SET_GET_SK_TEST(ossl_cmp, ctx, 0, 0, statusString, ASN1_UTF8STRING)
DEFINE_SET_GET_INT_TEST(ossl_cmp, ctx, failInfoCode)
DEFINE_SET_GET_TEST(ossl_cmp, ctx, 0, 0, 0, newCert, X509)
DEFINE_SET_GET_SK_X509_TEST(ossl_cmp, ctx, 1, 1, newChain)
DEFINE_SET_GET_SK_X509_TEST(ossl_cmp, ctx, 1, 1, caPubs)
DEFINE_SET_GET_SK_X509_TEST(ossl_cmp, ctx, 1, 1, extraCertsIn)

DEFINE_SET_TEST_DEFAULT(OSSL_CMP, CTX, 1, 1, transactionID, ASN1_OCTET_STRING,
                        IS_0)
DEFINE_SET_TEST(OSSL_CMP, CTX, 1, 1, senderNonce, ASN1_OCTET_STRING)
DEFINE_SET_TEST(ossl_cmp, ctx, 1, 1, recipNonce, ASN1_OCTET_STRING)

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    /* also tests OSSL_CMP_CTX_new() and OSSL_CMP_CTX_free(): */
    ADD_TEST(test_CTX_libctx_propq);
    ADD_TEST(test_CTX_reinit);

    /* various CMP options: */
    ADD_TEST(test_CTX_set_get_option_35);
    /* CMP-specific callback for logging and outputting the error queue: */
    ADD_TEST(test_CTX_set_get_log_cb);
    /*
     * also tests OSSL_CMP_log_open(), OSSL_CMP_CTX_set_log_verbosity(),
     * ossl_cmp_err(), ossl_cmp_warn(), * ossl_cmp_debug(),
     * ossl_cmp_log2(), ossl_cmp_log_parse_metadata(), and OSSL_CMP_log_close()
     * with OSSL_CMP_severity OSSL_CMP_LOG_ERR/WARNING/DEBUG/INFO:
     */
    ADD_TEST(test_cmp_ctx_log_cb);
#if !defined(OPENSSL_NO_ERR) && !defined(OPENSSL_NO_AUTOERRINIT)
    /*
     * also tests OSSL_CMP_CTX_set_log_cb(), OSSL_CMP_print_errors_cb(),
     * and the macros ossl_cmp_add_error_data and ossl_cmp_add_error_line:
     */
    ADD_TEST(test_CTX_print_errors);
#endif
    /* message transfer: */
    ADD_TEST(test_CTX_set1_get0_serverPath);
    ADD_TEST(test_CTX_set1_get0_server);
    ADD_TEST(test_CTX_set_get_serverPort);
    ADD_TEST(test_CTX_set1_get0_proxy);
    ADD_TEST(test_CTX_set1_get0_no_proxy);
#ifndef OPENSSL_NO_HTTP
    ADD_TEST(test_CTX_set_get_http_cb);
    ADD_TEST(test_CTX_set_get_http_cb_arg);
#endif
    ADD_TEST(test_CTX_set_get_transfer_cb);
    ADD_TEST(test_CTX_set_get_transfer_cb_arg);
    /* server authentication: */
    ADD_TEST(test_CTX_set1_get0_srvCert);
    ADD_TEST(test_CTX_set1_get0_validatedSrvCert);
    ADD_TEST(test_CTX_set1_get0_expected_sender);
    ADD_TEST(test_CTX_set0_get0_trusted);
    ADD_TEST(test_CTX_set1_get0_untrusted);
    /* client authentication: */
    ADD_TEST(test_CTX_set1_get0_cert);
    ADD_TEST(test_CTX_set1_get0_pkey);
    /* the following two also test ossl_cmp_asn1_octet_string_set1_bytes(): */
    ADD_TEST(test_CTX_set1_get1_referenceValue_str);
    ADD_TEST(test_CTX_set1_get1_secretValue_str);
    /* CMP message header and extra certificates: */
    ADD_TEST(test_CTX_set1_get0_recipient);
    ADD_TEST(test_CTX_push0_geninfo_ITAV);
    ADD_TEST(test_CTX_set1_get0_extraCertsOut);
    /* certificate template: */
    ADD_TEST(test_CTX_set0_get0_newPkey_1);
    ADD_TEST(test_CTX_set0_get0_newPkey_0);
    ADD_TEST(test_CTX_set1_get0_issuer);
    ADD_TEST(test_CTX_set1_get0_subjectName);
#ifdef ISSUE_9504_RESOLVED
    /*
     * test currently fails, see https://github.com/openssl/openssl/issues/9504
     */
    ADD_TEST(test_CTX_push1_subjectAltName);
#endif
    ADD_TEST(test_CTX_set0_get0_reqExtensions);
    ADD_TEST(test_CTX_reqExtensions_have_SAN);
    ADD_TEST(test_CTX_push0_policy);
    ADD_TEST(test_CTX_set1_get0_oldCert);
#ifdef ISSUE_9504_RESOLVED
    /*
     * test currently fails, see https://github.com/openssl/openssl/issues/9504
     */
    ADD_TEST(test_CTX_set1_get0_p10CSR);
#endif
    /* misc body contents: */
    ADD_TEST(test_CTX_push0_genm_ITAV);
    /* certificate confirmation: */
    ADD_TEST(test_CTX_set_get_certConf_cb);
    ADD_TEST(test_CTX_set_get_certConf_cb_arg);
    /* result fetching: */
    ADD_TEST(test_CTX_set_get_status);
    ADD_TEST(test_CTX_set0_get0_statusString);
    ADD_TEST(test_CTX_set_get_failInfoCode);
    ADD_TEST(test_CTX_set0_get0_newCert);
    ADD_TEST(test_CTX_set1_get1_newChain);
    ADD_TEST(test_CTX_set1_get1_caPubs);
    ADD_TEST(test_CTX_set1_get1_extraCertsIn);
    /* exported for testing and debugging purposes: */
    /* the following three also test ossl_cmp_asn1_octet_string_set1(): */
    ADD_TEST(test_CTX_set1_get0_transactionID);
    ADD_TEST(test_CTX_set1_get0_senderNonce);
    ADD_TEST(test_CTX_set1_get0_recipNonce);
    return 1;
}
