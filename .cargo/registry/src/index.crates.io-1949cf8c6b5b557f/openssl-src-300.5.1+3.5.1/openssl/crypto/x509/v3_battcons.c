/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/conf.h>
#include <openssl/x509v3.h>
#include "x509_local.h"
#include "ext_dat.h"

static STACK_OF(CONF_VALUE) *i2v_OSSL_BASIC_ATTR_CONSTRAINTS(
    X509V3_EXT_METHOD *method,
    OSSL_BASIC_ATTR_CONSTRAINTS *battcons,
    STACK_OF(CONF_VALUE)
    *extlist);
static OSSL_BASIC_ATTR_CONSTRAINTS *v2i_OSSL_BASIC_ATTR_CONSTRAINTS(
    X509V3_EXT_METHOD *method,
    X509V3_CTX *ctx,
    STACK_OF(CONF_VALUE) *values);

const X509V3_EXT_METHOD ossl_v3_battcons = {
    NID_basic_att_constraints, 0,
    ASN1_ITEM_ref(OSSL_BASIC_ATTR_CONSTRAINTS),
    0, 0, 0, 0,
    0, 0,
    (X509V3_EXT_I2V) i2v_OSSL_BASIC_ATTR_CONSTRAINTS,
    (X509V3_EXT_V2I)v2i_OSSL_BASIC_ATTR_CONSTRAINTS,
    NULL, NULL,
    NULL
};

ASN1_SEQUENCE(OSSL_BASIC_ATTR_CONSTRAINTS) = {
    ASN1_OPT(OSSL_BASIC_ATTR_CONSTRAINTS, authority, ASN1_FBOOLEAN),
    ASN1_OPT(OSSL_BASIC_ATTR_CONSTRAINTS, pathlen, ASN1_INTEGER)
} ASN1_SEQUENCE_END(OSSL_BASIC_ATTR_CONSTRAINTS)

IMPLEMENT_ASN1_FUNCTIONS(OSSL_BASIC_ATTR_CONSTRAINTS)

static STACK_OF(CONF_VALUE) *i2v_OSSL_BASIC_ATTR_CONSTRAINTS(
    X509V3_EXT_METHOD *method,
    OSSL_BASIC_ATTR_CONSTRAINTS *battcons,
    STACK_OF(CONF_VALUE) *extlist)
{
    X509V3_add_value_bool("authority", battcons->authority, &extlist);
    X509V3_add_value_int("pathlen", battcons->pathlen, &extlist);
    return extlist;
}

static OSSL_BASIC_ATTR_CONSTRAINTS *v2i_OSSL_BASIC_ATTR_CONSTRAINTS(
    X509V3_EXT_METHOD *method,
    X509V3_CTX *ctx,
    STACK_OF(CONF_VALUE) *values)
{
    OSSL_BASIC_ATTR_CONSTRAINTS *battcons = NULL;
    CONF_VALUE *val;
    int i;

    if ((battcons = OSSL_BASIC_ATTR_CONSTRAINTS_new()) == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        return NULL;
    }
    for (i = 0; i < sk_CONF_VALUE_num(values); i++) {
        val = sk_CONF_VALUE_value(values, i);
        if (strcmp(val->name, "authority") == 0) {
            if (!X509V3_get_value_bool(val, &battcons->authority))
                goto err;
        } else if (strcmp(val->name, "pathlen") == 0) {
            if (!X509V3_get_value_int(val, &battcons->pathlen))
                goto err;
        } else {
            ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_NAME);
            X509V3_conf_add_error_name_value(val);
            goto err;
        }
    }
    return battcons;
 err:
    OSSL_BASIC_ATTR_CONSTRAINTS_free(battcons);
    return NULL;
}
