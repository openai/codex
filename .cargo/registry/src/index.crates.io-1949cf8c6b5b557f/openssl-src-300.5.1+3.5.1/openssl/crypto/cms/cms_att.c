/*
 * Copyright 2008-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/asn1t.h>
#include <openssl/pem.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/cms.h>
#include "internal/nelem.h"
#include "crypto/x509.h"
#include "cms_local.h"

/*-
 * Attribute flags.
 * CMS attribute restrictions are discussed in
 *  - RFC 5652 Section 11.
 * ESS attribute restrictions are discussed in
 *  - RFC 2634 Section 1.3.4  AND
 *  - RFC 5035 Section 5.4
 */
/* This is a signed attribute */
#define CMS_ATTR_F_SIGNED         0x01
/* This is an unsigned attribute */
#define CMS_ATTR_F_UNSIGNED       0x02
/* Must be present if there are any other attributes of the same type */
#define CMS_ATTR_F_REQUIRED_COND  0x10
/* There can only be one instance of this attribute */
#define CMS_ATTR_F_ONLY_ONE       0x20
/* The Attribute's value must have exactly one entry */
#define CMS_ATTR_F_ONE_ATTR_VALUE 0x40

/* Attributes rules for different attributes */
static const struct {
    int nid;   /* The attribute id */
    int flags;
} cms_attribute_properties[] = {
    /* See RFC Section 11 */
    { NID_pkcs9_contentType, CMS_ATTR_F_SIGNED
                             | CMS_ATTR_F_ONLY_ONE
                             | CMS_ATTR_F_ONE_ATTR_VALUE
                             | CMS_ATTR_F_REQUIRED_COND },
    { NID_pkcs9_messageDigest, CMS_ATTR_F_SIGNED
                               | CMS_ATTR_F_ONLY_ONE
                               | CMS_ATTR_F_ONE_ATTR_VALUE
                               | CMS_ATTR_F_REQUIRED_COND },
    { NID_pkcs9_signingTime, CMS_ATTR_F_SIGNED
                             | CMS_ATTR_F_ONLY_ONE
                             | CMS_ATTR_F_ONE_ATTR_VALUE },
    { NID_pkcs9_countersignature, CMS_ATTR_F_UNSIGNED },
    /* ESS */
    { NID_id_smime_aa_signingCertificate, CMS_ATTR_F_SIGNED
                                          | CMS_ATTR_F_ONLY_ONE
                                          | CMS_ATTR_F_ONE_ATTR_VALUE },
    { NID_id_smime_aa_signingCertificateV2, CMS_ATTR_F_SIGNED
                                            | CMS_ATTR_F_ONLY_ONE
                                            | CMS_ATTR_F_ONE_ATTR_VALUE },
    { NID_id_smime_aa_receiptRequest, CMS_ATTR_F_SIGNED
                                      | CMS_ATTR_F_ONLY_ONE
                                      | CMS_ATTR_F_ONE_ATTR_VALUE }
};

/* CMS SignedData Attribute utilities */

int CMS_signed_get_attr_count(const CMS_SignerInfo *si)
{
    return X509at_get_attr_count(si->signedAttrs);
}

int CMS_signed_get_attr_by_NID(const CMS_SignerInfo *si, int nid, int lastpos)
{
    return X509at_get_attr_by_NID(si->signedAttrs, nid, lastpos);
}

int CMS_signed_get_attr_by_OBJ(const CMS_SignerInfo *si, const ASN1_OBJECT *obj,
                               int lastpos)
{
    return X509at_get_attr_by_OBJ(si->signedAttrs, obj, lastpos);
}

X509_ATTRIBUTE *CMS_signed_get_attr(const CMS_SignerInfo *si, int loc)
{
    return X509at_get_attr(si->signedAttrs, loc);
}

X509_ATTRIBUTE *CMS_signed_delete_attr(CMS_SignerInfo *si, int loc)
{
    return X509at_delete_attr(si->signedAttrs, loc);
}

int CMS_signed_add1_attr(CMS_SignerInfo *si, X509_ATTRIBUTE *attr)
{
    if (ossl_x509at_add1_attr(&si->signedAttrs, attr))
        return 1;
    return 0;
}

int CMS_signed_add1_attr_by_OBJ(CMS_SignerInfo *si,
                                const ASN1_OBJECT *obj, int type,
                                const void *bytes, int len)
{
    if (ossl_x509at_add1_attr_by_OBJ(&si->signedAttrs, obj, type, bytes, len))
        return 1;
    return 0;
}

int CMS_signed_add1_attr_by_NID(CMS_SignerInfo *si,
                                int nid, int type, const void *bytes, int len)
{
    if (ossl_x509at_add1_attr_by_NID(&si->signedAttrs, nid, type, bytes, len))
        return 1;
    return 0;
}

int CMS_signed_add1_attr_by_txt(CMS_SignerInfo *si,
                                const char *attrname, int type,
                                const void *bytes, int len)
{
    if (ossl_x509at_add1_attr_by_txt(&si->signedAttrs, attrname, type, bytes,
                                     len))
        return 1;
    return 0;
}

void *CMS_signed_get0_data_by_OBJ(const CMS_SignerInfo *si,
                                  const ASN1_OBJECT *oid,
                                  int lastpos, int type)
{
    return X509at_get0_data_by_OBJ(si->signedAttrs, oid, lastpos, type);
}

int CMS_unsigned_get_attr_count(const CMS_SignerInfo *si)
{
    return X509at_get_attr_count(si->unsignedAttrs);
}

