/*
 * Copyright 2017-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "../testutil.h"
#include "output.h"
#include "tu_local.h"

#include <errno.h>
#include <string.h>
#include <ctype.h>
#include <openssl/asn1.h>

/*
 * Output a failed test first line.
 * All items are optional are generally not printed if passed as NULL.
 * The special cases are for prefix where "ERROR" is assumed and for left
 * and right where a non-failure message is produced if either is NULL.
 */
void test_fail_message_prefix(const char *prefix, const char *file,
                              int line, const char *type,
                              const char *left, const char *right,
                              const char *op)
{
    test_printf_stderr("%s: ", prefix != NULL ? prefix : "ERROR");
    if (type)
        test_printf_stderr("(%s) ", type);
    if (op != NULL) {
        if (left != NULL && right != NULL)
            test_printf_stderr("'%s %s %s' failed", left, op, right);
        else
            test_printf_stderr("'%s'", op);
    }
    if (file != NULL) {
        test_printf_stderr(" @ %s:%d", file, line);
    }
    test_printf_stderr("\n");
}

/*
 * A common routine to output test failure messages.  Generally this should not
 * be called directly, rather it should be called by the following functions.
 *
 * |desc| is a printf formatted description with arguments |args| that is
 * supplied by the user and |desc| can be NULL.  |type| is the data type
 * that was tested (int, char, ptr, ...).  |fmt| is a system provided
 * printf format with following arguments that spell out the failure
 * details i.e. the actual values compared and the operator used.
 *
 * The typical use for this is from an utility test function:
 *
 * int test6(const char *file, int line, int n) {
 *     if (n != 6) {
 *         test_fail_message(1, file, line, "int", "value %d is not %d", n, 6);
 *         return 0;
 *     }
 *     return 1;
 * }
 *
 * calling test6(3, "oops") will return 0 and produce out along the lines of:
 *      FAIL oops: (int) value 3 is not 6\n
 */
static void test_fail_message(const char *prefix, const char *file, int line,
                              const char *type, const char *left,
                              const char *right, const char *op,
                              const char *fmt, ...)
            PRINTF_FORMAT(8, 9);

static void test_fail_message_va(const char *prefix, const char *file,
                                 int line, const char *type,
                                 const char *left, const char *right,
                                 const char *op, const char *fmt, va_list ap)
{
    test_fail_message_prefix(prefix, file, line, type, left, right, op);
    if (fmt != NULL) {
        test_vprintf_stderr(fmt, ap);
        test_printf_stderr("\n");
    }
    test_flush_stderr();
}

static void test_fail_message(const char *prefix, const char *file,
                              int line, const char *type,
                              const char *left, const char *right,
                              const char *op, const char *fmt, ...)
{
    va_list ap;

    va_start(ap, fmt);
    test_fail_message_va(prefix, file, line, type, left, right, op, fmt, ap);
    va_end(ap);
}

void test_info_c90(const char *desc, ...)
{
    va_list ap;

    va_start(ap, desc);
    test_fail_message_va("INFO", NULL, -1, NULL, NULL, NULL, NULL, desc, ap);
    va_end(ap);
}

void test_info(const char *file, int line, const char *desc, ...)
{
    va_list ap;

    va_start(ap, desc);
    test_fail_message_va("INFO", file, line, NULL, NULL, NULL, NULL, desc, ap);
    va_end(ap);
}

void test_error_c90(const char *desc, ...)
{
    va_list ap;

    va_start(ap, desc);
    test_fail_message_va(NULL, NULL, -1, NULL, NULL, NULL, NULL, desc, ap);
    va_end(ap);
    test_printf_stderr("\n");
}

void test_error(const char *file, int line, const char *desc, ...)
{
    va_list ap;

    va_start(ap, desc);
    test_fail_message_va(NULL, file, line, NULL, NULL, NULL, NULL, desc, ap);
    va_end(ap);
    test_printf_stderr("\n");
}

void test_perror(const char *s)
{
    /*
     * Using openssl_strerror_r causes linking issues since it isn't
     * exported from libcrypto.so
     */
    TEST_error("%s: %s", s, strerror(errno));
}

void test_note(const char *fmt, ...)
{
    test_flush_stdout();
    if (fmt != NULL) {
        va_list ap;

        va_start(ap, fmt);
        test_vprintf_stderr(fmt, ap);
        va_end(ap);
        test_printf_stderr("\n");
    }
    test_flush_stderr();
}


