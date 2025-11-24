/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include <openssl/rand.h>
#include <openssl/asn1t.h>
#include "internal/numbers.h"
#include "testutil.h"

#ifdef __GNUC__
# pragma GCC diagnostic ignored "-Wunused-function"
# pragma GCC diagnostic ignored "-Wformat"
#endif
#ifdef __clang__
# pragma clang diagnostic ignored "-Wunused-function"
# pragma clang diagnostic ignored "-Wformat"
#endif

/***** Custom test data ******************************************************/

/*
 * We conduct tests with these arrays for every type we try out.
 * You will find the expected results together with the test structures
 * for each type, further down.
 */

static unsigned char t_zero[] = {
    0x00
};
static unsigned char t_one[] = {
    0x01
};
static unsigned char t_one_neg[] = {
    0xff
};
static unsigned char t_minus_256[] = {
    0xff, 0x00
};
static unsigned char t_longundef[] = {
    0x7f, 0xff, 0xff, 0xff
};
static unsigned char t_9bytes_1[] = {
    0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
};
static unsigned char t_8bytes_1[] = {
    0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
};
static unsigned char t_8bytes_2[] = {
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
};
static unsigned char t_8bytes_3_pad[] = {
    0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
};
static unsigned char t_8bytes_4_neg[] = {
    0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
};
static unsigned char t_8bytes_5_negpad[] = {
    0xff, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
};

/* 32-bit long */
static unsigned char t_5bytes_1[] = {
    0x01, 0xff, 0xff, 0xff, 0xff
};
static unsigned char t_4bytes_1[] = {
    0x00, 0x80, 0x00, 0x00, 0x00
};
/* We make the last byte 0xfe to avoid a clash with ASN1_LONG_UNDEF */
static unsigned char t_4bytes_2[] = {
    0x7f, 0xff, 0xff, 0xfe
};
static unsigned char t_4bytes_3_pad[] = {
    0x00, 0x7f, 0xff, 0xff, 0xfe
};
static unsigned char t_4bytes_4_neg[] = {
    0x80, 0x00, 0x00, 0x00
};
static unsigned char t_4bytes_5_negpad[] = {
    0xff, 0x80, 0x00, 0x00, 0x00
};

typedef struct {
    unsigned char *bytes1;
    size_t nbytes1;
    unsigned char *bytes2;
    size_t nbytes2;
} TEST_CUSTOM_DATA;
#define CUSTOM_DATA(v)                          \
    { v, sizeof(v), t_one, sizeof(t_one) },     \
    { t_one, sizeof(t_one), v, sizeof(v) }

static TEST_CUSTOM_DATA test_custom_data[] = {
    CUSTOM_DATA(t_zero),
    CUSTOM_DATA(t_longundef),
    CUSTOM_DATA(t_one),
    CUSTOM_DATA(t_one_neg),
    CUSTOM_DATA(t_minus_256),
    CUSTOM_DATA(t_9bytes_1),
    CUSTOM_DATA(t_8bytes_1),
    CUSTOM_DATA(t_8bytes_2),
    CUSTOM_DATA(t_8bytes_3_pad),
    CUSTOM_DATA(t_8bytes_4_neg),
    CUSTOM_DATA(t_8bytes_5_negpad),
    CUSTOM_DATA(t_5bytes_1),
    CUSTOM_DATA(t_4bytes_1),
    CUSTOM_DATA(t_4bytes_2),
    CUSTOM_DATA(t_4bytes_3_pad),
    CUSTOM_DATA(t_4bytes_4_neg),
    CUSTOM_DATA(t_4bytes_5_negpad),
};


/***** Type specific test data ***********************************************/

/*
 * First, a few utility things that all type specific data can use, or in some
 * cases, MUST use.
 */

/*
 * For easy creation of arrays of expected data.  These macros correspond to
 * the uses of CUSTOM_DATA above.
 */
#define CUSTOM_EXPECTED_SUCCESS(num, znum)      \
    { 0xff, num, 1 },                           \
    { 0xff, 1, znum }
#define CUSTOM_EXPECTED_FAILURE                 \
    { 0, 0, 0 },                                \
    { 0, 0, 0 }

/*
 * A structure to collect all test information in.  There MUST be one instance
 * of this for each test
 */
