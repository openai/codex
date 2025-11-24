/*
 * Copyright 2017-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "../testutil.h"
#include "output.h"
#include "tu_local.h"

#include <string.h>
#include <ctype.h>

/* The size of memory buffers to display on failure */
#define MEM_BUFFER_SIZE     (2000)
#define MAX_STRING_WIDTH    (80)
#define BN_OUTPUT_SIZE      (8)

/* Output a diff header */
static void test_diff_header(const char *left, const char *right)
{
    test_printf_stderr("--- %s\n", left);
    test_printf_stderr("+++ %s\n", right);
}

/* Formatted string output routines */
static void test_string_null_empty(const char *m, char c)
{
    if (m == NULL)
        test_printf_stderr("%4s %c NULL\n", "", c);
    else
        test_printf_stderr("%4u:%c ''\n", 0u, c);
}

static void test_fail_string_common(const char *prefix, const char *file,
                                    int line, const char *type,
                                    const char *left, const char *right,
                                    const char *op, const char *m1, size_t l1,
                                    const char *m2, size_t l2)
{
    const size_t width =
        (MAX_STRING_WIDTH - BIO_get_indent(bio_err) - 12) / 16 * 16;
    char b1[MAX_STRING_WIDTH + 1], b2[MAX_STRING_WIDTH + 1];
    char bdiff[MAX_STRING_WIDTH + 1];
    size_t n1, n2, i;
    unsigned int cnt = 0, diff;

    test_fail_message_prefix(prefix, file, line, type, left, right, op);
    if (m1 == NULL)
        l1 = 0;
    if (m2 == NULL)
        l2 = 0;
    if (l1 == 0 && l2 == 0) {
        if ((m1 == NULL) == (m2 == NULL)) {
            test_string_null_empty(m1, ' ');
        } else {
            test_diff_header(left, right);
            test_string_null_empty(m1, '-');
            test_string_null_empty(m2, '+');
        }
        goto fin;
    }

    if (l1 != l2 || strncmp(m1, m2, l1) != 0)
        test_diff_header(left, right);

    while (l1 > 0 || l2 > 0) {
        n1 = n2 = 0;
        if (l1 > 0) {
            b1[n1 = l1 > width ? width : l1] = 0;
            for (i = 0; i < n1; i++)
                b1[i] = isprint((unsigned char)m1[i]) ? m1[i] : '.';
        }
        if (l2 > 0) {
            b2[n2 = l2 > width ? width : l2] = 0;
            for (i = 0; i < n2; i++)
                b2[i] = isprint((unsigned char)m2[i]) ? m2[i] : '.';
        }
        diff = 0;
        i = 0;
        if (n1 > 0 && n2 > 0) {
            const size_t j = n1 < n2 ? n1 : n2;

            for (; i < j; i++)
                if (m1[i] == m2[i]) {
                    bdiff[i] = ' ';
                } else {
                    bdiff[i] = '^';
                    diff = 1;
                }
            bdiff[i] = '\0';
        }
        if (n1 == n2 && !diff) {
            test_printf_stderr("%4u:  '%s'\n", cnt, n2 > n1 ? b2 : b1);
        } else {
            if (cnt == 0 && (m1 == NULL || *m1 == '\0'))
                test_string_null_empty(m1, '-');
            else if (n1 > 0)
                test_printf_stderr("%4u:- '%s'\n", cnt, b1);
            if (cnt == 0 && (m2 == NULL || *m2 == '\0'))
               test_string_null_empty(m2, '+');
            else if (n2 > 0)
                test_printf_stderr("%4u:+ '%s'\n", cnt, b2);
            if (diff && i > 0)
                test_printf_stderr("%4s    %s\n", "", bdiff);
        }
        if (m1 != NULL)
            m1 += n1;
        if (m2 != NULL)
            m2 += n2;
        l1 -= n1;
        l2 -= n2;
        cnt += width;
    }
fin:
    test_flush_stderr();
}

/*
 * Wrapper routines so that the underlying code can be shared.
 * The first is the call from inside the test utilities when a conditional
 * fails.  The second is the user's call to dump a string.
 */
void test_fail_string_message(const char *prefix, const char *file,
                              int line, const char *type,
                              const char *left, const char *right,
                              const char *op, const char *m1, size_t l1,
                              const char *m2, size_t l2)
{
    test_fail_string_common(prefix, file, line, type, left, right, op,
                            m1, l1, m2, l2);
    test_printf_stderr("\n");
}

void test_output_string(const char *name, const char *m, size_t l)
{
    test_fail_string_common("string", NULL, 0, NULL, NULL, NULL, name,
                            m, l, m, l);
}

