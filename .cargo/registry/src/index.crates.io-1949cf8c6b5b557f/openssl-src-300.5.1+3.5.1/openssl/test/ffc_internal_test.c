/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019-2020, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This is an internal test that is intentionally using internal APIs. Some of
 * those APIs are deprecated for public use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "internal/nelem.h"
#include <openssl/crypto.h>
#include <openssl/bio.h>
#include <openssl/bn.h>
#include <openssl/rand.h>
#include <openssl/err.h>
#include "testutil.h"

#include "internal/ffc.h"
#include "crypto/security_bits.h"

#ifndef OPENSSL_NO_DSA
static const unsigned char dsa_2048_224_sha224_p[] = {
    0x93, 0x57, 0x93, 0x62, 0x1b, 0x9a, 0x10, 0x9b, 0xc1, 0x56, 0x0f, 0x24,
    0x71, 0x76, 0x4e, 0xd3, 0xed, 0x78, 0x78, 0x7a, 0xbf, 0x89, 0x71, 0x67,
    0x8e, 0x03, 0xd8, 0x5b, 0xcd, 0x22, 0x8f, 0x70, 0x74, 0xff, 0x22, 0x05,
    0x07, 0x0c, 0x4c, 0x60, 0xed, 0x41, 0xe1, 0x9e, 0x9c, 0xaa, 0x3e, 0x19,
    0x5c, 0x3d, 0x80, 0x58, 0xb2, 0x7f, 0x5f, 0x89, 0xec, 0xb5, 0x19, 0xdb,
    0x06, 0x11, 0xe9, 0x78, 0x5c, 0xf9, 0xa0, 0x9e, 0x70, 0x62, 0x14, 0x7b,
    0xda, 0x92, 0xbf, 0xb2, 0x6b, 0x01, 0x6f, 0xb8, 0x68, 0x9c, 0x89, 0x36,
    0x89, 0x72, 0x79, 0x49, 0x93, 0x3d, 0x14, 0xb2, 0x2d, 0xbb, 0xf0, 0xdf,
    0x94, 0x45, 0x0b, 0x5f, 0xf1, 0x75, 0x37, 0xeb, 0x49, 0xb9, 0x2d, 0xce,
    0xb7, 0xf4, 0x95, 0x77, 0xc2, 0xe9, 0x39, 0x1c, 0x4e, 0x0c, 0x40, 0x62,
    0x33, 0x0a, 0xe6, 0x29, 0x6f, 0xba, 0xef, 0x02, 0xdd, 0x0d, 0xe4, 0x04,
    0x01, 0x70, 0x40, 0xb9, 0xc9, 0x7e, 0x2f, 0x10, 0x37, 0xe9, 0xde, 0xb0,
    0xf6, 0xeb, 0x71, 0x7f, 0x9c, 0x35, 0x16, 0xf3, 0x0d, 0xc4, 0xe8, 0x02,
    0x37, 0x6c, 0xdd, 0xb3, 0x8d, 0x2d, 0x1e, 0x28, 0x13, 0x22, 0x89, 0x40,
    0xe5, 0xfa, 0x16, 0x67, 0xd6, 0xda, 0x12, 0xa2, 0x38, 0x83, 0x25, 0xcc,
    0x26, 0xc1, 0x27, 0x74, 0xfe, 0xf6, 0x7a, 0xb6, 0xa1, 0xe4, 0xe8, 0xdf,
    0x5d, 0xd2, 0x9c, 0x2f, 0xec, 0xea, 0x08, 0xca, 0x48, 0xdb, 0x18, 0x4b,
    0x12, 0xee, 0x16, 0x9b, 0xa6, 0x00, 0xa0, 0x18, 0x98, 0x7d, 0xce, 0x6c,
    0x6d, 0xf8, 0xfc, 0x95, 0x51, 0x1b, 0x0a, 0x40, 0xb6, 0xfc, 0xe5, 0xe2,
    0xb0, 0x26, 0x53, 0x4c, 0xd7, 0xfe, 0xaa, 0x6d, 0xbc, 0xdd, 0xc0, 0x61,
    0x65, 0xe4, 0x89, 0x44, 0x18, 0x6f, 0xd5, 0x39, 0xcf, 0x75, 0x6d, 0x29,
    0xcc, 0xf8, 0x40, 0xab
};
static const unsigned char dsa_2048_224_sha224_q[] = {
    0xf2, 0x5e, 0x4e, 0x9a, 0x15, 0xa8, 0x13, 0xdf, 0xa3, 0x17, 0x90, 0xc6,
    0xd6, 0x5e, 0xb1, 0xfb, 0x31, 0xf8, 0xb5, 0xb1, 0x4b, 0xa7, 0x6d, 0xde,
    0x57, 0x76, 0x6f, 0x11
};
static const unsigned char dsa_2048_224_sha224_seed[] = {
    0xd2, 0xb1, 0x36, 0xd8, 0x5b, 0x8e, 0xa4, 0xb2, 0x6a, 0xab, 0x4e, 0x85,
    0x8b, 0x49, 0xf9, 0xdd, 0xe6, 0xa1, 0xcd, 0xad, 0x49, 0x52, 0xe9, 0xb3,
    0x36, 0x17, 0x06, 0xcf
};
static const unsigned char dsa_2048_224_sha224_bad_seed[] = {
    0xd2, 0xb1, 0x36, 0xd8, 0x5b, 0x8e, 0xa4, 0xb2, 0x6a, 0xab, 0x4e, 0x85,
    0x8b, 0x49, 0xf9, 0xdd, 0xe6, 0xa1, 0xcd, 0xad, 0x49, 0x52, 0xe9, 0xb3,
    0x36, 0x17, 0x06, 0xd0
};
static int dsa_2048_224_sha224_counter = 2878;