typedef int i2d_fn(void *a, unsigned char **pp);
typedef void *d2i_fn(void **a, unsigned char **pp, long length);
typedef void ifree_fn(void *a);
typedef struct {
    ASN1_ITEM_EXP *asn1_type;
    const char *name;
    int skip;                    /* 1 if this package should be skipped */

    /* An array of structures to compare decoded custom data with */
    void *encode_expectations;
    size_t encode_expectations_size;
    size_t encode_expectations_elem_size;

    /*
     * An array of structures that are encoded into a DER blob, which is
     * then decoded, and result gets compared with the original.
     */
    void *encdec_data;
    size_t encdec_data_size;
    size_t encdec_data_elem_size;

    /* The i2d function to use with this type */
    i2d_fn *i2d;
    /* The d2i function to use with this type */
    d2i_fn *d2i;
    /* Function to free a decoded structure */
    ifree_fn *ifree;
} TEST_PACKAGE;

/* To facilitate the creation of an encdec_data array */
#define ENCDEC_DATA(num, znum)                  \
    { 0xff, num, 1 }, { 0xff, 1, znum }
#define ENCDEC_ARRAY(max, zmax, min, zmin)      \
    ENCDEC_DATA(max,zmax),                      \
    ENCDEC_DATA(min,zmin),                      \
    ENCDEC_DATA(1, 1),                          \
    ENCDEC_DATA(-1, -1),                        \
    ENCDEC_DATA(0, ASN1_LONG_UNDEF)

#ifndef OPENSSL_NO_DEPRECATED_3_0
/***** LONG ******************************************************************/

typedef struct {
    /* If decoding is expected to succeed, set this to 1, otherwise 0 */
    ASN1_BOOLEAN success;
    long test_long;
    long test_zlong;
} ASN1_LONG_DATA;

ASN1_SEQUENCE(ASN1_LONG_DATA) = {
    ASN1_SIMPLE(ASN1_LONG_DATA, success, ASN1_BOOLEAN),
    ASN1_SIMPLE(ASN1_LONG_DATA, test_long, LONG),
    ASN1_EXP_OPT(ASN1_LONG_DATA, test_zlong, ZLONG, 0)
} static_ASN1_SEQUENCE_END(ASN1_LONG_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_LONG_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_LONG_DATA)

static ASN1_LONG_DATA long_expected_32bit[] = {
    /* The following should fail on the second because it's the default */
    { 0xff, 0, 1 }, { 0, 0, 0 }, /* t_zero */
    { 0, 0, 0 }, { 0xff, 1, 0x7fffffff }, /* t_longundef */
    CUSTOM_EXPECTED_SUCCESS(1, 1), /* t_one */
    CUSTOM_EXPECTED_SUCCESS(-1, -1), /* t_one_neg */
    CUSTOM_EXPECTED_SUCCESS(-256, -256), /* t_minus_256 */
    CUSTOM_EXPECTED_FAILURE,     /* t_9bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_3_pad */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_5_negpad */
    CUSTOM_EXPECTED_FAILURE,     /* t_5bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_1 (too large positive) */
    CUSTOM_EXPECTED_SUCCESS(INT32_MAX - 1, INT32_MAX -1), /* t_4bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_3_pad (illegal padding) */
    CUSTOM_EXPECTED_SUCCESS(INT32_MIN, INT32_MIN), /* t_4bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_5_negpad (illegal padding) */
};
static ASN1_LONG_DATA long_encdec_data_32bit[] = {
    ENCDEC_ARRAY(LONG_MAX - 1, LONG_MAX, LONG_MIN, LONG_MIN),
    /* Check that default numbers fail */
    { 0, ASN1_LONG_UNDEF, 1 }, { 0, 1, 0 }
};

static TEST_PACKAGE long_test_package_32bit = {
    ASN1_ITEM_ref(ASN1_LONG_DATA), "LONG", sizeof(long) != 4,
    long_expected_32bit,
    sizeof(long_expected_32bit), sizeof(long_expected_32bit[0]),
    long_encdec_data_32bit,
    sizeof(long_encdec_data_32bit), sizeof(long_encdec_data_32bit[0]),
    (i2d_fn *)i2d_ASN1_LONG_DATA, (d2i_fn *)d2i_ASN1_LONG_DATA,
    (ifree_fn *)ASN1_LONG_DATA_free
};

