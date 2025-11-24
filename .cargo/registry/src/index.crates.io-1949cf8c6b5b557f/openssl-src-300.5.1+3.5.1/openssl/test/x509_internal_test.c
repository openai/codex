/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Internal tests for the x509 and x509v3 modules */

#include <stdio.h>
#include <string.h>

#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include "testutil.h"
#include "internal/nelem.h"

/**********************************************************************
 *
 * Test of x509v3
 *
 ***/

#include "../crypto/x509/ext_dat.h"
#include "../crypto/x509/standard_exts.h"

static int test_standard_exts(void)
{
    size_t i;
    int prev = -1, good = 1;
    const X509V3_EXT_METHOD **tmp;

    tmp = standard_exts;
    for (i = 0; i < OSSL_NELEM(standard_exts); i++, tmp++) {
        if ((*tmp)->ext_nid < prev)
            good = 0;
        prev = (*tmp)->ext_nid;

    }
    if (!good) {
        tmp = standard_exts;
        TEST_error("Extensions out of order!");
        for (i = 0; i < STANDARD_EXTENSION_COUNT; i++, tmp++)
            TEST_note("%d : %s", (*tmp)->ext_nid, OBJ_nid2sn((*tmp)->ext_nid));
    }
    return good;
}

typedef struct {
    const char *ipasc;
    const char *data;
    int length;
} IP_TESTDATA;

static IP_TESTDATA a2i_ipaddress_tests[] = {
    {"127.0.0.1", "\x7f\x00\x00\x01", 4},
    {"1.2.3.4", "\x01\x02\x03\x04", 4},
    {"1.2.3.255", "\x01\x02\x03\xff", 4},
    {"255.255.255.255", "\xff\xff\xff\xff", 4},

    {"::", "\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00", 16},
    {"::1", "\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01", 16},
    {"::01", "\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01", 16},
    {"::0001", "\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01", 16},
    {"ffff::", "\xff\xff\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00", 16},
    {"ffff::1", "\xff\xff\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01", 16},
    {"1::2", "\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02", 16},
    {"1:1:1:1:1:1:1:1", "\x00\x01\x00\x01\x00\x01\x00\x01\x00\x01\x00\x01\x00\x01\x00\x01", 16},
    {"2001:db8::ff00:42:8329", "\x20\x01\x0d\xb8\x00\x00\x00\x00\x00\x00\xff\x00\x00\x42\x83\x29", 16},
    {"::1.2.3.4", "\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\x02\x03\x04", 16},
    {"ffff:ffff:ffff:ffff:ffff:ffff:1.2.3.4", "\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\x01\x02\x03\x04", 16},

    {"1:1:1:1:1:1:1:1.test", NULL, 0},
    {":::1", NULL, 0},
    {"2001::123g", NULL, 0},

    /* Too few IPv4 components. */
    {"1", NULL, 0 },
    {"1.", NULL, 0 },
    {"1.2", NULL, 0 },
    {"1.2.", NULL, 0 },
    {"1.2.3", NULL, 0 },
    {"1.2.3.", NULL, 0 },

    /* Invalid embedded IPv4 address. */
    {"::1.2.3", NULL, 0 },

    /* IPv4 literals take the place of two IPv6 components. */
    {"1:2:3:4:5:6:7:1.2.3.4", NULL, 0 },

    /* '::' should have fewer than 16 components or it is redundant. */
    {"1:2:3:4:5:6:7::8", NULL, 0 },

    /* Embedded IPv4 addresses must be at the end. */
    {"::1.2.3.4:1", NULL, 0 },

    /* Too many components. */
    {"1.2.3.4.5", NULL, 0 },
    {"1:2:3:4:5:6:7:8:9", NULL, 0 },
    {"1:2:3:4:5::6:7:8:9", NULL, 0 },

    /* Stray whitespace or other invalid characters. */
    {"1.2.3.4 ", NULL, 0 },
    {"1.2.3 .4", NULL, 0 },
    {"1.2.3. 4", NULL, 0 },
    {" 1.2.3.4", NULL, 0 },
    {"1.2.3.4.", NULL, 0 },
    {"1.2.3.+4", NULL, 0 },
    {"1.2.3.-4", NULL, 0 },
    {"1.2.3.4.example.test", NULL, 0 },
    {"::1 ", NULL, 0 },
    {" ::1", NULL, 0 },
    {":: 1", NULL, 0 },
    {": :1", NULL, 0 },
    {"1.2.3.nope", NULL, 0 },
    {"::nope", NULL, 0 },

    /* Components too large. */
    {"1.2.3.256", NULL, 0},  /* Overflows when adding */
    {"1.2.3.260", NULL, 0},  /* Overflows when multiplying by 10 */
    {"1.2.3.999999999999999999999999999999999999999999", NULL, 0 },
    {"::fffff", NULL, 0 },

    /* Although not an overflow, more than four hex digits is an error. */
    {"::00000", NULL, 0 },

    /* Too many colons. */
    {":::", NULL, 0 },
    {"1:::", NULL, 0 },
    {":::2", NULL, 0 },
    {"1:::2", NULL, 0 },

    /* Only one group of zeros may be elided. */
    {"1::2::3", NULL, 0 },

    /* We only support decimal. */
    {"1.2.3.01", NULL, 0 },
    {"1.2.3.0x1", NULL, 0 },

    /* Random garbage. */
    {"example.test", NULL, 0 },
    {"", NULL, 0},
    {" 1.2.3.4", NULL, 0},
    {" 1.2.3.4 ", NULL, 0},
    {"1.2.3.4.example.test", NULL, 0},
};


