/*
 * Copyright 1999-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/pkcs12.h>
#include "p12_local.h"

/* Add a local keyid to a safebag */

int PKCS12_add_localkeyid(PKCS12_SAFEBAG *bag, unsigned char *name,
                          int namelen)
{
    if (X509at_add1_attr_by_NID(&bag->attrib, NID_localKeyID,
                                V_ASN1_OCTET_STRING, name, namelen) != NULL)
        return 1;
    else
        return 0;
}

/* Add key usage to PKCS#8 structure */

int PKCS8_add_keyusage(PKCS8_PRIV_KEY_INFO *p8, int usage)
{
    unsigned char us_val = (unsigned char)usage;
    return PKCS8_pkey_add1_attr_by_NID(p8, NID_key_usage,
                                       V_ASN1_BIT_STRING, &us_val, 1);
}

/* Add a friendlyname to a safebag */

int PKCS12_add_friendlyname_asc(PKCS12_SAFEBAG *bag, const char *name,
                                int namelen)
{
    if (X509at_add1_attr_by_NID(&bag->attrib, NID_friendlyName,
                                MBSTRING_ASC, (unsigned char *)name, namelen) != NULL)
        return 1;
    else
        return 0;
}

int PKCS12_add_friendlyname_utf8(PKCS12_SAFEBAG *bag, const char *name,
                                int namelen)
{
    if (X509at_add1_attr_by_NID(&bag->attrib, NID_friendlyName,
                                MBSTRING_UTF8, (unsigned char *)name, namelen) != NULL)
        return 1;
    else
        return 0;
}

int PKCS12_add_friendlyname_uni(PKCS12_SAFEBAG *bag,
                                const unsigned char *name, int namelen)
{
    if (X509at_add1_attr_by_NID(&bag->attrib, NID_friendlyName,
                                MBSTRING_BMP, name, namelen) != NULL)
        return 1;
    else
        return 0;
}

int PKCS12_add_CSPName_asc(PKCS12_SAFEBAG *bag, const char *name, int namelen)
{
    if (X509at_add1_attr_by_NID(&bag->attrib, NID_ms_csp_name,
                                MBSTRING_ASC, (unsigned char *)name, namelen) != NULL)
        return 1;
    else
        return 0;
}

int PKCS12_add1_attr_by_NID(PKCS12_SAFEBAG *bag, int nid, int type,
                            const unsigned char *bytes, int len)
{
    if (X509at_add1_attr_by_NID(&bag->attrib, nid, type, bytes, len) != NULL)
        return 1;
    else
        return 0;
}

int PKCS12_add1_attr_by_txt(PKCS12_SAFEBAG *bag, const char *attrname, int type,
                            const unsigned char *bytes, int len)
{
    if (X509at_add1_attr_by_txt(&bag->attrib, attrname, type, bytes, len) != NULL)
        return 1;
    else
        return 0;
}

ASN1_TYPE *PKCS12_get_attr_gen(const STACK_OF(X509_ATTRIBUTE) *attrs,
                               int attr_nid)
{
    int i = X509at_get_attr_by_NID(attrs, attr_nid, -1);

    if (i < 0)
        return NULL;
    return X509_ATTRIBUTE_get0_type(X509at_get_attr(attrs, i), 0);
}

char *PKCS12_get_friendlyname(PKCS12_SAFEBAG *bag)
{
    const ASN1_TYPE *atype;

    if ((atype = PKCS12_SAFEBAG_get0_attr(bag, NID_friendlyName)) == NULL)
        return NULL;
    if (atype->type != V_ASN1_BMPSTRING)
        return NULL;
    return OPENSSL_uni2utf8(atype->value.bmpstring->data,
                            atype->value.bmpstring->length);
}

const STACK_OF(X509_ATTRIBUTE) *
PKCS12_SAFEBAG_get0_attrs(const PKCS12_SAFEBAG *bag)
{
    return bag->attrib;
}

void PKCS12_SAFEBAG_set0_attrs(PKCS12_SAFEBAG *bag, STACK_OF(X509_ATTRIBUTE) *attrs)
{
    if (bag->attrib != attrs)
       sk_X509_ATTRIBUTE_free(bag->attrib);

    bag->attrib = attrs;
}
