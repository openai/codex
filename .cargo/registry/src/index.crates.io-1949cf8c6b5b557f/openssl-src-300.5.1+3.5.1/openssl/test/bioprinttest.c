/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define TESTUTIL_NO_size_t_COMPARISON

#include <stdio.h>
#include <string.h>
#include <openssl/bio.h>
#include "internal/nelem.h"
#include "internal/numbers.h"
#include "testutil.h"
#include "testutil/output.h"

static int justprint = 0;

static char *fpexpected[][11][5] = {
    {
        /*  0.00 */ { "0.0000e+00", "0.0000", "0", "0.0000E+00", "0" },
        /*  0.01 */ { "6.7000e-01", "0.6700", "0.67", "6.7000E-01", "0.67" },
        /*  0.02 */ { "6.6667e-01", "0.6667", "0.6667", "6.6667E-01", "0.6667" },
        /*  0.03 */ { "6.6667e-04", "0.0007", "0.0006667", "6.6667E-04", "0.0006667" },
        /*  0.04 */ { "6.6667e-05", "0.0001", "6.667e-05", "6.6667E-05", "6.667E-05" },
        /*  0.05 */ { "6.6667e+00", "6.6667", "6.667", "6.6667E+00", "6.667" },
        /*  0.06 */ { "6.6667e+01", "66.6667", "66.67", "6.6667E+01", "66.67" },
        /*  0.07 */ { "6.6667e+02", "666.6667", "666.7", "6.6667E+02", "666.7" },
        /*  0.08 */ { "6.6667e+03", "6666.6667", "6667", "6.6667E+03", "6667" },
        /*  0.09 */ { "6.6667e+04", "66666.6667", "6.667e+04", "6.6667E+04", "6.667E+04" },
        /*  0.10 */ { "-6.6667e+04", "-66666.6667", "-6.667e+04", "-6.6667E+04", "-6.667E+04" },
    },
    {
        /*  1.00 */ { "0.00000e+00", "0.00000", "0", "0.00000E+00", "0" },
        /*  1.01 */ { "6.70000e-01", "0.67000", "0.67", "6.70000E-01", "0.67" },
        /*  1.02 */ { "6.66667e-01", "0.66667", "0.66667", "6.66667E-01", "0.66667" },
        /*  1.03 */ { "6.66667e-04", "0.00067", "0.00066667", "6.66667E-04", "0.00066667" },
        /*  1.04 */ { "6.66667e-05", "0.00007", "6.6667e-05", "6.66667E-05", "6.6667E-05" },
        /*  1.05 */ { "6.66667e+00", "6.66667", "6.6667", "6.66667E+00", "6.6667" },
        /*  1.06 */ { "6.66667e+01", "66.66667", "66.667", "6.66667E+01", "66.667" },
        /*  1.07 */ { "6.66667e+02", "666.66667", "666.67", "6.66667E+02", "666.67" },
        /*  1.08 */ { "6.66667e+03", "6666.66667", "6666.7", "6.66667E+03", "6666.7" },
        /*  1.09 */ { "6.66667e+04", "66666.66667", "66667", "6.66667E+04", "66667" },
        /*  1.10 */ { "-6.66667e+04", "-66666.66667", "-66667", "-6.66667E+04", "-66667" },
    },
    {
        /*  2.00 */ { "  0.0000e+00", "      0.0000", "           0", "  0.0000E+00", "           0" },
        /*  2.01 */ { "  6.7000e-01", "      0.6700", "        0.67", "  6.7000E-01", "        0.67" },
        /*  2.02 */ { "  6.6667e-01", "      0.6667", "      0.6667", "  6.6667E-01", "      0.6667" },
        /*  2.03 */ { "  6.6667e-04", "      0.0007", "   0.0006667", "  6.6667E-04", "   0.0006667" },
        /*  2.04 */ { "  6.6667e-05", "      0.0001", "   6.667e-05", "  6.6667E-05", "   6.667E-05" },
        /*  2.05 */ { "  6.6667e+00", "      6.6667", "       6.667", "  6.6667E+00", "       6.667" },
        /*  2.06 */ { "  6.6667e+01", "     66.6667", "       66.67", "  6.6667E+01", "       66.67" },
        /*  2.07 */ { "  6.6667e+02", "    666.6667", "       666.7", "  6.6667E+02", "       666.7" },
        /*  2.08 */ { "  6.6667e+03", "   6666.6667", "        6667", "  6.6667E+03", "        6667" },
        /*  2.09 */ { "  6.6667e+04", "  66666.6667", "   6.667e+04", "  6.6667E+04", "   6.667E+04" },
        /*  2.10 */ { " -6.6667e+04", " -66666.6667", "  -6.667e+04", " -6.6667E+04", "  -6.667E+04" },
    },
    {
        /*  3.00 */ { " 0.00000e+00", "     0.00000", "           0", " 0.00000E+00", "           0" },
        /*  3.01 */ { " 6.70000e-01", "     0.67000", "        0.67", " 6.70000E-01", "        0.67" },
        /*  3.02 */ { " 6.66667e-01", "     0.66667", "     0.66667", " 6.66667E-01", "     0.66667" },
        /*  3.03 */ { " 6.66667e-04", "     0.00067", "  0.00066667", " 6.66667E-04", "  0.00066667" },
        /*  3.04 */ { " 6.66667e-05", "     0.00007", "  6.6667e-05", " 6.66667E-05", "  6.6667E-05" },
        /*  3.05 */ { " 6.66667e+00", "     6.66667", "      6.6667", " 6.66667E+00", "      6.6667" },
        /*  3.06 */ { " 6.66667e+01", "    66.66667", "      66.667", " 6.66667E+01", "      66.667" },
        /*  3.07 */ { " 6.66667e+02", "   666.66667", "      666.67", " 6.66667E+02", "      666.67" },
        /*  3.08 */ { " 6.66667e+03", "  6666.66667", "      6666.7", " 6.66667E+03", "      6666.7" },
        /*  3.09 */ { " 6.66667e+04", " 66666.66667", "       66667", " 6.66667E+04", "       66667" },
        /*  3.10 */ { "-6.66667e+04", "-66666.66667", "      -66667", "-6.66667E+04", "      -66667" },
    },
    {
        /*  4.00 */ { "0e+00", "0", "0", "0E+00", "0" },
        /*  4.01 */ { "7e-01", "1", "0.7", "7E-01", "0.7" },
        /*  4.02 */ { "7e-01", "1", "0.7", "7E-01", "0.7" },
        /*  4.03 */ { "7e-04", "0", "0.0007", "7E-04", "0.0007" },
        /*  4.04 */ { "7e-05", "0", "7e-05", "7E-05", "7E-05" },
        /*  4.05 */ { "7e+00", "7", "7", "7E+00", "7" },
        /*  4.06 */ { "7e+01", "67", "7e+01", "7E+01", "7E+01" },
        /*  4.07 */ { "7e+02", "667", "7e+02", "7E+02", "7E+02" },
        /*  4.08 */ { "7e+03", "6667", "7e+03", "7E+03", "7E+03" },
        /*  4.09 */ { "7e+04", "66667", "7e+04", "7E+04", "7E+04" },
        /*  4.10 */ { "-7e+04", "-66667", "-7e+04", "-7E+04", "-7E+04" },
    },
    {
        /*  5.00 */ { "0.000000e+00", "0.000000", "0", "0.000000E+00", "0" },
        /*  5.01 */ { "6.700000e-01", "0.670000", "0.67", "6.700000E-01", "0.67" },
        /*  5.02 */ { "6.666667e-01", "0.666667", "0.666667", "6.666667E-01", "0.666667" },
        /*  5.03 */ { "6.666667e-04", "0.000667", "0.000666667", "6.666667E-04", "0.000666667" },
        /*  5.04 */ { "6.666667e-05", "0.000067", "6.66667e-05", "6.666667E-05", "6.66667E-05" },
        /*  5.05 */ { "6.666667e+00", "6.666667", "6.66667", "6.666667E+00", "6.66667" },
        /*  5.06 */ { "6.666667e+01", "66.666667", "66.6667", "6.666667E+01", "66.6667" },
        /*  5.07 */ { "6.666667e+02", "666.666667", "666.667", "6.666667E+02", "666.667" },
        /*  5.08 */ { "6.666667e+03", "6666.666667", "6666.67", "6.666667E+03", "6666.67" },
        /*  5.09 */ { "6.666667e+04", "66666.666667", "66666.7", "6.666667E+04", "66666.7" },
        /*  5.10 */ { "-6.666667e+04", "-66666.666667", "-66666.7", "-6.666667E+04", "-66666.7" },
    },
    {
        /*  6.00 */ { "0.0000e+00", "000.0000", "00000000", "0.0000E+00", "00000000" },
        /*  6.01 */ { "6.7000e-01", "000.6700", "00000.67", "6.7000E-01", "00000.67" },
        /*  6.02 */ { "6.6667e-01", "000.6667", "000.6667", "6.6667E-01", "000.6667" },
        /*  6.03 */ { "6.6667e-04", "000.0007", "0.0006667", "6.6667E-04", "0.0006667" },
        /*  6.04 */ { "6.6667e-05", "000.0001", "6.667e-05", "6.6667E-05", "6.667E-05" },
        /*  6.05 */ { "6.6667e+00", "006.6667", "0006.667", "6.6667E+00", "0006.667" },
        /*  6.06 */ { "6.6667e+01", "066.6667", "00066.67", "6.6667E+01", "00066.67" },
        /*  6.07 */ { "6.6667e+02", "666.6667", "000666.7", "6.6667E+02", "000666.7" },
        /*  6.08 */ { "6.6667e+03", "6666.6667", "00006667", "6.6667E+03", "00006667" },
        /*  6.09 */ { "6.6667e+04", "66666.6667", "6.667e+04", "6.6667E+04", "6.667E+04" },
        /*  6.10 */ { "-6.6667e+04", "-66666.6667", "-6.667e+04", "-6.6667E+04", "-6.667E+04" },
    },
};