/* BIGNUM formatted output routines */

/*
 * A basic memory byte to hex digit converter with allowance for spacing
 * every so often.
 */
static void hex_convert_memory(const unsigned char *m, size_t n, char *b,
                               size_t width)
{
    size_t i;

    for (i = 0; i < n; i++) {
        const unsigned char c = *m++;

        *b++ = "0123456789abcdef"[c >> 4];
        *b++ = "0123456789abcdef"[c & 15];
        if (i % width == width - 1 && i != n - 1)
            *b++ = ' ';
    }
    *b = '\0';
}

/*
 * Constants to define the number of bytes to display per line and the number
 * of characters these take.
 */
static const int bn_bytes = (MAX_STRING_WIDTH - 9) / (BN_OUTPUT_SIZE * 2 + 1)
                            * BN_OUTPUT_SIZE;
static const int bn_chars = (MAX_STRING_WIDTH - 9) / (BN_OUTPUT_SIZE * 2 + 1)
                            * (BN_OUTPUT_SIZE * 2 + 1) - 1;

/*
 * Output the header line for the bignum
 */
static void test_bignum_header_line(void)
{
    test_printf_stderr(" %*s\n", bn_chars + 6, "bit position");
}

static const char *test_bignum_zero_null(const BIGNUM *bn)
{
    if (bn != NULL)
        return BN_is_negative(bn) ? "-0" : "0";
    return "NULL";
}

/*
 * Print a bignum zero taking care to include the correct sign.
 * This routine correctly deals with a NULL bignum pointer as input.
 */
static void test_bignum_zero_print(const BIGNUM *bn, char sep)
{
    const char *v = test_bignum_zero_null(bn);
    const char *suf = bn != NULL ? ":    0" : "";

    test_printf_stderr("%c%*s%s\n", sep, bn_chars, v, suf);
}

/*
 * Convert a section of memory from inside a bignum into a displayable
 * string with appropriate visual aid spaces inserted.
 */
static int convert_bn_memory(const unsigned char *in, size_t bytes,
                             char *out, int *lz, const BIGNUM *bn)
{
    int n = bytes * 2, i;
    char *p = out, *q = NULL;
    const char *r;

    if (bn != NULL && !BN_is_zero(bn)) {
        hex_convert_memory(in, bytes, out, BN_OUTPUT_SIZE);
        if (*lz) {
            for (; *p == '0' || *p == ' '; p++)
                if (*p == '0') {
                    q = p;
                    *p = ' ';
                    n--;
                }
            if (*p == '\0') {
                /*
                 * in[bytes] is defined because we're converting a non-zero
                 * number and we've not seen a non-zero yet.
                 */
                if ((in[bytes] & 0xf0) != 0 && BN_is_negative(bn)) {
                    *lz = 0;
                    *q = '-';
                    n++;
                }
            } else {
                *lz = 0;
                if (BN_is_negative(bn)) {
                    /*
                     * This is valid because we always convert more digits than
                     * the number holds.
                     */
                    *q = '-';
                    n++;
                }
            }
        }
       return n;
    }

    for (i = 0; i < n; i++) {
        *p++ = ' ';
        if (i % (2 * BN_OUTPUT_SIZE) == 2 * BN_OUTPUT_SIZE - 1 && i != n - 1)
            *p++ = ' ';
    }
    *p = '\0';
    if (bn == NULL)
        r = "NULL";
    else
        r = BN_is_negative(bn) ? "-0" : "0";
    strcpy(p - strlen(r), r);
    return 0;
}

/*
 * Common code to display either one or two bignums, including the diff
 * pointers for changes (only when there are two).
 */
