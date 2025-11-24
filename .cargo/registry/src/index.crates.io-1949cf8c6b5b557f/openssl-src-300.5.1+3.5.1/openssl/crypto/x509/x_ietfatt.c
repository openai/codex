/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/err.h>
#include <openssl/asn1t.h>
#include <openssl/x509_acert.h>

/*-
 * Definition of IetfAttrSyntax from RFC 5755 4.4
 *
 * IetfAttrSyntax ::= SEQUENCE {
 *   policyAuthority [0] GeneralNames    OPTIONAL,
 *   values          SEQUENCE OF CHOICE {
 *                     octets    OCTET STRING,
 *                     oid       OBJECT IDENTIFIER,
 *                     string    UTF8String
 *                   }
 * }
 *
 * Section 4.4.2 states that all values in the sequence MUST use the
 * same choice of value (octet, oid or string).
 */

struct OSSL_IETF_ATTR_SYNTAX_VALUE_st {
    int type;
    union {
        ASN1_OCTET_STRING *octets;
        ASN1_OBJECT *oid;
        ASN1_UTF8STRING *string;
    } u;
};

struct OSSL_IETF_ATTR_SYNTAX_st {
    GENERAL_NAMES *policyAuthority;
    int type;
    STACK_OF(OSSL_IETF_ATTR_SYNTAX_VALUE) *values;
};

ASN1_CHOICE(OSSL_IETF_ATTR_SYNTAX_VALUE) = {
    ASN1_SIMPLE(OSSL_IETF_ATTR_SYNTAX_VALUE, u.octets, ASN1_OCTET_STRING),
    ASN1_SIMPLE(OSSL_IETF_ATTR_SYNTAX_VALUE, u.oid, ASN1_OBJECT),
    ASN1_SIMPLE(OSSL_IETF_ATTR_SYNTAX_VALUE, u.string, ASN1_UTF8STRING),
} ASN1_CHOICE_END(OSSL_IETF_ATTR_SYNTAX_VALUE)

ASN1_SEQUENCE(OSSL_IETF_ATTR_SYNTAX) = {
    ASN1_IMP_SEQUENCE_OF_OPT(OSSL_IETF_ATTR_SYNTAX, policyAuthority, GENERAL_NAME, 0),
    ASN1_SEQUENCE_OF(OSSL_IETF_ATTR_SYNTAX, values, OSSL_IETF_ATTR_SYNTAX_VALUE),
} ASN1_SEQUENCE_END(OSSL_IETF_ATTR_SYNTAX)

IMPLEMENT_ASN1_ALLOC_FUNCTIONS(OSSL_IETF_ATTR_SYNTAX)
IMPLEMENT_ASN1_ALLOC_FUNCTIONS(OSSL_IETF_ATTR_SYNTAX_VALUE)

OSSL_IETF_ATTR_SYNTAX *d2i_OSSL_IETF_ATTR_SYNTAX (OSSL_IETF_ATTR_SYNTAX **a,
                                                  const unsigned char **in,
                                                  long len)
{
    OSSL_IETF_ATTR_SYNTAX *ias;
    int i;

    ias = (OSSL_IETF_ATTR_SYNTAX *) ASN1_item_d2i((ASN1_VALUE **)a, in, len,
                                                  OSSL_IETF_ATTR_SYNTAX_it());
    if (ias == NULL)
        return ias;

    for (i = 0; i < sk_OSSL_IETF_ATTR_SYNTAX_VALUE_num(ias->values); i++)
    {
        OSSL_IETF_ATTR_SYNTAX_VALUE *val;

        val = sk_OSSL_IETF_ATTR_SYNTAX_VALUE_value(ias->values, i);
        if (i == 0)
            ias->type = val->type;
        else if (val->type != ias->type)
            goto invalid_types;
    }

    return ias;

invalid_types:
    OSSL_IETF_ATTR_SYNTAX_free(ias);
    if (a)
        *a = NULL;
    ERR_raise(ERR_LIB_X509V3, ERR_R_PASSED_INVALID_ARGUMENT);
    return NULL;
}

int i2d_OSSL_IETF_ATTR_SYNTAX (const OSSL_IETF_ATTR_SYNTAX *a,
                               unsigned char **out)
{
    return ASN1_item_i2d((const ASN1_VALUE *)a, out, OSSL_IETF_ATTR_SYNTAX_it());
}

