/*
 * Copyright 2001-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include "internal/refcount.h"
#include <openssl/asn1.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include "crypto/x509.h"

int X509_CRL_set_version(X509_CRL *x, long version)
{
    if (x == NULL)
        return 0;
    if (x->crl.version == NULL) {
        if ((x->crl.version = ASN1_INTEGER_new()) == NULL)
            return 0;
    }
    if (!ASN1_INTEGER_set(x->crl.version, version))
        return 0;
    x->crl.enc.modified = 1;
    return 1;
}

int X509_CRL_set_issuer_name(X509_CRL *x, const X509_NAME *name)
{
    if (x == NULL)
        return 0;
    if (!X509_NAME_set(&x->crl.issuer, name))
        return 0;
    x->crl.enc.modified = 1;
    return 1;
}

int X509_CRL_set1_lastUpdate(X509_CRL *x, const ASN1_TIME *tm)
{
    if (x == NULL || tm == NULL)
        return 0;
    return ossl_x509_set1_time(&x->crl.enc.modified, &x->crl.lastUpdate, tm);
}

int X509_CRL_set1_nextUpdate(X509_CRL *x, const ASN1_TIME *tm)
{
    if (x == NULL)
        return 0;
    return ossl_x509_set1_time(&x->crl.enc.modified, &x->crl.nextUpdate, tm);
}

int X509_CRL_sort(X509_CRL *c)
{
    int i;
    X509_REVOKED *r;

    /*
     * sort the data so it will be written in serial number order
     */
    sk_X509_REVOKED_sort(c->crl.revoked);
    for (i = 0; i < sk_X509_REVOKED_num(c->crl.revoked); i++) {
        r = sk_X509_REVOKED_value(c->crl.revoked, i);
        r->sequence = i;
    }
    c->crl.enc.modified = 1;
    return 1;
}

int X509_CRL_up_ref(X509_CRL *crl)
{
    int i;

    if (CRYPTO_UP_REF(&crl->references, &i) <= 0)
        return 0;

    REF_PRINT_COUNT("X509_CRL", i, crl);
    REF_ASSERT_ISNT(i < 2);
    return i > 1;
}

long X509_CRL_get_version(const X509_CRL *crl)
{
    return ASN1_INTEGER_get(crl->crl.version);
}

const ASN1_TIME *X509_CRL_get0_lastUpdate(const X509_CRL *crl)
{
    return crl->crl.lastUpdate;
}

const ASN1_TIME *X509_CRL_get0_nextUpdate(const X509_CRL *crl)
{
    return crl->crl.nextUpdate;
}

#ifndef OPENSSL_NO_DEPRECATED_1_1_0
ASN1_TIME *X509_CRL_get_lastUpdate(X509_CRL *crl)
{
    return crl->crl.lastUpdate;
}

ASN1_TIME *X509_CRL_get_nextUpdate(X509_CRL *crl)
{
    return crl->crl.nextUpdate;
}
#endif

X509_NAME *X509_CRL_get_issuer(const X509_CRL *crl)
{
    return crl->crl.issuer;
}

const STACK_OF(X509_EXTENSION) *X509_CRL_get0_extensions(const X509_CRL *crl)
{
    return crl->crl.extensions;
}

STACK_OF(X509_REVOKED) *X509_CRL_get_REVOKED(X509_CRL *crl)
{
    return crl->crl.revoked;
}

void X509_CRL_get0_signature(const X509_CRL *crl, const ASN1_BIT_STRING **psig,
                             const X509_ALGOR **palg)
{
    if (psig != NULL)
        *psig = &crl->signature;
    if (palg != NULL)
        *palg = &crl->sig_alg;
}

int X509_CRL_get_signature_nid(const X509_CRL *crl)
{
    return OBJ_obj2nid(crl->sig_alg.algorithm);
}

const ASN1_TIME *X509_REVOKED_get0_revocationDate(const X509_REVOKED *x)
{
    return x->revocationDate;
}

int X509_REVOKED_set_revocationDate(X509_REVOKED *x, ASN1_TIME *tm)
{
    if (x == NULL || tm == NULL)
        return 0;
    return ossl_x509_set1_time(NULL, &x->revocationDate, tm);
}

const ASN1_INTEGER *X509_REVOKED_get0_serialNumber(const X509_REVOKED *x)
{
    return &x->serialNumber;
}

int X509_REVOKED_set_serialNumber(X509_REVOKED *x, ASN1_INTEGER *serial)
{
    ASN1_INTEGER *in;

    if (x == NULL)
        return 0;
    in = &x->serialNumber;
    if (in != serial)
        return ASN1_STRING_copy(in, serial);
    return 1;
}

const STACK_OF(X509_EXTENSION) *X509_REVOKED_get0_extensions(const
                                                             X509_REVOKED *r)
{
    return r->extensions;
}

int i2d_re_X509_CRL_tbs(X509_CRL *crl, unsigned char **pp)
{
    crl->crl.enc.modified = 1;
    return i2d_X509_CRL_INFO(&crl->crl, pp);
}
