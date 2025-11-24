/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include <openssl/bn.h>
#include "crypto/asn1_dsa.h"
#include "testutil.h"

static unsigned char t_dsa_sig[] = {
    0x30, 0x06,                  /* SEQUENCE tag + length */
    0x02, 0x01, 0x01,            /* INTEGER tag + length + content */
    0x02, 0x01, 0x02             /* INTEGER tag + length + content */
};

static unsigned char t_dsa_sig_extra[] = {
    0x30, 0x06,                  /* SEQUENCE tag + length */
    0x02, 0x01, 0x01,            /* INTEGER tag + length + content */
    0x02, 0x01, 0x02,            /* INTEGER tag + length + content */
    0x05, 0x00                   /* NULL tag + length */
};

static unsigned char t_dsa_sig_msb[] = {
    0x30, 0x08,                  /* SEQUENCE tag + length */
    0x02, 0x02, 0x00, 0x81,      /* INTEGER tag + length + content */
    0x02, 0x02, 0x00, 0x82       /* INTEGER tag + length + content */
};

static unsigned char t_dsa_sig_two[] = {
    0x30, 0x08,                  /* SEQUENCE tag + length */
    0x02, 0x02, 0x01, 0x00,      /* INTEGER tag + length + content */
    0x02, 0x02, 0x02, 0x00       /* INTEGER tag + length + content */
};

/*
 * Badly coded ASN.1 INTEGER zero wrapped in a sequence along with another
 * (valid) INTEGER.
 */
static unsigned char t_invalid_int_zero[] = {
    0x30, 0x05,                  /* SEQUENCE tag + length */
    0x02, 0x00,                  /* INTEGER tag + length */
    0x02, 0x01, 0x2a             /* INTEGER tag + length */
};

/*
 * Badly coded ASN.1 INTEGER (with leading zeros) wrapped in a sequence along
 * with another (valid) INTEGER.
 */
static unsigned char t_invalid_int[] = {
    0x30, 0x07,                  /* SEQUENCE tag + length */
    0x02, 0x02, 0x00, 0x7f,      /* INTEGER tag + length */
    0x02, 0x01, 0x2a             /* INTEGER tag + length */
};

/*
 * Negative ASN.1 INTEGER wrapped in a sequence along with another
 * (valid) INTEGER.
 */
static unsigned char t_neg_int[] = {
    0x30, 0x06,                  /* SEQUENCE tag + length */
    0x02, 0x01, 0xaa,            /* INTEGER tag + length */
    0x02, 0x01, 0x2a             /* INTEGER tag + length */
};

static unsigned char t_trunc_der[] = {
    0x30, 0x08,                  /* SEQUENCE tag + length */
    0x02, 0x02, 0x00, 0x81,      /* INTEGER tag + length */
    0x02, 0x02, 0x00             /* INTEGER tag + length */
};

static unsigned char t_trunc_seq[] = {
    0x30, 0x07,                  /* SEQUENCE tag + length */
    0x02, 0x02, 0x00, 0x81,      /* INTEGER tag + length */
    0x02, 0x02, 0x00, 0x82       /* INTEGER tag + length */
};

static int test_decode(void)
{
    int rv = 0;
    BIGNUM *r;
    BIGNUM *s;
    const unsigned char *pder;

    r = BN_new();
    s = BN_new();

    /* Positive tests */
    pder = t_dsa_sig;
    if (ossl_decode_der_dsa_sig(r, s, &pder, sizeof(t_dsa_sig)) == 0
            || !TEST_ptr_eq(pder, (t_dsa_sig + sizeof(t_dsa_sig)))
            || !TEST_BN_eq_word(r, 1) || !TEST_BN_eq_word(s, 2)) {
        TEST_info("asn1_dsa test_decode: t_dsa_sig failed");
        goto fail;
    }

    BN_clear(r);
    BN_clear(s);
    pder = t_dsa_sig_extra;
    if (ossl_decode_der_dsa_sig(r, s, &pder, sizeof(t_dsa_sig_extra)) == 0
            || !TEST_ptr_eq(pder,
                            (t_dsa_sig_extra + sizeof(t_dsa_sig_extra) - 2))
            || !TEST_BN_eq_word(r, 1) || !TEST_BN_eq_word(s, 2)) {
        TEST_info("asn1_dsa test_decode: t_dsa_sig_extra failed");
        goto fail;
    }

    BN_clear(r);
    BN_clear(s);
    pder = t_dsa_sig_msb;
    if (ossl_decode_der_dsa_sig(r, s, &pder, sizeof(t_dsa_sig_msb)) == 0
            || !TEST_ptr_eq(pder, (t_dsa_sig_msb + sizeof(t_dsa_sig_msb)))
            || !TEST_BN_eq_word(r, 0x81) || !TEST_BN_eq_word(s, 0x82)) {
        TEST_info("asn1_dsa test_decode: t_dsa_sig_msb failed");
        goto fail;
    }

    BN_clear(r);
    BN_clear(s);
    pder = t_dsa_sig_two;
    if (ossl_decode_der_dsa_sig(r, s, &pder, sizeof(t_dsa_sig_two)) == 0
            || !TEST_ptr_eq(pder, (t_dsa_sig_two + sizeof(t_dsa_sig_two)))
            || !TEST_BN_eq_word(r, 0x100) || !TEST_BN_eq_word(s, 0x200)) {
        TEST_info("asn1_dsa test_decode: t_dsa_sig_two failed");
        goto fail;
    }

    /* Negative tests */
    pder = t_invalid_int_zero;
    if (ossl_decode_der_dsa_sig(r, s, &pder, sizeof(t_invalid_int_zero)) != 0) {
        TEST_info("asn1_dsa test_decode: Expected t_invalid_int_zero to fail");
        goto fail;
    }

    BN_clear(r);
    BN_clear(s);
    pder = t_invalid_int;
    if (ossl_decode_der_dsa_sig(r, s, &pder, sizeof(t_invalid_int)) != 0) {
        TEST_info("asn1_dsa test_decode: Expected t_invalid_int to fail");
        goto fail;
    }

    BN_clear(r);
    BN_clear(s);
    pder = t_neg_int;
    if (ossl_decode_der_dsa_sig(r, s, &pder, sizeof(t_neg_int)) != 0) {
        TEST_info("asn1_dsa test_decode: Expected t_neg_int to fail");
        goto fail;
    }

    BN_clear(r);
    BN_clear(s);
    pder = t_trunc_der;
    if (ossl_decode_der_dsa_sig(r, s, &pder, sizeof(t_trunc_der)) != 0) {
        TEST_info("asn1_dsa test_decode: Expected fail t_trunc_der");
        goto fail;
    }

    BN_clear(r);
    BN_clear(s);
    pder = t_trunc_seq;
    if (ossl_decode_der_dsa_sig(r, s, &pder, sizeof(t_trunc_seq)) != 0) {
        TEST_info("asn1_dsa test_decode: Expected fail t_trunc_seq");
        goto fail;
    }

    rv = 1;
fail:
    BN_free(r);
    BN_free(s);
    return rv;
}

int setup_tests(void)
{
    ADD_TEST(test_decode);
    return 1;
}