typedef struct z_data_st {
    size_t value;
    const char *format;
    const char *expected;
} z_data;

static z_data zu_data[] = {
    { SIZE_MAX, "%zu", (sizeof(size_t) == 4 ? "4294967295"
                        : sizeof(size_t) == 8 ? "18446744073709551615"
                        : "") },
    /*
     * in 2-complement, the unsigned number divided by two plus one becomes the
     * smallest possible negative signed number of the corresponding type
     */
    { SIZE_MAX / 2 + 1, "%zi", (sizeof(size_t) == 4 ? "-2147483648"
                                : sizeof(size_t) == 8 ? "-9223372036854775808"
                                : "") },
    { 0, "%zu", "0" },
    { 0, "%zi", "0" },
};

static int test_zu(int i)
{
    char bio_buf[80];
    const z_data *data = &zu_data[i];

    BIO_snprintf(bio_buf, sizeof(bio_buf) - 1, data->format, data->value);
    if (!TEST_str_eq(bio_buf, data->expected))
        return 0;
    return 1;
}

typedef struct j_data_st {
    uint64_t value;
    const char *format;
    const char *expected;
} j_data;

static j_data jf_data[] = {
    { 0xffffffffffffffffULL, "%ju", "18446744073709551615" },
    { 0xffffffffffffffffULL, "%jx", "ffffffffffffffff" },
    { 0x8000000000000000ULL, "%ju", "9223372036854775808" },
    /*
     * These tests imply two's complement, but it's the only binary
     * representation we support, see test/sanitytest.c...
     */
    { 0x8000000000000000ULL, "%ji", "-9223372036854775808" },
};

