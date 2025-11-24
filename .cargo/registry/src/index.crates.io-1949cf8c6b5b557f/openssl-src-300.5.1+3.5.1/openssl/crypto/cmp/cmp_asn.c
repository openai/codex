/*
 * Copyright 2007-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/asn1t.h>

#include "cmp_local.h"
#include "internal/crmf.h"

/* explicit #includes not strictly needed since implied by the above: */
#include <openssl/cmp.h>
#include <openssl/crmf.h>

/* ASN.1 declarations from RFC4210 */
ASN1_SEQUENCE(OSSL_CMP_REVANNCONTENT) = {
    /* OSSL_CMP_PKISTATUS is effectively ASN1_INTEGER so it is used directly */
    ASN1_SIMPLE(OSSL_CMP_REVANNCONTENT, status, ASN1_INTEGER),
    ASN1_SIMPLE(OSSL_CMP_REVANNCONTENT, certId, OSSL_CRMF_CERTID),
    ASN1_SIMPLE(OSSL_CMP_REVANNCONTENT, willBeRevokedAt, ASN1_GENERALIZEDTIME),
    ASN1_SIMPLE(OSSL_CMP_REVANNCONTENT, badSinceDate, ASN1_GENERALIZEDTIME),
    ASN1_OPT(OSSL_CMP_REVANNCONTENT, crlDetails, X509_EXTENSIONS)
} ASN1_SEQUENCE_END(OSSL_CMP_REVANNCONTENT)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_REVANNCONTENT)

ASN1_SEQUENCE(OSSL_CMP_CHALLENGE) = {
    ASN1_OPT(OSSL_CMP_CHALLENGE, owf, X509_ALGOR),
    ASN1_SIMPLE(OSSL_CMP_CHALLENGE, witness, ASN1_OCTET_STRING),
    ASN1_SIMPLE(OSSL_CMP_CHALLENGE, challenge, ASN1_OCTET_STRING)
} ASN1_SEQUENCE_END(OSSL_CMP_CHALLENGE)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CHALLENGE)

ASN1_ITEM_TEMPLATE(OSSL_CMP_POPODECKEYCHALLCONTENT) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0,
                          OSSL_CMP_POPODECKEYCHALLCONTENT, OSSL_CMP_CHALLENGE)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_POPODECKEYCHALLCONTENT)

ASN1_ITEM_TEMPLATE(OSSL_CMP_POPODECKEYRESPCONTENT) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0,
                          OSSL_CMP_POPODECKEYRESPCONTENT, ASN1_INTEGER)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_POPODECKEYRESPCONTENT)

ASN1_SEQUENCE(OSSL_CMP_CAKEYUPDANNCONTENT) = {
    /* OSSL_CMP_CMPCERTIFICATE is effectively X509 so it is used directly */
    ASN1_SIMPLE(OSSL_CMP_CAKEYUPDANNCONTENT, oldWithNew, X509),
    /* OSSL_CMP_CMPCERTIFICATE is effectively X509 so it is used directly */
    ASN1_SIMPLE(OSSL_CMP_CAKEYUPDANNCONTENT, newWithOld, X509),
    /* OSSL_CMP_CMPCERTIFICATE is effectively X509 so it is used directly */
    ASN1_SIMPLE(OSSL_CMP_CAKEYUPDANNCONTENT, newWithNew, X509)
} ASN1_SEQUENCE_END(OSSL_CMP_CAKEYUPDANNCONTENT)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CAKEYUPDANNCONTENT)

ASN1_SEQUENCE(OSSL_CMP_ERRORMSGCONTENT) = {
    ASN1_SIMPLE(OSSL_CMP_ERRORMSGCONTENT, pKIStatusInfo, OSSL_CMP_PKISI),
    ASN1_OPT(OSSL_CMP_ERRORMSGCONTENT, errorCode, ASN1_INTEGER),
    /* OSSL_CMP_PKIFREETEXT is a ASN1_UTF8STRING sequence, so used directly */
    ASN1_SEQUENCE_OF_OPT(OSSL_CMP_ERRORMSGCONTENT, errorDetails,
                         ASN1_UTF8STRING)
} ASN1_SEQUENCE_END(OSSL_CMP_ERRORMSGCONTENT)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_ERRORMSGCONTENT)

ASN1_ADB_TEMPLATE(infotypeandvalue_default) = ASN1_OPT(OSSL_CMP_ITAV,
                                                       infoValue.other,
                                                       ASN1_ANY);