static int test_a2i_ipaddress(int idx)
{
    int good = 1;
    ASN1_OCTET_STRING *ip;
    int len = a2i_ipaddress_tests[idx].length;

    ip = a2i_IPADDRESS(a2i_ipaddress_tests[idx].ipasc);
    if (len == 0) {
        if (!TEST_ptr_null(ip)) {
            good = 0;
            TEST_note("'%s' should not be parsed as IP address", a2i_ipaddress_tests[idx].ipasc);
        }
    } else {
        if (!TEST_ptr(ip)
            || !TEST_int_eq(ASN1_STRING_length(ip), len)
            || !TEST_mem_eq(ASN1_STRING_get0_data(ip), len,
                            a2i_ipaddress_tests[idx].data, len)) {
            good = 0;
        }
    }
    ASN1_OCTET_STRING_free(ip);
    return good;
}

static int ck_purp(ossl_unused const X509_PURPOSE *purpose,
                   ossl_unused const X509 *x, int ca)
{
    return 1;
}

static int tests_X509_PURPOSE(void)
{
    OSSL_LIB_CTX *libctx = NULL;
    int id, idx, *p;
    X509_PURPOSE *xp;

#undef LN
#define LN "LN_test"
#undef SN
#define SN "SN_test"
#undef ARGS
#define ARGS(id, sn) id, X509_TRUST_MAX, 0, ck_purp, LN, sn, NULL
    return TEST_int_gt((id = X509_PURPOSE_get_unused_id(libctx)), X509_PURPOSE_MAX)
        && TEST_int_eq(X509_PURPOSE_get_count() + 1, id)
        && TEST_int_eq(X509_PURPOSE_get_by_id(id), -1)
        && TEST_int_eq(X509_PURPOSE_get_by_sname(SN), -1)

        /* add new entry with fresh id and fresh sname: */
        && TEST_int_eq(X509_PURPOSE_add(ARGS(id, SN)), 1)
        && TEST_int_ne((idx = X509_PURPOSE_get_by_sname(SN)), -1)
        && TEST_int_eq(X509_PURPOSE_get_by_id(id), idx)

        /* overwrite same entry, should be idempotent: */
        && TEST_int_eq(X509_PURPOSE_add(ARGS(id, SN)), 1)
        && TEST_int_eq(X509_PURPOSE_get_by_sname(SN), idx)
        && TEST_int_eq(X509_PURPOSE_get_by_id(id), idx)

        /* fail adding entry with same sname but existing conflicting id: */
        && TEST_int_eq(X509_PURPOSE_add(ARGS(X509_PURPOSE_MAX, SN)), 0)
        /* fail adding entry with same existing id but conflicting sname: */
        && TEST_int_eq(X509_PURPOSE_add(ARGS(id, SN"_different")), 0)

        && TEST_ptr((xp = X509_PURPOSE_get0(idx)))
        && TEST_int_eq(X509_PURPOSE_get_id(xp), id)
        && TEST_str_eq(X509_PURPOSE_get0_name(xp), LN)
        && TEST_str_eq(X509_PURPOSE_get0_sname(xp), SN)
        && TEST_int_eq(X509_PURPOSE_get_trust(xp), X509_TRUST_MAX)

        && TEST_int_eq(*(p = &xp->purpose), id)
        && TEST_int_eq(X509_PURPOSE_set(p, X509_PURPOSE_DEFAULT_ANY), 1)
        && TEST_int_eq(X509_PURPOSE_get_id(xp), X509_PURPOSE_DEFAULT_ANY);
}

int setup_tests(void)
{
    ADD_TEST(test_standard_exts);
    ADD_ALL_TESTS(test_a2i_ipaddress, OSSL_NELEM(a2i_ipaddress_tests));
    ADD_TEST(tests_X509_PURPOSE);
    return 1;
}
