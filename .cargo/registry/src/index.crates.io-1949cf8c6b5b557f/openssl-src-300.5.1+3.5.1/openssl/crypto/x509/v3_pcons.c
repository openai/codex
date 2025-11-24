/*
 * Copyright 2003-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/conf.h>
#include <openssl/x509v3.h>
#include "ext_dat.h"

static STACK_OF(CONF_VALUE) *i2v_POLICY_CONSTRAINTS(const X509V3_EXT_METHOD
                                                    *method, void *bcons, STACK_OF(CONF_VALUE)
                                                    *extlist);
static void *v2i_POLICY_CONSTRAINTS(const X509V3_EXT_METHOD *method,
                                    X509V3_CTX *ctx,
                                    STACK_OF(CONF_VALUE) *values);

const X509V3_EXT_METHOD ossl_v3_policy_constraints = {
    NID_policy_constraints, 0,
    ASN1_ITEM_ref(POLICY_CONSTRAINTS),
    0, 0, 0, 0,
    0, 0,
    i2v_POLICY_CONSTRAINTS,
    v2i_POLICY_CONSTRAINTS,
    NULL, NULL,
    NULL
};

ASN1_SEQUENCE(POLICY_CONSTRAINTS) = {
        ASN1_IMP_OPT(POLICY_CONSTRAINTS, requireExplicitPolicy, ASN1_INTEGER,0),
        ASN1_IMP_OPT(POLICY_CONSTRAINTS, inhibitPolicyMapping, ASN1_INTEGER,1)
} ASN1_SEQUENCE_END(POLICY_CONSTRAINTS)

IMPLEMENT_ASN1_ALLOC_FUNCTIONS(POLICY_CONSTRAINTS)

static STACK_OF(CONF_VALUE) *i2v_POLICY_CONSTRAINTS(const X509V3_EXT_METHOD
                                                    *method, void *a, STACK_OF(CONF_VALUE)
                                                    *extlist)
{
    POLICY_CONSTRAINTS *pcons = a;
    X509V3_add_value_int("Require Explicit Policy",
                         pcons->requireExplicitPolicy, &extlist);
    X509V3_add_value_int("Inhibit Policy Mapping",
                         pcons->inhibitPolicyMapping, &extlist);
    return extlist;
}

static void *v2i_POLICY_CONSTRAINTS(const X509V3_EXT_METHOD *method,
                                    X509V3_CTX *ctx,
                                    STACK_OF(CONF_VALUE) *values)
{
    POLICY_CONSTRAINTS *pcons = NULL;
    CONF_VALUE *val;
    int i;

    if ((pcons = POLICY_CONSTRAINTS_new()) == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        return NULL;
    }
    for (i = 0; i < sk_CONF_VALUE_num(values); i++) {
        val = sk_CONF_VALUE_value(values, i);
        if (strcmp(val->name, "requireExplicitPolicy") == 0) {
            if (!X509V3_get_value_int(val, &pcons->requireExplicitPolicy))
                goto err;
        } else if (strcmp(val->name, "inhibitPolicyMapping") == 0) {
            if (!X509V3_get_value_int(val, &pcons->inhibitPolicyMapping))
                goto err;
        } else {
            ERR_raise_data(ERR_LIB_X509V3, X509V3_R_INVALID_NAME,
                           "%s", val->name);
            goto err;
        }
    }
    if (pcons->inhibitPolicyMapping == NULL
            && pcons->requireExplicitPolicy == NULL) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_ILLEGAL_EMPTY_EXTENSION);
        goto err;
    }

    return pcons;
 err:
    POLICY_CONSTRAINTS_free(pcons);
    return NULL;
}
