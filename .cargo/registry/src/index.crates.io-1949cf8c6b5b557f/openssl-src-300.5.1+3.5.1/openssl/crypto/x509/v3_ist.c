/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/conf.h>
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/x509v3.h>
#include "ext_dat.h"

/*
 * Issuer Sign Tool (1.2.643.100.112) The name of the tool used to signs the subject (ASN1_SEQUENCE)
 * This extension is required to obtain the status of a qualified certificate at Russian Federation.
 * RFC-style description is available here: https://tools.ietf.org/html/draft-deremin-rfc4491-bis-04#section-5
 * Russian Federal Law 63 "Digital Sign" is available here:  http://www.consultant.ru/document/cons_doc_LAW_112701/
 */

ASN1_SEQUENCE(ISSUER_SIGN_TOOL) = {
        ASN1_SIMPLE(ISSUER_SIGN_TOOL, signTool, ASN1_UTF8STRING),
        ASN1_SIMPLE(ISSUER_SIGN_TOOL, cATool, ASN1_UTF8STRING),
        ASN1_SIMPLE(ISSUER_SIGN_TOOL, signToolCert, ASN1_UTF8STRING),
        ASN1_SIMPLE(ISSUER_SIGN_TOOL, cAToolCert, ASN1_UTF8STRING)
} ASN1_SEQUENCE_END(ISSUER_SIGN_TOOL)

IMPLEMENT_ASN1_FUNCTIONS(ISSUER_SIGN_TOOL)


static ISSUER_SIGN_TOOL *v2i_issuer_sign_tool(X509V3_EXT_METHOD *method, X509V3_CTX *ctx,
                        STACK_OF(CONF_VALUE) *nval)
{
    ISSUER_SIGN_TOOL *ist = ISSUER_SIGN_TOOL_new();
    int i;

    if (ist == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        return NULL;
    }
    for (i = 0; i < sk_CONF_VALUE_num(nval); ++i) {
        CONF_VALUE *cnf = sk_CONF_VALUE_value(nval, i);

        if (cnf == NULL) {
            continue;
        }
        if (strcmp(cnf->name, "signTool") == 0) {
            ist->signTool = ASN1_UTF8STRING_new();
            if (ist->signTool == NULL
                || cnf->value == NULL
                || !ASN1_STRING_set(ist->signTool, cnf->value, strlen(cnf->value))) {
                ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
                goto err;
            }
        } else if (strcmp(cnf->name, "cATool") == 0) {
            ist->cATool = ASN1_UTF8STRING_new();
            if (ist->cATool == NULL
                || cnf->value == NULL
                || !ASN1_STRING_set(ist->cATool, cnf->value, strlen(cnf->value))) {
                ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
                goto err;
            }
        } else if (strcmp(cnf->name, "signToolCert") == 0) {
            ist->signToolCert = ASN1_UTF8STRING_new();
            if (ist->signToolCert == NULL
                || cnf->value == NULL
                || !ASN1_STRING_set(ist->signToolCert, cnf->value, strlen(cnf->value))) {
                ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
                goto err;
            }
        } else if (strcmp(cnf->name, "cAToolCert") == 0) {
            ist->cAToolCert = ASN1_UTF8STRING_new();
            if (ist->cAToolCert == NULL
                || cnf->value == NULL
                || !ASN1_STRING_set(ist->cAToolCert, cnf->value, strlen(cnf->value))) {
                ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
                goto err;
            }
        } else {
            ERR_raise(ERR_LIB_X509V3, ERR_R_PASSED_INVALID_ARGUMENT);
            goto err;
        }
    }
    return ist;

err:
    ISSUER_SIGN_TOOL_free(ist);
    return NULL;
}

static int i2r_issuer_sign_tool(X509V3_EXT_METHOD *method,
                                 ISSUER_SIGN_TOOL *ist, BIO *out,
                                 int indent)
{
    int new_line = 0;

    if (ist == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ist->signTool != NULL) {
        BIO_printf(out, "%*ssignTool    : ", indent, "");
        BIO_write(out, ist->signTool->data, ist->signTool->length);
        new_line = 1;
    }
    if (ist->cATool != NULL) {
        if (new_line == 1) {
            BIO_write(out, "\n", 1);
        }
        BIO_printf(out, "%*scATool      : ", indent, "");
        BIO_write(out, ist->cATool->data, ist->cATool->length);
        new_line = 1;
    }
    if (ist->signToolCert != NULL) {
        if (new_line == 1) {
            BIO_write(out, "\n", 1);
        }
        BIO_printf(out, "%*ssignToolCert: ", indent, "");
        BIO_write(out, ist->signToolCert->data, ist->signToolCert->length);
        new_line = 1;
    }
    if (ist->cAToolCert != NULL) {
        if (new_line == 1) {
            BIO_write(out, "\n", 1);
        }
        BIO_printf(out, "%*scAToolCert  : ", indent, "");
        BIO_write(out, ist->cAToolCert->data, ist->cAToolCert->length);
        new_line = 1;
    }
    return 1;
}

const X509V3_EXT_METHOD ossl_v3_issuer_sign_tool = {
    NID_issuerSignTool,                   /* nid */
    X509V3_EXT_MULTILINE,                 /* flags */
    ASN1_ITEM_ref(ISSUER_SIGN_TOOL),      /* template */
    0, 0, 0, 0,                           /* old functions, ignored */
    0,                                    /* i2s */
    0,                                    /* s2i */
    0,                                    /* i2v */
    (X509V3_EXT_V2I)v2i_issuer_sign_tool, /* v2i */
    (X509V3_EXT_I2R)i2r_issuer_sign_tool, /* i2r */
    0,                                    /* r2i */
    NULL                                  /* extension-specific data */
};
