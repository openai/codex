/*
 * Copyright 2006-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <openssl/cms.h>
#include <openssl/err.h>
#include <openssl/core_names.h>
#include "crypto/asn1.h"
#include "crypto/rsa.h"
#include "crypto/evp.h"
#include "cms_local.h"

static RSA_OAEP_PARAMS *rsa_oaep_decode(const X509_ALGOR *alg)
{
    RSA_OAEP_PARAMS *oaep;

    oaep = ASN1_TYPE_unpack_sequence(ASN1_ITEM_rptr(RSA_OAEP_PARAMS),
                                     alg->parameter);

    if (oaep == NULL)
        return NULL;

    if (oaep->maskGenFunc != NULL) {
        oaep->maskHash = ossl_x509_algor_mgf1_decode(oaep->maskGenFunc);
        if (oaep->maskHash == NULL) {
            RSA_OAEP_PARAMS_free(oaep);
            return NULL;
        }
    }
    return oaep;
}

static int rsa_cms_decrypt(CMS_RecipientInfo *ri)
{
    EVP_PKEY_CTX *pkctx;
    X509_ALGOR *cmsalg;
    int nid;
    int rv = -1;
    unsigned char *label = NULL;
    int labellen = 0;
    const EVP_MD *mgf1md = NULL, *md = NULL;
    RSA_OAEP_PARAMS *oaep;

    pkctx = CMS_RecipientInfo_get0_pkey_ctx(ri);
    if (pkctx == NULL)
        return 0;
    if (!CMS_RecipientInfo_ktri_get0_algs(ri, NULL, NULL, &cmsalg))
        return -1;
    nid = OBJ_obj2nid(cmsalg->algorithm);
    if (nid == NID_rsaEncryption)
        return 1;
    if (nid != NID_rsaesOaep) {
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_ENCRYPTION_TYPE);
        return -1;
    }
    /* Decode OAEP parameters */
    oaep = rsa_oaep_decode(cmsalg);

    if (oaep == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_INVALID_OAEP_PARAMETERS);
        goto err;
    }

    mgf1md = ossl_x509_algor_get_md(oaep->maskHash);
    if (mgf1md == NULL)
        goto err;
    md = ossl_x509_algor_get_md(oaep->hashFunc);
    if (md == NULL)
        goto err;

    if (oaep->pSourceFunc != NULL) {
        X509_ALGOR *plab = oaep->pSourceFunc;

        if (OBJ_obj2nid(plab->algorithm) != NID_pSpecified) {
            ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_LABEL_SOURCE);
            goto err;
        }
        if (plab->parameter->type != V_ASN1_OCTET_STRING) {
            ERR_raise(ERR_LIB_CMS, CMS_R_INVALID_LABEL);
            goto err;
        }

        label = plab->parameter->value.octet_string->data;
        /* Stop label being freed when OAEP parameters are freed */
        plab->parameter->value.octet_string->data = NULL;
        labellen = plab->parameter->value.octet_string->length;
    }

    if (EVP_PKEY_CTX_set_rsa_padding(pkctx, RSA_PKCS1_OAEP_PADDING) <= 0)
        goto err;
    if (EVP_PKEY_CTX_set_rsa_oaep_md(pkctx, md) <= 0)
        goto err;
    if (EVP_PKEY_CTX_set_rsa_mgf1_md(pkctx, mgf1md) <= 0)
        goto err;
    if (label != NULL
            && EVP_PKEY_CTX_set0_rsa_oaep_label(pkctx, label, labellen) <= 0) {
        OPENSSL_free(label);
        goto err;
    }
    /* Carry on */
    rv = 1;

 err:
    RSA_OAEP_PARAMS_free(oaep);
    return rv;
}

static int rsa_cms_encrypt(CMS_RecipientInfo *ri)
{
    const EVP_MD *md, *mgf1md;
    RSA_OAEP_PARAMS *oaep = NULL;
    ASN1_STRING *os = NULL;
    ASN1_OCTET_STRING *los = NULL;
    X509_ALGOR *alg;
    EVP_PKEY_CTX *pkctx = CMS_RecipientInfo_get0_pkey_ctx(ri);
    int pad_mode = RSA_PKCS1_PADDING, rv = 0, labellen;
    unsigned char *label;

    if (CMS_RecipientInfo_ktri_get0_algs(ri, NULL, NULL, &alg) <= 0)
        return 0;
    if (pkctx != NULL) {
        if (EVP_PKEY_CTX_get_rsa_padding(pkctx, &pad_mode) <= 0)
            return 0;
    }
    if (pad_mode == RSA_PKCS1_PADDING)
        return X509_ALGOR_set0(alg, OBJ_nid2obj(NID_rsaEncryption),
                               V_ASN1_NULL, NULL);

    /* Not supported */
    if (pad_mode != RSA_PKCS1_OAEP_PADDING)
        return 0;
    if (EVP_PKEY_CTX_get_rsa_oaep_md(pkctx, &md) <= 0)
        goto err;
    if (EVP_PKEY_CTX_get_rsa_mgf1_md(pkctx, &mgf1md) <= 0)
        goto err;
    labellen = EVP_PKEY_CTX_get0_rsa_oaep_label(pkctx, &label);
    if (labellen < 0)
        goto err;
    oaep = RSA_OAEP_PARAMS_new();
    if (oaep == NULL)
        goto err;
    if (!ossl_x509_algor_new_from_md(&oaep->hashFunc, md))
        goto err;
    if (!ossl_x509_algor_md_to_mgf1(&oaep->maskGenFunc, mgf1md))
        goto err;
    if (labellen > 0) {
        los = ASN1_OCTET_STRING_new();

        if (los == NULL)
            goto err;
        if (!ASN1_OCTET_STRING_set(los, label, labellen))
            goto err;

        oaep->pSourceFunc = ossl_X509_ALGOR_from_nid(NID_pSpecified,
                                                     V_ASN1_OCTET_STRING, los);
        if (oaep->pSourceFunc == NULL)
            goto err;

        los = NULL;
    }
    /* create string with oaep parameter encoding. */
    if (!ASN1_item_pack(oaep, ASN1_ITEM_rptr(RSA_OAEP_PARAMS), &os))
        goto err;
    if (!X509_ALGOR_set0(alg, OBJ_nid2obj(NID_rsaesOaep), V_ASN1_SEQUENCE, os))
        goto err;
    os = NULL;
    rv = 1;
 err:
    RSA_OAEP_PARAMS_free(oaep);
    ASN1_STRING_free(os);
    ASN1_OCTET_STRING_free(los);
    return rv;
}