static const unsigned char dsa_3072_256_sha512_p[] = {
    0x9a, 0x82, 0x8b, 0x8d, 0xea, 0xd0, 0x56, 0x23, 0x88, 0x2d, 0x5d, 0x41,
    0x42, 0x4c, 0x13, 0x5a, 0x15, 0x81, 0x59, 0x02, 0xc5, 0x00, 0x82, 0x28,
    0x01, 0xee, 0x8f, 0x99, 0xfd, 0x6a, 0x95, 0xf2, 0x0f, 0xae, 0x34, 0x77,
    0x29, 0xcc, 0xc7, 0x50, 0x0e, 0x03, 0xef, 0xb0, 0x4d, 0xe5, 0x10, 0x00,
    0xa8, 0x7b, 0xce, 0x8c, 0xc6, 0xb2, 0x01, 0x74, 0x23, 0x1b, 0x7f, 0xe8,
    0xf9, 0x71, 0x28, 0x39, 0xcf, 0x18, 0x04, 0xb2, 0x95, 0x61, 0x2d, 0x11,
    0x71, 0x6b, 0xdd, 0x0d, 0x0b, 0xf0, 0xe6, 0x97, 0x52, 0x29, 0x9d, 0x45,
    0xb1, 0x23, 0xda, 0xb0, 0xd5, 0xcb, 0x51, 0x71, 0x8e, 0x40, 0x9c, 0x97,
    0x13, 0xea, 0x1f, 0x4b, 0x32, 0x5d, 0x27, 0x74, 0x81, 0x8d, 0x47, 0x8a,
    0x08, 0xce, 0xf4, 0xd1, 0x28, 0xa2, 0x0f, 0x9b, 0x2e, 0xc9, 0xa3, 0x0e,
    0x5d, 0xde, 0x47, 0x19, 0x6d, 0x5f, 0x98, 0xe0, 0x8e, 0x7f, 0x60, 0x8f,
    0x25, 0xa7, 0xa4, 0xeb, 0xb9, 0xf3, 0x24, 0xa4, 0x9e, 0xc1, 0xbd, 0x14,
    0x27, 0x7c, 0x27, 0xc8, 0x4f, 0x5f, 0xed, 0xfd, 0x86, 0xc8, 0xf1, 0xd7,
    0x82, 0xe2, 0xeb, 0xe5, 0xd2, 0xbe, 0xb0, 0x65, 0x28, 0xab, 0x99, 0x9e,
    0xcd, 0xd5, 0x22, 0xf8, 0x1b, 0x3b, 0x01, 0xe9, 0x20, 0x3d, 0xe4, 0x98,
    0x22, 0xfe, 0xfc, 0x09, 0x7e, 0x95, 0x20, 0xda, 0xb6, 0x12, 0x2c, 0x94,
    0x5c, 0xea, 0x74, 0x71, 0xbd, 0x19, 0xac, 0x78, 0x43, 0x02, 0x51, 0xb8,
    0x5f, 0x06, 0x1d, 0xea, 0xc8, 0xa4, 0x3b, 0xc9, 0x78, 0xa3, 0x2b, 0x09,
    0xdc, 0x76, 0x74, 0xc4, 0x23, 0x14, 0x48, 0x2e, 0x84, 0x2b, 0xa3, 0x82,
    0xc1, 0xba, 0x0b, 0x39, 0x2a, 0x9f, 0x24, 0x7b, 0xd6, 0xc2, 0xea, 0x5a,
    0xb6, 0xbd, 0x15, 0x82, 0x21, 0x85, 0xe0, 0x6b, 0x12, 0x4f, 0x8d, 0x64,
    0x75, 0xeb, 0x7e, 0xa1, 0xdb, 0xe0, 0x9d, 0x25, 0xae, 0x3b, 0xe9, 0x9b,
    0x21, 0x7f, 0x9a, 0x3d, 0x66, 0xd0, 0x52, 0x1d, 0x39, 0x8b, 0xeb, 0xfc,
    0xec, 0xbe, 0x72, 0x20, 0x5a, 0xdf, 0x1b, 0x00, 0xf1, 0x0e, 0xed, 0xc6,
    0x78, 0x6f, 0xc9, 0xab, 0xe4, 0xd6, 0x81, 0x8b, 0xcc, 0xf6, 0xd4, 0x6a,
    0x31, 0x62, 0x08, 0xd9, 0x38, 0x21, 0x8f, 0xda, 0x9e, 0xb1, 0x2b, 0x9c,
    0xc0, 0xbe, 0xf7, 0x9a, 0x43, 0x2d, 0x07, 0x59, 0x46, 0x0e, 0xd5, 0x23,
    0x4e, 0xaa, 0x4a, 0x04, 0xc2, 0xde, 0x33, 0xa6, 0x34, 0xba, 0xac, 0x4f,
    0x78, 0xd8, 0xca, 0x76, 0xce, 0x5e, 0xd4, 0xf6, 0x85, 0x4c, 0x6a, 0x60,
    0x08, 0x5d, 0x0e, 0x34, 0x8b, 0xf2, 0xb6, 0xe3, 0xb7, 0x51, 0xca, 0x43,
    0xaa, 0x68, 0x7b, 0x0a, 0x6e, 0xea, 0xce, 0x1e, 0x2c, 0x34, 0x8e, 0x0f,
    0xe2, 0xcc, 0x38, 0xf2, 0x9a, 0x98, 0xef, 0xe6, 0x7f, 0xf6, 0x62, 0xbb
};
static const unsigned char dsa_3072_256_sha512_q[] = {
    0xc1, 0xdb, 0xc1, 0x21, 0x50, 0x49, 0x63, 0xa3, 0x77, 0x6d, 0x4c, 0x92,
    0xed, 0x58, 0x9e, 0x98, 0xea, 0xac, 0x7a, 0x90, 0x13, 0x24, 0xf7, 0xcd,
    0xd7, 0xe6, 0xd4, 0x8f, 0xf0, 0x45, 0x4b, 0xf7
};
static const unsigned char dsa_3072_256_sha512_seed[] = {
    0x35, 0x24, 0xb5, 0x59, 0xd5, 0x27, 0x58, 0x10, 0xf6, 0xa2, 0x7c, 0x9a,
    0x0d, 0xc2, 0x70, 0x8a, 0xb0, 0x41, 0x4a, 0x84, 0x0b, 0xfe, 0x66, 0xf5,
    0x3a, 0xbf, 0x4a, 0xa9, 0xcb, 0xfc, 0xa6, 0x22
};
static int dsa_3072_256_sha512_counter = 1604;