static int test_j(int i)
{
    const j_data *data = &jf_data[i];
    char bio_buf[80];

    BIO_snprintf(bio_buf, sizeof(bio_buf) - 1, data->format, data->value);
    if (!TEST_str_eq(bio_buf, data->expected))
        return 0;
    return 1;
}


/* Precision and width. */
typedef struct pw_st {
    int p;
    const char *w;
} pw;

static pw pw_params[] = {
    { 4, "" },
    { 5, "" },
    { 4, "12" },
    { 5, "12" },
    { 0, "" },
    { -1, "" },
    { 4, "08" }
};

static int dofptest(int test, int sub, double val, const char *width, int prec)
{
    static const char *fspecs[] = {
        "e", "f", "g", "E", "G"
    };
    char format[80], result[80];
    int ret = 1, i;

    for (i = 0; i < (int)OSSL_NELEM(fspecs); i++) {
        const char *fspec = fspecs[i];

        if (prec >= 0)
            BIO_snprintf(format, sizeof(format), "%%%s.%d%s", width, prec,
                         fspec);
        else
            BIO_snprintf(format, sizeof(format), "%%%s%s", width, fspec);
        BIO_snprintf(result, sizeof(result), format, val);

        if (justprint) {
            if (i == 0)
                printf("    /*  %d.%02d */ { \"%s\"", test, sub, result);
            else
                printf(", \"%s\"", result);
        } else if (!TEST_str_eq(fpexpected[test][sub][i], result)) {
            TEST_info("test %d format=|%s| exp=|%s|, ret=|%s|",
                    test, format, fpexpected[test][sub][i], result);
            ret = 0;
        }
    }
    if (justprint)
        printf(" },\n");
    return ret;
}