int test_skip(const char *file, int line, const char *desc, ...)
{
    va_list ap;

    va_start(ap, desc);
    test_fail_message_va("SKIP", file, line, NULL, NULL, NULL, NULL, desc, ap);
    va_end(ap);
    return TEST_SKIP_CODE;
}

int test_skip_c90(const char *desc, ...)
{
    va_list ap;

    va_start(ap, desc);
    test_fail_message_va("SKIP", NULL, -1, NULL, NULL, NULL, NULL, desc, ap);
    va_end(ap);
    test_printf_stderr("\n");
    return TEST_SKIP_CODE;
}


void test_openssl_errors(void)
{
    ERR_print_errors_cb(openssl_error_cb, NULL);
    ERR_clear_error();
}

/*
 * Define some comparisons between pairs of various types.
 * These functions return 1 if the test is true.
 * Otherwise, they return 0 and pretty-print diagnostics.
 *
 * In each case the functions produced are:
 *  int test_name_eq(const type t1, const type t2, const char *desc, ...);
 *  int test_name_ne(const type t1, const type t2, const char *desc, ...);
 *  int test_name_lt(const type t1, const type t2, const char *desc, ...);
 *  int test_name_le(const type t1, const type t2, const char *desc, ...);
 *  int test_name_gt(const type t1, const type t2, const char *desc, ...);
 *  int test_name_ge(const type t1, const type t2, const char *desc, ...);
 *
 * The t1 and t2 arguments are to be compared for equality, inequality,
 * less than, less than or equal to, greater than and greater than or
 * equal to respectively.  If the specified condition holds, the functions
 * return 1.  If the condition does not hold, the functions print a diagnostic
 * message and return 0.
 *
 * The desc argument is a printf format string followed by its arguments and
 * this is included in the output if the condition being tested for is false.
 */
#define DEFINE_COMPARISON(type, name, opname, op, fmt, cast)            \
    int test_ ## name ## _ ## opname(const char *file, int line,        \
                                     const char *s1, const char *s2,    \
                                     const type t1, const type t2)      \
    {                                                                   \
        if (t1 op t2)                                                   \
            return 1;                                                   \
        test_fail_message(NULL, file, line, #type, s1, s2, #op,         \
                          "[" fmt "] compared to [" fmt "]",            \
                          (cast)t1, (cast)t2);                          \
        return 0;                                                       \
    }

#define DEFINE_COMPARISONS(type, name, fmt, cast)                       \
    DEFINE_COMPARISON(type, name, eq, ==, fmt, cast)                    \
    DEFINE_COMPARISON(type, name, ne, !=, fmt, cast)                    \
    DEFINE_COMPARISON(type, name, lt, <, fmt, cast)                     \
    DEFINE_COMPARISON(type, name, le, <=, fmt, cast)                    \
    DEFINE_COMPARISON(type, name, gt, >, fmt, cast)                     \
    DEFINE_COMPARISON(type, name, ge, >=, fmt, cast)

DEFINE_COMPARISONS(int, int, "%d", int)
DEFINE_COMPARISONS(unsigned int, uint, "%u", unsigned int)
DEFINE_COMPARISONS(char, char, "%c", char)
DEFINE_COMPARISONS(unsigned char, uchar, "%u", unsigned char)
DEFINE_COMPARISONS(long, long, "%ld", long)
DEFINE_COMPARISONS(unsigned long, ulong, "%lu", unsigned long)
DEFINE_COMPARISONS(int64_t, int64_t, "%lld", long long)
DEFINE_COMPARISONS(uint64_t, uint64_t, "%llu", unsigned long long)
DEFINE_COMPARISONS(size_t, size_t, "%zu", size_t)
DEFINE_COMPARISONS(double, double, "%g", double)

DEFINE_COMPARISON(void *, ptr, eq, ==, "%p", void *)
DEFINE_COMPARISON(void *, ptr, ne, !=, "%p", void *)

int test_ptr_null(const char *file, int line, const char *s, const void *p)
{
    if (p == NULL)
        return 1;
    test_fail_message(NULL, file, line, "ptr", s, "NULL", "==", "%p", p);
    return 0;
}

int test_ptr(const char *file, int line, const char *s, const void *p)
{
    if (p != NULL)
        return 1;
    test_fail_message(NULL, file, line, "ptr", s, "NULL", "!=", "%p", p);
    return 0;
}

