/*
 * Copyright 2006-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <openssl/cms.h>
#include <openssl/dh.h>
#include <openssl/err.h>
#include <openssl/core_names.h>
#include "internal/sizes.h"
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "cms_local.h"

static int dh_cms_set_peerkey(EVP_PKEY_CTX *pctx,
                              X509_ALGOR *alg, ASN1_BIT_STRING *pubkey)
{
    const ASN1_OBJECT *aoid;
    int atype;
    const void *aval;
    ASN1_INTEGER *public_key = NULL;
    int rv = 0;
    EVP_PKEY *pkpeer = NULL, *pk = NULL;
    BIGNUM *bnpub = NULL;
    const unsigned char *p;
    unsigned char *buf = NULL;
    int plen;

    X509_ALGOR_get0(&aoid, &atype, &aval, alg);
    if (OBJ_obj2nid(aoid) != NID_dhpublicnumber)
        goto err;
    /* Only absent parameters allowed in RFC XXXX */
    if (atype != V_ASN1_UNDEF && atype != V_ASN1_NULL)
        goto err;

    pk = EVP_PKEY_CTX_get0_pkey(pctx);
    if (pk == NULL || !EVP_PKEY_is_a(pk, "DHX"))
        goto err;

    /* Get public key */
    plen = ASN1_STRING_length(pubkey);
    p = ASN1_STRING_get0_data(pubkey);
    if (p == NULL || plen == 0)
        goto err;

    if ((public_key = d2i_ASN1_INTEGER(NULL, &p, plen)) == NULL)
        goto err;
    /*
     * Pad to full p parameter size as that is checked by
     * EVP_PKEY_set1_encoded_public_key()
     */
    plen = EVP_PKEY_get_size(pk);
    if ((bnpub = ASN1_INTEGER_to_BN(public_key, NULL)) == NULL)
        goto err;
    if ((buf = OPENSSL_malloc(plen)) == NULL)
        goto err;
    if (BN_bn2binpad(bnpub, buf, plen) < 0)
        goto err;

    pkpeer = EVP_PKEY_new();
    if (pkpeer == NULL
            || !EVP_PKEY_copy_parameters(pkpeer, pk)
            || EVP_PKEY_set1_encoded_public_key(pkpeer, buf, plen) <= 0)
        goto err;

    if (EVP_PKEY_derive_set_peer(pctx, pkpeer) > 0)
        rv = 1;
 err:
    ASN1_INTEGER_free(public_key);
    BN_free(bnpub);
    OPENSSL_free(buf);
    EVP_PKEY_free(pkpeer);
    return rv;
}

