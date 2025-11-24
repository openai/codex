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
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/obj_mac.h>
#include "internal/numbers.h"
#include "testutil.h"

#ifdef __GNUC__
# pragma GCC diagnostic ignored "-Wunused-function"
#endif
#ifdef __clang__
# pragma clang diagnostic ignored "-Wunused-function"
#endif

/* Badly coded ASN.1 INTEGER zero wrapped in a sequence */
static unsigned char t_invalid_zero[] = {
    0x30, 0x02,                  /* SEQUENCE tag + length */
    0x02, 0x00                   /* INTEGER tag + length */
};

#ifndef OPENSSL_NO_DEPRECATED_3_0
/* LONG case ************************************************************* */

typedef struct {
    long test_long;
} ASN1_LONG_DATA;

ASN1_SEQUENCE(ASN1_LONG_DATA) = {
    ASN1_EMBED(ASN1_LONG_DATA, test_long, LONG),
} static_ASN1_SEQUENCE_END(ASN1_LONG_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_LONG_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_LONG_DATA)

static int test_long(void)
{
    const unsigned char *p = t_invalid_zero;
    ASN1_LONG_DATA *dectst =
        d2i_ASN1_LONG_DATA(NULL, &p, sizeof(t_invalid_zero));

    if (dectst == NULL)
        return 0;                /* Fail */

    ASN1_LONG_DATA_free(dectst);
    return 1;
}
#endif

/* INT32 case ************************************************************* */

typedef struct {
    int32_t test_int32;
} ASN1_INT32_DATA;

ASN1_SEQUENCE(ASN1_INT32_DATA) = {
    ASN1_EMBED(ASN1_INT32_DATA, test_int32, INT32),
} static_ASN1_SEQUENCE_END(ASN1_INT32_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_INT32_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_INT32_DATA)

static int test_int32(void)
{
    const unsigned char *p = t_invalid_zero;
    ASN1_INT32_DATA *dectst =
        d2i_ASN1_INT32_DATA(NULL, &p, sizeof(t_invalid_zero));

    if (dectst == NULL)
        return 0;                /* Fail */

    ASN1_INT32_DATA_free(dectst);
    return 1;
}

/* UINT32 case ************************************************************* */

typedef struct {
    uint32_t test_uint32;
} ASN1_UINT32_DATA;

ASN1_SEQUENCE(ASN1_UINT32_DATA) = {
    ASN1_EMBED(ASN1_UINT32_DATA, test_uint32, UINT32),
} static_ASN1_SEQUENCE_END(ASN1_UINT32_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_UINT32_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_UINT32_DATA)

static int test_uint32(void)
{
    const unsigned char *p = t_invalid_zero;
    ASN1_UINT32_DATA *dectst =
        d2i_ASN1_UINT32_DATA(NULL, &p, sizeof(t_invalid_zero));

    if (dectst == NULL)
        return 0;                /* Fail */

    ASN1_UINT32_DATA_free(dectst);
    return 1;
}

/* INT64 case ************************************************************* */

typedef struct {
    int64_t test_int64;
} ASN1_INT64_DATA;

ASN1_SEQUENCE(ASN1_INT64_DATA) = {
    ASN1_EMBED(ASN1_INT64_DATA, test_int64, INT64),
} static_ASN1_SEQUENCE_END(ASN1_INT64_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_INT64_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_INT64_DATA)

static int test_int64(void)
{
    const unsigned char *p = t_invalid_zero;
    ASN1_INT64_DATA *dectst =
        d2i_ASN1_INT64_DATA(NULL, &p, sizeof(t_invalid_zero));

    if (dectst == NULL)
        return 0;                /* Fail */

    ASN1_INT64_DATA_free(dectst);
    return 1;
}

/* UINT64 case ************************************************************* */

typedef struct {
    uint64_t test_uint64;
} ASN1_UINT64_DATA;

ASN1_SEQUENCE(ASN1_UINT64_DATA) = {
    ASN1_EMBED(ASN1_UINT64_DATA, test_uint64, UINT64),
} static_ASN1_SEQUENCE_END(ASN1_UINT64_DATA)

IMPLEMENT_STATIC_ASN1_ENCODE_FUNCTIONS(ASN1_UINT64_DATA)
IMPLEMENT_STATIC_ASN1_ALLOC_FUNCTIONS(ASN1_UINT64_DATA)