static const unsigned char dsa_2048_224_sha256_p[] = {
    0xe9, 0x13, 0xbc, 0xf2, 0x14, 0x5d, 0xf9, 0x79, 0xd6, 0x6d, 0xf5, 0xc5,
    0xbe, 0x7b, 0x6f, 0x90, 0x63, 0xd0, 0xfd, 0xee, 0x4f, 0xc4, 0x65, 0x83,
    0xbf, 0xec, 0xc3, 0x2c, 0x5d, 0x30, 0xc8, 0xa4, 0x3b, 0x2f, 0x3b, 0x29,
    0x43, 0x69, 0xfb, 0x6e, 0xa9, 0xa4, 0x07, 0x6c, 0xcd, 0xb0, 0xd2, 0xd9,
    0xd3, 0xe6, 0xf4, 0x87, 0x16, 0xb7, 0xe5, 0x06, 0xb9, 0xba, 0xd6, 0x87,
    0xbc, 0x01, 0x9e, 0xba, 0xc2, 0xcf, 0x39, 0xb6, 0xec, 0xdc, 0x75, 0x07,
    0xc1, 0x39, 0x2d, 0x6a, 0x95, 0x31, 0x97, 0xda, 0x54, 0x20, 0x29, 0xe0,
    0x1b, 0xf9, 0x74, 0x65, 0xaa, 0xc1, 0x47, 0xd3, 0x9e, 0xb4, 0x3c, 0x1d,
    0xe0, 0xdc, 0x2d, 0x21, 0xab, 0x12, 0x3b, 0xa5, 0x51, 0x1e, 0xc6, 0xbc,
    0x6b, 0x4c, 0x22, 0xd1, 0x7c, 0xc6, 0xce, 0xcb, 0x8c, 0x1d, 0x1f, 0xce,
    0x1c, 0xe2, 0x75, 0x49, 0x6d, 0x2c, 0xee, 0x7f, 0x5f, 0xb8, 0x74, 0x42,
    0x5c, 0x96, 0x77, 0x13, 0xff, 0x80, 0xf3, 0x05, 0xc7, 0xfe, 0x08, 0x3b,
    0x25, 0x36, 0x46, 0xa2, 0xc4, 0x26, 0xb4, 0xb0, 0x3b, 0xd5, 0xb2, 0x4c,
    0x13, 0x29, 0x0e, 0x47, 0x31, 0x66, 0x7d, 0x78, 0x57, 0xe6, 0xc2, 0xb5,
    0x9f, 0x46, 0x17, 0xbc, 0xa9, 0x9a, 0x49, 0x1c, 0x0f, 0x45, 0xe0, 0x88,
    0x97, 0xa1, 0x30, 0x7c, 0x42, 0xb7, 0x2c, 0x0a, 0xce, 0xb3, 0xa5, 0x7a,
    0x61, 0x8e, 0xab, 0x44, 0xc1, 0xdc, 0x70, 0xe5, 0xda, 0x78, 0x2a, 0xb4,
    0xe6, 0x3c, 0xa0, 0x58, 0xda, 0x62, 0x0a, 0xb2, 0xa9, 0x3d, 0xaa, 0x49,
    0x7e, 0x7f, 0x9a, 0x19, 0x67, 0xee, 0xd6, 0xe3, 0x67, 0x13, 0xe8, 0x6f,
    0x79, 0x50, 0x76, 0xfc, 0xb3, 0x9d, 0x7e, 0x9e, 0x3e, 0x6e, 0x47, 0xb1,
    0x11, 0x5e, 0xc8, 0x83, 0x3a, 0x3c, 0xfc, 0x82, 0x5c, 0x9d, 0x34, 0x65,
    0x73, 0xb4, 0x56, 0xd5
};
static const unsigned char dsa_2048_224_sha256_q[] = {
    0xb0, 0xdf, 0xa1, 0x7b, 0xa4, 0x77, 0x64, 0x0e, 0xb9, 0x28, 0xbb, 0xbc,
    0xd4, 0x60, 0x02, 0xaf, 0x21, 0x8c, 0xb0, 0x69, 0x0f, 0x8a, 0x7b, 0xc6,
    0x80, 0xcb, 0x0a, 0x45
};
static const unsigned char dsa_2048_224_sha256_g[] = {
    0x11, 0x7c, 0x5f, 0xf6, 0x99, 0x44, 0x67, 0x5b, 0x69, 0xa3, 0x83, 0xef,
    0xb5, 0x85, 0xa2, 0x19, 0x35, 0x18, 0x2a, 0xf2, 0x58, 0xf4, 0xc9, 0x58,
    0x9e, 0xb9, 0xe8, 0x91, 0x17, 0x2f, 0xb0, 0x60, 0x85, 0x95, 0xa6, 0x62,
    0x36, 0xd0, 0xff, 0x94, 0xb9, 0xa6, 0x50, 0xad, 0xa6, 0xf6, 0x04, 0x28,
    0xc2, 0xc9, 0xb9, 0x75, 0xf3, 0x66, 0xb4, 0xeb, 0xf6, 0xd5, 0x06, 0x13,
    0x01, 0x64, 0x82, 0xa9, 0xf1, 0xd5, 0x41, 0xdc, 0xf2, 0x08, 0xfc, 0x2f,
    0xc4, 0xa1, 0x21, 0xee, 0x7d, 0xbc, 0xda, 0x5a, 0xa4, 0xa2, 0xb9, 0x68,
    0x87, 0x36, 0xba, 0x53, 0x9e, 0x14, 0x4e, 0x76, 0x5c, 0xba, 0x79, 0x3d,
    0x0f, 0xe5, 0x99, 0x1c, 0x27, 0xfc, 0xaf, 0x10, 0x63, 0x87, 0x68, 0x0e,
    0x3e, 0x6e, 0xaa, 0xf3, 0xdf, 0x76, 0x7e, 0x02, 0x9a, 0x41, 0x96, 0xa1,
    0x6c, 0xbb, 0x67, 0xee, 0x0c, 0xad, 0x72, 0x65, 0xf1, 0x70, 0xb0, 0x39,
    0x9b, 0x54, 0x5f, 0xd7, 0x6c, 0xc5, 0x9a, 0x90, 0x53, 0x18, 0xde, 0x5e,
    0x62, 0x89, 0xb9, 0x2f, 0x66, 0x59, 0x3a, 0x3d, 0x10, 0xeb, 0xa5, 0x99,
    0xf6, 0x21, 0x7d, 0xf2, 0x7b, 0x42, 0x15, 0x1c, 0x55, 0x79, 0x15, 0xaa,
    0xa4, 0x17, 0x2e, 0x48, 0xc3, 0xa8, 0x36, 0xf5, 0x1a, 0x97, 0xce, 0xbd,
    0x72, 0xef, 0x1d, 0x50, 0x5b, 0xb1, 0x60, 0x0a, 0x5c, 0x0b, 0xa6, 0x21,
    0x38, 0x28, 0x4e, 0x89, 0x33, 0x1d, 0xb5, 0x7e, 0x5c, 0xf1, 0x6b, 0x2c,
    0xbd, 0xad, 0x84, 0xb2, 0x8e, 0x96, 0xe2, 0x30, 0xe7, 0x54, 0xb8, 0xc9,
    0x70, 0xcb, 0x10, 0x30, 0x63, 0x90, 0xf4, 0x45, 0x64, 0x93, 0x09, 0x38,
    0x6a, 0x47, 0x58, 0x31, 0x04, 0x1a, 0x18, 0x04, 0x1a, 0xe0, 0xd7, 0x0b,
    0x3c, 0xbe, 0x2a, 0x9c, 0xec, 0xcc, 0x0d, 0x0c, 0xed, 0xde, 0x54, 0xbc,
    0xe6, 0x93, 0x59, 0xfc
};

