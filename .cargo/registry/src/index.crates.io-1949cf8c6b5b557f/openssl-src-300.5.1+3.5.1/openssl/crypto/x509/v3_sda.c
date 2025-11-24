/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/asn1t.h>
#include <openssl/x509v3.h>
#include <crypto/x509.h>
#include "ext_dat.h"

ASN1_ITEM_TEMPLATE(OSSL_ATTRIBUTES_SYNTAX) =
        ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, Attributes, X509_ATTRIBUTE)
ASN1_ITEM_TEMPLATE_END(OSSL_ATTRIBUTES_SYNTAX)

IMPLEMENT_ASN1_FUNCTIONS(OSSL_ATTRIBUTES_SYNTAX)

static int i2r_ATTRIBUTES_SYNTAX(X509V3_EXT_METHOD *method,
                                 OSSL_ATTRIBUTES_SYNTAX *attrlst,
                                 BIO *out, int indent)
{
    X509_ATTRIBUTE *attr;
    ASN1_TYPE *av;
    int i, j, attr_nid;

    if (!attrlst) {
        if (BIO_printf(out, "<No Attributes>\n") <= 0)
            return 0;
        return 1;
    }
    if (!sk_X509_ATTRIBUTE_num(attrlst)) {
        if (BIO_printf(out, "<Empty Attributes>\n") <= 0)
            return 0;
        return 1;
    }

    for (i = 0; i < sk_X509_ATTRIBUTE_num(attrlst); i++) {
        ASN1_OBJECT *attr_obj;
        attr = sk_X509_ATTRIBUTE_value(attrlst, i);
        attr_obj = X509_ATTRIBUTE_get0_object(attr);
        attr_nid = OBJ_obj2nid(attr_obj);
        if (indent && BIO_printf(out, "%*s", indent, "") <= 0)
            return 0;
        if (attr_nid == NID_undef) {
            if (i2a_ASN1_OBJECT(out, attr_obj) <= 0)
                return 0;
            if (BIO_puts(out, ":\n") <= 0)
                return 0;
        } else if (BIO_printf(out, "%s:\n", OBJ_nid2ln(attr_nid)) <= 0) {
            return 0;
        }

        if (X509_ATTRIBUTE_count(attr)) {
            for (j = 0; j < X509_ATTRIBUTE_count(attr); j++)
            {
                av = X509_ATTRIBUTE_get0_type(attr, j);
                if (ossl_print_attribute_value(out, attr_nid, av, indent + 4) <= 0)
                    return 0;
                if (BIO_puts(out, "\n") <= 0)
                    return 0;
            }
        } else if (BIO_printf(out, "%*s<No Values>\n", indent + 4, "") <= 0) {
            return 0;
        }
    }
    return 1;
}

const X509V3_EXT_METHOD ossl_v3_subj_dir_attrs = {
    NID_subject_directory_attributes, X509V3_EXT_MULTILINE,
    ASN1_ITEM_ref(OSSL_ATTRIBUTES_SYNTAX),
    0, 0, 0, 0,
    0, 0, 0, 0,
    (X509V3_EXT_I2R)i2r_ATTRIBUTES_SYNTAX,
    0,
    NULL
};

const X509V3_EXT_METHOD ossl_v3_associated_info = {
    NID_associated_information, X509V3_EXT_MULTILINE,
    ASN1_ITEM_ref(OSSL_ATTRIBUTES_SYNTAX),
    0, 0, 0, 0,
    0, 0, 0, 0,
    (X509V3_EXT_I2R)i2r_ATTRIBUTES_SYNTAX,
    0,
    NULL
};