/* ITAV means InfoTypeAndValue */
ASN1_ADB(OSSL_CMP_ITAV) = {
    /* OSSL_CMP_CMPCERTIFICATE is effectively X509 so it is used directly */
    ADB_ENTRY(NID_id_it_caProtEncCert, ASN1_OPT(OSSL_CMP_ITAV,
                                                infoValue.caProtEncCert, X509)),
    ADB_ENTRY(NID_id_it_signKeyPairTypes,
              ASN1_SEQUENCE_OF_OPT(OSSL_CMP_ITAV,
                                   infoValue.signKeyPairTypes, X509_ALGOR)),
    ADB_ENTRY(NID_id_it_encKeyPairTypes,
              ASN1_SEQUENCE_OF_OPT(OSSL_CMP_ITAV,
                                   infoValue.encKeyPairTypes, X509_ALGOR)),
    ADB_ENTRY(NID_id_it_preferredSymmAlg,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.preferredSymmAlg,
                       X509_ALGOR)),
    ADB_ENTRY(NID_id_it_caKeyUpdateInfo,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.caKeyUpdateInfo,
                       OSSL_CMP_CAKEYUPDANNCONTENT)),
    ADB_ENTRY(NID_id_it_currentCRL,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.currentCRL, X509_CRL)),
    ADB_ENTRY(NID_id_it_unsupportedOIDs,
              ASN1_SEQUENCE_OF_OPT(OSSL_CMP_ITAV,
                                   infoValue.unsupportedOIDs, ASN1_OBJECT)),
    ADB_ENTRY(NID_id_it_keyPairParamReq,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.keyPairParamReq,
                       ASN1_OBJECT)),
    ADB_ENTRY(NID_id_it_keyPairParamRep,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.keyPairParamRep,
                       X509_ALGOR)),
    ADB_ENTRY(NID_id_it_revPassphrase,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.revPassphrase,
                       OSSL_CRMF_ENCRYPTEDVALUE)),
    ADB_ENTRY(NID_id_it_implicitConfirm,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.implicitConfirm,
                       ASN1_NULL)),
    ADB_ENTRY(NID_id_it_confirmWaitTime,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.confirmWaitTime,
                       ASN1_GENERALIZEDTIME)),
    ADB_ENTRY(NID_id_it_origPKIMessage,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.origPKIMessage,
                       OSSL_CMP_MSGS)),
    ADB_ENTRY(NID_id_it_suppLangTags,
              ASN1_SEQUENCE_OF_OPT(OSSL_CMP_ITAV, infoValue.suppLangTagsValue,
                                   ASN1_UTF8STRING)),
    ADB_ENTRY(NID_id_it_caCerts,
              ASN1_SEQUENCE_OF_OPT(OSSL_CMP_ITAV, infoValue.caCerts, X509)),
    ADB_ENTRY(NID_id_it_rootCaCert,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.rootCaCert, X509)),
    ADB_ENTRY(NID_id_it_rootCaKeyUpdate,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.rootCaKeyUpdate,
                       OSSL_CMP_ROOTCAKEYUPDATE)),
    ADB_ENTRY(NID_id_it_certReqTemplate,
              ASN1_OPT(OSSL_CMP_ITAV, infoValue.certReqTemplate,
                       OSSL_CMP_CERTREQTEMPLATE)),
    ADB_ENTRY(NID_id_it_certProfile,
              ASN1_SEQUENCE_OF_OPT(OSSL_CMP_ITAV, infoValue.certProfile,
                                   ASN1_UTF8STRING)),
    ADB_ENTRY(NID_id_it_crlStatusList,
              ASN1_SEQUENCE_OF_OPT(OSSL_CMP_ITAV, infoValue.crlStatusList,
                                   OSSL_CMP_CRLSTATUS)),
    ADB_ENTRY(NID_id_it_crls,
              ASN1_SEQUENCE_OF_OPT(OSSL_CMP_ITAV, infoValue.crls, X509_CRL))
} ASN1_ADB_END(OSSL_CMP_ITAV, 0, infoType, 0,
               &infotypeandvalue_default_tt, NULL);

ASN1_SEQUENCE(OSSL_CMP_ITAV) = {
    ASN1_SIMPLE(OSSL_CMP_ITAV, infoType, ASN1_OBJECT),
    ASN1_ADB_OBJECT(OSSL_CMP_ITAV)
} ASN1_SEQUENCE_END(OSSL_CMP_ITAV)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_ITAV)
IMPLEMENT_ASN1_DUP_FUNCTION(OSSL_CMP_ITAV)

ASN1_SEQUENCE(OSSL_CMP_ROOTCAKEYUPDATE) = {
    /* OSSL_CMP_CMPCERTIFICATE is effectively X509 so it is used directly */
    ASN1_SIMPLE(OSSL_CMP_ROOTCAKEYUPDATE, newWithNew, X509),
    ASN1_EXP_OPT(OSSL_CMP_ROOTCAKEYUPDATE, newWithOld, X509, 0),
    ASN1_EXP_OPT(OSSL_CMP_ROOTCAKEYUPDATE, oldWithNew, X509, 1)
} ASN1_SEQUENCE_END(OSSL_CMP_ROOTCAKEYUPDATE)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_ROOTCAKEYUPDATE)

ASN1_ITEM_TEMPLATE(OSSL_CMP_ATAVS) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0,
                          OSSL_CMP_ATAVS, OSSL_CRMF_ATTRIBUTETYPEANDVALUE)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_ATAVS)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_ATAVS)

ASN1_SEQUENCE(OSSL_CMP_CERTREQTEMPLATE) = {
    ASN1_SIMPLE(OSSL_CMP_CERTREQTEMPLATE, certTemplate, OSSL_CRMF_CERTTEMPLATE),
    ASN1_SEQUENCE_OF_OPT(OSSL_CMP_CERTREQTEMPLATE, keySpec,
                         OSSL_CRMF_ATTRIBUTETYPEANDVALUE)
} ASN1_SEQUENCE_END(OSSL_CMP_CERTREQTEMPLATE)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CERTREQTEMPLATE)

ASN1_CHOICE(OSSL_CMP_CRLSOURCE) = {
    ASN1_EXP(OSSL_CMP_CRLSOURCE, value.dpn, DIST_POINT_NAME, 0),
    ASN1_EXP(OSSL_CMP_CRLSOURCE, value.issuer, GENERAL_NAMES, 1),
} ASN1_CHOICE_END(OSSL_CMP_CRLSOURCE)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CRLSOURCE)
#define OSSL_CMP_CRLSOURCE_DPN 0
#define OSSL_CMP_CRLSOURCE_ISSUER 1

ASN1_SEQUENCE(OSSL_CMP_CRLSTATUS) = {
    ASN1_SIMPLE(OSSL_CMP_CRLSTATUS, source, OSSL_CMP_CRLSOURCE),
    ASN1_OPT(OSSL_CMP_CRLSTATUS, thisUpdate, ASN1_TIME)
} ASN1_SEQUENCE_END(OSSL_CMP_CRLSTATUS)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CRLSTATUS)

OSSL_CMP_ITAV *OSSL_CMP_ITAV_create(ASN1_OBJECT *type, ASN1_TYPE *value)
{
    OSSL_CMP_ITAV *itav;

    if (type == NULL || (itav = OSSL_CMP_ITAV_new()) == NULL)
        return NULL;
    OSSL_CMP_ITAV_set0(itav, type, value);
    return itav;
}

void OSSL_CMP_ITAV_set0(OSSL_CMP_ITAV *itav, ASN1_OBJECT *type,
                        ASN1_TYPE *value)
{
    itav->infoType = type;
    itav->infoValue.other = value;
}

ASN1_OBJECT *OSSL_CMP_ITAV_get0_type(const OSSL_CMP_ITAV *itav)
{
    if (itav == NULL)
        return NULL;
    return itav->infoType;
}

ASN1_TYPE *OSSL_CMP_ITAV_get0_value(const OSSL_CMP_ITAV *itav)
{
    if (itav == NULL)
        return NULL;
    return itav->infoValue.other;
}