static int test_fp(int i)
{
    int t = 0, r;
    const double frac = 2.0 / 3.0;
    const pw *pwp = &pw_params[i];

    if (justprint)
        printf("    {\n");
    r = TEST_true(dofptest(i, t++, 0.0, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, 0.67, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, frac, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, frac / 1000, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, frac / 10000, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, 6.0 + frac, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, 66.0 + frac, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, 666.0 + frac, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, 6666.0 + frac, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, 66666.0 + frac, pwp->w, pwp->p))
        && TEST_true(dofptest(i, t++, -66666.0 - frac, pwp->w, pwp->p));
    if (justprint)
        printf("    },\n");
    return r;
}

static int test_big(void)
{
    char buf[80];

    /* Test excessively big number. Should fail */
    if (!TEST_int_eq(BIO_snprintf(buf, sizeof(buf),
                                  "%f\n", 2 * (double)ULONG_MAX), -1))
        return 0;

    return 1;
}

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_PRINT,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "expected", OPT_PRINT, '-', "Output values" },
        { NULL }
    };
    return options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_PRINT:
            justprint = 1;
            break;
        case OPT_TEST_CASES:
            break;
        default:
            return 0;
        }
    }

    ADD_TEST(test_big);
    ADD_ALL_TESTS(test_fp, OSSL_NELEM(pw_params));
    ADD_ALL_TESTS(test_zu, OSSL_NELEM(zu_data));
    ADD_ALL_TESTS(test_j, OSSL_NELEM(jf_data));
    return 1;
}

/*
 * Replace testutil output routines.  We do this to eliminate possible sources
 * of BIO error
 */
BIO *bio_out = NULL;
BIO *bio_err = NULL;

static int tap_level = 0;

void test_open_streams(void)
{
}

void test_adjust_streams_tap_level(int level)
{
    tap_level = level;
}

void test_close_streams(void)
{
}

/*
 * This works out as long as caller doesn't use any "fancy" formats.
 * But we are caller's caller, and test_str_eq is the only one called,
 * and it uses only "%s", which is not "fancy"...
 */
int test_vprintf_stdout(const char *fmt, va_list ap)
{
    return fprintf(stdout, "%*s# ", tap_level, "") + vfprintf(stdout, fmt, ap);
}

int test_vprintf_stderr(const char *fmt, va_list ap)
{
    return fprintf(stderr, "%*s# ", tap_level, "") + vfprintf(stderr, fmt, ap);
}

int test_flush_stdout(void)
{
    return fflush(stdout);
}

int test_flush_stderr(void)
{
    return fflush(stderr);
}

int test_vprintf_tapout(const char *fmt, va_list ap)
{
    return fprintf(stdout, "%*s", tap_level, "") + vfprintf(stdout, fmt, ap);
}

int test_vprintf_taperr(const char *fmt, va_list ap)
{
    return fprintf(stderr, "%*s", tap_level, "") + vfprintf(stderr, fmt, ap);
}

int test_flush_tapout(void)
{
    return fflush(stdout);
}

int test_flush_taperr(void)
{
    return fflush(stderr);
}