static int dh_cms_set_shared_info(EVP_PKEY_CTX *pctx, CMS_RecipientInfo *ri)
{
    int rv = 0;
    X509_ALGOR *alg, *kekalg = NULL;
    ASN1_OCTET_STRING *ukm;
    const unsigned char *p;
    unsigned char *dukm = NULL;
    size_t dukmlen = 0;
    int keylen, plen;
    EVP_CIPHER *kekcipher = NULL;
    EVP_CIPHER_CTX *kekctx;
    char name[OSSL_MAX_NAME_SIZE];

    if (!CMS_RecipientInfo_kari_get0_alg(ri, &alg, &ukm))
        goto err;

    /*
     * For DH we only have one OID permissible. If ever any more get defined
     * we will need something cleverer.
     */
    if (OBJ_obj2nid(alg->algorithm) != NID_id_smime_alg_ESDH) {
        ERR_raise(ERR_LIB_CMS, CMS_R_KDF_PARAMETER_ERROR);
        goto err;
    }

    if (EVP_PKEY_CTX_set_dh_kdf_type(pctx, EVP_PKEY_DH_KDF_X9_42) <= 0
            || EVP_PKEY_CTX_set_dh_kdf_md(pctx, EVP_sha1()) <= 0)
        goto err;

    if (alg->parameter->type != V_ASN1_SEQUENCE)
        goto err;

    p = alg->parameter->value.sequence->data;
    plen = alg->parameter->value.sequence->length;
    kekalg = d2i_X509_ALGOR(NULL, &p, plen);
    if (kekalg == NULL)
        goto err;
    kekctx = CMS_RecipientInfo_kari_get0_ctx(ri);
    if (kekctx == NULL)
        goto err;

    if (OBJ_obj2txt(name, sizeof(name), kekalg->algorithm, 0) <= 0)
        goto err;

    kekcipher = EVP_CIPHER_fetch(pctx->libctx, name, pctx->propquery);
    if (kekcipher == NULL
        || EVP_CIPHER_get_mode(kekcipher) != EVP_CIPH_WRAP_MODE)
        goto err;
    if (!EVP_EncryptInit_ex(kekctx, kekcipher, NULL, NULL, NULL))
        goto err;
    if (EVP_CIPHER_asn1_to_param(kekctx, kekalg->parameter) <= 0)
        goto err;

    keylen = EVP_CIPHER_CTX_get_key_length(kekctx);
    if (EVP_PKEY_CTX_set_dh_kdf_outlen(pctx, keylen) <= 0)
        goto err;
    /* Use OBJ_nid2obj to ensure we use built in OID that isn't freed */
    if (EVP_PKEY_CTX_set0_dh_kdf_oid(pctx,
                                     OBJ_nid2obj(EVP_CIPHER_get_type(kekcipher)))
        <= 0)
        goto err;

    if (ukm != NULL) {
        dukmlen = ASN1_STRING_length(ukm);
        dukm = OPENSSL_memdup(ASN1_STRING_get0_data(ukm), dukmlen);
        if (dukm == NULL)
            goto err;
    }

    if (EVP_PKEY_CTX_set0_dh_kdf_ukm(pctx, dukm, dukmlen) <= 0)
        goto err;
    dukm = NULL;

    rv = 1;
 err:
    X509_ALGOR_free(kekalg);
    EVP_CIPHER_free(kekcipher);
    OPENSSL_free(dukm);
    return rv;
}

static int dh_cms_decrypt(CMS_RecipientInfo *ri)
{
    EVP_PKEY_CTX *pctx = CMS_RecipientInfo_get0_pkey_ctx(ri);

    if (pctx == NULL)
        return 0;
    /* See if we need to set peer key */
    if (!EVP_PKEY_CTX_get0_peerkey(pctx)) {
        X509_ALGOR *alg;
        ASN1_BIT_STRING *pubkey;

        if (!CMS_RecipientInfo_kari_get0_orig_id(ri, &alg, &pubkey,
                                                 NULL, NULL, NULL))
            return 0;
        if (alg ==  NULL || pubkey == NULL)
            return 0;
        if (!dh_cms_set_peerkey(pctx, alg, pubkey)) {
            ERR_raise(ERR_LIB_CMS, CMS_R_PEER_KEY_ERROR);
            return 0;
        }
    }
    /* Set DH derivation parameters and initialise unwrap context */
    if (!dh_cms_set_shared_info(pctx, ri)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_SHARED_INFO_ERROR);
        return 0;
    }
    return 1;
}

