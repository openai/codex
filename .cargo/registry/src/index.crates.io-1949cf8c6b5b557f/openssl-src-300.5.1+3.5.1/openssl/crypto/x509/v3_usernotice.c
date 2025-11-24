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
#include "ext_dat.h"

ASN1_ITEM_TEMPLATE(OSSL_USER_NOTICE_SYNTAX) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, OSSL_USER_NOTICE_SYNTAX, USERNOTICE)
ASN1_ITEM_TEMPLATE_END(OSSL_USER_NOTICE_SYNTAX)

IMPLEMENT_ASN1_FUNCTIONS(OSSL_USER_NOTICE_SYNTAX)

static int print_notice(BIO *out, USERNOTICE *notice, int indent)
{
    int i;
    ASN1_INTEGER *num;
    char *tmp;

    if (notice->noticeref) {
        NOTICEREF *ref;
        ref = notice->noticeref;
        if (BIO_printf(out, "%*sOrganization: %.*s\n", indent, "",
                   ref->organization->length,
                   ref->organization->data) <= 0)
            return 0;
        if (BIO_printf(out, "%*sNumber%s: ", indent, "",
                   sk_ASN1_INTEGER_num(ref->noticenos) > 1 ? "s" : "") <= 0)
            return 0;
        for (i = 0; i < sk_ASN1_INTEGER_num(ref->noticenos); i++) {
            num = sk_ASN1_INTEGER_value(ref->noticenos, i);
            if (i && BIO_puts(out, ", ") <= 0)
                return 0;
            if (num == NULL && BIO_puts(out, "(null)") <= 0)
                return 0;
            else {
                tmp = i2s_ASN1_INTEGER(NULL, num);
                if (tmp == NULL)
                    return 0;
                if (BIO_puts(out, tmp) <= 0) {
                    OPENSSL_free(tmp);
                    return 0;
                }
                OPENSSL_free(tmp);
            }
        }
        if (notice->exptext && BIO_puts(out, "\n") <= 0)
            return 0;
    }
    if (notice->exptext == NULL)
        return 1;

    return BIO_printf(out, "%*sExplicit Text: %.*s", indent, "",
                notice->exptext->length,
                notice->exptext->data) >= 0;
}

static int i2r_USER_NOTICE_SYNTAX(X509V3_EXT_METHOD *method,
                                  OSSL_USER_NOTICE_SYNTAX *uns,
                                  BIO *out, int indent)
{
    int i;
    USERNOTICE *unotice;

    if (BIO_printf(out, "%*sUser Notices:\n", indent, "") <= 0)
        return 0;

    for (i = 0; i < sk_USERNOTICE_num(uns); i++) {
        unotice = sk_USERNOTICE_value(uns, i);
        if (!print_notice(out, unotice, indent + 4))
            return 0;
        if (BIO_puts(out, "\n\n") <= 0)
            return 0;
    }
    return 1;
}

const X509V3_EXT_METHOD ossl_v3_user_notice = {
    NID_user_notice, 0,
    ASN1_ITEM_ref(OSSL_USER_NOTICE_SYNTAX),
    0, 0, 0, 0,
    0,
    0,
    0, 0,
    (X509V3_EXT_I2R)i2r_USER_NOTICE_SYNTAX,
    0,
    NULL
};
