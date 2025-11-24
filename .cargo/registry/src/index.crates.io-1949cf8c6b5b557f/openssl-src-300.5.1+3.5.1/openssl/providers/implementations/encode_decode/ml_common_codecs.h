/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef PROV_ML_COMMON_CODECS_H
# define PROV_ML_COMMON_CODECS_H
# pragma once

#  include <openssl/e_os2.h>
#  include "crypto/ml_dsa.h"
#  include "prov/provider_ctx.h"

 /*-
  * The DER ASN.1 encoding of ML-DSA and ML-KEM public keys prepends 22 bytes
  * to the encoded public key:
  *
  * - 4 byte outer sequence tag and length
  * -  2 byte algorithm sequence tag and length
  * -    2 byte algorithm OID tag and length
  * -      9 byte algorithm OID (from NIST CSOR OID arc)
  * -  4 byte bit string tag and length
  * -    1 bitstring lead byte
  */
#  define ML_COMMON_SPKI_OVERHEAD   22
typedef struct {
    const uint8_t asn1_prefix[ML_COMMON_SPKI_OVERHEAD];
} ML_COMMON_SPKI_FMT;

/*-
* For each parameter set we support a few PKCS#8 input formats, three
 * corresponding to the "either or both" variants of:
 *
 *  ML-DSA-PrivateKey ::= CHOICE {
 *    seed [0] IMPLICIT OCTET STRING (SIZE (32)),
 *    expandedKey OCTET STRING (SIZE (2560 | 4032 | 4896)),
 *    both SEQUENCE {
 *      seed OCTET STRING (SIZE (32)),
 *      expandedKey OCTET STRING (SIZE (2560 | 4032 | 4896)) } }
 *
 *  ML-KEM-PrivateKey ::= CHOICE {
 *    seed [0] IMPLICIT OCTET STRING (SIZE (64)),
 *    expandedKey OCTET STRING (SIZE (1632 | 2400 | 3168)),
 *    both SEQUENCE {
 *      seed OCTET STRING (SIZE (64)),
 *      expandedKey OCTET STRING SIZE ((1632 | 2400 | 3168)) } }
 *
 * one more for a historical OQS encoding:
 *
 * - OQS private + public key: OCTET STRING
 *   (The public key is ignored, just as with PKCS#8 v2.)
 *
 * and two more that are the minimal IETF non-ASN.1 seed encoding:
 *
 * - Bare seed (just the 32 or 64 bytes)
 * - Bare priv (just the key bytes)
 *
 * A length of zero means that particular field is absent.
 *
 * The p8_shift is 0 when the top-level tag+length occupy four bytes, 2 when
 * they occupy two by†es, and 4 when no tag is used at all.
 */
#define NUM_PKCS8_FORMATS   6

typedef struct {
    const char *p8_name;    /* Format name */
    size_t p8_bytes;        /* Total P8 encoding length */
    int    p8_shift;        /* 4 - (top-level tag + len) */
    uint32_t p8_magic;      /* The tag + len value */
    uint16_t seed_magic;    /* Interior tag + len for the seed */
    size_t seed_offset;     /* Seed offset from start */
    size_t seed_length;     /* Seed bytes */
    uint32_t priv_magic;    /* Interior tag + len for the key */
    size_t priv_offset;     /* Key offset from start */
    size_t priv_length;     /* Key bytes */
    size_t pub_offset;      /* Pubkey offset */
    size_t pub_length;      /* Pubkey bytes */
} ML_COMMON_PKCS8_FMT;

typedef struct {
    const ML_COMMON_SPKI_FMT *spkifmt;
    const ML_COMMON_PKCS8_FMT *p8fmt;
} ML_COMMON_CODEC;

typedef struct {
    const ML_COMMON_PKCS8_FMT *fmt;
    int pref;
} ML_COMMON_PKCS8_FMT_PREF;

ML_COMMON_PKCS8_FMT_PREF *
ossl_ml_common_pkcs8_fmt_order(const char *algorithm_name,
                               const ML_COMMON_PKCS8_FMT *p8fmt,
                               const char *direction, const char *formats);
#endif