static ASN1_LONG_DATA long_expected_64bit[] = {
    /* The following should fail on the second because it's the default */
    { 0xff, 0, 1 }, { 0, 0, 0 }, /* t_zero */
    { 0, 0, 0 }, { 0xff, 1, 0x7fffffff }, /* t_longundef */
    CUSTOM_EXPECTED_SUCCESS(1, 1), /* t_one */
    CUSTOM_EXPECTED_SUCCESS(-1, -1), /* t_one_neg */
    CUSTOM_EXPECTED_SUCCESS(-256, -256), /* t_minus_256 */
    CUSTOM_EXPECTED_FAILURE,     /* t_9bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_1 */
    CUSTOM_EXPECTED_SUCCESS(LONG_MAX, LONG_MAX), /* t_8bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_3_pad (illegal padding) */
    CUSTOM_EXPECTED_SUCCESS(LONG_MIN, LONG_MIN), /* t_8bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_5_negpad (illegal padding) */
    CUSTOM_EXPECTED_SUCCESS((long)0x1ffffffff, (long)0x1ffffffff), /* t_5bytes_1 */
    CUSTOM_EXPECTED_SUCCESS((long)0x80000000, (long)0x80000000), /* t_4bytes_1 */
    CUSTOM_EXPECTED_SUCCESS(INT32_MAX - 1, INT32_MAX -1), /* t_4bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_3_pad (illegal padding) */
    CUSTOM_EXPECTED_SUCCESS(INT32_MIN, INT32_MIN), /* t_4bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_5_negpad (illegal padding) */
};
static ASN1_LONG_DATA long_encdec_data_64bit[] = {
    ENCDEC_ARRAY(LONG_MAX, LONG_MAX, LONG_MIN, LONG_MIN),
    /* Check that default numbers fail */
    { 0, ASN1_LONG_UNDEF, 1 }, { 0, 1, 0 }
};

static TEST_PACKAGE long_test_package_64bit = {
    ASN1_ITEM_ref(ASN1_LONG_DATA), "LONG", sizeof(long) != 8,
    long_expected_64bit,
    sizeof(long_expected_64bit), sizeof(long_expected_64bit[0]),
    long_encdec_data_64bit,
    sizeof(long_encdec_data_64bit), sizeof(long_encdec_data_64bit[0]),
    (i2d_fn *)i2d_ASN1_LONG_DATA, (d2i_fn *)d2i_ASN1_LONG_DATA,
    (ifree_fn *)ASN1_LONG_DATA_free
};
#endif

/***** INT32 *****************************************************************/

typedef struct {
    ASN1_BOOLEAN success;
    int32_t test_int32;
    int32_t test_zint32;
} ASN1_INT32_DATA;

ASN1_SEQUENCE(ASN1_INT32_DATA) = {
    ASN1_SIMPLE(ASN1_INT32_DATA, success, ASN1_BOOLEAN),
    ASN1_EMBED(ASN1_INT32_DATA, test_int32, INT32),
    ASN1_EXP_OPT_EMBED(ASN1_INT32_DATA, test_zint32, ZINT32, 0)
} static_ASN1_SEQUENCE_END(ASN1_INT32_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_INT32_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_INT32_DATA)

static ASN1_INT32_DATA int32_expected[] = {
    CUSTOM_EXPECTED_SUCCESS(0, 0), /* t_zero */
    CUSTOM_EXPECTED_SUCCESS(ASN1_LONG_UNDEF, ASN1_LONG_UNDEF), /* t_zero */
    CUSTOM_EXPECTED_SUCCESS(1, 1), /* t_one */
    CUSTOM_EXPECTED_SUCCESS(-1, -1), /* t_one_neg */
    CUSTOM_EXPECTED_SUCCESS(-256, -256), /* t_minus_256 */
    CUSTOM_EXPECTED_FAILURE,     /* t_9bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_3_pad */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_5_negpad */
    CUSTOM_EXPECTED_FAILURE,     /* t_5bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_1 (too large positive) */
    CUSTOM_EXPECTED_SUCCESS(INT32_MAX - 1, INT32_MAX -1), /* t_4bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_3_pad (illegal padding) */
    CUSTOM_EXPECTED_SUCCESS(INT32_MIN, INT32_MIN), /* t_4bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_5_negpad (illegal padding) */
};
static ASN1_INT32_DATA int32_encdec_data[] = {
    ENCDEC_ARRAY(INT32_MAX, INT32_MAX, INT32_MIN, INT32_MIN),
};