int test_true(const char *file, int line, const char *s, int b)
{
    if (b)
        return 1;
    test_fail_message(NULL, file, line, "bool", s, "true", "==", "false");
    return 0;
}

int test_false(const char *file, int line, const char *s, int b)
{
    if (!b)
        return 1;
    test_fail_message(NULL, file, line, "bool", s, "false", "==", "true");
    return 0;
}

int test_str_eq(const char *file, int line, const char *st1, const char *st2,
                const char *s1, const char *s2)
{
    if (s1 == NULL && s2 == NULL)
      return 1;
    if (s1 == NULL || s2 == NULL || strcmp(s1, s2) != 0) {
        test_fail_string_message(NULL, file, line, "string", st1, st2, "==",
                                 s1, s1 == NULL ? 0 : strlen(s1),
                                 s2, s2 == NULL ? 0 : strlen(s2));
        return 0;
    }
    return 1;
}

int test_str_ne(const char *file, int line, const char *st1, const char *st2,
                const char *s1, const char *s2)
{
    if ((s1 == NULL) ^ (s2 == NULL))
      return 1;
    if (s1 == NULL || strcmp(s1, s2) == 0) {
        test_fail_string_message(NULL, file, line, "string", st1, st2, "!=",
                                 s1, s1 == NULL ? 0 : strlen(s1),
                                 s2, s2 == NULL ? 0 : strlen(s2));
        return 0;
    }
    return 1;
}

int test_strn_eq(const char *file, int line, const char *st1, const char *st2,
                 const char *s1, size_t n1, const char *s2, size_t n2)
{
    if (s1 == NULL && s2 == NULL)
      return 1;
    if (n1 != n2 || s1 == NULL || s2 == NULL || strncmp(s1, s2, n1) != 0) {
        test_fail_string_message(NULL, file, line, "string", st1, st2, "==",
                                 s1, s1 == NULL ? 0 : OPENSSL_strnlen(s1, n1),
                                 s2, s2 == NULL ? 0 : OPENSSL_strnlen(s2, n2));
        return 0;
    }
    return 1;
}

int test_strn_ne(const char *file, int line, const char *st1, const char *st2,
                 const char *s1, size_t n1, const char *s2, size_t n2)
{
    if ((s1 == NULL) ^ (s2 == NULL))
      return 1;
    if (n1 != n2 || s1 == NULL || strncmp(s1, s2, n1) == 0) {
        test_fail_string_message(NULL, file, line, "string", st1, st2, "!=",
                                 s1, s1 == NULL ? 0 : OPENSSL_strnlen(s1, n1),
                                 s2, s2 == NULL ? 0 : OPENSSL_strnlen(s2, n2));
        return 0;
    }
    return 1;
}

int test_mem_eq(const char *file, int line, const char *st1, const char *st2,
                const void *s1, size_t n1, const void *s2, size_t n2)
{
    if (s1 == NULL && s2 == NULL)
        return 1;
    if (n1 != n2 || s1 == NULL || s2 == NULL || memcmp(s1, s2, n1) != 0) {
        test_fail_memory_message(NULL, file, line, "memory", st1, st2, "==",
                                 s1, n1, s2, n2);
        return 0;
    }
    return 1;
}

int test_mem_ne(const char *file, int line, const char *st1, const char *st2,
                const void *s1, size_t n1, const void *s2, size_t n2)
{
    if ((s1 == NULL) ^ (s2 == NULL))
        return 1;
    if (n1 != n2)
        return 1;
    if (s1 == NULL || memcmp(s1, s2, n1) == 0) {
        test_fail_memory_message(NULL, file, line, "memory", st1, st2, "!=",
                                 s1, n1, s2, n2);
        return 0;
    }
    return 1;
}