static int test_uint64(void)
{
    const unsigned char *p = t_invalid_zero;
    ASN1_UINT64_DATA *dectst =
        d2i_ASN1_UINT64_DATA(NULL, &p, sizeof(t_invalid_zero));

    if (dectst == NULL)
        return 0;                /* Fail */

    ASN1_UINT64_DATA_free(dectst);
    return 1;
}

/* GeneralizedTime underflow *********************************************** */

static int test_gentime(void)
{
    /* Underflowing GeneralizedTime 161208193400Z (YYMMDDHHMMSSZ) */
    const unsigned char der[] = {
        0x18, 0x0d, 0x31, 0x36, 0x31, 0x32, 0x30, 0x38, 0x31,
        0x39, 0x33, 0x34, 0x30, 0x30, 0x5a,
    };
    const unsigned char *p;
    int der_len, rc = 1;
    ASN1_GENERALIZEDTIME *gentime;

    p = der;
    der_len = sizeof(der);
    gentime = d2i_ASN1_GENERALIZEDTIME(NULL, &p, der_len);

    if (!TEST_ptr_null(gentime))
        rc = 0; /* fail */

    ASN1_GENERALIZEDTIME_free(gentime);
    return rc;
}

/* UTCTime underflow ******************************************************* */

static int test_utctime(void)
{
    /* Underflowing UTCTime 0205104700Z (MMDDHHMMSSZ) */
    const unsigned char der[] = {
        0x17, 0x0b, 0x30, 0x32, 0x30, 0x35, 0x31, 0x30,
        0x34, 0x37, 0x30, 0x30, 0x5a,
    };
    const unsigned char *p;
    int der_len, rc = 1;
    ASN1_UTCTIME *utctime;

    p = der;
    der_len = sizeof(der);
    utctime = d2i_ASN1_UTCTIME(NULL, &p, der_len);

    if (!TEST_ptr_null(utctime))
        rc = 0; /* fail */

    ASN1_UTCTIME_free(utctime);
    return rc;
}

/* Invalid template ******************************************************** */

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

/* Empty sequence for invalid template test */
static unsigned char t_invalid_template[] = {
    0x30, 0x03,                  /* SEQUENCE tag + length */
    0x0c, 0x01, 0x41             /* UTF8String, length 1, "A" */
};

static int test_invalid_template(void)
{
    const unsigned char *p = t_invalid_template;
    INVALIDTEMPLATE *tmp = d2i_INVALIDTEMPLATE(NULL, &p,
                                               sizeof(t_invalid_template));

    /* We expect a NULL pointer return */
    if (TEST_ptr_null(tmp))
        return 1;

    INVALIDTEMPLATE_free(tmp);
    return 0;
}

static int test_reuse_asn1_object(void)
{
    static unsigned char cn_der[] = { 0x06, 0x03, 0x55, 0x04, 0x06 };
    static unsigned char oid_der[] = {
        0x06, 0x06, 0x2a, 0x03, 0x04, 0x05, 0x06, 0x07
    };
    int ret = 0;
    ASN1_OBJECT *obj;
    unsigned char const *p = oid_der;

    /* Create an object that owns dynamically allocated 'sn' and 'ln' fields */

    if (!TEST_ptr(obj = ASN1_OBJECT_create(NID_undef, cn_der, sizeof(cn_der),
                                           "C", "countryName")))
        goto err;
    /* reuse obj - this should not leak sn and ln */
    if (!TEST_ptr(d2i_ASN1_OBJECT(&obj, &p, sizeof(oid_der))))
        goto err;
    ret = 1;
err:
    ASN1_OBJECT_free(obj);
    return ret;
}

int setup_tests(void)
{
#ifndef OPENSSL_NO_DEPRECATED_3_0
    ADD_TEST(test_long);
#endif
    ADD_TEST(test_int32);
    ADD_TEST(test_uint32);
    ADD_TEST(test_int64);
    ADD_TEST(test_uint64);
    ADD_TEST(test_gentime);
    ADD_TEST(test_utctime);
    ADD_TEST(test_invalid_template);
    ADD_TEST(test_reuse_asn1_object);
    return 1;
}