int OSSL_CMP_ITAV_push0_stack_item(STACK_OF(OSSL_CMP_ITAV) **itav_sk_p,
                                   OSSL_CMP_ITAV *itav)
{
    int created = 0;

    if (itav_sk_p == NULL || itav == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        goto err;
    }

    if (*itav_sk_p == NULL) {
        if ((*itav_sk_p = sk_OSSL_CMP_ITAV_new_null()) == NULL)
            goto err;
        created = 1;
    }
    if (!sk_OSSL_CMP_ITAV_push(*itav_sk_p, itav))
        goto err;
    return 1;

 err:
    if (created) {
        sk_OSSL_CMP_ITAV_free(*itav_sk_p);
        *itav_sk_p = NULL;
    }
    return 0;
}

OSSL_CMP_ITAV
*OSSL_CMP_ITAV_new0_certProfile(STACK_OF(ASN1_UTF8STRING) *certProfile)
{
    OSSL_CMP_ITAV *itav;

    if ((itav = OSSL_CMP_ITAV_new()) == NULL)
        return NULL;
    itav->infoType = OBJ_nid2obj(NID_id_it_certProfile);
    itav->infoValue.certProfile = certProfile;
    return itav;
}

int OSSL_CMP_ITAV_get0_certProfile(const OSSL_CMP_ITAV *itav,
                                   STACK_OF(ASN1_UTF8STRING) **out)
{
    if (itav == NULL || out == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (OBJ_obj2nid(itav->infoType) != NID_id_it_certProfile) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    *out = itav->infoValue.certProfile;
    return 1;
}

OSSL_CMP_ITAV *OSSL_CMP_ITAV_new_caCerts(const STACK_OF(X509) *caCerts)
{
    OSSL_CMP_ITAV *itav = OSSL_CMP_ITAV_new();

    if (itav == NULL)
        return NULL;
    if (sk_X509_num(caCerts) > 0
        && (itav->infoValue.caCerts =
            sk_X509_deep_copy(caCerts, X509_dup, X509_free)) == NULL) {
        OSSL_CMP_ITAV_free(itav);
        return NULL;
    }
    itav->infoType = OBJ_nid2obj(NID_id_it_caCerts);
    return itav;
}

int OSSL_CMP_ITAV_get0_caCerts(const OSSL_CMP_ITAV *itav, STACK_OF(X509) **out)
{
    if (itav == NULL || out == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (OBJ_obj2nid(itav->infoType) != NID_id_it_caCerts) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    *out = sk_X509_num(itav->infoValue.caCerts) > 0
        ? itav->infoValue.caCerts : NULL;
    return 1;
}

OSSL_CMP_ITAV *OSSL_CMP_ITAV_new_rootCaCert(const X509 *rootCaCert)
{
    OSSL_CMP_ITAV *itav = OSSL_CMP_ITAV_new();

    if (itav == NULL)
        return NULL;
    if (rootCaCert != NULL
            && (itav->infoValue.rootCaCert = X509_dup(rootCaCert)) == NULL) {
        OSSL_CMP_ITAV_free(itav);
        return NULL;
    }
    itav->infoType = OBJ_nid2obj(NID_id_it_rootCaCert);
    return itav;
}

int OSSL_CMP_ITAV_get0_rootCaCert(const OSSL_CMP_ITAV *itav, X509 **out)
{
    if (itav == NULL || out == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (OBJ_obj2nid(itav->infoType) != NID_id_it_rootCaCert) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    *out = itav->infoValue.rootCaCert;
    return 1;
}
OSSL_CMP_ITAV *OSSL_CMP_ITAV_new_rootCaKeyUpdate(const X509 *newWithNew,
                                                 const X509 *newWithOld,
                                                 const X509 *oldWithNew)
{
    OSSL_CMP_ITAV *itav;
    OSSL_CMP_ROOTCAKEYUPDATE *upd = NULL;

    if (newWithNew != NULL) {
        upd = OSSL_CMP_ROOTCAKEYUPDATE_new();
        if (upd == NULL)
            return NULL;

        if ((upd->newWithNew = X509_dup(newWithNew)) == NULL)
            goto err;
        if (newWithOld != NULL
            && (upd->newWithOld = X509_dup(newWithOld)) == NULL)
            goto err;
        if (oldWithNew != NULL
            && (upd->oldWithNew = X509_dup(oldWithNew)) == NULL)
            goto err;
    }

    if ((itav = OSSL_CMP_ITAV_new()) == NULL)
        goto err;
    itav->infoType = OBJ_nid2obj(NID_id_it_rootCaKeyUpdate);
    itav->infoValue.rootCaKeyUpdate = upd;
    return itav;

 err:
    OSSL_CMP_ROOTCAKEYUPDATE_free(upd);
    return NULL;
}

int OSSL_CMP_ITAV_get0_rootCaKeyUpdate(const OSSL_CMP_ITAV *itav,
                                       X509 **newWithNew,
                                       X509 **newWithOld,
                                       X509 **oldWithNew)
{
    OSSL_CMP_ROOTCAKEYUPDATE *upd;

    if (itav == NULL || newWithNew == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (OBJ_obj2nid(itav->infoType) != NID_id_it_rootCaKeyUpdate) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    upd = itav->infoValue.rootCaKeyUpdate;
    *newWithNew = upd != NULL ? upd->newWithNew : NULL;
    if (newWithOld != NULL)
        *newWithOld = upd != NULL ? upd->newWithOld : NULL;
    if (oldWithNew != NULL)
        *oldWithNew = upd != NULL ? upd->oldWithNew : NULL;
    return 1;
}

OSSL_CMP_ITAV
*OSSL_CMP_ITAV_new0_certReqTemplate(OSSL_CRMF_CERTTEMPLATE *certTemplate,
                                    OSSL_CMP_ATAVS *keySpec)
{
    OSSL_CMP_ITAV *itav;
    OSSL_CMP_CERTREQTEMPLATE *tmpl;

    if (certTemplate == NULL && keySpec != NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }
    if ((itav = OSSL_CMP_ITAV_new()) == NULL)
        return NULL;
    itav->infoType = OBJ_nid2obj(NID_id_it_certReqTemplate);
    if (certTemplate == NULL)
        return itav;

    if ((tmpl = OSSL_CMP_CERTREQTEMPLATE_new()) == NULL) {
        OSSL_CMP_ITAV_free(itav);
        return NULL;
    }
    itav->infoValue.certReqTemplate = tmpl;
    tmpl->certTemplate = certTemplate;
    tmpl->keySpec = keySpec;
    return itav;
}

int OSSL_CMP_ITAV_get1_certReqTemplate(const OSSL_CMP_ITAV *itav,
                                       OSSL_CRMF_CERTTEMPLATE **certTemplate,
                                       OSSL_CMP_ATAVS **keySpec)
{
    OSSL_CMP_CERTREQTEMPLATE *tpl;

    if (itav == NULL || certTemplate == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    *certTemplate = NULL;
    if (keySpec != NULL)
        *keySpec = NULL;

    if (OBJ_obj2nid(itav->infoType) != NID_id_it_certReqTemplate) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    tpl = itav->infoValue.certReqTemplate;
    if (tpl == NULL) /* no requirements available */
        return 1;

    if ((*certTemplate = OSSL_CRMF_CERTTEMPLATE_dup(tpl->certTemplate)) == NULL)
        return 0;
    if (keySpec != NULL && tpl->keySpec != NULL) {
        int i, n = sk_OSSL_CMP_ATAV_num(tpl->keySpec);

        *keySpec = sk_OSSL_CRMF_ATTRIBUTETYPEANDVALUE_new_reserve(NULL, n);
        if (*keySpec == NULL)
            goto err;
        for (i = 0; i < n; i++) {
            OSSL_CMP_ATAV *atav = sk_OSSL_CMP_ATAV_value(tpl->keySpec, i);
            ASN1_OBJECT *type = OSSL_CMP_ATAV_get0_type(atav /* may be NULL */);
            int nid;
            const char *name;

            if (type == NULL) {
                ERR_raise_data(ERR_LIB_CMP, CMP_R_INVALID_KEYSPEC,
                               "keySpec with index %d in certReqTemplate does not exist",
                               i);
                goto err;
            }
            nid = OBJ_obj2nid(type);

            if (nid != NID_id_regCtrl_algId
                    && nid != NID_id_regCtrl_rsaKeyLen) {
                name = OBJ_nid2ln(nid);
                if (name == NULL)
                    name = OBJ_nid2sn(nid);
                if (name == NULL)
                    name = "<undef>";
                ERR_raise_data(ERR_LIB_CMP, CMP_R_INVALID_KEYSPEC,
                               "keySpec with index %d in certReqTemplate has invalid type %s",
                               i, name);
                goto err;
            }
            OSSL_CMP_ATAV_push1(keySpec, atav);
        }
    }
    return 1;

 err:
    OSSL_CRMF_CERTTEMPLATE_free(*certTemplate);
    *certTemplate = NULL;
    sk_OSSL_CMP_ATAV_pop_free(*keySpec, OSSL_CMP_ATAV_free);
    if (keySpec != NULL)
        *keySpec = NULL;
    return 0;
}

OSSL_CMP_ATAV *OSSL_CMP_ATAV_create(ASN1_OBJECT *type, ASN1_TYPE *value)
{
    OSSL_CMP_ATAV *atav;

    if ((atav = OSSL_CRMF_ATTRIBUTETYPEANDVALUE_new()) == NULL)
        return NULL;
    OSSL_CMP_ATAV_set0(atav, type, value);
    return atav;
}

void OSSL_CMP_ATAV_set0(OSSL_CMP_ATAV *atav, ASN1_OBJECT *type,
                        ASN1_TYPE *value)
{
    atav->type = type;
    atav->value.other = value;
}

ASN1_OBJECT *OSSL_CMP_ATAV_get0_type(const OSSL_CMP_ATAV *atav)
{
    if (atav == NULL)
        return NULL;
    return atav->type;
}

OSSL_CMP_ATAV *OSSL_CMP_ATAV_new_algId(const X509_ALGOR *alg)
{
    X509_ALGOR *dup;
    OSSL_CMP_ATAV *res;

    if (alg == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }
    if ((dup = X509_ALGOR_dup(alg)) == NULL)
        return NULL;
    res = OSSL_CMP_ATAV_create(OBJ_nid2obj(NID_id_regCtrl_algId),
                               (ASN1_TYPE *)dup);
    if (res == NULL)
        X509_ALGOR_free(dup);
    return res;
}

X509_ALGOR *OSSL_CMP_ATAV_get0_algId(const OSSL_CMP_ATAV *atav)
{
    if (atav == NULL || OBJ_obj2nid(atav->type) != NID_id_regCtrl_algId)
        return NULL;
    return atav->value.algId;
}

OSSL_CMP_ATAV *OSSL_CMP_ATAV_new_rsaKeyLen(int len)
{
    ASN1_INTEGER *aint;
    OSSL_CMP_ATAV *res = NULL;

    if (len <= 0) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }
    if ((aint = ASN1_INTEGER_new()) == NULL)
        return NULL;
    if (!ASN1_INTEGER_set(aint, len)
        || (res = OSSL_CMP_ATAV_create(OBJ_nid2obj(NID_id_regCtrl_rsaKeyLen),
                                       (ASN1_TYPE *)aint)) == NULL)
        ASN1_INTEGER_free(aint);
    return res;
}

int OSSL_CMP_ATAV_get_rsaKeyLen(const OSSL_CMP_ATAV *atav)
{
    int64_t val;

    if (atav == NULL || OBJ_obj2nid(atav->type) != NID_id_regCtrl_rsaKeyLen
            || !ASN1_INTEGER_get_int64(&val, atav->value.rsaKeyLen))
        return -1;
    if (val <= 0 || val > INT_MAX)
        return -2;
    return (int)val;
}

ASN1_TYPE *OSSL_CMP_ATAV_get0_value(const OSSL_CMP_ATAV *atav)
{
    if (atav == NULL)
        return NULL;
    return atav->value.other;
}

int OSSL_CMP_ATAV_push1(OSSL_CMP_ATAVS **sk_p, const OSSL_CMP_ATAV *atav)
{
    int created = 0;
    OSSL_CMP_ATAV *dup;

    if (sk_p == NULL || atav == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        goto err;
    }

    if (*sk_p == NULL) {
        if ((*sk_p = sk_OSSL_CRMF_ATTRIBUTETYPEANDVALUE_new_null()) == NULL)
            goto err;
        created = 1;
    }

    if ((dup = OSSL_CRMF_ATTRIBUTETYPEANDVALUE_dup((OSSL_CRMF_ATTRIBUTETYPEANDVALUE *)atav)) == NULL)
        goto err;
    if (sk_OSSL_CRMF_ATTRIBUTETYPEANDVALUE_push(*sk_p, dup))
        return 1;
    OSSL_CRMF_ATTRIBUTETYPEANDVALUE_free(dup);

 err:
    if (created) {
        sk_OSSL_CRMF_ATTRIBUTETYPEANDVALUE_free(*sk_p);
        *sk_p = NULL;
    }
    return 0;
}

OSSL_CMP_ITAV
*OSSL_CMP_ITAV_new0_crlStatusList(STACK_OF(OSSL_CMP_CRLSTATUS) *crlStatusList)
{
    OSSL_CMP_ITAV *itav;

    if ((itav = OSSL_CMP_ITAV_new()) == NULL)
        return NULL;
    itav->infoType = OBJ_nid2obj(NID_id_it_crlStatusList);
    itav->infoValue.crlStatusList = crlStatusList;
    return itav;
}

int OSSL_CMP_ITAV_get0_crlStatusList(const OSSL_CMP_ITAV *itav,
                                     STACK_OF(OSSL_CMP_CRLSTATUS) **out)
{
    if (itav == NULL || out == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (OBJ_obj2nid(itav->infoType) != NID_id_it_crlStatusList) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    *out = itav->infoValue.crlStatusList;
    return 1;
}

OSSL_CMP_CRLSTATUS *OSSL_CMP_CRLSTATUS_new1(const DIST_POINT_NAME *dpn,
                                            const GENERAL_NAMES *issuer,
                                            const ASN1_TIME *thisUpdate)
{
    OSSL_CMP_CRLSOURCE *crlsource;
    OSSL_CMP_CRLSTATUS *crlstatus;

    if (dpn == NULL && issuer == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (dpn != NULL && issuer != NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }

    if ((crlstatus = OSSL_CMP_CRLSTATUS_new()) == NULL)
        return NULL;
    crlsource = crlstatus->source;

    if (dpn != NULL) {
        crlsource->type = OSSL_CMP_CRLSOURCE_DPN;
        if ((crlsource->value.dpn = DIST_POINT_NAME_dup(dpn)) == NULL)
            goto err;
    } else {
        crlsource->type = OSSL_CMP_CRLSOURCE_ISSUER;
        if ((crlsource->value.issuer =
             sk_GENERAL_NAME_deep_copy(issuer, GENERAL_NAME_dup,
                                       GENERAL_NAME_free)) == NULL)
            goto err;
    }

    if (thisUpdate != NULL
            && (crlstatus->thisUpdate = ASN1_TIME_dup(thisUpdate)) == NULL)
        goto err;
    return crlstatus;

 err:
    OSSL_CMP_CRLSTATUS_free(crlstatus);
    return NULL;
}

static GENERAL_NAMES *gennames_new(const X509_NAME *nm)
{
    GENERAL_NAMES *names;
    GENERAL_NAME *name = NULL;

    if ((names = sk_GENERAL_NAME_new_reserve(NULL, 1)) == NULL)
        return NULL;
    if (!GENERAL_NAME_set1_X509_NAME(&name, nm)) {
        sk_GENERAL_NAME_free(names);
        return NULL;
    }
    (void)sk_GENERAL_NAME_push(names, name); /* cannot fail */
    return names;
}

static int gennames_allowed(GENERAL_NAMES *names, int only_DN)
{
    if (names == NULL)
        return 0;
    if (!only_DN)
        return 1;
    return sk_GENERAL_NAME_num(names) == 1
        && sk_GENERAL_NAME_value(names, 0)->type == GEN_DIRNAME;
}

OSSL_CMP_CRLSTATUS *OSSL_CMP_CRLSTATUS_create(const X509_CRL *crl,
                                              const X509 *cert, int only_DN)
{
    STACK_OF(DIST_POINT) *crldps = NULL;
    ISSUING_DIST_POINT *idp = NULL;
    DIST_POINT_NAME *dpn = NULL;
    AUTHORITY_KEYID *akid = NULL;
    GENERAL_NAMES *issuers = NULL;
    const GENERAL_NAMES *CRLissuer = NULL;
    const ASN1_TIME *last = crl == NULL ? NULL : X509_CRL_get0_lastUpdate(crl);
    OSSL_CMP_CRLSTATUS *status = NULL;
    int i, NID_akid = NID_authority_key_identifier;

    /*
     * Note:
     * X509{,_CRL}_get_ext_d2i(..., NID, ..., NULL) return the 1st extension with
     * given NID that is available, if any. If there are more, this is an error.
     */
    if (cert != NULL) {
        crldps = X509_get_ext_d2i(cert, NID_crl_distribution_points, NULL, NULL);
        /* if available, take the first suitable element */
        for (i = 0; i < sk_DIST_POINT_num(crldps); i++) {
            DIST_POINT *dp = sk_DIST_POINT_value(crldps, i);

            if (dp == NULL)
                continue;
            if ((dpn = dp->distpoint) != NULL) {
                CRLissuer = NULL;
                break;
            }
            if (gennames_allowed(dp->CRLissuer, only_DN) && CRLissuer == NULL)
                /* don't break because any dp->distpoint in list is preferred */
                CRLissuer = dp->CRLissuer;
        }
    } else {
        if (crl == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
            return NULL;
        }
        idp = X509_CRL_get_ext_d2i(crl,
                                   NID_issuing_distribution_point, NULL, NULL);
        if (idp != NULL && idp->distpoint != NULL)
            dpn = idp->distpoint;
    }

    if (dpn == NULL && CRLissuer == NULL) {
        if (cert != NULL) {
            akid = X509_get_ext_d2i(cert, NID_akid, NULL, NULL);
            if (akid != NULL && gennames_allowed(akid->issuer, only_DN))
                CRLissuer = akid->issuer;
            else
                CRLissuer = issuers = gennames_new(X509_get_issuer_name(cert));
        }
        if (CRLissuer == NULL && crl != NULL) {
            akid = X509_CRL_get_ext_d2i(crl, NID_akid, NULL, NULL);
            if (akid != NULL && gennames_allowed(akid->issuer, only_DN))
                CRLissuer = akid->issuer;
            else
                CRLissuer = issuers = gennames_new(X509_CRL_get_issuer(crl));
        }
        if (CRLissuer == NULL)
            goto end;
    }

    status = OSSL_CMP_CRLSTATUS_new1(dpn, CRLissuer, last);
 end:
    sk_DIST_POINT_pop_free(crldps, DIST_POINT_free);
    ISSUING_DIST_POINT_free(idp);
    AUTHORITY_KEYID_free(akid);
    sk_GENERAL_NAME_pop_free(issuers, GENERAL_NAME_free);
    return status;
}

int OSSL_CMP_CRLSTATUS_get0(const OSSL_CMP_CRLSTATUS *crlstatus,
                            DIST_POINT_NAME **dpn, GENERAL_NAMES **issuer,
                            ASN1_TIME **thisUpdate)
{
    OSSL_CMP_CRLSOURCE *crlsource;

    if (crlstatus == NULL || dpn == NULL || issuer == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if ((crlsource = crlstatus->source) == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    if (crlsource->type == OSSL_CMP_CRLSOURCE_DPN) {
        *dpn = crlsource->value.dpn;
        *issuer = NULL;
    } else if (crlsource->type == OSSL_CMP_CRLSOURCE_ISSUER) {
        *dpn = NULL;
        *issuer = crlsource->value.issuer;
    } else {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (thisUpdate != NULL)
        *thisUpdate = crlstatus->thisUpdate;
    return 1;
}

OSSL_CMP_ITAV *OSSL_CMP_ITAV_new_crls(const X509_CRL *crl)
{
    OSSL_CMP_ITAV *itav;
    X509_CRL *crl_copy = NULL;
    STACK_OF(X509_CRL) *crls = NULL;

    if ((itav = OSSL_CMP_ITAV_new()) == NULL)
        return NULL;

    if (crl != NULL) {
        if ((crls = sk_X509_CRL_new_reserve(NULL, 1)) == NULL
                || (crl_copy = X509_CRL_dup(crl)) == NULL
                || !sk_X509_CRL_push(crls, crl_copy))
            goto err;
        crl_copy = NULL; /* ownership transferred to crls */
    }

    itav->infoType = OBJ_nid2obj(NID_id_it_crls);
    itav->infoValue.crls = crls;
    return itav;

 err:
    OPENSSL_free(crl_copy);
    sk_X509_CRL_free(crls);
    OSSL_CMP_ITAV_free(itav);
    return NULL;
}

int OSSL_CMP_ITAV_get0_crls(const OSSL_CMP_ITAV *itav, STACK_OF(X509_CRL) **out)
{
    if (itav == NULL || out == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (OBJ_obj2nid(itav->infoType) != NID_id_it_crls) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    *out = itav->infoValue.crls;
    return 1;
}

/* get ASN.1 encoded integer, return -2 on error; -1 is valid for certReqId */
int ossl_cmp_asn1_get_int(const ASN1_INTEGER *a)
{
    int64_t res;

    if (!ASN1_INTEGER_get_int64(&res, a)) {
        ERR_raise(ERR_LIB_CMP, ASN1_R_INVALID_NUMBER);
        return -2;
    }
    if (res < INT_MIN) {
        ERR_raise(ERR_LIB_CMP, ASN1_R_TOO_SMALL);
        return -2;
    }
    if (res > INT_MAX) {
        ERR_raise(ERR_LIB_CMP, ASN1_R_TOO_LARGE);
        return -2;
    }
    return (int)res;
}

static int ossl_cmp_msg_cb(int operation, ASN1_VALUE **pval,
                           ossl_unused const ASN1_ITEM *it, void *exarg)
{
    OSSL_CMP_MSG *msg = (OSSL_CMP_MSG *)*pval;

    switch (operation) {
    case ASN1_OP_FREE_POST:
        OPENSSL_free(msg->propq);
        break;

    case ASN1_OP_DUP_POST:
        {
            OSSL_CMP_MSG *old = exarg;

            if (!ossl_cmp_msg_set0_libctx(msg, old->libctx, old->propq))
                return 0;
        }
        break;
    case ASN1_OP_GET0_LIBCTX:
        {
            OSSL_LIB_CTX **libctx = exarg;

            *libctx = msg->libctx;
        }
        break;
    case ASN1_OP_GET0_PROPQ:
        {
            const char **propq = exarg;

            *propq = msg->propq;
        }
        break;
    default:
        break;
    }

    return 1;
}

ASN1_CHOICE(OSSL_CMP_CERTORENCCERT) = {
    /* OSSL_CMP_CMPCERTIFICATE is effectively X509 so it is used directly */
    ASN1_EXP(OSSL_CMP_CERTORENCCERT, value.certificate, X509, 0),
    ASN1_EXP(OSSL_CMP_CERTORENCCERT, value.encryptedCert,
             OSSL_CRMF_ENCRYPTEDKEY, 1),
} ASN1_CHOICE_END(OSSL_CMP_CERTORENCCERT)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CERTORENCCERT)

ASN1_SEQUENCE(OSSL_CMP_CERTIFIEDKEYPAIR) = {
    ASN1_SIMPLE(OSSL_CMP_CERTIFIEDKEYPAIR, certOrEncCert,
                OSSL_CMP_CERTORENCCERT),
    ASN1_EXP_OPT(OSSL_CMP_CERTIFIEDKEYPAIR, privateKey,
                 OSSL_CRMF_ENCRYPTEDKEY, 0),
    ASN1_EXP_OPT(OSSL_CMP_CERTIFIEDKEYPAIR, publicationInfo,
                 OSSL_CRMF_PKIPUBLICATIONINFO, 1)
} ASN1_SEQUENCE_END(OSSL_CMP_CERTIFIEDKEYPAIR)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CERTIFIEDKEYPAIR)

ASN1_SEQUENCE(OSSL_CMP_REVDETAILS) = {
    ASN1_SIMPLE(OSSL_CMP_REVDETAILS, certDetails, OSSL_CRMF_CERTTEMPLATE),
    ASN1_OPT(OSSL_CMP_REVDETAILS, crlEntryDetails, X509_EXTENSIONS)
} ASN1_SEQUENCE_END(OSSL_CMP_REVDETAILS)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_REVDETAILS)

ASN1_ITEM_TEMPLATE(OSSL_CMP_REVREQCONTENT) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, OSSL_CMP_REVREQCONTENT,
                          OSSL_CMP_REVDETAILS)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_REVREQCONTENT)

ASN1_SEQUENCE(OSSL_CMP_REVREPCONTENT) = {
    ASN1_SEQUENCE_OF(OSSL_CMP_REVREPCONTENT, status, OSSL_CMP_PKISI),
    ASN1_EXP_SEQUENCE_OF_OPT(OSSL_CMP_REVREPCONTENT, revCerts, OSSL_CRMF_CERTID,
                             0),
    ASN1_EXP_SEQUENCE_OF_OPT(OSSL_CMP_REVREPCONTENT, crls, X509_CRL, 1)
} ASN1_SEQUENCE_END(OSSL_CMP_REVREPCONTENT)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_REVREPCONTENT)

ASN1_SEQUENCE(OSSL_CMP_KEYRECREPCONTENT) = {
    ASN1_SIMPLE(OSSL_CMP_KEYRECREPCONTENT, status, OSSL_CMP_PKISI),
    ASN1_EXP_OPT(OSSL_CMP_KEYRECREPCONTENT, newSigCert, X509, 0),
    ASN1_EXP_SEQUENCE_OF_OPT(OSSL_CMP_KEYRECREPCONTENT, caCerts, X509, 1),
    ASN1_EXP_SEQUENCE_OF_OPT(OSSL_CMP_KEYRECREPCONTENT, keyPairHist,
                             OSSL_CMP_CERTIFIEDKEYPAIR, 2)
} ASN1_SEQUENCE_END(OSSL_CMP_KEYRECREPCONTENT)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_KEYRECREPCONTENT)

ASN1_ITEM_TEMPLATE(OSSL_CMP_PKISTATUS) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_UNIVERSAL, 0, status, ASN1_INTEGER)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_PKISTATUS)

ASN1_SEQUENCE(OSSL_CMP_PKISI) = {
    ASN1_SIMPLE(OSSL_CMP_PKISI, status, OSSL_CMP_PKISTATUS),
    /* OSSL_CMP_PKIFREETEXT is a ASN1_UTF8STRING sequence, so used directly */
    ASN1_SEQUENCE_OF_OPT(OSSL_CMP_PKISI, statusString, ASN1_UTF8STRING),
    /* OSSL_CMP_PKIFAILUREINFO is effectively ASN1_BIT_STRING, used directly */
    ASN1_OPT(OSSL_CMP_PKISI, failInfo, ASN1_BIT_STRING)
} ASN1_SEQUENCE_END(OSSL_CMP_PKISI)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_PKISI)
IMPLEMENT_ASN1_DUP_FUNCTION(OSSL_CMP_PKISI)

