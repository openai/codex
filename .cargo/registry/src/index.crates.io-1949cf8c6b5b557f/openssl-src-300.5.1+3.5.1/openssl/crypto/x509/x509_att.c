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
#include <openssl/safestack.h>
#include <openssl/asn1.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include "crypto/x509.h"
#include "x509_local.h"

int X509at_get_attr_count(const STACK_OF(X509_ATTRIBUTE) *x)
{
    return sk_X509_ATTRIBUTE_num(x);
}

int X509at_get_attr_by_NID(const STACK_OF(X509_ATTRIBUTE) *x, int nid,
                           int lastpos)
{
    const ASN1_OBJECT *obj = OBJ_nid2obj(nid);

    if (obj == NULL)
        return -2;
    return X509at_get_attr_by_OBJ(x, obj, lastpos);
}

int X509at_get_attr_by_OBJ(const STACK_OF(X509_ATTRIBUTE) *sk,
                           const ASN1_OBJECT *obj, int lastpos)
{
    int n;
    X509_ATTRIBUTE *ex;

    if (sk == NULL)
        return -1;
    lastpos++;
    if (lastpos < 0)
        lastpos = 0;
    n = sk_X509_ATTRIBUTE_num(sk);
    for (; lastpos < n; lastpos++) {
        ex = sk_X509_ATTRIBUTE_value(sk, lastpos);
        if (OBJ_cmp(ex->object, obj) == 0)
            return lastpos;
    }
    return -1;
}

X509_ATTRIBUTE *X509at_get_attr(const STACK_OF(X509_ATTRIBUTE) *x, int loc)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (sk_X509_ATTRIBUTE_num(x) <= loc || loc < 0) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }
    return sk_X509_ATTRIBUTE_value(x, loc);
}

X509_ATTRIBUTE *X509at_delete_attr(STACK_OF(X509_ATTRIBUTE) *x, int loc)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (sk_X509_ATTRIBUTE_num(x) <= loc || loc < 0) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }
    return sk_X509_ATTRIBUTE_delete(x, loc);
}

STACK_OF(X509_ATTRIBUTE) *ossl_x509at_add1_attr(STACK_OF(X509_ATTRIBUTE) **x,
                                                const X509_ATTRIBUTE *attr)
{
    X509_ATTRIBUTE *new_attr = NULL;
    STACK_OF(X509_ATTRIBUTE) *sk = NULL;

    if (x == NULL || attr == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if (*x == NULL) {
        if ((sk = sk_X509_ATTRIBUTE_new_null()) == NULL) {
            ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
            goto err;
        }
    } else {
        sk = *x;
    }

    if ((new_attr = X509_ATTRIBUTE_dup(attr)) == NULL)
        goto err;
    if (!sk_X509_ATTRIBUTE_push(sk, new_attr)) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto err;
    }
    if (*x == NULL)
        *x = sk;
    return sk;
 err:
    X509_ATTRIBUTE_free(new_attr);
    if (*x == NULL)
        sk_X509_ATTRIBUTE_free(sk);
    return NULL;
}