static void test_fail_bignum_common(const char *prefix, const char *file,
                                    int line, const char *type,
                                    const char *left, const char *right,
                                    const char *op,
                                    const BIGNUM *bn1, const BIGNUM *bn2)
{
    const size_t bytes = bn_bytes;
    char b1[MAX_STRING_WIDTH + 1], b2[MAX_STRING_WIDTH + 1];
    char *p, bdiff[MAX_STRING_WIDTH + 1];
    size_t l1, l2, n1, n2, i, len;
    unsigned int cnt, diff, real_diff;
    unsigned char *m1 = NULL, *m2 = NULL;
    int lz1 = 1, lz2 = 1;
    unsigned char buffer[MEM_BUFFER_SIZE * 2], *bufp = buffer;

    test_fail_message_prefix(prefix, file, line, type, left, right, op);
    l1 = bn1 == NULL ? 0 : (BN_num_bytes(bn1) + (BN_is_negative(bn1) ? 1 : 0));
    l2 = bn2 == NULL ? 0 : (BN_num_bytes(bn2) + (BN_is_negative(bn2) ? 1 : 0));
    if (l1 == 0 && l2 == 0) {
        if ((bn1 == NULL) == (bn2 == NULL)) {
            test_bignum_header_line();
            test_bignum_zero_print(bn1, ' ');
        } else {
            test_diff_header(left, right);
            test_bignum_header_line();
            test_bignum_zero_print(bn1, '-');
            test_bignum_zero_print(bn2, '+');
        }
        goto fin;
    }

    if (l1 != l2 || bn1 == NULL || bn2 == NULL || BN_cmp(bn1, bn2) != 0)
        test_diff_header(left, right);
    test_bignum_header_line();

    len = ((l1 > l2 ? l1 : l2) + bytes - 1) / bytes * bytes;

    if (len > MEM_BUFFER_SIZE && (bufp = OPENSSL_malloc(len * 2)) == NULL) {
        bufp = buffer;
        len = MEM_BUFFER_SIZE;
        test_printf_stderr("WARNING: these BIGNUMs have been truncated\n");
    }

    if (bn1 != NULL) {
        m1 = bufp;
        BN_bn2binpad(bn1, m1, len);
    }
    if (bn2 != NULL) {
        m2 = bufp + len;
        BN_bn2binpad(bn2, m2, len);
    }

    while (len > 0) {
        cnt = 8 * (len - bytes);
        n1 = convert_bn_memory(m1, bytes, b1, &lz1, bn1);
        n2 = convert_bn_memory(m2, bytes, b2, &lz2, bn2);

        diff = real_diff = 0;
        i = 0;
        p = bdiff;
        for (i=0; b1[i] != '\0'; i++)
            if (b1[i] == b2[i] || b1[i] == ' ' || b2[i] == ' ') {
                *p++ = ' ';
                diff |= b1[i] != b2[i];
            } else {
                *p++ = '^';
                real_diff = diff = 1;
            }
        *p++ = '\0';
        if (!diff) {
            test_printf_stderr(" %s:% 5d\n", n2 > n1 ? b2 : b1, cnt);
        } else {
            if (cnt == 0 && bn1 == NULL)
                test_printf_stderr("-%s\n", b1);
            else if (cnt == 0 || n1 > 0)
                test_printf_stderr("-%s:% 5d\n", b1, cnt);
            if (cnt == 0 && bn2 == NULL)
                test_printf_stderr("+%s\n", b2);
            else if (cnt == 0 || n2 > 0)
                test_printf_stderr("+%s:% 5d\n", b2, cnt);
            if (real_diff && (cnt == 0 || (n1 > 0 && n2 > 0))
                    && bn1 != NULL && bn2 != NULL)
                test_printf_stderr(" %s\n", bdiff);
        }
        if (m1 != NULL)
            m1 += bytes;
        if (m2 != NULL)
            m2 += bytes;
        len -= bytes;
    }
fin:
    test_flush_stderr();
    if (bufp != buffer)
        OPENSSL_free(bufp);
}

/*
 * Wrapper routines so that the underlying code can be shared.
 * The first two are calls from inside the test utilities when a conditional
 * fails.  The third is the user's call to dump a bignum.
 */
void test_fail_bignum_message(const char *prefix, const char *file,
                              int line, const char *type,
                              const char *left, const char *right,
                              const char *op,
                              const BIGNUM *bn1, const BIGNUM *bn2)
{
    test_fail_bignum_common(prefix, file, line, type, left, right, op, bn1, bn2);
    test_printf_stderr("\n");
}

void test_fail_bignum_mono_message(const char *prefix, const char *file,
                                   int line, const char *type,
                                   const char *left, const char *right,
                                   const char *op, const BIGNUM *bn)
{
    test_fail_bignum_common(prefix, file, line, type, left, right, op, bn, bn);
    test_printf_stderr("\n");
}

void test_output_bignum(const char *name, const BIGNUM *bn)
{
    if (bn == NULL || BN_is_zero(bn)) {
        test_printf_stderr("bignum: '%s' = %s\n", name,
                           test_bignum_zero_null(bn));
    } else if (BN_num_bytes(bn) <= BN_OUTPUT_SIZE) {
        unsigned char buf[BN_OUTPUT_SIZE];
        char out[2 * sizeof(buf) + 1];
        char *p = out;
        int n = BN_bn2bin(bn, buf);

        hex_convert_memory(buf, n, p, BN_OUTPUT_SIZE);
        while (*p == '0' && *++p != '\0')
            ;
        test_printf_stderr("bignum: '%s' = %s0x%s\n", name,
                           BN_is_negative(bn) ? "-" : "", p);
    } else {
        test_fail_bignum_common("bignum", NULL, 0, NULL, NULL, NULL, name,
                                bn, bn);
    }
}