ASN1_SEQUENCE(OSSL_CMP_CERTSTATUS) = {
    ASN1_SIMPLE(OSSL_CMP_CERTSTATUS, certHash, ASN1_OCTET_STRING),
    ASN1_SIMPLE(OSSL_CMP_CERTSTATUS, certReqId, ASN1_INTEGER),
    ASN1_OPT(OSSL_CMP_CERTSTATUS, statusInfo, OSSL_CMP_PKISI),
    ASN1_EXP_OPT(OSSL_CMP_CERTSTATUS, hashAlg, X509_ALGOR, 0)
} ASN1_SEQUENCE_END(OSSL_CMP_CERTSTATUS)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CERTSTATUS)

ASN1_ITEM_TEMPLATE(OSSL_CMP_CERTCONFIRMCONTENT) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, OSSL_CMP_CERTCONFIRMCONTENT,
                          OSSL_CMP_CERTSTATUS)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_CERTCONFIRMCONTENT)

ASN1_SEQUENCE(OSSL_CMP_CERTRESPONSE) = {
    ASN1_SIMPLE(OSSL_CMP_CERTRESPONSE, certReqId, ASN1_INTEGER),
    ASN1_SIMPLE(OSSL_CMP_CERTRESPONSE, status, OSSL_CMP_PKISI),
    ASN1_OPT(OSSL_CMP_CERTRESPONSE, certifiedKeyPair,
             OSSL_CMP_CERTIFIEDKEYPAIR),
    ASN1_OPT(OSSL_CMP_CERTRESPONSE, rspInfo, ASN1_OCTET_STRING)
} ASN1_SEQUENCE_END(OSSL_CMP_CERTRESPONSE)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CERTRESPONSE)

