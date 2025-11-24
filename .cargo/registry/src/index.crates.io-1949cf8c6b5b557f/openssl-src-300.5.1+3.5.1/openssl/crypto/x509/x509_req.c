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
#include <openssl/bn.h>
#include <openssl/evp.h>
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/x509.h>
#include "crypto/x509.h"
#include <openssl/objects.h>
#include <openssl/buffer.h>
#include <openssl/pem.h>

X509_REQ *X509_to_X509_REQ(X509 *x, EVP_PKEY *pkey, const EVP_MD *md)
{
    X509_REQ *ret;
    X509_REQ_INFO *ri;
    int i;
    EVP_PKEY *pktmp;

    ret = X509_REQ_new_ex(x->libctx, x->propq);
    if (ret == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        goto err;
    }

    ri = &ret->req_info;

    ri->version->length = 1;
    ri->version->data = OPENSSL_malloc(1);
    if (ri->version->data == NULL)
        goto err;
    ri->version->data[0] = 0;   /* version == 0 */

    if (!X509_REQ_set_subject_name(ret, X509_get_subject_name(x)))
        goto err;

    pktmp = X509_get0_pubkey(x);
    if (pktmp == NULL)
        goto err;
    i = X509_REQ_set_pubkey(ret, pktmp);
    if (!i)
        goto err;

    if (pkey != NULL) {
        if (!X509_REQ_sign(ret, pkey, md))
            goto err;
    }
    return ret;
 err:
    X509_REQ_free(ret);
    return NULL;
}

EVP_PKEY *X509_REQ_get_pubkey(X509_REQ *req)
{
    if (req == NULL)
        return NULL;
    return X509_PUBKEY_get(req->req_info.pubkey);
}

EVP_PKEY *X509_REQ_get0_pubkey(const X509_REQ *req)
{
    if (req == NULL)
        return NULL;
    return X509_PUBKEY_get0(req->req_info.pubkey);
}

X509_PUBKEY *X509_REQ_get_X509_PUBKEY(X509_REQ *req)
{
    return req->req_info.pubkey;
}

int X509_REQ_check_private_key(const X509_REQ *req, EVP_PKEY *pkey)
{
    return ossl_x509_check_private_key(X509_REQ_get0_pubkey(req), pkey);
}

/*
 * It seems several organisations had the same idea of including a list of
 * extensions in a certificate request. There are at least two OIDs that are
 * used and there may be more: so the list is configurable.
 */

static int ext_nid_list[] = { NID_ext_req, NID_ms_ext_req, NID_undef };

static int *ext_nids = ext_nid_list;

int X509_REQ_extension_nid(int req_nid)
{
    int i, nid;

    for (i = 0;; i++) {
        nid = ext_nids[i];
        if (nid == NID_undef)
            return 0;
        else if (req_nid == nid)
            return 1;
    }
}

int *X509_REQ_get_extension_nids(void)
{
    return ext_nids;
}

void X509_REQ_set_extension_nids(int *nids)
{
    ext_nids = nids;
}

static STACK_OF(X509_EXTENSION) *get_extensions_by_nid(const X509_REQ *req,
                                                       int nid)
{
    X509_ATTRIBUTE *attr;
    ASN1_TYPE *ext = NULL;
    const unsigned char *p;
    int idx = X509_REQ_get_attr_by_NID(req, nid, -1);

    if (idx < 0) /* no extensions is not an error */
        return sk_X509_EXTENSION_new_null();
    attr = X509_REQ_get_attr(req, idx);
    ext = X509_ATTRIBUTE_get0_type(attr, 0);
    if (ext == NULL || ext->type != V_ASN1_SEQUENCE) {
        ERR_raise(ERR_LIB_X509, X509_R_WRONG_TYPE);
        return NULL;
    }
    p = ext->value.sequence->data;
    return (STACK_OF(X509_EXTENSION) *)
        ASN1_item_d2i(NULL, &p, ext->value.sequence->length,
                      ASN1_ITEM_rptr(X509_EXTENSIONS));
}

STACK_OF(X509_EXTENSION) *X509_REQ_get_extensions(OSSL_FUTURE_CONST X509_REQ *req)
{
    STACK_OF(X509_EXTENSION) *exts = NULL;
    int *pnid;

    if (req == NULL || ext_nids == NULL)
        return NULL;
    for (pnid = ext_nids; *pnid != NID_undef; pnid++) {
        exts = get_extensions_by_nid(req, *pnid);
        if (exts == NULL)
            return NULL;
        if (sk_X509_EXTENSION_num(exts) > 0)
            return exts;
        sk_X509_EXTENSION_free(exts);
    }
    /* no extensions is not an error */
    return sk_X509_EXTENSION_new_null();
}

/*
 * Add a STACK_OF extensions to a certificate request: allow alternative OIDs
 * in case we want to create a non standard one.
 */