static TEST_PACKAGE int32_test_package = {
    ASN1_ITEM_ref(ASN1_INT32_DATA), "INT32", 0,
    int32_expected, sizeof(int32_expected), sizeof(int32_expected[0]),
    int32_encdec_data, sizeof(int32_encdec_data), sizeof(int32_encdec_data[0]),
    (i2d_fn *)i2d_ASN1_INT32_DATA, (d2i_fn *)d2i_ASN1_INT32_DATA,
    (ifree_fn *)ASN1_INT32_DATA_free
};

/***** UINT32 ****************************************************************/

typedef struct {
    ASN1_BOOLEAN success;
    uint32_t test_uint32;
    uint32_t test_zuint32;
} ASN1_UINT32_DATA;

ASN1_SEQUENCE(ASN1_UINT32_DATA) = {
    ASN1_SIMPLE(ASN1_UINT32_DATA, success, ASN1_BOOLEAN),
    ASN1_EMBED(ASN1_UINT32_DATA, test_uint32, UINT32),
    ASN1_EXP_OPT_EMBED(ASN1_UINT32_DATA, test_zuint32, ZUINT32, 0)
} static_ASN1_SEQUENCE_END(ASN1_UINT32_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_UINT32_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_UINT32_DATA)

static ASN1_UINT32_DATA uint32_expected[] = {
    CUSTOM_EXPECTED_SUCCESS(0, 0), /* t_zero */
    CUSTOM_EXPECTED_SUCCESS(ASN1_LONG_UNDEF, ASN1_LONG_UNDEF), /* t_zero */
    CUSTOM_EXPECTED_SUCCESS(1, 1), /* t_one */
    CUSTOM_EXPECTED_FAILURE,     /* t_one_neg (illegal negative value) */
    CUSTOM_EXPECTED_FAILURE,     /* t_minus_256 (illegal negative value) */
    CUSTOM_EXPECTED_FAILURE,     /* t_9bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_3_pad */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_5_negpad */
    CUSTOM_EXPECTED_FAILURE,     /* t_5bytes_1 */
    CUSTOM_EXPECTED_SUCCESS(0x80000000, 0x80000000), /* t_4bytes_1 */
    CUSTOM_EXPECTED_SUCCESS(INT32_MAX - 1, INT32_MAX -1), /* t_4bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_3_pad (illegal padding) */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_4_neg (illegal negative value) */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_5_negpad (illegal padding) */
};
static ASN1_UINT32_DATA uint32_encdec_data[] = {
    ENCDEC_ARRAY(UINT32_MAX, UINT32_MAX, 0, 0),
};

static TEST_PACKAGE uint32_test_package = {
    ASN1_ITEM_ref(ASN1_UINT32_DATA), "UINT32", 0,
    uint32_expected, sizeof(uint32_expected), sizeof(uint32_expected[0]),
    uint32_encdec_data, sizeof(uint32_encdec_data), sizeof(uint32_encdec_data[0]),
    (i2d_fn *)i2d_ASN1_UINT32_DATA, (d2i_fn *)d2i_ASN1_UINT32_DATA,
    (ifree_fn *)ASN1_UINT32_DATA_free
};

/***** INT64 *****************************************************************/

typedef struct {
    ASN1_BOOLEAN success;
    int64_t test_int64;
    int64_t test_zint64;
} ASN1_INT64_DATA;

ASN1_SEQUENCE(ASN1_INT64_DATA) = {
    ASN1_SIMPLE(ASN1_INT64_DATA, success, ASN1_BOOLEAN),
    ASN1_EMBED(ASN1_INT64_DATA, test_int64, INT64),
    ASN1_EXP_OPT_EMBED(ASN1_INT64_DATA, test_zint64, ZINT64, 0)
} static_ASN1_SEQUENCE_END(ASN1_INT64_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_INT64_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_INT64_DATA)