static int ffc_params_validate_g_unverified_test(void)
{
    int ret = 0, res;
    FFC_PARAMS params;
    BIGNUM *p = NULL, *q = NULL, *g = NULL;
    BIGNUM *p1 = NULL, *g1 = NULL;

    ossl_ffc_params_init(&params);

    if (!TEST_ptr(p = BN_bin2bn(dsa_2048_224_sha256_p,
                                sizeof(dsa_2048_224_sha256_p), NULL)))
        goto err;
    p1 = p;
    if (!TEST_ptr(q = BN_bin2bn(dsa_2048_224_sha256_q,
                                sizeof(dsa_2048_224_sha256_q), NULL)))
        goto err;
    if (!TEST_ptr(g = BN_bin2bn(dsa_2048_224_sha256_g,
                                sizeof(dsa_2048_224_sha256_g), NULL)))
        goto err;
    g1 = g;

    /* Fail if g is NULL */
    ossl_ffc_params_set0_pqg(&params, p, q, NULL);
    p = NULL;
    q = NULL;
    ossl_ffc_params_set_flags(&params, FFC_PARAM_FLAG_VALIDATE_G);
    ossl_ffc_set_digest(&params, "SHA256", NULL);

    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;

    ossl_ffc_params_set0_pqg(&params, p, q, g);
    g = NULL;
    if (!TEST_true(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                      FFC_PARAM_TYPE_DSA,
                                                      &res, NULL)))
        goto err;

    /* incorrect g */
    BN_add_word(g1, 1);
    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;

    /* fail if g < 2 */
    BN_set_word(g1, 1);
    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;

    BN_copy(g1, p1);
    /* Fail if g >= p */
    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;

    ret = 1;
