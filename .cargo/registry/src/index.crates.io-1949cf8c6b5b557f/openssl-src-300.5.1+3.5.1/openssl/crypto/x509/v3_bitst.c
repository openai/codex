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
#include <openssl/conf.h>
#include <openssl/x509v3.h>
#include "ext_dat.h"

static BIT_STRING_BITNAME ns_cert_type_table[] = {
    {0, "SSL Client", "client"},
    {1, "SSL Server", "server"},
    {2, "S/MIME", "email"},
    {3, "Object Signing", "objsign"},
    {4, "Unused", "reserved"},
    {5, "SSL CA", "sslCA"},
    {6, "S/MIME CA", "emailCA"},
    {7, "Object Signing CA", "objCA"},
    {-1, NULL, NULL}
};

static BIT_STRING_BITNAME key_usage_type_table[] = {
    {0, "Digital Signature", "digitalSignature"},
    {1, "Non Repudiation", "nonRepudiation"},
    {2, "Key Encipherment", "keyEncipherment"},
    {3, "Data Encipherment", "dataEncipherment"},
    {4, "Key Agreement", "keyAgreement"},
    {5, "Certificate Sign", "keyCertSign"},
    {6, "CRL Sign", "cRLSign"},
    {7, "Encipher Only", "encipherOnly"},
    {8, "Decipher Only", "decipherOnly"},
    {-1, NULL, NULL}
};

const X509V3_EXT_METHOD ossl_v3_nscert =
EXT_BITSTRING(NID_netscape_cert_type, ns_cert_type_table);
const X509V3_EXT_METHOD ossl_v3_key_usage =
EXT_BITSTRING(NID_key_usage, key_usage_type_table);

STACK_OF(CONF_VALUE) *i2v_ASN1_BIT_STRING(X509V3_EXT_METHOD *method,
                                          ASN1_BIT_STRING *bits,
                                          STACK_OF(CONF_VALUE) *ret)
{
    BIT_STRING_BITNAME *bnam;
    for (bnam = method->usr_data; bnam->lname; bnam++) {
        if (ASN1_BIT_STRING_get_bit(bits, bnam->bitnum))
            X509V3_add_value(bnam->lname, NULL, &ret);
    }
    return ret;
}

ASN1_BIT_STRING *v2i_ASN1_BIT_STRING(X509V3_EXT_METHOD *method,
                                     X509V3_CTX *ctx,
                                     STACK_OF(CONF_VALUE) *nval)
{
    CONF_VALUE *val;
    ASN1_BIT_STRING *bs;
    int i;
    BIT_STRING_BITNAME *bnam;
    if ((bs = ASN1_BIT_STRING_new()) == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        return NULL;
    }
    for (i = 0; i < sk_CONF_VALUE_num(nval); i++) {
        val = sk_CONF_VALUE_value(nval, i);
        for (bnam = method->usr_data; bnam->lname; bnam++) {
            if (strcmp(bnam->sname, val->name) == 0
                || strcmp(bnam->lname, val->name) == 0) {
                if (!ASN1_BIT_STRING_set_bit(bs, bnam->bitnum, 1)) {
                    ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
                    ASN1_BIT_STRING_free(bs);
                    return NULL;
                }
                break;
            }
        }
        if (!bnam->lname) {
            ERR_raise_data(ERR_LIB_X509V3, X509V3_R_UNKNOWN_BIT_STRING_ARGUMENT,
                           "%s", val->name);
            ASN1_BIT_STRING_free(bs);
            return NULL;
        }
    }
    return bs;
}