static ASN1_INT64_DATA int64_expected[] = {
    CUSTOM_EXPECTED_SUCCESS(0, 0), /* t_zero */
    CUSTOM_EXPECTED_SUCCESS(ASN1_LONG_UNDEF, ASN1_LONG_UNDEF), /* t_zero */
    CUSTOM_EXPECTED_SUCCESS(1, 1), /* t_one */
    CUSTOM_EXPECTED_SUCCESS(-1, -1), /* t_one_neg */
    CUSTOM_EXPECTED_SUCCESS(-256, -256), /* t_minus_256 */
    CUSTOM_EXPECTED_FAILURE,     /* t_9bytes_1 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_1 (too large positive) */
    CUSTOM_EXPECTED_SUCCESS(INT64_MAX, INT64_MAX), /* t_8bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_3_pad (illegal padding) */
    CUSTOM_EXPECTED_SUCCESS(INT64_MIN, INT64_MIN), /* t_8bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_5_negpad (illegal padding) */
    CUSTOM_EXPECTED_SUCCESS(0x1ffffffffULL, 0x1ffffffffULL), /* t_5bytes_1 */
    CUSTOM_EXPECTED_SUCCESS(0x80000000, 0x80000000), /* t_4bytes_1 */
    CUSTOM_EXPECTED_SUCCESS(INT32_MAX - 1, INT32_MAX -1), /* t_4bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_3_pad (illegal padding) */
    CUSTOM_EXPECTED_SUCCESS(INT32_MIN, INT32_MIN), /* t_4bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_5_negpad (illegal padding) */
};
static ASN1_INT64_DATA int64_encdec_data[] = {
    ENCDEC_ARRAY(INT64_MAX, INT64_MAX, INT64_MIN, INT64_MIN),
    ENCDEC_ARRAY(INT32_MAX, INT32_MAX, INT32_MIN, INT32_MIN),
};

static TEST_PACKAGE int64_test_package = {
    ASN1_ITEM_ref(ASN1_INT64_DATA), "INT64", 0,
    int64_expected, sizeof(int64_expected), sizeof(int64_expected[0]),
    int64_encdec_data, sizeof(int64_encdec_data), sizeof(int64_encdec_data[0]),
    (i2d_fn *)i2d_ASN1_INT64_DATA, (d2i_fn *)d2i_ASN1_INT64_DATA,
    (ifree_fn *)ASN1_INT64_DATA_free
};

/***** UINT64 ****************************************************************/

typedef struct {
    ASN1_BOOLEAN success;
    uint64_t test_uint64;
    uint64_t test_zuint64;
} ASN1_UINT64_DATA;

ASN1_SEQUENCE(ASN1_UINT64_DATA) = {
    ASN1_SIMPLE(ASN1_UINT64_DATA, success, ASN1_BOOLEAN),
    ASN1_EMBED(ASN1_UINT64_DATA, test_uint64, UINT64),
    ASN1_EXP_OPT_EMBED(ASN1_UINT64_DATA, test_zuint64, ZUINT64, 0)
} static_ASN1_SEQUENCE_END(ASN1_UINT64_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_UINT64_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_UINT64_DATA)

static ASN1_UINT64_DATA uint64_expected[] = {
    CUSTOM_EXPECTED_SUCCESS(0, 0), /* t_zero */
    CUSTOM_EXPECTED_SUCCESS(ASN1_LONG_UNDEF, ASN1_LONG_UNDEF), /* t_zero */
    CUSTOM_EXPECTED_SUCCESS(1, 1), /* t_one */
    CUSTOM_EXPECTED_FAILURE,     /* t_one_neg (illegal negative value) */
    CUSTOM_EXPECTED_FAILURE,     /* t_minus_256 (illegal negative value) */
    CUSTOM_EXPECTED_FAILURE,     /* t_9bytes_1 */
    CUSTOM_EXPECTED_SUCCESS((uint64_t)INT64_MAX+1, (uint64_t)INT64_MAX+1),
                                 /* t_8bytes_1 */
    CUSTOM_EXPECTED_SUCCESS(INT64_MAX, INT64_MAX), /* t_8bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_3_pad */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_4_neg */
    CUSTOM_EXPECTED_FAILURE,     /* t_8bytes_5_negpad */
    CUSTOM_EXPECTED_SUCCESS(0x1ffffffffULL, 0x1ffffffffULL), /* t_5bytes_1 */
    CUSTOM_EXPECTED_SUCCESS(0x80000000, 0x80000000), /* t_4bytes_1 */
    CUSTOM_EXPECTED_SUCCESS(INT32_MAX - 1, INT32_MAX -1), /* t_4bytes_2 */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_3_pad (illegal padding) */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_4_neg (illegal negative value) */
    CUSTOM_EXPECTED_FAILURE,     /* t_4bytes_5_negpad (illegal padding) */
};
static ASN1_UINT64_DATA uint64_encdec_data[] = {
    ENCDEC_ARRAY(UINT64_MAX, UINT64_MAX, 0, 0),
};

