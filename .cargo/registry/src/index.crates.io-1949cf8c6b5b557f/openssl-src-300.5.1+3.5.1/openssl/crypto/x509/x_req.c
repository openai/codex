/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/x509.h>
#include "crypto/x509.h"

/*-
 * X509_REQ_INFO is handled in an unusual way to get round
 * invalid encodings. Some broken certificate requests don't
 * encode the attributes field if it is empty. This is in
 * violation of PKCS#10 but we need to tolerate it. We do
 * this by making the attributes field OPTIONAL then using
 * the callback to initialise it to an empty STACK.
 *
 * This means that the field will be correctly encoded unless
 * we NULL out the field.
 *
 * As a result we no longer need the req_kludge field because
 * the information is now contained in the attributes field:
 * 1. If it is NULL then it's the invalid omission.
 * 2. If it is empty it is the correct encoding.
 * 3. If it is not empty then some attributes are present.
 *
 */

static int rinf_cb(int operation, ASN1_VALUE **pval, const ASN1_ITEM *it,
                   void *exarg)
{
    X509_REQ_INFO *rinf = (X509_REQ_INFO *)*pval;

    if (operation == ASN1_OP_NEW_POST) {
        rinf->attributes = sk_X509_ATTRIBUTE_new_null();
        if (!rinf->attributes)
            return 0;
    }
    return 1;
}

static int req_cb(int operation, ASN1_VALUE **pval, const ASN1_ITEM *it,
                  void *exarg)
{
    X509_REQ *ret = (X509_REQ *)*pval;

    switch (operation) {
    case ASN1_OP_D2I_PRE:
        ASN1_OCTET_STRING_free(ret->distinguishing_id);
        /* fall through */
    case ASN1_OP_NEW_POST:
        ret->distinguishing_id = NULL;
        break;

    case ASN1_OP_FREE_POST:
        ASN1_OCTET_STRING_free(ret->distinguishing_id);
        OPENSSL_free(ret->propq);
        break;
    case ASN1_OP_DUP_POST:
        {
            X509_REQ *old = exarg;

            if (!ossl_x509_req_set0_libctx(ret, old->libctx, old->propq))
                return 0;
            if (old->req_info.pubkey != NULL) {
                EVP_PKEY *pkey = X509_PUBKEY_get0(old->req_info.pubkey);

                if (pkey != NULL) {
                    pkey = EVP_PKEY_dup(pkey);
                    if (pkey == NULL) {
                        ERR_raise(ERR_LIB_X509, ERR_R_EVP_LIB);
                        return 0;
                    }
                    if (!X509_PUBKEY_set(&ret->req_info.pubkey, pkey)) {
                        EVP_PKEY_free(pkey);
                        ERR_raise(ERR_LIB_X509, ERR_R_INTERNAL_ERROR);
                        return 0;
                    }
                    EVP_PKEY_free(pkey);
                }
            }
        }
        break;
    case ASN1_OP_GET0_LIBCTX:
        {
            OSSL_LIB_CTX **libctx = exarg;

            *libctx = ret->libctx;
        }
        break;
    case ASN1_OP_GET0_PROPQ:
        {
            const char **propq = exarg;

            *propq = ret->propq;
        }
        break;
    }

    return 1;
}

ASN1_SEQUENCE_enc(X509_REQ_INFO, enc, rinf_cb) = {
        ASN1_SIMPLE(X509_REQ_INFO, version, ASN1_INTEGER),
        ASN1_SIMPLE(X509_REQ_INFO, subject, X509_NAME),
        ASN1_SIMPLE(X509_REQ_INFO, pubkey, X509_PUBKEY),
        /* This isn't really OPTIONAL but it gets round invalid
         * encodings
         */
        ASN1_IMP_SET_OF_OPT(X509_REQ_INFO, attributes, X509_ATTRIBUTE, 0)
} ASN1_SEQUENCE_END_enc(X509_REQ_INFO, X509_REQ_INFO)

IMPLEMENT_ASN1_FUNCTIONS(X509_REQ_INFO)

ASN1_SEQUENCE_ref(X509_REQ, req_cb) = {
        ASN1_EMBED(X509_REQ, req_info, X509_REQ_INFO),
        ASN1_EMBED(X509_REQ, sig_alg, X509_ALGOR),
        ASN1_SIMPLE(X509_REQ, signature, ASN1_BIT_STRING)
} ASN1_SEQUENCE_END_ref(X509_REQ, X509_REQ)

IMPLEMENT_ASN1_FUNCTIONS(X509_REQ)

IMPLEMENT_ASN1_DUP_FUNCTION(X509_REQ)

void X509_REQ_set0_distinguishing_id(X509_REQ *x, ASN1_OCTET_STRING *d_id)
{
    ASN1_OCTET_STRING_free(x->distinguishing_id);
    x->distinguishing_id = d_id;
}

ASN1_OCTET_STRING *X509_REQ_get0_distinguishing_id(X509_REQ *x)
{
    return x->distinguishing_id;
}

/*
 * This should only be used if the X509_REQ object was embedded inside another
 * asn1 object and it needs a libctx to operate.
 * Use X509_REQ_new_ex() instead if possible.
 */
int ossl_x509_req_set0_libctx(X509_REQ *x, OSSL_LIB_CTX *libctx,
                              const char *propq)
{
    if (x != NULL) {
        x->libctx = libctx;
        OPENSSL_free(x->propq);
        x->propq = NULL;
        if (propq != NULL) {
            x->propq = OPENSSL_strdup(propq);
            if (x->propq == NULL)
                return 0;
        }
    }
    return 1;
}

X509_REQ *X509_REQ_new_ex(OSSL_LIB_CTX *libctx, const char *propq)
{
    X509_REQ *req = NULL;

    req = (X509_REQ *)ASN1_item_new((X509_REQ_it()));
    if (!ossl_x509_req_set0_libctx(req, libctx, propq)) {
        X509_REQ_free(req);
        req = NULL;
    }
    return req;
}
