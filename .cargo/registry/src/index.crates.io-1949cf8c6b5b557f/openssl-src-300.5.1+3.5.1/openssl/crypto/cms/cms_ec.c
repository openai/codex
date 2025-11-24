/*
 * Copyright 2006-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <limits.h>
#include <openssl/cms.h>
#include <openssl/err.h>
#include <openssl/decoder.h>
#include "internal/sizes.h"
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "cms_local.h"

static EVP_PKEY *pkey_type2param(int ptype, const void *pval,
                                 OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *pctx = NULL;
    OSSL_DECODER_CTX *ctx = NULL;

    if (ptype == V_ASN1_SEQUENCE) {
        const ASN1_STRING *pstr = pval;
        const unsigned char *pm = pstr->data;
        size_t pmlen = (size_t)pstr->length;
        int selection = OSSL_KEYMGMT_SELECT_ALL_PARAMETERS;

        ctx = OSSL_DECODER_CTX_new_for_pkey(&pkey, "DER", NULL, "EC",
                                            selection, libctx, propq);
        if (ctx == NULL)
            goto err;

        if (!OSSL_DECODER_from_data(ctx, &pm, &pmlen)) {
            ERR_raise(ERR_LIB_CMS, CMS_R_DECODE_ERROR);
            goto err;
        }
        OSSL_DECODER_CTX_free(ctx);
        return pkey;
    } else if (ptype == V_ASN1_OBJECT) {
        const ASN1_OBJECT *poid = pval;
        char groupname[OSSL_MAX_NAME_SIZE];

        /* type == V_ASN1_OBJECT => the parameters are given by an asn1 OID */
        pctx = EVP_PKEY_CTX_new_from_name(libctx, "EC", propq);
        if (pctx == NULL || EVP_PKEY_paramgen_init(pctx) <= 0)
            goto err;
        if (OBJ_obj2txt(groupname, sizeof(groupname), poid, 0) <= 0
                || EVP_PKEY_CTX_set_group_name(pctx, groupname) <= 0) {
            ERR_raise(ERR_LIB_CMS, CMS_R_DECODE_ERROR);
            goto err;
        }
        if (EVP_PKEY_paramgen(pctx, &pkey) <= 0)
            goto err;
        EVP_PKEY_CTX_free(pctx);
        return pkey;
    }

    ERR_raise(ERR_LIB_CMS, CMS_R_DECODE_ERROR);
    return NULL;

 err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(pctx);
    OSSL_DECODER_CTX_free(ctx);
    return NULL;
}

static int ecdh_cms_set_peerkey(EVP_PKEY_CTX *pctx,
                                X509_ALGOR *alg, ASN1_BIT_STRING *pubkey)
{
    const ASN1_OBJECT *aoid;
    int atype;
    const void *aval;
    int rv = 0;
    EVP_PKEY *pkpeer = NULL;
    const unsigned char *p;
    int plen;

    X509_ALGOR_get0(&aoid, &atype, &aval, alg);
    if (OBJ_obj2nid(aoid) != NID_X9_62_id_ecPublicKey)
        goto err;

    /* If absent parameters get group from main key */
    if (atype == V_ASN1_UNDEF || atype == V_ASN1_NULL) {
        EVP_PKEY *pk;

        pk = EVP_PKEY_CTX_get0_pkey(pctx);
        if (pk == NULL)
            goto err;

        pkpeer = EVP_PKEY_new();
        if (pkpeer == NULL)
            goto err;
        if (!EVP_PKEY_copy_parameters(pkpeer, pk))
            goto err;
    } else {
        pkpeer = pkey_type2param(atype, aval,
                                 EVP_PKEY_CTX_get0_libctx(pctx),
                                 EVP_PKEY_CTX_get0_propq(pctx));
        if (pkpeer == NULL)
            goto err;
    }
    /* We have parameters now set public key */
    plen = ASN1_STRING_length(pubkey);
    p = ASN1_STRING_get0_data(pubkey);
    if (p == NULL || plen == 0)
        goto err;

    if (EVP_PKEY_set1_encoded_public_key(pkpeer, p, plen) <= 0)
        goto err;

    if (EVP_PKEY_derive_set_peer(pctx, pkpeer) > 0)
        rv = 1;
 err:
    EVP_PKEY_free(pkpeer);
    return rv;
}

/* Set KDF parameters based on KDF NID */
static int ecdh_cms_set_kdf_param(EVP_PKEY_CTX *pctx, int eckdf_nid)
{
    int kdf_nid, kdfmd_nid, cofactor;
    const EVP_MD *kdf_md;

    if (eckdf_nid == NID_undef)
        return 0;

    /* Lookup KDF type, cofactor mode and digest */
    if (!OBJ_find_sigid_algs(eckdf_nid, &kdfmd_nid, &kdf_nid))
        return 0;

    if (kdf_nid == NID_dh_std_kdf)
        cofactor = 0;
    else if (kdf_nid == NID_dh_cofactor_kdf)
        cofactor = 1;
    else
        return 0;

    if (EVP_PKEY_CTX_set_ecdh_cofactor_mode(pctx, cofactor) <= 0)
        return 0;

    if (EVP_PKEY_CTX_set_ecdh_kdf_type(pctx, EVP_PKEY_ECDH_KDF_X9_63) <= 0)
        return 0;

    kdf_md = EVP_get_digestbynid(kdfmd_nid);
    if (!kdf_md)
        return 0;

    if (EVP_PKEY_CTX_set_ecdh_kdf_md(pctx, kdf_md) <= 0)
        return 0;
    return 1;
}