static TEST_PACKAGE uint64_test_package = {
    ASN1_ITEM_ref(ASN1_UINT64_DATA), "UINT64", 0,
    uint64_expected, sizeof(uint64_expected), sizeof(uint64_expected[0]),
    uint64_encdec_data, sizeof(uint64_encdec_data), sizeof(uint64_encdec_data[0]),
    (i2d_fn *)i2d_ASN1_UINT64_DATA, (d2i_fn *)d2i_ASN1_UINT64_DATA,
    (ifree_fn *)ASN1_UINT64_DATA_free
};

/***** General testing functions *********************************************/


/* Template structure to map onto any test data structure */
typedef struct {
    ASN1_BOOLEAN success;
    unsigned char bytes[1];       /* In reality, there's more */
} EXPECTED;

/*
 * do_decode returns a tristate:
 *
 *      -1      Couldn't decode
 *      0       decoded structure wasn't what was expected (failure)
 *      1       decoded structure was what was expected (success)
 */
static int do_decode(unsigned char *bytes, long nbytes,
                     const EXPECTED *expected, size_t expected_size,
                     const TEST_PACKAGE *package)
{
    EXPECTED *enctst = NULL;
    const unsigned char *start;
    int ret = 0;

    start = bytes;
    enctst = package->d2i(NULL, &bytes, nbytes);
    if (enctst == NULL) {
        if (expected->success == 0) {
            ret = 1;
            ERR_clear_error();
        } else {
            ret = -1;
        }
    } else {
        if (start + nbytes == bytes
            && memcmp(enctst, expected, expected_size) == 0)
            ret = 1;
        else
            ret = 0;
    }

    package->ifree(enctst);
    return ret;
}

/*
 * do_encode returns a tristate:
 *
 *      -1      Couldn't encode
 *      0       encoded DER wasn't what was expected (failure)
 *      1       encoded DER was what was expected (success)
 */
static int do_encode(EXPECTED *input,
                     const unsigned char *expected, size_t expected_len,
                     const TEST_PACKAGE *package)
{
    unsigned char *data = NULL;
    int len;
    int ret = 0;

    len = package->i2d(input, &data);
    if (len < 0)
        return -1;

    if ((size_t)len != expected_len
        || memcmp(data, expected, expected_len) != 0) {
        if (input->success == 0) {
            ret = 1;
            ERR_clear_error();
        } else {
            ret = 0;
        }
    } else {
        ret = 1;
    }

    OPENSSL_free(data);
    return ret;
}

/* Do an encode/decode round trip */
static int do_enc_dec(EXPECTED *bytes, long nbytes,
                      const TEST_PACKAGE *package)
{
    unsigned char *data = NULL;
    int len;
    int ret = 0;
    void *p = bytes;

    len = package->i2d(p, &data);
    if (len < 0)
        return -1;

    ret = do_decode(data, len, bytes, nbytes, package);
    OPENSSL_free(data);
    return ret;
}

static size_t der_encode_length(size_t len, unsigned char **pp)
{
    size_t lenbytes;

    OPENSSL_assert(len < 0x8000);
    if (len > 255)
        lenbytes = 3;
    else if (len > 127)
        lenbytes = 2;
    else
        lenbytes = 1;

    if (pp != NULL) {
        if (lenbytes == 1) {
            *(*pp)++ = (unsigned char)len;
        } else {
            *(*pp)++ = (unsigned char)(lenbytes - 1);
            if (lenbytes == 2) {
                *(*pp)++ = (unsigned char)(0x80 | len);
            } else {
                *(*pp)++ = (unsigned char)(0x80 | (len >> 8));
                *(*pp)++ = (unsigned char)(len);
            }
        }
    }
    return lenbytes;
}