static int dh_cms_encrypt(CMS_RecipientInfo *ri)
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
    unsigned char *penc = NULL, *dukm = NULL;
    int penclen;
    size_t dukmlen = 0;
    int rv = 0;
    int kdf_type, wrap_nid;
    const EVP_MD *kdf_md;

    pctx = CMS_RecipientInfo_get0_pkey_ctx(ri);
    if (pctx == NULL)
        return 0;
    /* Get ephemeral key */
    pkey = EVP_PKEY_CTX_get0_pkey(pctx);
    if (!CMS_RecipientInfo_kari_get0_orig_id(ri, &talg, &pubkey,
                                             NULL, NULL, NULL))
        goto err;

    /* Is everything uninitialised? */
    X509_ALGOR_get0(&aoid, NULL, NULL, talg);
    if (aoid == OBJ_nid2obj(NID_undef)) {
        BIGNUM *bn_pub_key = NULL;
        ASN1_INTEGER *pubk;

        if (!EVP_PKEY_get_bn_param(pkey, OSSL_PKEY_PARAM_PUB_KEY, &bn_pub_key))
            goto err;

        pubk = BN_to_ASN1_INTEGER(bn_pub_key, NULL);
        BN_free(bn_pub_key);
        if (pubk == NULL)
            goto err;

        /* Set the key */
        penclen = i2d_ASN1_INTEGER(pubk, &penc);
        ASN1_INTEGER_free(pubk);
        if (penclen <= 0)
            goto err;
        ASN1_STRING_set0(pubkey, penc, penclen);
        ossl_asn1_string_set_bits_left(pubkey, 0);

        penc = NULL;
        (void)X509_ALGOR_set0(talg, OBJ_nid2obj(NID_dhpublicnumber),
                              V_ASN1_UNDEF, NULL); /* cannot fail */
    }

    /* See if custom parameters set */
    kdf_type = EVP_PKEY_CTX_get_dh_kdf_type(pctx);
    if (kdf_type <= 0 || EVP_PKEY_CTX_get_dh_kdf_md(pctx, &kdf_md) <= 0)
        goto err;

    if (kdf_type == EVP_PKEY_DH_KDF_NONE) {
        kdf_type = EVP_PKEY_DH_KDF_X9_42;
        if (EVP_PKEY_CTX_set_dh_kdf_type(pctx, kdf_type) <= 0)
            goto err;
    } else if (kdf_type != EVP_PKEY_DH_KDF_X9_42)
        /* Unknown KDF */
        goto err;
    if (kdf_md == NULL) {
        /* Only SHA1 supported */
        kdf_md = EVP_sha1();
        if (EVP_PKEY_CTX_set_dh_kdf_md(pctx, kdf_md) <= 0)
            goto err;
    } else if (EVP_MD_get_type(kdf_md) != NID_sha1)
        /* Unsupported digest */
        goto err;

    if (!CMS_RecipientInfo_kari_get0_alg(ri, &talg, &ukm))
        goto err;

    /* Get wrap NID */
    ctx = CMS_RecipientInfo_kari_get0_ctx(ri);
    wrap_nid = EVP_CIPHER_CTX_get_type(ctx);
    if (EVP_PKEY_CTX_set0_dh_kdf_oid(pctx, OBJ_nid2obj(wrap_nid)) <= 0)
        goto err;
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

    if (EVP_PKEY_CTX_set_dh_kdf_outlen(pctx, keylen) <= 0)
        goto err;

    if (ukm != NULL) {
        dukmlen = ASN1_STRING_length(ukm);
        dukm = OPENSSL_memdup(ASN1_STRING_get0_data(ukm), dukmlen);
        if (dukm == NULL)
            goto err;
    }

    if (EVP_PKEY_CTX_set0_dh_kdf_ukm(pctx, dukm, dukmlen) <= 0)
        goto err;
    dukm = NULL;

    /*
     * Now need to wrap encoding of wrap AlgorithmIdentifier into parameter
     * of another AlgorithmIdentifier.
     */
    penc = NULL;
    penclen = i2d_X509_ALGOR(wrap_alg, &penc);
    if (penclen <= 0)
        goto err;
    wrap_str = ASN1_STRING_new();
    if (wrap_str == NULL)
        goto err;
    ASN1_STRING_set0(wrap_str, penc, penclen);
    penc = NULL;
    rv = X509_ALGOR_set0(talg, OBJ_nid2obj(NID_id_smime_alg_ESDH),
                         V_ASN1_SEQUENCE, wrap_str);
    if (!rv)
        ASN1_STRING_free(wrap_str);

 err:
    OPENSSL_free(penc);
    X509_ALGOR_free(wrap_alg);
    OPENSSL_free(dukm);
    return rv;
}

int ossl_cms_dh_envelope(CMS_RecipientInfo *ri, int decrypt)
{
    assert(decrypt == 0 || decrypt == 1);

    if (decrypt == 1)
        return dh_cms_decrypt(ri);

    if (decrypt == 0)
        return dh_cms_encrypt(ri);

    ERR_raise(ERR_LIB_CMS, CMS_R_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
    return 0;
}
