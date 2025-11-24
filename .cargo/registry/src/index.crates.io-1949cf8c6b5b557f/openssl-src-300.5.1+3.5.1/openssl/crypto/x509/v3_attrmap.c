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

ASN1_SEQUENCE(OSSL_ATAV) = {
    ASN1_SIMPLE(OSSL_ATAV, type, ASN1_OBJECT),
    ASN1_SIMPLE(OSSL_ATAV, value, ASN1_ANY)
} ASN1_SEQUENCE_END(OSSL_ATAV)

ASN1_SEQUENCE(OSSL_ATTRIBUTE_TYPE_MAPPING) = {
    ASN1_IMP(OSSL_ATTRIBUTE_TYPE_MAPPING, local, ASN1_OBJECT, 0),
    ASN1_IMP(OSSL_ATTRIBUTE_TYPE_MAPPING, remote, ASN1_OBJECT, 1),
} ASN1_SEQUENCE_END(OSSL_ATTRIBUTE_TYPE_MAPPING)

ASN1_SEQUENCE(OSSL_ATTRIBUTE_VALUE_MAPPING) = {
    ASN1_IMP(OSSL_ATTRIBUTE_VALUE_MAPPING, local, OSSL_ATAV, 0),
    ASN1_IMP(OSSL_ATTRIBUTE_VALUE_MAPPING, remote, OSSL_ATAV, 1),
} ASN1_SEQUENCE_END(OSSL_ATTRIBUTE_VALUE_MAPPING)

ASN1_CHOICE(OSSL_ATTRIBUTE_MAPPING) = {
    ASN1_IMP(OSSL_ATTRIBUTE_MAPPING, choice.typeMappings,
             OSSL_ATTRIBUTE_TYPE_MAPPING, OSSL_ATTR_MAP_TYPE),
    ASN1_IMP(OSSL_ATTRIBUTE_MAPPING, choice.typeValueMappings,
             OSSL_ATTRIBUTE_VALUE_MAPPING, OSSL_ATTR_MAP_VALUE),
} ASN1_CHOICE_END(OSSL_ATTRIBUTE_MAPPING)

ASN1_ITEM_TEMPLATE(OSSL_ATTRIBUTE_MAPPINGS) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SET_OF, 0, OSSL_ATTRIBUTE_MAPPINGS, OSSL_ATTRIBUTE_MAPPING)
ASN1_ITEM_TEMPLATE_END(OSSL_ATTRIBUTE_MAPPINGS)

IMPLEMENT_ASN1_FUNCTIONS(OSSL_ATAV)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_ATTRIBUTE_TYPE_MAPPING)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_ATTRIBUTE_VALUE_MAPPING)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_ATTRIBUTE_MAPPING)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_ATTRIBUTE_MAPPINGS)

static int i2r_ATTRIBUTE_MAPPING(X509V3_EXT_METHOD *method,
                                 OSSL_ATTRIBUTE_MAPPING *am,
                                 BIO *out, int indent)
{
    ASN1_OBJECT *local_type, *remote_type;
    int local_attr_nid, remote_attr_nid;
    ASN1_TYPE *local_val, *remote_val;

    switch (am->type) {
    case (OSSL_ATTR_MAP_TYPE):
        if (i2a_ASN1_OBJECT(out, am->choice.typeMappings->local) <= 0)
            return 0;
        if (BIO_puts(out, " == ") <= 0)
            return 0;
        return i2a_ASN1_OBJECT(out, am->choice.typeMappings->remote);
    case (OSSL_ATTR_MAP_VALUE):
        local_type = am->choice.typeValueMappings->local->type;
        remote_type = am->choice.typeValueMappings->remote->type;
        local_val = am->choice.typeValueMappings->local->value;
        remote_val = am->choice.typeValueMappings->remote->value;
        local_attr_nid = OBJ_obj2nid(local_type);
        remote_attr_nid = OBJ_obj2nid(remote_type);
        if (i2a_ASN1_OBJECT(out, local_type) <= 0)
            return 0;
        if (BIO_puts(out, ":") <= 0)
            return 0;
        if (ossl_print_attribute_value(out, local_attr_nid, local_val, 0) <= 0)
            return 0;
        if (BIO_puts(out, " == ") <= 0)
            return 0;
        if (i2a_ASN1_OBJECT(out, remote_type) <= 0)
            return 0;
        if (BIO_puts(out, ":") <= 0)
            return 0;
        return ossl_print_attribute_value(out, remote_attr_nid, remote_val, 0);
    default:
        return 0;
    }
    return 1;
}

static int i2r_ATTRIBUTE_MAPPINGS(X509V3_EXT_METHOD *method,
                                  OSSL_ATTRIBUTE_MAPPINGS *ams,
                                  BIO *out, int indent)
{
    int i;
    OSSL_ATTRIBUTE_MAPPING *am;

    for (i = 0; i < sk_OSSL_ATTRIBUTE_MAPPING_num(ams); i++) {
        am = sk_OSSL_ATTRIBUTE_MAPPING_value(ams, i);
        if (BIO_printf(out, "%*s", indent, "") <= 0)
            return 0;
        if (i2r_ATTRIBUTE_MAPPING(method, am, out, indent + 4) <= 0)
            return 0;
        if (BIO_puts(out, "\n") <= 0)
            return 0;
    }
    return 1;
}

const X509V3_EXT_METHOD ossl_v3_attribute_mappings = {
    NID_attribute_mappings, X509V3_EXT_MULTILINE,
    ASN1_ITEM_ref(OSSL_ATTRIBUTE_MAPPINGS),
    0, 0, 0, 0,
    0, 0,
    0,
    0,
    (X509V3_EXT_I2R)i2r_ATTRIBUTE_MAPPINGS,
    0,
    NULL
};
