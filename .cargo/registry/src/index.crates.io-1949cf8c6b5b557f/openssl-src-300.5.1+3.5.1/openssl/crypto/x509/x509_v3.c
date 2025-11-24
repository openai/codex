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
#include "x509_local.h"

int X509v3_get_ext_count(const STACK_OF(X509_EXTENSION) *x)
{
    int ret;

    if (x == NULL)
        return 0;
    ret = sk_X509_EXTENSION_num(x);
    return ret > 0 ? ret : 0;
}

int X509v3_get_ext_by_NID(const STACK_OF(X509_EXTENSION) *x, int nid,
                          int lastpos)
{
    ASN1_OBJECT *obj;

    obj = OBJ_nid2obj(nid);
    if (obj == NULL)
        return -2;
    return X509v3_get_ext_by_OBJ(x, obj, lastpos);
}

int X509v3_get_ext_by_OBJ(const STACK_OF(X509_EXTENSION) *sk,
                          const ASN1_OBJECT *obj, int lastpos)
{
    int n;
    X509_EXTENSION *ex;

    if (sk == NULL)
        return -1;
    lastpos++;
    if (lastpos < 0)
        lastpos = 0;
    n = sk_X509_EXTENSION_num(sk);
    for (; lastpos < n; lastpos++) {
        ex = sk_X509_EXTENSION_value(sk, lastpos);
        if (OBJ_cmp(ex->object, obj) == 0)
            return lastpos;
    }
    return -1;
}

int X509v3_get_ext_by_critical(const STACK_OF(X509_EXTENSION) *sk, int crit,
                               int lastpos)
{
    int n, c;
    X509_EXTENSION *ex;

    if (sk == NULL)
        return -1;
    lastpos++;
    if (lastpos < 0)
        lastpos = 0;
    n = sk_X509_EXTENSION_num(sk);
    for (; lastpos < n; lastpos++) {
        ex = sk_X509_EXTENSION_value(sk, lastpos);
        c = X509_EXTENSION_get_critical(ex);
        crit = crit != 0;
        if (c == crit)
            return lastpos;
    }
    return -1;
}

X509_EXTENSION *X509v3_get_ext(const STACK_OF(X509_EXTENSION) *x, int loc)
{
    if (x == NULL || sk_X509_EXTENSION_num(x) <= loc || loc < 0)
        return NULL;
    else
        return sk_X509_EXTENSION_value(x, loc);
}

X509_EXTENSION *X509v3_delete_ext(STACK_OF(X509_EXTENSION) *x, int loc)
{
    X509_EXTENSION *ret;

    if (x == NULL || sk_X509_EXTENSION_num(x) <= loc || loc < 0)
        return NULL;
    ret = sk_X509_EXTENSION_delete(x, loc);
    return ret;
}

STACK_OF(X509_EXTENSION) *X509v3_add_ext(STACK_OF(X509_EXTENSION) **x,
                                         X509_EXTENSION *ex, int loc)
{
    X509_EXTENSION *new_ex = NULL;
    int n;
    STACK_OF(X509_EXTENSION) *sk = NULL;

    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        goto err;
    }

    if (*x == NULL) {
        if ((sk = sk_X509_EXTENSION_new_null()) == NULL) {
            ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
            goto err;
        }
    } else
        sk = *x;

    n = sk_X509_EXTENSION_num(sk);
    if (loc > n)
        loc = n;
    else if (loc < 0)
        loc = n;

    if ((new_ex = X509_EXTENSION_dup(ex)) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        goto err;
    }
    if (!sk_X509_EXTENSION_insert(sk, new_ex, loc)) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto err;
    }
    if (*x == NULL)
        *x = sk;
    return sk;
 err:
    X509_EXTENSION_free(new_ex);
    if (x != NULL && *x == NULL)
        sk_X509_EXTENSION_free(sk);
    return NULL;
}

