/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/asn1t.h>
#include <openssl/x509v3.h>
#include <crypto/x509_acert.h>
#include <openssl/x509_acert.h>
#include "crypto/asn1.h"
#include "ext_dat.h"

DECLARE_ASN1_ITEM(OSSL_ISSUER_SERIAL)

ASN1_ITEM_TEMPLATE(OSSL_AUTHORITY_ATTRIBUTE_ID_SYNTAX) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, OSSL_AUTHORITY_ATTRIBUTE_ID_SYNTAX, OSSL_ISSUER_SERIAL)
ASN1_ITEM_TEMPLATE_END(OSSL_AUTHORITY_ATTRIBUTE_ID_SYNTAX)

IMPLEMENT_ASN1_FUNCTIONS(OSSL_AUTHORITY_ATTRIBUTE_ID_SYNTAX)

static int i2r_ISSUER_SERIAL(X509V3_EXT_METHOD *method,
                             OSSL_ISSUER_SERIAL *iss,
                             BIO *out, int indent)
{
    if (iss->issuer != NULL) {
        BIO_printf(out, "%*sIssuer Names:\n", indent, "");
        OSSL_GENERAL_NAMES_print(out, iss->issuer, indent);
        BIO_puts(out, "\n");
    } else {
        BIO_printf(out, "%*sIssuer Names: <none>\n", indent, "");
    }
    BIO_printf(out, "%*sIssuer Serial: ", indent, "");
    if (i2a_ASN1_INTEGER(out, &(iss->serial)) <= 0)
        return 0;
    BIO_puts(out, "\n");
    if (iss->issuerUID != NULL) {
        BIO_printf(out, "%*sIssuer UID: ", indent, "");
        if (i2a_ASN1_STRING(out, iss->issuerUID, V_ASN1_BIT_STRING) <= 0)
            return 0;
        BIO_puts(out, "\n");
    } else {
        BIO_printf(out, "%*sIssuer UID: <none>\n", indent, "");
    }
    return 1;
}

static int i2r_auth_attr_id(X509V3_EXT_METHOD *method,
                            OSSL_AUTHORITY_ATTRIBUTE_ID_SYNTAX *aids,
                            BIO *out, int indent)
{
    int i;
    OSSL_ISSUER_SERIAL *aid;

    for (i = 0; i < sk_OSSL_ISSUER_SERIAL_num(aids); i++) {
        if (BIO_printf(out, "%*sIssuer-Serials:\n", indent, "") <= 0)
            return 0;
        aid = sk_OSSL_ISSUER_SERIAL_value(aids, i);
        if (i2r_ISSUER_SERIAL(method, aid, out, indent + 4) <= 0)
            return 0;
        if (BIO_puts(out, "\n") <= 0)
            return 0;
    }
    return 1;
}

const X509V3_EXT_METHOD ossl_v3_authority_attribute_identifier = {
    NID_authority_attribute_identifier, X509V3_EXT_MULTILINE,
    ASN1_ITEM_ref(OSSL_AUTHORITY_ATTRIBUTE_ID_SYNTAX),
    0, 0, 0, 0,
    0,
    0,
    0, 0,
    (X509V3_EXT_I2R)i2r_auth_attr_id,
    0,
    NULL
};