static size_t make_custom_der(const TEST_CUSTOM_DATA *custom_data,
                              unsigned char **encoding, int explicit_default)
{
    size_t firstbytes, secondbytes = 0, secondbytesinner = 0, seqbytes;
    const unsigned char t_true[] = { V_ASN1_BOOLEAN, 0x01, 0xff };
    unsigned char *p = NULL;
    size_t i;

    /*
     * The first item is just an INTEGER tag, INTEGER length and INTEGER content
     */
    firstbytes =
        1 + der_encode_length(custom_data->nbytes1, NULL)
        + custom_data->nbytes1;

    for (i = custom_data->nbytes2; i > 0; i--) {
        if (custom_data->bytes2[i - 1] != '\0')
            break;
    }
    if (explicit_default || i > 0) {
        /*
         * The second item is an explicit tag, content length, INTEGER tag,
         * INTEGER length, INTEGER bytes
         */
        secondbytesinner =
            1 + der_encode_length(custom_data->nbytes2, NULL)
            + custom_data->nbytes2;
        secondbytes =
            1 + der_encode_length(secondbytesinner, NULL) + secondbytesinner;
    }

    /*
     * The whole sequence is the sequence tag, content length, BOOLEAN true
     * (copied from t_true), the first (firstbytes) and second (secondbytes)
     * items
     */
    seqbytes =
        1 + der_encode_length(sizeof(t_true) + firstbytes + secondbytes, NULL)
        + sizeof(t_true) + firstbytes + secondbytes;

    *encoding = p = OPENSSL_malloc(seqbytes);
    if (*encoding == NULL)
        return 0;

    /* Sequence tag */
    *p++ = 0x30;
    der_encode_length(sizeof(t_true) + firstbytes + secondbytes, &p);

    /* ASN1_BOOLEAN TRUE */
    memcpy(p, t_true, sizeof(t_true)); /* Marks decoding success */
    p += sizeof(t_true);

    /* First INTEGER item (non-optional) */
    *p++ = V_ASN1_INTEGER;
    der_encode_length(custom_data->nbytes1, &p);
    memcpy(p, custom_data->bytes1, custom_data->nbytes1);
    p += custom_data->nbytes1;

    if (secondbytes > 0) {
        /* Second INTEGER item (optional) */
        /* Start with the explicit optional tag */
        *p++ = 0xa0;
        der_encode_length(secondbytesinner, &p);
        *p++ = V_ASN1_INTEGER;
        der_encode_length(custom_data->nbytes2, &p);
        memcpy(p, custom_data->bytes2, custom_data->nbytes2);
        p += custom_data->nbytes2;
    }

    OPENSSL_assert(seqbytes == (size_t)(p - *encoding));

    return seqbytes;
}

/* Attempt to decode a custom encoding of the test structure */
static int do_decode_custom(const TEST_CUSTOM_DATA *custom_data,
                            const EXPECTED *expected, size_t expected_size,
                            const TEST_PACKAGE *package)
{
    unsigned char *encoding = NULL;
    /*
     * We force the defaults to be explicitly encoded to make sure we test
     * for defaults that shouldn't be present (i.e. we check for failure)
     */
    size_t encoding_length = make_custom_der(custom_data, &encoding, 1);
    int ret;

    if (encoding_length == 0)
        return -1;

    ret = do_decode(encoding, encoding_length, expected, expected_size,
                    package);
    OPENSSL_free(encoding);

    return ret;
}

/* Attempt to encode the test structure and compare it to custom DER */
static int do_encode_custom(EXPECTED *input,
                            const TEST_CUSTOM_DATA *custom_data,
                            const TEST_PACKAGE *package)
{
    unsigned char *expected = NULL;
    size_t expected_length = make_custom_der(custom_data, &expected, 0);
    int ret;

    if (expected_length == 0)
        return -1;

    ret = do_encode(input, expected, expected_length, package);
    OPENSSL_free(expected);

    return ret;
}

static int do_print_item(const TEST_PACKAGE *package)
{
#define DATA_BUF_SIZE 256
    const ASN1_ITEM *i = ASN1_ITEM_ptr(package->asn1_type);
    ASN1_VALUE *o;
    int ret;

    OPENSSL_assert(package->encode_expectations_elem_size <= DATA_BUF_SIZE);
    if ((o = OPENSSL_malloc(DATA_BUF_SIZE)) == NULL)
        return 0;

    (void)RAND_bytes((unsigned char*)o,
                     (int)package->encode_expectations_elem_size);
    ret = ASN1_item_print(bio_err, o, 0, i, NULL);
    OPENSSL_free(o);

    return ret;
}