int X509_REQ_add_extensions_nid(X509_REQ *req,
                                const STACK_OF(X509_EXTENSION) *exts, int nid)
{
    int extlen;
    int rv = 0;
    unsigned char *ext = NULL;
    STACK_OF(X509_EXTENSION) *mod_exts = NULL;
    int loc;

    if (sk_X509_EXTENSION_num(exts) <= 0)
        return 1; /* adding NULL or empty list of exts is a no-op */

    loc = X509at_get_attr_by_NID(req->req_info.attributes, nid, -1);
    if (loc != -1) {
        if ((mod_exts = get_extensions_by_nid(req, nid)) == NULL)
            return 0;
        if (X509v3_add_extensions(&mod_exts, exts) == NULL)
            goto end;
    }

    /* Generate encoding of extensions */
    extlen = ASN1_item_i2d((const ASN1_VALUE *)
                           (mod_exts == NULL ? exts : mod_exts),
                           &ext, ASN1_ITEM_rptr(X509_EXTENSIONS));
    if (extlen <= 0)
        goto end;
    if (mod_exts != NULL) {
        X509_ATTRIBUTE *att = X509at_delete_attr(req->req_info.attributes, loc);

        if (att == NULL)
            goto end;
        X509_ATTRIBUTE_free(att);
    }

    rv = X509_REQ_add1_attr_by_NID(req, nid, V_ASN1_SEQUENCE, ext, extlen);
    OPENSSL_free(ext);

 end:
    sk_X509_EXTENSION_pop_free(mod_exts, X509_EXTENSION_free);
    return rv;
}

/* This is the normal usage: use the "official" OID */
int X509_REQ_add_extensions(X509_REQ *req, const STACK_OF(X509_EXTENSION) *exts)
{
    return X509_REQ_add_extensions_nid(req, exts, NID_ext_req);
}

/* Request attribute functions */

int X509_REQ_get_attr_count(const X509_REQ *req)
{
    return X509at_get_attr_count(req->req_info.attributes);
}

int X509_REQ_get_attr_by_NID(const X509_REQ *req, int nid, int lastpos)
{
    return X509at_get_attr_by_NID(req->req_info.attributes, nid, lastpos);
}

int X509_REQ_get_attr_by_OBJ(const X509_REQ *req, const ASN1_OBJECT *obj,
                             int lastpos)
{
    return X509at_get_attr_by_OBJ(req->req_info.attributes, obj, lastpos);
}

X509_ATTRIBUTE *X509_REQ_get_attr(const X509_REQ *req, int loc)
{
    return X509at_get_attr(req->req_info.attributes, loc);
}

X509_ATTRIBUTE *X509_REQ_delete_attr(X509_REQ *req, int loc)
{
    X509_ATTRIBUTE *attr;

    if (req == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    attr = X509at_delete_attr(req->req_info.attributes, loc);
    if (attr != NULL)
        req->req_info.enc.modified = 1;
    return attr;
}

int X509_REQ_add1_attr(X509_REQ *req, X509_ATTRIBUTE *attr)
{
    if (req == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (!X509at_add1_attr(&req->req_info.attributes, attr))
        return 0;
    req->req_info.enc.modified = 1;
    return 1;
}

int X509_REQ_add1_attr_by_OBJ(X509_REQ *req,
                              const ASN1_OBJECT *obj, int type,
                              const unsigned char *bytes, int len)
{
    if (req == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (!X509at_add1_attr_by_OBJ(&req->req_info.attributes, obj,
                                 type, bytes, len))
        return 0;
    req->req_info.enc.modified = 1;
    return 1;
}

int X509_REQ_add1_attr_by_NID(X509_REQ *req,
                              int nid, int type,
                              const unsigned char *bytes, int len)
{
    if (req == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (!X509at_add1_attr_by_NID(&req->req_info.attributes, nid,
                                 type, bytes, len))
        return 0;
    req->req_info.enc.modified = 1;
    return 1;
}

int X509_REQ_add1_attr_by_txt(X509_REQ *req,
                              const char *attrname, int type,
                              const unsigned char *bytes, int len)
{
    if (req == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (!X509at_add1_attr_by_txt(&req->req_info.attributes, attrname,
                                 type, bytes, len))
        return 0;
    req->req_info.enc.modified = 1;
    return 1;
}

long X509_REQ_get_version(const X509_REQ *req)
{
    return ASN1_INTEGER_get(req->req_info.version);
}

X509_NAME *X509_REQ_get_subject_name(const X509_REQ *req)
{
    return req->req_info.subject;
}

void X509_REQ_get0_signature(const X509_REQ *req, const ASN1_BIT_STRING **psig,
                             const X509_ALGOR **palg)
{
    if (psig != NULL)
        *psig = req->signature;
    if (palg != NULL)
        *palg = &req->sig_alg;
}

void X509_REQ_set0_signature(X509_REQ *req, ASN1_BIT_STRING *psig)
{
    if (req->signature)
        ASN1_BIT_STRING_free(req->signature);
    req->signature = psig;
}

int X509_REQ_set1_signature_algo(X509_REQ *req, X509_ALGOR *palg)
{
    return X509_ALGOR_copy(&req->sig_alg, palg);
}

int X509_REQ_get_signature_nid(const X509_REQ *req)
{
    return OBJ_obj2nid(req->sig_alg.algorithm);
}

int i2d_re_X509_REQ_tbs(X509_REQ *req, unsigned char **pp)
{
    if (req == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    req->req_info.enc.modified = 1;
    return i2d_X509_REQ_INFO(&req->req_info, pp);
}
