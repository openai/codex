/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/asn1.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include "crypto/x509.h"

int X509_REQ_set_version(X509_REQ *x, long version)
{
    if (x == NULL || version != X509_REQ_VERSION_1) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    x->req_info.enc.modified = 1;
    return ASN1_INTEGER_set(x->req_info.version, version);
}

int X509_REQ_set_subject_name(X509_REQ *x, const X509_NAME *name)
{
    if (x == NULL)
        return 0;
    x->req_info.enc.modified = 1;
    return X509_NAME_set(&x->req_info.subject, name);
}

int X509_REQ_set_pubkey(X509_REQ *x, EVP_PKEY *pkey)
{
    if (x == NULL)
        return 0;
    x->req_info.enc.modified = 1;
    return X509_PUBKEY_set(&x->req_info.pubkey, pkey);
}