ASN1_SEQUENCE(OSSL_CMP_POLLREQ) = {
    ASN1_SIMPLE(OSSL_CMP_POLLREQ, certReqId, ASN1_INTEGER)
} ASN1_SEQUENCE_END(OSSL_CMP_POLLREQ)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_POLLREQ)

ASN1_ITEM_TEMPLATE(OSSL_CMP_POLLREQCONTENT) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, OSSL_CMP_POLLREQCONTENT,
                          OSSL_CMP_POLLREQ)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_POLLREQCONTENT)

ASN1_SEQUENCE(OSSL_CMP_POLLREP) = {
    ASN1_SIMPLE(OSSL_CMP_POLLREP, certReqId, ASN1_INTEGER),
    ASN1_SIMPLE(OSSL_CMP_POLLREP, checkAfter, ASN1_INTEGER),
    ASN1_SEQUENCE_OF_OPT(OSSL_CMP_POLLREP, reason, ASN1_UTF8STRING),
} ASN1_SEQUENCE_END(OSSL_CMP_POLLREP)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_POLLREP)

ASN1_ITEM_TEMPLATE(OSSL_CMP_POLLREPCONTENT) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0,
                          OSSL_CMP_POLLREPCONTENT,
                          OSSL_CMP_POLLREP)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_POLLREPCONTENT)