/* Memory output routines */

/*
 * Handle zero length blocks of memory or NULL pointers to memory
 */
static void test_memory_null_empty(const unsigned char *m, char c)
{
    if (m == NULL)
        test_printf_stderr("%4s %c%s\n", "", c, "NULL");
    else
        test_printf_stderr("%04x %c%s\n", 0u, c, "empty");
}

/*
 * Common code to display one or two blocks of memory.
 */
static void test_fail_memory_common(const char *prefix, const char *file,
                                    int line, const char *type,
                                    const char *left, const char *right,
                                    const char *op,
                                    const unsigned char *m1, size_t l1,
                                    const unsigned char *m2, size_t l2)
{
    const size_t bytes = (MAX_STRING_WIDTH - 9) / 17 * 8;
    char b1[MAX_STRING_WIDTH + 1], b2[MAX_STRING_WIDTH + 1];
    char *p, bdiff[MAX_STRING_WIDTH + 1];
    size_t n1, n2, i;
    unsigned int cnt = 0, diff;

    test_fail_message_prefix(prefix, file, line, type, left, right, op);
    if (m1 == NULL)
        l1 = 0;
    if (m2 == NULL)
        l2 = 0;
    if (l1 == 0 && l2 == 0) {
        if ((m1 == NULL) == (m2 == NULL)) {
            test_memory_null_empty(m1, ' ');
        } else {
            test_diff_header(left, right);
            test_memory_null_empty(m1, '-');
            test_memory_null_empty(m2, '+');
        }
        goto fin;
    }

    if (l1 != l2 || (m1 != m2 && memcmp(m1, m2, l1) != 0))
        test_diff_header(left, right);

    while (l1 > 0 || l2 > 0) {
        n1 = n2 = 0;
        if (l1 > 0) {
            n1 = l1 > bytes ? bytes : l1;
            hex_convert_memory(m1, n1, b1, 8);
        }
        if (l2 > 0) {
            n2 = l2 > bytes ? bytes : l2;
            hex_convert_memory(m2, n2, b2, 8);
        }

        diff = 0;
        i = 0;
        p = bdiff;
        if (n1 > 0 && n2 > 0) {
            const size_t j = n1 < n2 ? n1 : n2;

            for (; i < j; i++) {
                if (m1[i] == m2[i]) {
                    *p++ = ' ';
                    *p++ = ' ';
                } else {
                    *p++ = '^';
                    *p++ = '^';
                    diff = 1;
                }
                if (i % 8 == 7 && i != j - 1)
                    *p++ = ' ';
            }
            *p++ = '\0';
        }

        if (n1 == n2 && !diff) {
            test_printf_stderr("%04x: %s\n", cnt, b1);
        } else {
            if (cnt == 0 && (m1 == NULL || l1 == 0))
                test_memory_null_empty(m1, '-');
            else if (n1 > 0)
                test_printf_stderr("%04x:-%s\n", cnt, b1);
            if (cnt == 0 && (m2 == NULL || l2 == 0))
                test_memory_null_empty(m2, '+');
            else if (n2 > 0)
                test_printf_stderr("%04x:+%s\n", cnt, b2);
            if (diff && i > 0)
                test_printf_stderr("%4s  %s\n", "", bdiff);
        }
        if (m1 != NULL)
            m1 += n1;
        if (m2 != NULL)
            m2 += n2;
        l1 -= n1;
        l2 -= n2;
        cnt += bytes;
    }
fin:
    test_flush_stderr();
}

/*
 * Wrapper routines so that the underlying code can be shared.
 * The first is the call from inside the test utilities when a conditional
 * fails.  The second is the user's call to dump memory.
 */
void test_fail_memory_message(const char *prefix, const char *file,
                              int line, const char *type,
                              const char *left, const char *right,
                              const char *op,
                              const unsigned char *m1, size_t l1,
                              const unsigned char *m2, size_t l2)
{
    test_fail_memory_common(prefix, file, line, type, left, right, op,
                            m1, l1, m2, l2);
    test_printf_stderr("\n");
}

void test_output_memory(const char *name, const unsigned char *m, size_t l)
{
    test_fail_memory_common("memory", NULL, 0, NULL, NULL, NULL, name,
                            m, l, m, l);
}