int OSSL_IETF_ATTR_SYNTAX_get_value_num(const OSSL_IETF_ATTR_SYNTAX *a)
{
    if (a->values == NULL)
        return 0;

    return sk_OSSL_IETF_ATTR_SYNTAX_VALUE_num(a->values);
}

const GENERAL_NAMES *
OSSL_IETF_ATTR_SYNTAX_get0_policyAuthority(const OSSL_IETF_ATTR_SYNTAX *a)
{
    return a->policyAuthority;
}

void OSSL_IETF_ATTR_SYNTAX_set0_policyAuthority(OSSL_IETF_ATTR_SYNTAX *a,
                                                GENERAL_NAMES *names)
{
    GENERAL_NAMES_free(a->policyAuthority);
    a->policyAuthority = names;
}

void *OSSL_IETF_ATTR_SYNTAX_get0_value(const OSSL_IETF_ATTR_SYNTAX *a,
                                       int ind, int *type)
{
    OSSL_IETF_ATTR_SYNTAX_VALUE *val;

    val = sk_OSSL_IETF_ATTR_SYNTAX_VALUE_value(a->values, ind);
    if (val == NULL)
        return NULL;

    if (type != NULL)
        *type = val->type;

    switch (val->type) {
    case OSSL_IETFAS_OCTETS:
        return val->u.octets;
    case OSSL_IETFAS_OID:
        return val->u.oid;
    case OSSL_IETFAS_STRING:
        return val->u.string;
    }

    return NULL;
}

int OSSL_IETF_ATTR_SYNTAX_add1_value(OSSL_IETF_ATTR_SYNTAX *a, int type,
                                     void *data)
{
    OSSL_IETF_ATTR_SYNTAX_VALUE *val;

    if (data == NULL)
        return 0;

    if (a->values == NULL) {
        if ((a->values = sk_OSSL_IETF_ATTR_SYNTAX_VALUE_new_null()) == NULL)
            goto err;
        a->type = type;
    }

    if (type != a->type) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    if ((val = OSSL_IETF_ATTR_SYNTAX_VALUE_new()) == NULL)
        goto err;

    val->type = type;
    switch (type) {
    case OSSL_IETFAS_OCTETS:
        val->u.octets = data;
        break;
    case OSSL_IETFAS_OID:
        val->u.oid = data;
        break;
    case OSSL_IETFAS_STRING:
        val->u.string = data;
        break;
    default:
        OSSL_IETF_ATTR_SYNTAX_VALUE_free(val);
        ERR_raise(ERR_LIB_X509V3, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    if (sk_OSSL_IETF_ATTR_SYNTAX_VALUE_push(a->values, val) <= 0) {
        OSSL_IETF_ATTR_SYNTAX_VALUE_free(val);
        return 0;
    }

    return 1;

err:
    ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
    return 0;
}

int OSSL_IETF_ATTR_SYNTAX_print(BIO *bp, OSSL_IETF_ATTR_SYNTAX *a, int indent)
{
    int i;

    if (a->policyAuthority != NULL) {
        for (i = 0; i < sk_GENERAL_NAME_num(a->policyAuthority); i++) {
            if (BIO_printf(bp, "%*s", indent, "") <= 0)
                goto err;

            if (GENERAL_NAME_print(bp, sk_GENERAL_NAME_value(a->policyAuthority,
                                                             i)) <= 0)
                goto err;

            if (BIO_printf(bp, "\n") <= 0)
                goto err;
        }
    }

    for (i = 0; i < OSSL_IETF_ATTR_SYNTAX_get_value_num(a); i++) {
        char oidstr[80];
        int ietf_type;
        void *attr_value = OSSL_IETF_ATTR_SYNTAX_get0_value(a, i, &ietf_type);

        if (attr_value == NULL)
            goto err;

        if (BIO_printf(bp, "%*s", indent, "") <= 0)
            goto err;

        switch (ietf_type) {
        case OSSL_IETFAS_OID:
            OBJ_obj2txt(oidstr, sizeof(oidstr), attr_value, 0);
            BIO_printf(bp, "%.*s", (int) sizeof(oidstr), oidstr);
            break;
        case OSSL_IETFAS_OCTETS:
        case OSSL_IETFAS_STRING:
            ASN1_STRING_print(bp, attr_value);
            break;
        }
    }
    if (BIO_printf(bp, "\n") <= 0)
        goto err;

    return 1;

err:
    return 0;
}