ASN1_SEQUENCE(OSSL_CMP_CERTREPMESSAGE) = {
    /* OSSL_CMP_CMPCERTIFICATE is effectively X509 so it is used directly */
    ASN1_EXP_SEQUENCE_OF_OPT(OSSL_CMP_CERTREPMESSAGE, caPubs, X509, 1),
    ASN1_SEQUENCE_OF(OSSL_CMP_CERTREPMESSAGE, response, OSSL_CMP_CERTRESPONSE)
} ASN1_SEQUENCE_END(OSSL_CMP_CERTREPMESSAGE)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_CERTREPMESSAGE)

ASN1_ITEM_TEMPLATE(OSSL_CMP_GENMSGCONTENT) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, OSSL_CMP_GENMSGCONTENT,
                          OSSL_CMP_ITAV)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_GENMSGCONTENT)

ASN1_ITEM_TEMPLATE(OSSL_CMP_GENREPCONTENT) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, OSSL_CMP_GENREPCONTENT,
                          OSSL_CMP_ITAV)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_GENREPCONTENT)

ASN1_ITEM_TEMPLATE(OSSL_CMP_CRLANNCONTENT) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0,
                          OSSL_CMP_CRLANNCONTENT, X509_CRL)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_CRLANNCONTENT)

ASN1_CHOICE(OSSL_CMP_PKIBODY) = {
    ASN1_EXP(OSSL_CMP_PKIBODY, value.ir, OSSL_CRMF_MSGS, 0),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.ip, OSSL_CMP_CERTREPMESSAGE, 1),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.cr, OSSL_CRMF_MSGS, 2),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.cp, OSSL_CMP_CERTREPMESSAGE, 3),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.p10cr, X509_REQ, 4),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.popdecc,
             OSSL_CMP_POPODECKEYCHALLCONTENT, 5),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.popdecr,
             OSSL_CMP_POPODECKEYRESPCONTENT, 6),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.kur, OSSL_CRMF_MSGS, 7),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.kup, OSSL_CMP_CERTREPMESSAGE, 8),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.krr, OSSL_CRMF_MSGS, 9),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.krp, OSSL_CMP_KEYRECREPCONTENT, 10),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.rr, OSSL_CMP_REVREQCONTENT, 11),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.rp, OSSL_CMP_REVREPCONTENT, 12),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.ccr, OSSL_CRMF_MSGS, 13),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.ccp, OSSL_CMP_CERTREPMESSAGE, 14),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.ckuann, OSSL_CMP_CAKEYUPDANNCONTENT, 15),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.cann, X509, 16),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.rann, OSSL_CMP_REVANNCONTENT, 17),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.crlann, OSSL_CMP_CRLANNCONTENT, 18),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.pkiconf, ASN1_ANY, 19),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.nested, OSSL_CMP_MSGS, 20),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.genm, OSSL_CMP_GENMSGCONTENT, 21),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.genp, OSSL_CMP_GENREPCONTENT, 22),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.error, OSSL_CMP_ERRORMSGCONTENT, 23),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.certConf, OSSL_CMP_CERTCONFIRMCONTENT, 24),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.pollReq, OSSL_CMP_POLLREQCONTENT, 25),
    ASN1_EXP(OSSL_CMP_PKIBODY, value.pollRep, OSSL_CMP_POLLREPCONTENT, 26),
} ASN1_CHOICE_END(OSSL_CMP_PKIBODY)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_PKIBODY)