/* This returns NULL also in non-error case *target == NULL && sk_X509_EXTENSION_num(exts) <= 0 */
STACK_OF(X509_EXTENSION) *X509v3_add_extensions(STACK_OF(X509_EXTENSION) **target,
                                                const STACK_OF(X509_EXTENSION) *exts)
{
    int i;

    if (target == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    for (i = 0; i < sk_X509_EXTENSION_num(exts); i++) {
        X509_EXTENSION *ext = sk_X509_EXTENSION_value(exts, i);
        ASN1_OBJECT *obj = X509_EXTENSION_get_object(ext);
        int idx = X509v3_get_ext_by_OBJ(*target, obj, -1);

        /* Does extension exist in target? */
        if (idx != -1) {
            /* Delete all extensions of same type */
            do {
                X509_EXTENSION_free(sk_X509_EXTENSION_delete(*target, idx));
                idx = X509v3_get_ext_by_OBJ(*target, obj, -1);
            } while (idx != -1);
        }
        if (!X509v3_add_ext(target, ext, -1))
            return NULL;
    }
    return *target;
}

X509_EXTENSION *X509_EXTENSION_create_by_NID(X509_EXTENSION **ex, int nid,
                                             int crit,
                                             ASN1_OCTET_STRING *data)
{
    ASN1_OBJECT *obj;
    X509_EXTENSION *ret;

    obj = OBJ_nid2obj(nid);
    if (obj == NULL) {
        ERR_raise(ERR_LIB_X509, X509_R_UNKNOWN_NID);
        return NULL;
    }
    ret = X509_EXTENSION_create_by_OBJ(ex, obj, crit, data);
    if (ret == NULL)
        ASN1_OBJECT_free(obj);
    return ret;
}

X509_EXTENSION *X509_EXTENSION_create_by_OBJ(X509_EXTENSION **ex,
                                             const ASN1_OBJECT *obj, int crit,
                                             ASN1_OCTET_STRING *data)
{
    X509_EXTENSION *ret;

    if ((ex == NULL) || (*ex == NULL)) {
        if ((ret = X509_EXTENSION_new()) == NULL) {
            ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
            return NULL;
        }
    } else
        ret = *ex;

    if (!X509_EXTENSION_set_object(ret, obj))
        goto err;
    if (!X509_EXTENSION_set_critical(ret, crit))
        goto err;
    if (!X509_EXTENSION_set_data(ret, data))
        goto err;

    if ((ex != NULL) && (*ex == NULL))
        *ex = ret;
    return ret;
 err:
    if ((ex == NULL) || (ret != *ex))
        X509_EXTENSION_free(ret);
    return NULL;
}

int X509_EXTENSION_set_object(X509_EXTENSION *ex, const ASN1_OBJECT *obj)
{
    if ((ex == NULL) || (obj == NULL))
        return 0;
    ASN1_OBJECT_free(ex->object);
    ex->object = OBJ_dup(obj);
    return ex->object != NULL;
}

int X509_EXTENSION_set_critical(X509_EXTENSION *ex, int crit)
{
    if (ex == NULL)
        return 0;
    ex->critical = (crit) ? 0xFF : 0;
    return 1;
}

int X509_EXTENSION_set_data(X509_EXTENSION *ex, ASN1_OCTET_STRING *data)
{
    int i;

    if (ex == NULL)
        return 0;
    i = ASN1_OCTET_STRING_set(&ex->value, data->data, data->length);
    if (!i)
        return 0;
    return 1;
}

ASN1_OBJECT *X509_EXTENSION_get_object(X509_EXTENSION *ex)
{
    if (ex == NULL)
        return NULL;
    return ex->object;
}

ASN1_OCTET_STRING *X509_EXTENSION_get_data(X509_EXTENSION *ex)
{
    if (ex == NULL)
        return NULL;
    return &ex->value;
}

int X509_EXTENSION_get_critical(const X509_EXTENSION *ex)
{
    if (ex == NULL)
        return 0;
    if (ex->critical > 0)
        return 1;
    return 0;
}