#define DEFINE_BN_COMPARISONS(opname, op, zero_cond)                    \
    int test_BN_ ## opname(const char *file, int line,                  \
                           const char *s1, const char *s2,              \
                           const BIGNUM *t1, const BIGNUM *t2)          \
    {                                                                   \
        if (BN_cmp(t1, t2) op 0)                                        \
            return 1;                                                   \
        test_fail_bignum_message(NULL, file, line, "BIGNUM", s1, s2,    \
                                 #op, t1, t2);                          \
        return 0;                                                       \
    }                                                                   \
    int test_BN_ ## opname ## _zero(const char *file, int line,         \
                                    const char *s, const BIGNUM *a)     \
    {                                                                   \
        if (a != NULL &&(zero_cond))                                    \
            return 1;                                                   \
        test_fail_bignum_mono_message(NULL, file, line, "BIGNUM",       \
                                      s, "0", #op, a);                  \
        return 0;                                                       \
    }

DEFINE_BN_COMPARISONS(eq, ==, BN_is_zero(a))
DEFINE_BN_COMPARISONS(ne, !=, !BN_is_zero(a))
DEFINE_BN_COMPARISONS(gt, >,  !BN_is_negative(a) && !BN_is_zero(a))
DEFINE_BN_COMPARISONS(ge, >=, !BN_is_negative(a) || BN_is_zero(a))
DEFINE_BN_COMPARISONS(lt, <,  BN_is_negative(a) && !BN_is_zero(a))
DEFINE_BN_COMPARISONS(le, <=, BN_is_negative(a) || BN_is_zero(a))

int test_BN_eq_one(const char *file, int line, const char *s, const BIGNUM *a)
{
    if (a != NULL && BN_is_one(a))
        return 1;
    test_fail_bignum_mono_message(NULL, file, line, "BIGNUM", s, "1", "==", a);
    return 0;
}

int test_BN_odd(const char *file, int line, const char *s, const BIGNUM *a)
{
    if (a != NULL && BN_is_odd(a))
        return 1;
    test_fail_bignum_mono_message(NULL, file, line, "BIGNUM", "ODD(", ")", s, a);
    return 0;
}

int test_BN_even(const char *file, int line, const char *s, const BIGNUM *a)
{
    if (a != NULL && !BN_is_odd(a))
        return 1;
    test_fail_bignum_mono_message(NULL, file, line, "BIGNUM", "EVEN(", ")", s,
                                  a);
    return 0;
}

int test_BN_eq_word(const char *file, int line, const char *bns, const char *ws,
                    const BIGNUM *a, BN_ULONG w)
{
    BIGNUM *bw;

    if (a != NULL && BN_is_word(a, w))
        return 1;
    if ((bw = BN_new()) != NULL)
        BN_set_word(bw, w);
    test_fail_bignum_message(NULL, file, line, "BIGNUM", bns, ws, "==", a, bw);
    BN_free(bw);
    return 0;
}

int test_BN_abs_eq_word(const char *file, int line, const char *bns,
                        const char *ws, const BIGNUM *a, BN_ULONG w)
{
    BIGNUM *bw, *aa;

    if (a != NULL && BN_abs_is_word(a, w))
        return 1;
    if ((aa = BN_dup(a)) != NULL)
        BN_set_negative(aa, 0);
    if ((bw = BN_new()) != NULL)
        BN_set_word(bw, w);
    test_fail_bignum_message(NULL, file, line, "BIGNUM", bns, ws, "abs==",
                             aa, bw);
    BN_free(bw);
    BN_free(aa);
    return 0;
}

static const char *print_time(const ASN1_TIME *t)
{
    return t == NULL ? "<null>" : (const char *)ASN1_STRING_get0_data(t);
}

#define DEFINE_TIME_T_COMPARISON(opname, op)                            \
    int test_time_t_ ## opname(const char *file, int line,              \
                               const char *s1, const char *s2,          \
                               const time_t t1, const time_t t2)        \
    {                                                                   \
        ASN1_TIME *at1 = ASN1_TIME_set(NULL, t1);                       \
        ASN1_TIME *at2 = ASN1_TIME_set(NULL, t2);                       \
        int r = at1 != NULL && at2 != NULL                              \
                && ASN1_TIME_compare(at1, at2) op 0;                    \
        if (!r)                                                         \
            test_fail_message(NULL, file, line, "time_t", s1, s2, #op,  \
                              "[%s] compared to [%s]",                  \
                              print_time(at1), print_time(at2));        \
        ASN1_STRING_free(at1);                                          \
        ASN1_STRING_free(at2);                                          \
        return r;                                                       \
    }
DEFINE_TIME_T_COMPARISON(eq, ==)
DEFINE_TIME_T_COMPARISON(ne, !=)
DEFINE_TIME_T_COMPARISON(gt, >)
DEFINE_TIME_T_COMPARISON(ge, >=)
DEFINE_TIME_T_COMPARISON(lt, <)
DEFINE_TIME_T_COMPARISON(le, <=)
