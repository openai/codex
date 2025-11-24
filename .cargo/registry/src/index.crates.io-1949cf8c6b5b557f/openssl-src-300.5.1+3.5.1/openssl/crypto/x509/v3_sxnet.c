/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
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

/* Support for Thawte strong extranet extension */

#define SXNET_TEST

static int sxnet_i2r(X509V3_EXT_METHOD *method, SXNET *sx, BIO *out,
                     int indent);
#ifdef SXNET_TEST
static SXNET *sxnet_v2i(X509V3_EXT_METHOD *method, X509V3_CTX *ctx,
                        STACK_OF(CONF_VALUE) *nval);
#endif
const X509V3_EXT_METHOD ossl_v3_sxnet = {
    NID_sxnet, X509V3_EXT_MULTILINE, ASN1_ITEM_ref(SXNET),
    0, 0, 0, 0,
    0, 0,
    0,
#ifdef SXNET_TEST
    (X509V3_EXT_V2I)sxnet_v2i,
#else
    0,
#endif
    (X509V3_EXT_I2R)sxnet_i2r,
    0,
    NULL
};

ASN1_SEQUENCE(SXNETID) = {
        ASN1_SIMPLE(SXNETID, zone, ASN1_INTEGER),
        ASN1_SIMPLE(SXNETID, user, ASN1_OCTET_STRING)
} ASN1_SEQUENCE_END(SXNETID)

IMPLEMENT_ASN1_FUNCTIONS(SXNETID)

ASN1_SEQUENCE(SXNET) = {
        ASN1_SIMPLE(SXNET, version, ASN1_INTEGER),
        ASN1_SEQUENCE_OF(SXNET, ids, SXNETID)
} ASN1_SEQUENCE_END(SXNET)

IMPLEMENT_ASN1_FUNCTIONS(SXNET)

static int sxnet_i2r(X509V3_EXT_METHOD *method, SXNET *sx, BIO *out,
                     int indent)
{
    int64_t v;
    char *tmp;
    SXNETID *id;
    int i;

    /*
     * Since we add 1 to the version number to display it, we don't support
     * LONG_MAX since that would cause on overflow.
     */
    if (!ASN1_INTEGER_get_int64(&v, sx->version)
            || v >= LONG_MAX
            || v < LONG_MIN) {
        BIO_printf(out, "%*sVersion: <unsupported>", indent, "");
    } else {
        long vl = (long)v;

        BIO_printf(out, "%*sVersion: %ld (0x%lX)", indent, "", vl + 1, vl);
    }
    for (i = 0; i < sk_SXNETID_num(sx->ids); i++) {
        id = sk_SXNETID_value(sx->ids, i);
        tmp = i2s_ASN1_INTEGER(NULL, id->zone);
        if (tmp == NULL)
            return 0;
        BIO_printf(out, "\n%*sZone: %s, User: ", indent, "", tmp);
        OPENSSL_free(tmp);
        ASN1_STRING_print(out, id->user);
    }
    return 1;
}

#ifdef SXNET_TEST

/*
 * NBB: this is used for testing only. It should *not* be used for anything
 * else because it will just take static IDs from the configuration file and
 * they should really be separate values for each user.
 */

static SXNET *sxnet_v2i(X509V3_EXT_METHOD *method, X509V3_CTX *ctx,
                        STACK_OF(CONF_VALUE) *nval)
{
    CONF_VALUE *cnf;
    SXNET *sx = NULL;
    int i;
    for (i = 0; i < sk_CONF_VALUE_num(nval); i++) {
        cnf = sk_CONF_VALUE_value(nval, i);
        if (!SXNET_add_id_asc(&sx, cnf->name, cnf->value, -1)) {
            SXNET_free(sx);
            return NULL;
	}
    }
    return sx;
}

#endif

/* Strong Extranet utility functions */

/* Add an id given the zone as an ASCII number */