static int ecdh_cms_set_shared_info(EVP_PKEY_CTX *pctx, CMS_RecipientInfo *ri)
{
    int rv = 0;
    X509_ALGOR *alg, *kekalg = NULL;
    ASN1_OCTET_STRING *ukm;
    const unsigned char *p;
    unsigned char *der = NULL;
    int plen, keylen;
    EVP_CIPHER *kekcipher = NULL;
    EVP_CIPHER_CTX *kekctx;
    char name[OSSL_MAX_NAME_SIZE];

    if (!CMS_RecipientInfo_kari_get0_alg(ri, &alg, &ukm))
        return 0;

    if (!ecdh_cms_set_kdf_param(pctx, OBJ_obj2nid(alg->algorithm))) {
        ERR_raise(ERR_LIB_CMS, CMS_R_KDF_PARAMETER_ERROR);
        return 0;
    }

    if (alg->parameter->type != V_ASN1_SEQUENCE)
        return 0;

    p = alg->parameter->value.sequence->data;
    plen = alg->parameter->value.sequence->length;
    kekalg = d2i_X509_ALGOR(NULL, &p, plen);
    if (kekalg == NULL)
        goto err;
    kekctx = CMS_RecipientInfo_kari_get0_ctx(ri);
    if (kekctx == NULL)
        goto err;
    OBJ_obj2txt(name, sizeof(name), kekalg->algorithm, 0);
    kekcipher = EVP_CIPHER_fetch(pctx->libctx, name, pctx->propquery);
    if (kekcipher == NULL || EVP_CIPHER_get_mode(kekcipher) != EVP_CIPH_WRAP_MODE)
        goto err;
    if (!EVP_EncryptInit_ex(kekctx, kekcipher, NULL, NULL, NULL))
        goto err;
    if (EVP_CIPHER_asn1_to_param(kekctx, kekalg->parameter) <= 0)
        goto err;

    keylen = EVP_CIPHER_CTX_get_key_length(kekctx);
    if (EVP_PKEY_CTX_set_ecdh_kdf_outlen(pctx, keylen) <= 0)
        goto err;

    plen = CMS_SharedInfo_encode(&der, kekalg, ukm, keylen);

    if (plen <= 0)
        goto err;

    if (EVP_PKEY_CTX_set0_ecdh_kdf_ukm(pctx, der, plen) <= 0)
        goto err;
    der = NULL;

    rv = 1;
 err:
    EVP_CIPHER_free(kekcipher);
    X509_ALGOR_free(kekalg);
    OPENSSL_free(der);
    return rv;
}

static int ecdh_cms_decrypt(CMS_RecipientInfo *ri)
{
    EVP_PKEY_CTX *pctx;

    pctx = CMS_RecipientInfo_get0_pkey_ctx(ri);
    if (pctx == NULL)
        return 0;
    /* See if we need to set peer key */
    if (!EVP_PKEY_CTX_get0_peerkey(pctx)) {
        X509_ALGOR *alg;
        ASN1_BIT_STRING *pubkey;

        if (!CMS_RecipientInfo_kari_get0_orig_id(ri, &alg, &pubkey,
                                                 NULL, NULL, NULL))
            return 0;
        if (alg == NULL || pubkey == NULL)
            return 0;
        if (!ecdh_cms_set_peerkey(pctx, alg, pubkey)) {
            ERR_raise(ERR_LIB_CMS, CMS_R_PEER_KEY_ERROR);
            return 0;
        }
    }
    /* Set ECDH derivation parameters and initialise unwrap context */
    if (!ecdh_cms_set_shared_info(pctx, ri)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_SHARED_INFO_ERROR);
        return 0;
    }
    return 1;
}