ASN1_SEQUENCE(OSSL_CMP_PKIHEADER) = {
    ASN1_SIMPLE(OSSL_CMP_PKIHEADER, pvno, ASN1_INTEGER),
    ASN1_SIMPLE(OSSL_CMP_PKIHEADER, sender, GENERAL_NAME),
    ASN1_SIMPLE(OSSL_CMP_PKIHEADER, recipient, GENERAL_NAME),
    ASN1_EXP_OPT(OSSL_CMP_PKIHEADER, messageTime, ASN1_GENERALIZEDTIME, 0),
    ASN1_EXP_OPT(OSSL_CMP_PKIHEADER, protectionAlg, X509_ALGOR, 1),
    ASN1_EXP_OPT(OSSL_CMP_PKIHEADER, senderKID, ASN1_OCTET_STRING, 2),
    ASN1_EXP_OPT(OSSL_CMP_PKIHEADER, recipKID, ASN1_OCTET_STRING, 3),
    ASN1_EXP_OPT(OSSL_CMP_PKIHEADER, transactionID, ASN1_OCTET_STRING, 4),
    ASN1_EXP_OPT(OSSL_CMP_PKIHEADER, senderNonce, ASN1_OCTET_STRING, 5),
    ASN1_EXP_OPT(OSSL_CMP_PKIHEADER, recipNonce, ASN1_OCTET_STRING, 6),
    /* OSSL_CMP_PKIFREETEXT is a ASN1_UTF8STRING sequence, so used directly */
    ASN1_EXP_SEQUENCE_OF_OPT(OSSL_CMP_PKIHEADER, freeText, ASN1_UTF8STRING, 7),
    ASN1_EXP_SEQUENCE_OF_OPT(OSSL_CMP_PKIHEADER, generalInfo,
                             OSSL_CMP_ITAV, 8)
} ASN1_SEQUENCE_END(OSSL_CMP_PKIHEADER)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_PKIHEADER)

