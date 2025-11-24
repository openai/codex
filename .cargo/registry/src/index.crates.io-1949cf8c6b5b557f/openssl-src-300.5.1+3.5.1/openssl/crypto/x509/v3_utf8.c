/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/asn1.h>
#include <openssl/conf.h>
#include <openssl/x509v3.h>
#include "ext_dat.h"

/*
 * Subject Sign Tool (1.2.643.100.111) The name of the tool used to signs the subject (UTF8String)
 * This extension is required to obtain the status of a qualified certificate at Russian Federation.
 * RFC-style description is available here: https://tools.ietf.org/html/draft-deremin-rfc4491-bis-04#section-5
 * Russian Federal Law 63 "Digital Sign" is available here:  http://www.consultant.ru/document/cons_doc_LAW_112701/
 */


const X509V3_EXT_METHOD ossl_v3_utf8_list[1] = {
    EXT_UTF8STRING(NID_subjectSignTool),
};

char *i2s_ASN1_UTF8STRING(X509V3_EXT_METHOD *method,
                          ASN1_UTF8STRING *utf8)
{
    char *tmp;

    if (utf8 == NULL || utf8->length == 0) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if ((tmp = OPENSSL_malloc(utf8->length + 1)) == NULL)
        return NULL;
    memcpy(tmp, utf8->data, utf8->length);
    tmp[utf8->length] = 0;
    return tmp;
}

ASN1_UTF8STRING *s2i_ASN1_UTF8STRING(X509V3_EXT_METHOD *method,
                                     X509V3_CTX *ctx, const char *str)
{
    ASN1_UTF8STRING *utf8;
    if (str == NULL) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_NULL_ARGUMENT);
        return NULL;
    }
    if ((utf8 = ASN1_UTF8STRING_new()) == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        return NULL;
    }
    if (!ASN1_STRING_set((ASN1_STRING *)utf8, str, strlen(str))) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        ASN1_UTF8STRING_free(utf8);
        return NULL;
    }
#ifdef CHARSET_EBCDIC
    ebcdic2ascii(utf8->data, utf8->data, utf8->length);
#endif                          /* CHARSET_EBCDIC */
    return utf8;
}