int SXNET_add_id_asc(SXNET **psx, const char *zone, const char *user, int userlen)
{
    ASN1_INTEGER *izone;

    if ((izone = s2i_ASN1_INTEGER(NULL, zone)) == NULL) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_ERROR_CONVERTING_ZONE);
        return 0;
    }
    if (!SXNET_add_id_INTEGER(psx, izone, user, userlen)) {
        ASN1_INTEGER_free(izone);
        return 0;
    }
    return 1;
}

/* Add an id given the zone as an unsigned long */

int SXNET_add_id_ulong(SXNET **psx, unsigned long lzone, const char *user,
                       int userlen)
{
    ASN1_INTEGER *izone;

    if ((izone = ASN1_INTEGER_new()) == NULL
        || !ASN1_INTEGER_set(izone, lzone)) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        ASN1_INTEGER_free(izone);
        return 0;
    }
    if (!SXNET_add_id_INTEGER(psx, izone, user, userlen)) {
        ASN1_INTEGER_free(izone);
        return 0;
    }
    return 1;
}

/*
 * Add an id given the zone as an ASN1_INTEGER. Note this version uses the
 * passed integer and doesn't make a copy so don't free it up afterwards.
 */

int SXNET_add_id_INTEGER(SXNET **psx, ASN1_INTEGER *zone, const char *user,
                         int userlen)
{
    SXNET *sx = NULL;
    SXNETID *id = NULL;

    if (psx == NULL || zone == NULL || user == NULL) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_NULL_ARGUMENT);
        return 0;
    }
    if (userlen == -1)
        userlen = strlen(user);
    if (userlen > 64) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_USER_TOO_LONG);
        return 0;
    }
    if (*psx == NULL) {
        if ((sx = SXNET_new()) == NULL) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
            goto err;
        }
        if (!ASN1_INTEGER_set(sx->version, 0)) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
            goto err;
        }
    } else
        sx = *psx;
    if (SXNET_get_id_INTEGER(sx, zone)) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_DUPLICATE_ZONE_ID);
        if (*psx == NULL)
            SXNET_free(sx);
        return 0;
    }

    if ((id = SXNETID_new()) == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        goto err;
    }

    if (!ASN1_OCTET_STRING_set(id->user, (const unsigned char *)user, userlen)){
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        goto err;
    }
    if (!sk_SXNETID_push(sx->ids, id)) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
        goto err;
    }
    ASN1_INTEGER_free(id->zone);
    id->zone = zone;
    *psx = sx;
    return 1;

 err:
    SXNETID_free(id);
    if (*psx == NULL)
        SXNET_free(sx);
    return 0;
}

ASN1_OCTET_STRING *SXNET_get_id_asc(SXNET *sx, const char *zone)
{
    ASN1_INTEGER *izone;
    ASN1_OCTET_STRING *oct;

    if ((izone = s2i_ASN1_INTEGER(NULL, zone)) == NULL) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_ERROR_CONVERTING_ZONE);
        return NULL;
    }
    oct = SXNET_get_id_INTEGER(sx, izone);
    ASN1_INTEGER_free(izone);
    return oct;
}

ASN1_OCTET_STRING *SXNET_get_id_ulong(SXNET *sx, unsigned long lzone)
{
    ASN1_INTEGER *izone;
    ASN1_OCTET_STRING *oct;

    if ((izone = ASN1_INTEGER_new()) == NULL
        || !ASN1_INTEGER_set(izone, lzone)) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        ASN1_INTEGER_free(izone);
        return NULL;
    }
    oct = SXNET_get_id_INTEGER(sx, izone);
    ASN1_INTEGER_free(izone);
    return oct;
}

ASN1_OCTET_STRING *SXNET_get_id_INTEGER(SXNET *sx, ASN1_INTEGER *zone)
{
    SXNETID *id;
    int i;
    for (i = 0; i < sk_SXNETID_num(sx->ids); i++) {
        id = sk_SXNETID_value(sx->ids, i);
        if (!ASN1_INTEGER_cmp(id->zone, zone))
            return id->user;
    }
    return NULL;
}