ASN1_SEQUENCE(OSSL_CMP_PROTECTEDPART) = {
    ASN1_SIMPLE(OSSL_CMP_MSG, header, OSSL_CMP_PKIHEADER),
    ASN1_SIMPLE(OSSL_CMP_MSG, body, OSSL_CMP_PKIBODY)
} ASN1_SEQUENCE_END(OSSL_CMP_PROTECTEDPART)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CMP_PROTECTEDPART)

ASN1_SEQUENCE_cb(OSSL_CMP_MSG, ossl_cmp_msg_cb) = {
    ASN1_SIMPLE(OSSL_CMP_MSG, header, OSSL_CMP_PKIHEADER),
    ASN1_SIMPLE(OSSL_CMP_MSG, body, OSSL_CMP_PKIBODY),
    ASN1_EXP_OPT(OSSL_CMP_MSG, protection, ASN1_BIT_STRING, 0),
    /* OSSL_CMP_CMPCERTIFICATE is effectively X509 so it is used directly */
    ASN1_EXP_SEQUENCE_OF_OPT(OSSL_CMP_MSG, extraCerts, X509, 1)
} ASN1_SEQUENCE_END_cb(OSSL_CMP_MSG, OSSL_CMP_MSG)
IMPLEMENT_ASN1_DUP_FUNCTION(OSSL_CMP_MSG)

ASN1_ITEM_TEMPLATE(OSSL_CMP_MSGS) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, OSSL_CMP_MSGS,
                          OSSL_CMP_MSG)
ASN1_ITEM_TEMPLATE_END(OSSL_CMP_MSGS)