err:
    ossl_ffc_params_cleanup(&params);
    BN_free(p);
    BN_free(q);
    BN_free(g);
    return ret;
}

static int ffc_params_validate_pq_test(void)
{
    int ret = 0, res = -1;
    FFC_PARAMS params;
    BIGNUM *p = NULL, *q = NULL;

    ossl_ffc_params_init(&params);
    if (!TEST_ptr(p = BN_bin2bn(dsa_2048_224_sha224_p,
                                   sizeof(dsa_2048_224_sha224_p),
                                   NULL)))
        goto err;
    if (!TEST_ptr(q = BN_bin2bn(dsa_2048_224_sha224_q,
                                   sizeof(dsa_2048_224_sha224_q),
                                   NULL)))
        goto err;

    /* No p */
    ossl_ffc_params_set0_pqg(&params, NULL, q, NULL);
    q = NULL;
    ossl_ffc_params_set_flags(&params, FFC_PARAM_FLAG_VALIDATE_PQ);
    ossl_ffc_set_digest(&params, "SHA224", NULL);

    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;

    /* Test valid case */
    ossl_ffc_params_set0_pqg(&params, p, NULL, NULL);
    p = NULL;
    ossl_ffc_params_set_validate_params(&params, dsa_2048_224_sha224_seed,
                                        sizeof(dsa_2048_224_sha224_seed),
                                        dsa_2048_224_sha224_counter);
    if (!TEST_true(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                      FFC_PARAM_TYPE_DSA,
                                                      &res, NULL)))
        goto err;

    /* Bad counter - so p is not prime */
    ossl_ffc_params_set_validate_params(&params, dsa_2048_224_sha224_seed,
                                        sizeof(dsa_2048_224_sha224_seed),
                                        1);
    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;

    /* seedlen smaller than N */
    ossl_ffc_params_set_validate_params(&params, dsa_2048_224_sha224_seed,
                                        sizeof(dsa_2048_224_sha224_seed)-1,
                                        dsa_2048_224_sha224_counter);
    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;

    /* Provided seed doesn't produce a valid prime q */
    ossl_ffc_params_set_validate_params(&params, dsa_2048_224_sha224_bad_seed,
                                        sizeof(dsa_2048_224_sha224_bad_seed),
                                        dsa_2048_224_sha224_counter);
    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;

    if (!TEST_ptr(p = BN_bin2bn(dsa_3072_256_sha512_p,
                                sizeof(dsa_3072_256_sha512_p), NULL)))
        goto err;
    if (!TEST_ptr(q = BN_bin2bn(dsa_3072_256_sha512_q,
                                sizeof(dsa_3072_256_sha512_q),
                                NULL)))
        goto err;


    ossl_ffc_params_set0_pqg(&params, p, q, NULL);
    p = q  = NULL;
    ossl_ffc_set_digest(&params, "SHA512", NULL);
    ossl_ffc_params_set_validate_params(&params, dsa_3072_256_sha512_seed,
                                        sizeof(dsa_3072_256_sha512_seed),
                                        dsa_3072_256_sha512_counter);
    /* Q doesn't div P-1 */
    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;

    /* Bad L/N for FIPS DH */
    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DH,
                                                       &res, NULL)))
        goto err;

    ret = 1;
