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

ASN1_SEQUENCE(OSSL_ROLE_SPEC_CERT_ID) = {
    ASN1_EXP(OSSL_ROLE_SPEC_CERT_ID, roleName, GENERAL_NAME, 0),
    ASN1_EXP(OSSL_ROLE_SPEC_CERT_ID, roleCertIssuer, GENERAL_NAME, 1),
    ASN1_IMP_OPT(OSSL_ROLE_SPEC_CERT_ID, roleCertSerialNumber, ASN1_INTEGER, 2),
    ASN1_IMP_SEQUENCE_OF_OPT(OSSL_ROLE_SPEC_CERT_ID, roleCertLocator, GENERAL_NAME, 3),
} ASN1_SEQUENCE_END(OSSL_ROLE_SPEC_CERT_ID)

IMPLEMENT_ASN1_FUNCTIONS(OSSL_ROLE_SPEC_CERT_ID)

ASN1_ITEM_TEMPLATE(OSSL_ROLE_SPEC_CERT_ID_SYNTAX) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF,
                          0, OSSL_ROLE_SPEC_CERT_ID_SYNTAX, OSSL_ROLE_SPEC_CERT_ID)
ASN1_ITEM_TEMPLATE_END(OSSL_ROLE_SPEC_CERT_ID_SYNTAX)

IMPLEMENT_ASN1_FUNCTIONS(OSSL_ROLE_SPEC_CERT_ID_SYNTAX)

static int i2r_OSSL_ROLE_SPEC_CERT_ID(X509V3_EXT_METHOD *method,
                                      OSSL_ROLE_SPEC_CERT_ID *rscid,
                                      BIO *out, int indent)
{
    if (BIO_printf(out, "%*sRole Name: ", indent, "") <= 0)
        return 0;
    if (GENERAL_NAME_print(out, rscid->roleName) <= 0)
        return 0;
    if (BIO_puts(out, "\n") <= 0)
        return 0;
    if (BIO_printf(out, "%*sRole Certificate Issuer: ", indent, "") <= 0)
        return 0;
    if (GENERAL_NAME_print(out, rscid->roleCertIssuer) <= 0)
        return 0;
    if (rscid->roleCertSerialNumber != NULL) {
        if (BIO_puts(out, "\n") <= 0)
            return 0;
        if (BIO_printf(out, "%*sRole Certificate Serial Number:", indent, "") <= 0)
            return 0;
        if (ossl_serial_number_print(out, rscid->roleCertSerialNumber, indent) != 0)
            return 0;
    }
    if (rscid->roleCertLocator != NULL) {
        if (BIO_puts(out, "\n") <= 0)
            return 0;
        if (BIO_printf(out, "%*sRole Certificate Locator:\n", indent, "") <= 0)
            return 0;
        if (OSSL_GENERAL_NAMES_print(out, rscid->roleCertLocator, indent) <= 0)
            return 0;
    }
    return BIO_puts(out, "\n");
}

static int i2r_OSSL_ROLE_SPEC_CERT_ID_SYNTAX(X509V3_EXT_METHOD *method,
                                             OSSL_ROLE_SPEC_CERT_ID_SYNTAX *rscids,
                                             BIO *out, int indent)
{
    OSSL_ROLE_SPEC_CERT_ID *rscid;
    int i;

    for (i = 0; i < sk_OSSL_ROLE_SPEC_CERT_ID_num(rscids); i++) {
        if (i > 0 && BIO_puts(out, "\n") <= 0)
            return 0;
        if (BIO_printf(out,
                       "%*sRole Specification Certificate Identifier #%d:\n",
                       indent, "", i + 1) <= 0)
            return 0;
        rscid = sk_OSSL_ROLE_SPEC_CERT_ID_value(rscids, i);
        if (i2r_OSSL_ROLE_SPEC_CERT_ID(method, rscid, out, indent + 4) != 1)
            return 0;
    }
    return 1;
}

const X509V3_EXT_METHOD ossl_v3_role_spec_cert_identifier = {
    NID_role_spec_cert_identifier, X509V3_EXT_MULTILINE,
    ASN1_ITEM_ref(OSSL_ROLE_SPEC_CERT_ID_SYNTAX),
    0, 0, 0, 0,
    0, 0,
    0,
    0,
    (X509V3_EXT_I2R)i2r_OSSL_ROLE_SPEC_CERT_ID_SYNTAX,
    NULL,
    NULL
};