int ossl_cms_rsa_envelope(CMS_RecipientInfo *ri, int decrypt)
{
    assert(decrypt == 0 || decrypt == 1);

    if (decrypt == 1)
        return rsa_cms_decrypt(ri);

    if (decrypt == 0)
        return rsa_cms_encrypt(ri);

    ERR_raise(ERR_LIB_CMS, CMS_R_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
    return 0;
}

static int rsa_cms_sign(CMS_SignerInfo *si)
{
    int pad_mode = RSA_PKCS1_PADDING;
    X509_ALGOR *alg;
    EVP_PKEY_CTX *pkctx = CMS_SignerInfo_get0_pkey_ctx(si);
    unsigned char aid[128];
    const unsigned char *pp = aid;
    size_t aid_len = 0;
    OSSL_PARAM params[2];

    CMS_SignerInfo_get0_algs(si, NULL, NULL, NULL, &alg);
    if (pkctx != NULL) {
        if (EVP_PKEY_CTX_get_rsa_padding(pkctx, &pad_mode) <= 0)
            return 0;
    }
    if (pad_mode == RSA_PKCS1_PADDING)
        return X509_ALGOR_set0(alg, OBJ_nid2obj(NID_rsaEncryption),
                               V_ASN1_NULL, NULL);

    /* We don't support it */
    if (pad_mode != RSA_PKCS1_PSS_PADDING)
        return 0;

    if (evp_pkey_ctx_is_legacy(pkctx)) {
        /* No provider -> we cannot query it for algorithm ID. */
        ASN1_STRING *os = NULL;

        os = ossl_rsa_ctx_to_pss_string(pkctx);
        if (os == NULL)
            return 0;
        if (X509_ALGOR_set0(alg, OBJ_nid2obj(EVP_PKEY_RSA_PSS), V_ASN1_SEQUENCE, os))
            return 1;
        ASN1_STRING_free(os);
        return 0;
    }

    params[0] = OSSL_PARAM_construct_octet_string(
        OSSL_SIGNATURE_PARAM_ALGORITHM_ID, aid, sizeof(aid));
    params[1] = OSSL_PARAM_construct_end();

    if (EVP_PKEY_CTX_get_params(pkctx, params) <= 0)
        return 0;
    if ((aid_len = params[0].return_size) == 0)
        return 0;
    if (d2i_X509_ALGOR(&alg, &pp, aid_len) == NULL)
        return 0;
    return 1;
}

static int rsa_cms_verify(CMS_SignerInfo *si)
{
    int nid, nid2;
    X509_ALGOR *alg;
    EVP_PKEY_CTX *pkctx = CMS_SignerInfo_get0_pkey_ctx(si);
    EVP_PKEY *pkey = EVP_PKEY_CTX_get0_pkey(pkctx);

    CMS_SignerInfo_get0_algs(si, NULL, NULL, NULL, &alg);
    nid = OBJ_obj2nid(alg->algorithm);
    if (nid == EVP_PKEY_RSA_PSS)
        return ossl_rsa_pss_to_ctx(NULL, pkctx, alg, NULL) > 0;
    /* Only PSS allowed for PSS keys */
    if (EVP_PKEY_is_a(pkey, "RSA-PSS")) {
        ERR_raise(ERR_LIB_RSA, RSA_R_ILLEGAL_OR_UNSUPPORTED_PADDING_MODE);
        return 0;
    }
    if (nid == NID_rsaEncryption)
        return 1;
    /* Workaround for some implementation that use a signature OID */
    if (OBJ_find_sigid_algs(nid, NULL, &nid2)) {
        if (nid2 == NID_rsaEncryption)
            return 1;
    }
    return 0;
}

int ossl_cms_rsa_sign(CMS_SignerInfo *si, int verify)
{
    assert(verify == 0 || verify == 1);

    if (verify == 1)
        return rsa_cms_verify(si);

    if (verify == 0)
        return rsa_cms_sign(si);

    ERR_raise(ERR_LIB_CMS, CMS_R_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
    return 0;
}
