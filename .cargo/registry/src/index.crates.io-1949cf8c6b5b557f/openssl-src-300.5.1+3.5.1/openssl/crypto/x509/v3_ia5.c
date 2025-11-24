/*
 * Copyright 1999-2021 The OpenSSL Project Authors. All Rights Reserved.
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

const X509V3_EXT_METHOD ossl_v3_ns_ia5_list[8] = {
    EXT_IA5STRING(NID_netscape_base_url),
    EXT_IA5STRING(NID_netscape_revocation_url),
    EXT_IA5STRING(NID_netscape_ca_revocation_url),
    EXT_IA5STRING(NID_netscape_renewal_url),
    EXT_IA5STRING(NID_netscape_ca_policy_url),
    EXT_IA5STRING(NID_netscape_ssl_server_name),
    EXT_IA5STRING(NID_netscape_comment),
    EXT_END
};

char *i2s_ASN1_IA5STRING(X509V3_EXT_METHOD *method, ASN1_IA5STRING *ia5)
{
    char *tmp;

    if (ia5 == NULL || ia5->length <= 0)
        return NULL;
    if ((tmp = OPENSSL_malloc(ia5->length + 1)) == NULL)
        return NULL;
    memcpy(tmp, ia5->data, ia5->length);
    tmp[ia5->length] = 0;
    return tmp;
}

ASN1_IA5STRING *s2i_ASN1_IA5STRING(X509V3_EXT_METHOD *method,
                                   X509V3_CTX *ctx, const char *str)
{
    ASN1_IA5STRING *ia5;
    if (str == NULL) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_NULL_ARGUMENT);
        return NULL;
    }
    if ((ia5 = ASN1_IA5STRING_new()) == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        return NULL;
    }
    if (!ASN1_STRING_set((ASN1_STRING *)ia5, str, strlen(str))) {
        ASN1_IA5STRING_free(ia5);
        return NULL;
    }
#ifdef CHARSET_EBCDIC
    ebcdic2ascii(ia5->data, ia5->data, ia5->length);
#endif                          /* CHARSET_EBCDIC */
    return ia5;
}
