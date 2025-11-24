/*
 * Copyright 1999-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include "dsa_local.h"
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/rand.h>
#include "crypto/asn1_dsa.h"

/* Override the default free and new methods */
static int dsa_cb(int operation, ASN1_VALUE **pval, const ASN1_ITEM *it,
                  void *exarg)
{
    if (operation == ASN1_OP_NEW_PRE) {
        *pval = (ASN1_VALUE *)DSA_new();
        if (*pval != NULL)
            return 2;
        return 0;
    } else if (operation == ASN1_OP_FREE_PRE) {
        DSA_free((DSA *)*pval);
        *pval = NULL;
        return 2;
    }
    return 1;
}

ASN1_SEQUENCE_cb(DSAPrivateKey, dsa_cb) = {
        ASN1_EMBED(DSA, version, INT32),
        ASN1_SIMPLE(DSA, params.p, BIGNUM),
        ASN1_SIMPLE(DSA, params.q, BIGNUM),
        ASN1_SIMPLE(DSA, params.g, BIGNUM),
        ASN1_SIMPLE(DSA, pub_key, BIGNUM),
        ASN1_SIMPLE(DSA, priv_key, CBIGNUM)
} static_ASN1_SEQUENCE_END_cb(DSA, DSAPrivateKey)

IMPLEMENT_ASN1_ENCODE_FUNCTIONS_fname(DSA, DSAPrivateKey, DSAPrivateKey)

ASN1_SEQUENCE_cb(DSAparams, dsa_cb) = {
        ASN1_SIMPLE(DSA, params.p, BIGNUM),
        ASN1_SIMPLE(DSA, params.q, BIGNUM),
        ASN1_SIMPLE(DSA, params.g, BIGNUM),
} static_ASN1_SEQUENCE_END_cb(DSA, DSAparams)

IMPLEMENT_ASN1_ENCODE_FUNCTIONS_fname(DSA, DSAparams, DSAparams)

ASN1_SEQUENCE_cb(DSAPublicKey, dsa_cb) = {
        ASN1_SIMPLE(DSA, pub_key, BIGNUM),
        ASN1_SIMPLE(DSA, params.p, BIGNUM),
        ASN1_SIMPLE(DSA, params.q, BIGNUM),
        ASN1_SIMPLE(DSA, params.g, BIGNUM)
} static_ASN1_SEQUENCE_END_cb(DSA, DSAPublicKey)

IMPLEMENT_ASN1_ENCODE_FUNCTIONS_fname(DSA, DSAPublicKey, DSAPublicKey)

DSA *DSAparams_dup(const DSA *dsa)
{
    return ASN1_item_dup(ASN1_ITEM_rptr(DSAparams), dsa);
}