err:
    ossl_ffc_params_cleanup(&params);
    BN_free(p);
    BN_free(q);
    return ret;
}
#endif /* OPENSSL_NO_DSA */

#ifndef OPENSSL_NO_DH
static int ffc_params_gen_test(void)
{
    int ret = 0, res = -1;
    FFC_PARAMS params;

    ossl_ffc_params_init(&params);
    if (!TEST_true(ossl_ffc_params_FIPS186_4_generate(NULL, &params,
                                                      FFC_PARAM_TYPE_DH,
                                                      2048, 256, &res, NULL)))
        goto err;
    if (!TEST_true(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                      FFC_PARAM_TYPE_DH,
                                                      &res, NULL)))
        goto err;

    ret = 1;
err:
    ossl_ffc_params_cleanup(&params);
    return ret;
}

static int ffc_params_gen_canonicalg_test(void)
{
    int ret = 0, res = -1;
    FFC_PARAMS params;

    ossl_ffc_params_init(&params);
    params.gindex = 1;
    if (!TEST_true(ossl_ffc_params_FIPS186_4_generate(NULL, &params,
                                                      FFC_PARAM_TYPE_DH,
                                                      2048, 256, &res, NULL)))
        goto err;
    if (!TEST_true(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                      FFC_PARAM_TYPE_DH,
                                                      &res, NULL)))
        goto err;

    if (!TEST_true(ossl_ffc_params_print(bio_out, &params, 4)))
        goto err;

    ret = 1;
err:
    ossl_ffc_params_cleanup(&params);
    return ret;
}

static int ffc_params_fips186_2_gen_validate_test(void)
{
    int ret = 0, res = -1;
    FFC_PARAMS params;
    BIGNUM *bn = NULL;

    ossl_ffc_params_init(&params);
    if (!TEST_ptr(bn = BN_new()))
        goto err;
    if (!TEST_true(ossl_ffc_params_FIPS186_2_generate(NULL, &params,
                                                      FFC_PARAM_TYPE_DH,
                                                      1024, 160, &res, NULL)))
        goto err;
    if (!TEST_true(ossl_ffc_params_FIPS186_2_validate(NULL, &params,
                                                      FFC_PARAM_TYPE_DH,
                                                      &res, NULL)))
        goto err;

    /*
     * The fips186-2 generation should produce a different q compared to
     * fips 186-4 given the same seed value. So validation of q will fail.
     */
    if (!TEST_false(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                       FFC_PARAM_TYPE_DSA,
                                                       &res, NULL)))
        goto err;
    /* As the params are randomly generated the error is one of the following */
    if (!TEST_true(res == FFC_CHECK_Q_MISMATCH || res == FFC_CHECK_Q_NOT_PRIME))
        goto err;

    ossl_ffc_params_set_flags(&params, FFC_PARAM_FLAG_VALIDATE_G);
    /* Partially valid g test will still pass */
    if (!TEST_int_eq(ossl_ffc_params_FIPS186_4_validate(NULL, &params,
                                                        FFC_PARAM_TYPE_DSA,
                                                        &res, NULL), 2))
        goto err;

    if (!TEST_true(ossl_ffc_params_print(bio_out, &params, 4)))
        goto err;

    ret = 1;