static int ecdh_cms_encrypt(CMS_RecipientInfo *ri)
{
    EVP_PKEY_CTX *pctx;
    EVP_PKEY *pkey;
    EVP_CIPHER_CTX *ctx;
    int keylen;
    X509_ALGOR *talg, *wrap_alg = NULL;
    const ASN1_OBJECT *aoid;
    ASN1_BIT_STRING *pubkey;
    ASN1_STRING *wrap_str;
    ASN1_OCTET_STRING *ukm;
    unsigned char *penc = NULL;
    int penclen;
    int rv = 0;
    int ecdh_nid, kdf_type, kdf_nid, wrap_nid;
    const EVP_MD *kdf_md;

    pctx = CMS_RecipientInfo_get0_pkey_ctx(ri);
    if (pctx == NULL)
        return 0;
    /* Get ephemeral key */
    pkey = EVP_PKEY_CTX_get0_pkey(pctx);
    if (!CMS_RecipientInfo_kari_get0_orig_id(ri, &talg, &pubkey,
                                             NULL, NULL, NULL))
        goto err;
    X509_ALGOR_get0(&aoid, NULL, NULL, talg);
    /* Is everything uninitialised? */
    if (aoid == OBJ_nid2obj(NID_undef)) {
        /* Set the key */
        size_t enckeylen;

        enckeylen = EVP_PKEY_get1_encoded_public_key(pkey, &penc);
        if (enckeylen > INT_MAX || enckeylen == 0)
            goto err;
        ASN1_STRING_set0(pubkey, penc, (int)enckeylen);
        ossl_asn1_string_set_bits_left(pubkey, 0);

        penc = NULL;
        (void)X509_ALGOR_set0(talg, OBJ_nid2obj(NID_X9_62_id_ecPublicKey),
                              V_ASN1_UNDEF, NULL); /* cannot fail */
    }

    /* See if custom parameters set */
    kdf_type = EVP_PKEY_CTX_get_ecdh_kdf_type(pctx);
    if (kdf_type <= 0)
        goto err;
    if (EVP_PKEY_CTX_get_ecdh_kdf_md(pctx, &kdf_md) <= 0)
        goto err;
    ecdh_nid = EVP_PKEY_CTX_get_ecdh_cofactor_mode(pctx);
    if (ecdh_nid < 0)
        goto err;
    else if (ecdh_nid == 0)
        ecdh_nid = NID_dh_std_kdf;
    else if (ecdh_nid == 1)
        ecdh_nid = NID_dh_cofactor_kdf;

    if (kdf_type == EVP_PKEY_ECDH_KDF_NONE) {
        kdf_type = EVP_PKEY_ECDH_KDF_X9_63;
        if (EVP_PKEY_CTX_set_ecdh_kdf_type(pctx, kdf_type) <= 0)
            goto err;
    } else
        /* Unknown KDF */
        goto err;
    if (kdf_md == NULL) {
        /* Fixme later for better MD */
        kdf_md = EVP_sha1();
        if (EVP_PKEY_CTX_set_ecdh_kdf_md(pctx, kdf_md) <= 0)
            goto err;
    }

    if (!CMS_RecipientInfo_kari_get0_alg(ri, &talg, &ukm))
        goto err;

    /* Lookup NID for KDF+cofactor+digest */

    if (!OBJ_find_sigid_by_algs(&kdf_nid, EVP_MD_get_type(kdf_md), ecdh_nid))
        goto err;
    /* Get wrap NID */
    ctx = CMS_RecipientInfo_kari_get0_ctx(ri);
    wrap_nid = EVP_CIPHER_CTX_get_type(ctx);
    keylen = EVP_CIPHER_CTX_get_key_length(ctx);

    /* Package wrap algorithm in an AlgorithmIdentifier */

    wrap_alg = X509_ALGOR_new();
    if (wrap_alg == NULL)
        goto err;
    wrap_alg->algorithm = OBJ_nid2obj(wrap_nid);
    wrap_alg->parameter = ASN1_TYPE_new();
    if (wrap_alg->parameter == NULL)
        goto err;
    if (EVP_CIPHER_param_to_asn1(ctx, wrap_alg->parameter) <= 0)
        goto err;
    if (ASN1_TYPE_get(wrap_alg->parameter) == NID_undef) {
        ASN1_TYPE_free(wrap_alg->parameter);
        wrap_alg->parameter = NULL;
    }

    if (EVP_PKEY_CTX_set_ecdh_kdf_outlen(pctx, keylen) <= 0)
        goto err;

    penclen = CMS_SharedInfo_encode(&penc, wrap_alg, ukm, keylen);

    if (penclen <= 0)
        goto err;

    if (EVP_PKEY_CTX_set0_ecdh_kdf_ukm(pctx, penc, penclen) <= 0)
        goto err;
    penc = NULL;

    /*
     * Now need to wrap encoding of wrap AlgorithmIdentifier into parameter
     * of another AlgorithmIdentifier.
     */
    penclen = i2d_X509_ALGOR(wrap_alg, &penc);
    if (penclen <= 0)
        goto err;
    wrap_str = ASN1_STRING_new();
    if (wrap_str == NULL)
        goto err;
    ASN1_STRING_set0(wrap_str, penc, penclen);
    penc = NULL;
    rv = X509_ALGOR_set0(talg, OBJ_nid2obj(kdf_nid), V_ASN1_SEQUENCE, wrap_str);
    if (!rv)
        ASN1_STRING_free(wrap_str);

 err:
    OPENSSL_free(penc);
    X509_ALGOR_free(wrap_alg);
    return rv;
}

int ossl_cms_ecdh_envelope(CMS_RecipientInfo *ri, int decrypt)
{
    assert(decrypt == 0 || decrypt == 1);

    if (decrypt == 1)
        return ecdh_cms_decrypt(ri);

    if (decrypt == 0)
        return ecdh_cms_encrypt(ri);

    ERR_raise(ERR_LIB_CMS, CMS_R_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
    return 0;
}