static int test_intern(const TEST_PACKAGE *package)
{
    unsigned int i;
    size_t nelems;
    int fail = 0;

    if (package->skip)
        return 1;

    /* Do decode_custom checks */
    nelems = package->encode_expectations_size
        / package->encode_expectations_elem_size;
    OPENSSL_assert(nelems == OSSL_NELEM(test_custom_data));
    for (i = 0; i < nelems; i++) {
        size_t pos = i * package->encode_expectations_elem_size;
        EXPECTED *expected
            = (EXPECTED *)&((unsigned char *)package->encode_expectations)[pos];

        switch (do_encode_custom(expected, &test_custom_data[i], package)) {
        case -1:
            if (expected->success) {
                TEST_error("Failed custom encode round trip %u of %s",
                           i, package->name);
                TEST_openssl_errors();
                fail++;
            }
            break;
        case 0:
            TEST_error("Custom encode round trip %u of %s mismatch",
                       i, package->name);
            TEST_openssl_errors();
            fail++;
            break;
        case 1:
            break;
        default:
            OPENSSL_die("do_encode_custom() return unknown value",
                        __FILE__, __LINE__);
        }
        switch (do_decode_custom(&test_custom_data[i], expected,
                                 package->encode_expectations_elem_size,
                                 package)) {
        case -1:
            if (expected->success) {
                TEST_error("Failed custom decode round trip %u of %s",
                           i, package->name);
                TEST_openssl_errors();
                fail++;
            }
            break;
        case 0:
            TEST_error("Custom decode round trip %u of %s mismatch",
                       i, package->name);
            TEST_openssl_errors();
            fail++;
            break;
        case 1:
            break;
        default:
            OPENSSL_die("do_decode_custom() return unknown value",
                        __FILE__, __LINE__);
        }
    }

    /* Do enc_dec checks */
    nelems = package->encdec_data_size / package->encdec_data_elem_size;
    for (i = 0; i < nelems; i++) {
        size_t pos = i * package->encdec_data_elem_size;
        EXPECTED *expected
            = (EXPECTED *)&((unsigned char *)package->encdec_data)[pos];

        switch (do_enc_dec(expected, package->encdec_data_elem_size, package)) {
        case -1:
            if (expected->success) {
                TEST_error("Failed encode/decode round trip %u of %s",
                           i, package->name);
                TEST_openssl_errors();
                fail++;
            }
            break;
        case 0:
            TEST_error("Encode/decode round trip %u of %s mismatch",
                       i, package->name);
            fail++;
            break;
        case 1:
            break;
        default:
            OPENSSL_die("do_enc_dec() return unknown value",
                        __FILE__, __LINE__);
        }
    }

    if (!do_print_item(package)) {
        TEST_error("Printing of %s failed", package->name);
        TEST_openssl_errors();
        fail++;
    }

    return fail == 0;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
static int test_long_32bit(void)
{
    return test_intern(&long_test_package_32bit);
}

static int test_long_64bit(void)
{
    return test_intern(&long_test_package_64bit);
}
#endif

static int test_int32(void)
{
    return test_intern(&int32_test_package);
}

static int test_uint32(void)
{
    return test_intern(&uint32_test_package);
}

static int test_int64(void)
{
    return test_intern(&int64_test_package);
}

static int test_uint64(void)
{
    return test_intern(&uint64_test_package);
}

typedef struct {
    ASN1_STRING *invalidDirString;
} INVALIDTEMPLATE;

ASN1_SEQUENCE(INVALIDTEMPLATE) = {
    /*
     * DirectoryString is a CHOICE type so it must use explicit tagging -
     * but we deliberately use implicit here, which makes this template invalid.
     */
    ASN1_IMP(INVALIDTEMPLATE, invalidDirString, DIRECTORYSTRING, 12)
} static_ASN1_SEQUENCE_END(INVALIDTEMPLATE)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(INVALIDTEMPLATE)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(INVALIDTEMPLATE)

static int test_invalid_template(void)
{
    INVALIDTEMPLATE *temp = INVALIDTEMPLATE_new();
    int ret;

    if (!TEST_ptr(temp))
        return 0;

    ret = i2d_INVALIDTEMPLATE(temp, NULL);

    INVALIDTEMPLATE_free(temp);

    /* We expect the i2d operation to fail */
    return ret < 0;
}


int setup_tests(void)
{
#ifndef OPENSSL_NO_DEPRECATED_3_0
    ADD_TEST(test_long_32bit);
    ADD_TEST(test_long_64bit);
#endif
    ADD_TEST(test_int32);
    ADD_TEST(test_uint32);
    ADD_TEST(test_int64);
    ADD_TEST(test_uint64);
    ADD_TEST(test_invalid_template);
    return 1;
}