err:
    BN_free(bn);
    ossl_ffc_params_cleanup(&params);
    return ret;
}

extern FFC_PARAMS *ossl_dh_get0_params(DH *dh);

static int ffc_public_validate_test(void)
{
    int ret = 0, res = -1;
    FFC_PARAMS *params;
    BIGNUM *pub = NULL;
    DH *dh = NULL;

    if (!TEST_ptr(pub = BN_new()))
        goto err;

    if (!TEST_ptr(dh = DH_new_by_nid(NID_ffdhe2048)))
        goto err;
    params = ossl_dh_get0_params(dh);

    if (!TEST_true(BN_set_word(pub, 1)))
        goto err;
    BN_set_negative(pub, 1);
    /* Check must succeed but set res if public key is negative */
    if (!TEST_true(ossl_ffc_validate_public_key(params, pub, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PUBKEY_TOO_SMALL, res))
        goto err;
    if (!TEST_true(BN_set_word(pub, 0)))
        goto err;
    /* Check must succeed but set res if public key is zero */
    if (!TEST_true(ossl_ffc_validate_public_key(params, pub, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PUBKEY_TOO_SMALL, res))
        goto err;
    /* Check must succeed but set res if public key is 1 */
    if (!TEST_true(ossl_ffc_validate_public_key(params, BN_value_one(), &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PUBKEY_TOO_SMALL, res))
        goto err;
    if (!TEST_true(BN_add_word(pub, 2)))
        goto err;
    /* Pass if public key >= 2 */
    if (!TEST_true(ossl_ffc_validate_public_key(params, pub, &res)))
        goto err;

    if (!TEST_ptr(BN_copy(pub, params->p)))
        goto err;
    /* Check must succeed but set res if public key = p */
    if (!TEST_true(ossl_ffc_validate_public_key(params, pub, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PUBKEY_TOO_LARGE, res))
        goto err;

    if (!TEST_true(BN_sub_word(pub, 1)))
        goto err;
    /* Check must succeed but set res if public key = p - 1 */
    if (!TEST_true(ossl_ffc_validate_public_key(params, pub, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PUBKEY_TOO_LARGE, res))
        goto err;

    if (!TEST_true(BN_sub_word(pub, 1)))
        goto err;
    /* Check must succeed but set res if public key is not related to p & q */
    if (!TEST_true(ossl_ffc_validate_public_key(params, pub, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PUBKEY_INVALID, res))
        goto err;

    if (!TEST_true(BN_sub_word(pub, 5)))
        goto err;
    /* Pass if public key is valid */
    if (!TEST_true(ossl_ffc_validate_public_key(params, pub, &res)))
        goto err;

    /* Check must succeed but set res if params is NULL */
    if (!TEST_true(ossl_ffc_validate_public_key(NULL, pub, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PASSED_NULL_PARAM, res))
        goto err;
    res = -1;
    /* Check must succeed but set res if pubkey is NULL */
    if (!TEST_true(ossl_ffc_validate_public_key(params, NULL, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PASSED_NULL_PARAM, res))
        goto err;
    res = -1;

    BN_free(params->p);
    params->p = NULL;
    /* Check must succeed but set res if params->p is NULL */
    if (!TEST_true(ossl_ffc_validate_public_key(params, pub, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PASSED_NULL_PARAM, res))
        goto err;

    ret = 1;
err:
    DH_free(dh);
    BN_free(pub);
    return ret;
}

static int ffc_private_validate_test(void)
{
    int ret = 0, res = -1;
    FFC_PARAMS *params;
    BIGNUM *priv = NULL;
    DH *dh = NULL;

    if (!TEST_ptr(priv = BN_new()))
        goto err;

    if (!TEST_ptr(dh = DH_new_by_nid(NID_ffdhe2048)))
        goto err;
    params = ossl_dh_get0_params(dh);

    if (!TEST_true(BN_set_word(priv, 1)))
        goto err;
    BN_set_negative(priv, 1);
    /* Fail if priv key is negative */
    if (!TEST_false(ossl_ffc_validate_private_key(params->q, priv, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PRIVKEY_TOO_SMALL, res))
        goto err;

    if (!TEST_true(BN_set_word(priv, 0)))
        goto err;
    /* Fail if priv key is zero */
    if (!TEST_false(ossl_ffc_validate_private_key(params->q, priv, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PRIVKEY_TOO_SMALL, res))
        goto err;

    /* Pass if priv key >= 1 */
    if (!TEST_true(ossl_ffc_validate_private_key(params->q, BN_value_one(),
                                                 &res)))
        goto err;

    if (!TEST_ptr(BN_copy(priv, params->q)))
        goto err;
    /* Fail if priv key = upper */
    if (!TEST_false(ossl_ffc_validate_private_key(params->q, priv, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PRIVKEY_TOO_LARGE, res))
        goto err;

    if (!TEST_true(BN_sub_word(priv, 1)))
        goto err;
    /* Pass if priv key <= upper - 1 */
    if (!TEST_true(ossl_ffc_validate_private_key(params->q, priv, &res)))
        goto err;

    if (!TEST_false(ossl_ffc_validate_private_key(NULL, priv, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PASSED_NULL_PARAM, res))
        goto err;
    res = -1;
    if (!TEST_false(ossl_ffc_validate_private_key(params->q, NULL, &res)))
        goto err;
    if (!TEST_int_eq(FFC_ERROR_PASSED_NULL_PARAM, res))
        goto err;

    ret = 1;
err:
    DH_free(dh);
    BN_free(priv);
    return ret;
}

static int ffc_private_gen_test(int index)
{
    int ret = 0, res = -1, N;
    FFC_PARAMS *params;
    BIGNUM *priv = NULL;
    DH *dh = NULL;
    BN_CTX *ctx = NULL;

    if (!TEST_ptr(ctx = BN_CTX_new_ex(NULL)))
        goto err;

    if (!TEST_ptr(priv = BN_new()))
        goto err;

    if (!TEST_ptr(dh = DH_new_by_nid(NID_ffdhe2048)))
        goto err;
    params = ossl_dh_get0_params(dh);

    N = BN_num_bits(params->q);
    /* Fail since N < 2*s - where s = 112*/
    if (!TEST_false(ossl_ffc_generate_private_key(ctx, params, 220, 112, priv)))
        goto err;
    /* fail since N > len(q) */
    if (!TEST_false(ossl_ffc_generate_private_key(ctx, params, N + 1, 112, priv)))
        goto err;
    /* s must be always set */
    if (!TEST_false(ossl_ffc_generate_private_key(ctx, params, N, 0, priv)))
        goto err;
    /* pass since 2s <= N <= len(q) */
    if (!TEST_true(ossl_ffc_generate_private_key(ctx, params, N, 112, priv)))
        goto err;
    /* pass since N = len(q) */
    if (!TEST_true(ossl_ffc_validate_private_key(params->q, priv, &res)))
        goto err;
    /* pass since 2s <= N < len(q) */
    if (!TEST_true(ossl_ffc_generate_private_key(ctx, params, N / 2, 112, priv)))
        goto err;
    if (!TEST_true(ossl_ffc_validate_private_key(params->q, priv, &res)))
        goto err;
    /* N is ignored in this case */
    if (!TEST_true(ossl_ffc_generate_private_key(ctx, params, 0,
                                                 ossl_ifc_ffc_compute_security_bits(BN_num_bits(params->p)),
                                                 priv)))
        goto err;
    if (!TEST_int_le(BN_num_bits(priv), 225))
        goto err;
    if (!TEST_true(ossl_ffc_validate_private_key(params->q, priv, &res)))
        goto err;

    ret = 1;
err:
    DH_free(dh);
    BN_free(priv);
    BN_CTX_free(ctx);
    return ret;
}

static int ffc_params_copy_test(void)
{
    int ret = 0;
    DH *dh = NULL;
    FFC_PARAMS *params, copy;

    ossl_ffc_params_init(&copy);

    if (!TEST_ptr(dh = DH_new_by_nid(NID_ffdhe3072)))
        goto err;
    params = ossl_dh_get0_params(dh);

    if (!TEST_int_eq(params->keylength, 275))
        goto err;

    if (!TEST_true(ossl_ffc_params_copy(&copy, params)))
        goto err;

    if (!TEST_int_eq(copy.keylength, 275))
        goto err;

    if (!TEST_true(ossl_ffc_params_cmp(&copy, params, 0)))
        goto err;

    ret = 1;
err:
    ossl_ffc_params_cleanup(&copy);
    DH_free(dh);
    return ret;
}
#endif /* OPENSSL_NO_DH */

int setup_tests(void)
{
#ifndef OPENSSL_NO_DSA
    ADD_TEST(ffc_params_validate_pq_test);
    ADD_TEST(ffc_params_validate_g_unverified_test);
#endif /* OPENSSL_NO_DSA */
#ifndef OPENSSL_NO_DH
    ADD_TEST(ffc_params_gen_test);
    ADD_TEST(ffc_params_gen_canonicalg_test);
    ADD_TEST(ffc_params_fips186_2_gen_validate_test);
    ADD_TEST(ffc_public_validate_test);
    ADD_TEST(ffc_private_validate_test);
    ADD_ALL_TESTS(ffc_private_gen_test, 10);
    ADD_TEST(ffc_params_copy_test);
#endif /* OPENSSL_NO_DH */
    return 1;
}