int CMS_unsigned_get_attr_by_NID(const CMS_SignerInfo *si, int nid,
                                 int lastpos)
{
    return X509at_get_attr_by_NID(si->unsignedAttrs, nid, lastpos);
}

int CMS_unsigned_get_attr_by_OBJ(const CMS_SignerInfo *si,
                                 const ASN1_OBJECT *obj, int lastpos)
{
    return X509at_get_attr_by_OBJ(si->unsignedAttrs, obj, lastpos);
}

X509_ATTRIBUTE *CMS_unsigned_get_attr(const CMS_SignerInfo *si, int loc)
{
    return X509at_get_attr(si->unsignedAttrs, loc);
}

X509_ATTRIBUTE *CMS_unsigned_delete_attr(CMS_SignerInfo *si, int loc)
{
    return X509at_delete_attr(si->unsignedAttrs, loc);
}

int CMS_unsigned_add1_attr(CMS_SignerInfo *si, X509_ATTRIBUTE *attr)
{
    if (ossl_x509at_add1_attr(&si->unsignedAttrs, attr))
        return 1;
    return 0;
}

int CMS_unsigned_add1_attr_by_OBJ(CMS_SignerInfo *si,
                                  const ASN1_OBJECT *obj, int type,
                                  const void *bytes, int len)
{
    if (ossl_x509at_add1_attr_by_OBJ(&si->unsignedAttrs, obj, type, bytes, len))
        return 1;
    return 0;
}

int CMS_unsigned_add1_attr_by_NID(CMS_SignerInfo *si,
                                  int nid, int type,
                                  const void *bytes, int len)
{
    if (ossl_x509at_add1_attr_by_NID(&si->unsignedAttrs, nid, type, bytes, len))
        return 1;
    return 0;
}

int CMS_unsigned_add1_attr_by_txt(CMS_SignerInfo *si,
                                  const char *attrname, int type,
                                  const void *bytes, int len)
{
    if (ossl_x509at_add1_attr_by_txt(&si->unsignedAttrs, attrname,
                                     type, bytes, len))
        return 1;
    return 0;
}

void *CMS_unsigned_get0_data_by_OBJ(CMS_SignerInfo *si, ASN1_OBJECT *oid,
                                    int lastpos, int type)
{
    return X509at_get0_data_by_OBJ(si->unsignedAttrs, oid, lastpos, type);
}

/*
 * Retrieve an attribute by nid from a stack of attributes starting at index
 * *lastpos + 1.
 * Returns the attribute or NULL if there is no attribute.
 * If an attribute was found *lastpos returns the index of the found attribute.
 */
static X509_ATTRIBUTE *cms_attrib_get(int nid,
                                      const STACK_OF(X509_ATTRIBUTE) *attrs,
                                      int *lastpos)
{
    X509_ATTRIBUTE *at;
    int loc;

    loc = X509at_get_attr_by_NID(attrs, nid, *lastpos);
    if (loc < 0)
        return NULL;

    at = X509at_get_attr(attrs, loc);
    *lastpos = loc;
    return at;
}

static int cms_check_attribute(int nid, int flags, int type,
                               const STACK_OF(X509_ATTRIBUTE) *attrs,
                               int have_attrs)
{
    int lastpos = -1;
    X509_ATTRIBUTE *at = cms_attrib_get(nid, attrs, &lastpos);

    if (at != NULL) {
        int count = X509_ATTRIBUTE_count(at);

        /* Is this attribute allowed? */
        if (((flags & type) == 0)
            /* check if multiple attributes of the same type are allowed */
            || (((flags & CMS_ATTR_F_ONLY_ONE) != 0)
                && cms_attrib_get(nid, attrs, &lastpos) != NULL)
            /* Check if attribute should have exactly one value in its set */
            || (((flags & CMS_ATTR_F_ONE_ATTR_VALUE) != 0)
                && count != 1)
            /* There should be at least one value */
            || count == 0)
        return 0;
    } else {
        /* fail if a required attribute is missing */
        if (have_attrs
            && ((flags & CMS_ATTR_F_REQUIRED_COND) != 0)
            && (flags & type) != 0)
            return 0;
    }
    return 1;
}

/*
 * Check that the signerinfo attributes obey the attribute rules which includes
 * the following checks
 * - If any signed attributes exist then there must be a Content Type
 * and Message Digest attribute in the signed attributes.
 * - The countersignature attribute is an optional unsigned attribute only.
 * - Content Type, Message Digest, and Signing time attributes are signed
 *     attributes. Only one instance of each is allowed, with each of these
 *     attributes containing a single attribute value in its set.
 */
int ossl_cms_si_check_attributes(const CMS_SignerInfo *si)
{
    int i;
    int have_signed_attrs = (CMS_signed_get_attr_count(si) > 0);
    int have_unsigned_attrs = (CMS_unsigned_get_attr_count(si) > 0);

    for (i = 0; i < (int)OSSL_NELEM(cms_attribute_properties); ++i) {
        int nid = cms_attribute_properties[i].nid;
        int flags = cms_attribute_properties[i].flags;

        if (!cms_check_attribute(nid, flags, CMS_ATTR_F_SIGNED,
                                 si->signedAttrs, have_signed_attrs)
            || !cms_check_attribute(nid, flags, CMS_ATTR_F_UNSIGNED,
                                    si->unsignedAttrs, have_unsigned_attrs)) {
            ERR_raise(ERR_LIB_CMS, CMS_R_ATTRIBUTE_ERROR);
            return 0;
        }
    }
    return 1;
}