STACK_OF(X509_ATTRIBUTE) *X509at_add1_attr(STACK_OF(X509_ATTRIBUTE) **x,
                                           X509_ATTRIBUTE *attr)
{
    if (x == NULL || attr == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (*x != NULL && X509at_get_attr_by_OBJ(*x, attr->object, -1) != -1) {
        ERR_raise_data(ERR_LIB_X509, X509_R_DUPLICATE_ATTRIBUTE,
                       "name=%s", OBJ_nid2sn(OBJ_obj2nid(attr->object)));
        return NULL;
    }

    return ossl_x509at_add1_attr(x, attr);
}

STACK_OF(X509_ATTRIBUTE) *ossl_x509at_add1_attr_by_OBJ(STACK_OF(X509_ATTRIBUTE) **x,
                                                       const ASN1_OBJECT *obj,
                                                       int type,
                                                       const unsigned char *bytes,
                                                       int len)
{
    X509_ATTRIBUTE *attr;
    STACK_OF(X509_ATTRIBUTE) *ret;

    attr = X509_ATTRIBUTE_create_by_OBJ(NULL, obj, type, bytes, len);
    if (attr == NULL)
        return 0;
    ret = ossl_x509at_add1_attr(x, attr);
    X509_ATTRIBUTE_free(attr);
    return ret;
}

STACK_OF(X509_ATTRIBUTE) *X509at_add1_attr_by_OBJ(STACK_OF(X509_ATTRIBUTE)
                                                  **x, const ASN1_OBJECT *obj,
                                                  int type,
                                                  const unsigned char *bytes,
                                                  int len)
{
    if (x == NULL || obj == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (*x != NULL && X509at_get_attr_by_OBJ(*x, obj, -1) != -1) {
        ERR_raise_data(ERR_LIB_X509, X509_R_DUPLICATE_ATTRIBUTE,
                       "name=%s", OBJ_nid2sn(OBJ_obj2nid(obj)));
        return NULL;
    }

    return ossl_x509at_add1_attr_by_OBJ(x, obj, type, bytes, len);
}

STACK_OF(X509_ATTRIBUTE) *ossl_x509at_add1_attr_by_NID(STACK_OF(X509_ATTRIBUTE) **x,
                                                       int nid, int type,
                                                       const unsigned char *bytes,
                                                       int len)
{
    X509_ATTRIBUTE *attr;
    STACK_OF(X509_ATTRIBUTE) *ret;

    attr = X509_ATTRIBUTE_create_by_NID(NULL, nid, type, bytes, len);
    if (attr == NULL)
        return 0;
    ret = ossl_x509at_add1_attr(x, attr);
    X509_ATTRIBUTE_free(attr);
    return ret;
}

STACK_OF(X509_ATTRIBUTE) *X509at_add1_attr_by_NID(STACK_OF(X509_ATTRIBUTE)
                                                  **x, int nid, int type,
                                                  const unsigned char *bytes,
                                                  int len)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (*x != NULL && X509at_get_attr_by_NID(*x, nid, -1) != -1) {
        ERR_raise_data(ERR_LIB_X509, X509_R_DUPLICATE_ATTRIBUTE,
                       "name=%s", OBJ_nid2sn(nid));
        return NULL;
    }

    return ossl_x509at_add1_attr_by_NID(x, nid, type, bytes, len);
}

STACK_OF(X509_ATTRIBUTE) *ossl_x509at_add1_attr_by_txt(STACK_OF(X509_ATTRIBUTE) **x,
                                                       const char *attrname,
                                                       int type,
                                                       const unsigned char *bytes,
                                                       int len)
{
    X509_ATTRIBUTE *attr;
    STACK_OF(X509_ATTRIBUTE) *ret;

    attr = X509_ATTRIBUTE_create_by_txt(NULL, attrname, type, bytes, len);
    if (attr == NULL)
        return 0;
    ret = ossl_x509at_add1_attr(x, attr);
    X509_ATTRIBUTE_free(attr);
    return ret;
}

STACK_OF(X509_ATTRIBUTE) *X509at_add1_attr_by_txt(STACK_OF(X509_ATTRIBUTE)
                                                  **x, const char *attrname,
                                                  int type,
                                                  const unsigned char *bytes,
                                                  int len)
{
    X509_ATTRIBUTE *attr;
    STACK_OF(X509_ATTRIBUTE) *ret;

    attr = X509_ATTRIBUTE_create_by_txt(NULL, attrname, type, bytes, len);
    if (attr == NULL)
        return 0;
    ret = X509at_add1_attr(x, attr);
    X509_ATTRIBUTE_free(attr);
    return ret;
}

void *X509at_get0_data_by_OBJ(const STACK_OF(X509_ATTRIBUTE) *x,
                              const ASN1_OBJECT *obj, int lastpos, int type)
{
    int i = X509at_get_attr_by_OBJ(x, obj, lastpos);
    X509_ATTRIBUTE *at;

    if (i == -1)
        return NULL;
    if (lastpos <= -2 && X509at_get_attr_by_OBJ(x, obj, i) != -1)
        return NULL;
    at = X509at_get_attr(x, i);
    if (lastpos <= -3 && X509_ATTRIBUTE_count(at) != 1)
        return NULL;
    return X509_ATTRIBUTE_get0_data(at, 0, type, NULL);
}

STACK_OF(X509_ATTRIBUTE) *ossl_x509at_dup(const STACK_OF(X509_ATTRIBUTE) *x)
{
    int i, n = sk_X509_ATTRIBUTE_num(x);
    STACK_OF(X509_ATTRIBUTE) *sk = NULL;

    for (i = 0; i < n; ++i) {
        if (X509at_add1_attr(&sk, sk_X509_ATTRIBUTE_value(x, i)) == NULL) {
            sk_X509_ATTRIBUTE_pop_free(sk, X509_ATTRIBUTE_free);
            return NULL;
        }
    }
    return sk;
}

X509_ATTRIBUTE *X509_ATTRIBUTE_create_by_NID(X509_ATTRIBUTE **attr, int nid,
                                             int atrtype, const void *data,
                                             int len)
{
    ASN1_OBJECT *obj = OBJ_nid2obj(nid);
    X509_ATTRIBUTE *ret;

    if (obj == NULL) {
        ERR_raise(ERR_LIB_X509, X509_R_UNKNOWN_NID);
        return NULL;
    }
    ret = X509_ATTRIBUTE_create_by_OBJ(attr, obj, atrtype, data, len);
    if (ret == NULL)
        ASN1_OBJECT_free(obj);
    return ret;
}

X509_ATTRIBUTE *X509_ATTRIBUTE_create_by_OBJ(X509_ATTRIBUTE **attr,
                                             const ASN1_OBJECT *obj,
                                             int atrtype, const void *data,
                                             int len)
{
    X509_ATTRIBUTE *ret;

    if (attr == NULL || *attr == NULL) {
        if ((ret = X509_ATTRIBUTE_new()) == NULL) {
            ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
            return NULL;
        }
    } else {
        ret = *attr;
    }

    if (!X509_ATTRIBUTE_set1_object(ret, obj))
        goto err;
    if (!X509_ATTRIBUTE_set1_data(ret, atrtype, data, len))
        goto err;

    if (attr != NULL && *attr == NULL)
        *attr = ret;
    return ret;
 err:
    if (attr == NULL || ret != *attr)
        X509_ATTRIBUTE_free(ret);
    return NULL;
}

X509_ATTRIBUTE *X509_ATTRIBUTE_create_by_txt(X509_ATTRIBUTE **attr,
                                             const char *atrname, int type,
                                             const unsigned char *bytes,
                                             int len)
{
    ASN1_OBJECT *obj = OBJ_txt2obj(atrname, 0);
    X509_ATTRIBUTE *nattr;

    if (obj == NULL) {
        ERR_raise_data(ERR_LIB_X509, X509_R_INVALID_FIELD_NAME,
                       "name=%s", atrname);
        return NULL;
    }
    nattr = X509_ATTRIBUTE_create_by_OBJ(attr, obj, type, bytes, len);
    ASN1_OBJECT_free(obj);
    return nattr;
}

int X509_ATTRIBUTE_set1_object(X509_ATTRIBUTE *attr, const ASN1_OBJECT *obj)
{
    if (attr == NULL || obj == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ASN1_OBJECT_free(attr->object);
    attr->object = OBJ_dup(obj);
    return attr->object != NULL;
}

int X509_ATTRIBUTE_set1_data(X509_ATTRIBUTE *attr, int attrtype,
                             const void *data, int len)
{
    ASN1_TYPE *ttmp = NULL;
    ASN1_STRING *stmp = NULL;
    int atype = 0;

    if (attr == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if ((attrtype & MBSTRING_FLAG) != 0) {
        stmp = ASN1_STRING_set_by_NID(NULL, data, len, attrtype,
                                      OBJ_obj2nid(attr->object));
        if (stmp == NULL) {
            ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
            return 0;
        }
        atype = stmp->type;
    } else if (len != -1) {
        if ((stmp = ASN1_STRING_type_new(attrtype)) == NULL
            || !ASN1_STRING_set(stmp, data, len)) {
            ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
            goto err;
        }
        atype = attrtype;
    }
    /*
     * This is a bit naughty because the attribute should really have at
     * least one value but some types use and zero length SET and require
     * this.
     */
    if (attrtype == 0) {
        ASN1_STRING_free(stmp);
        return 1;
    }
    if ((ttmp = ASN1_TYPE_new()) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        goto err;
    }
    if (len == -1 && (attrtype & MBSTRING_FLAG) == 0) {
        if (!ASN1_TYPE_set1(ttmp, attrtype, data)) {
            ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
            goto err;
        }
    } else {
        ASN1_TYPE_set(ttmp, atype, stmp);
        stmp = NULL;
    }
    if (!sk_ASN1_TYPE_push(attr->set, ttmp)) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto err;
    }
    return 1;
 err:
    ASN1_TYPE_free(ttmp);
    ASN1_STRING_free(stmp);
    return 0;
}

int X509_ATTRIBUTE_count(const X509_ATTRIBUTE *attr)
{
    if (attr == NULL)
        return 0;
    return sk_ASN1_TYPE_num(attr->set);
}

ASN1_OBJECT *X509_ATTRIBUTE_get0_object(X509_ATTRIBUTE *attr)
{
    if (attr == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    return attr->object;
}

void *X509_ATTRIBUTE_get0_data(X509_ATTRIBUTE *attr, int idx,
                               int atrtype, void *data)
{
    ASN1_TYPE *ttmp = X509_ATTRIBUTE_get0_type(attr, idx);

    if (ttmp == NULL)
        return NULL;
    if (atrtype == V_ASN1_BOOLEAN
            || atrtype == V_ASN1_NULL
            || atrtype != ASN1_TYPE_get(ttmp)) {
        ERR_raise(ERR_LIB_X509, X509_R_WRONG_TYPE);
        return NULL;
    }
    return ttmp->value.ptr;
}

ASN1_TYPE *X509_ATTRIBUTE_get0_type(X509_ATTRIBUTE *attr, int idx)
{
    if (attr == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    return sk_ASN1_TYPE_value(attr->set, idx);
}
