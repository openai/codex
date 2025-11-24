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
#include <openssl/core.h>
#include <openssl/core_names.h>
#include <openssl/pkcs12.h>
#include "p12_local.h"
#include "crypto/pkcs7/pk7_local.h"

/* Pack an object into an OCTET STRING and turn into a safebag */

PKCS12_SAFEBAG *PKCS12_item_pack_safebag(void *obj, const ASN1_ITEM *it,
                                         int nid1, int nid2)
{
    PKCS12_BAGS *bag;
    PKCS12_SAFEBAG *safebag;

    if ((bag = PKCS12_BAGS_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        return NULL;
    }
    bag->type = OBJ_nid2obj(nid1);
    if (!ASN1_item_pack(obj, it, &bag->value.octet)) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        goto err;
    }
    if ((safebag = PKCS12_SAFEBAG_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        goto err;
    }
    safebag->value.bag = bag;
    safebag->type = OBJ_nid2obj(nid2);
    return safebag;

 err:
    PKCS12_BAGS_free(bag);
    return NULL;
}

/* Turn a stack of SAFEBAGS into a PKCS#7 data Contentinfo */
PKCS7 *PKCS12_pack_p7data(STACK_OF(PKCS12_SAFEBAG) *sk)
{
    PKCS7 *p7;

    if ((p7 = PKCS7_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        return NULL;
    }
    p7->type = OBJ_nid2obj(NID_pkcs7_data);
    if ((p7->d.data = ASN1_OCTET_STRING_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        goto err;
    }

    if (!ASN1_item_pack(sk, ASN1_ITEM_rptr(PKCS12_SAFEBAGS), &p7->d.data)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_CANT_PACK_STRUCTURE);
        goto err;
    }
    return p7;

 err:
    PKCS7_free(p7);
    return NULL;
}

/* Unpack SAFEBAGS from PKCS#7 data ContentInfo */
STACK_OF(PKCS12_SAFEBAG) *PKCS12_unpack_p7data(PKCS7 *p7)
{
    if (!PKCS7_type_is_data(p7)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_CONTENT_TYPE_NOT_DATA);
        return NULL;
    }

    if (p7->d.data == NULL) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_DECODE_ERROR);
        return NULL;
    }

    return ASN1_item_unpack_ex(p7->d.data, ASN1_ITEM_rptr(PKCS12_SAFEBAGS),
                               ossl_pkcs7_ctx_get0_libctx(&p7->ctx),
                               ossl_pkcs7_ctx_get0_propq(&p7->ctx));
}

/* Turn a stack of SAFEBAGS into a PKCS#7 encrypted data ContentInfo */

PKCS7 *PKCS12_pack_p7encdata_ex(int pbe_nid, const char *pass, int passlen,
                                unsigned char *salt, int saltlen, int iter,
                                STACK_OF(PKCS12_SAFEBAG) *bags,
                                OSSL_LIB_CTX *ctx, const char *propq)
{
    PKCS7 *p7;
    X509_ALGOR *pbe;
    const EVP_CIPHER *pbe_ciph = NULL;
    EVP_CIPHER *pbe_ciph_fetch = NULL;

    if ((p7 = PKCS7_new_ex(ctx, propq)) == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        return NULL;
    }
    if (!PKCS7_set_type(p7, NID_pkcs7_encrypted)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_ERROR_SETTING_ENCRYPTED_DATA_TYPE);
        goto err;
    }

    ERR_set_mark();
    pbe_ciph = pbe_ciph_fetch = EVP_CIPHER_fetch(ctx, OBJ_nid2sn(pbe_nid), propq);
    if (pbe_ciph == NULL)
        pbe_ciph = EVP_get_cipherbynid(pbe_nid);
    ERR_pop_to_mark();

    if (pbe_ciph != NULL) {
        pbe = PKCS5_pbe2_set_iv_ex(pbe_ciph, iter, salt, saltlen, NULL, -1, ctx);
    } else {
        pbe = PKCS5_pbe_set_ex(pbe_nid, iter, salt, saltlen, ctx);
    }

    if (pbe == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        goto err;
    }
    X509_ALGOR_free(p7->d.encrypted->enc_data->algorithm);
    p7->d.encrypted->enc_data->algorithm = pbe;
    ASN1_OCTET_STRING_free(p7->d.encrypted->enc_data->enc_data);
    if (!(p7->d.encrypted->enc_data->enc_data =
          PKCS12_item_i2d_encrypt_ex(pbe, ASN1_ITEM_rptr(PKCS12_SAFEBAGS), pass,
                                     passlen, bags, 1, ctx, propq))) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_ENCRYPT_ERROR);
        goto err;
    }

    EVP_CIPHER_free(pbe_ciph_fetch);
    return p7;

 err:
    PKCS7_free(p7);
    EVP_CIPHER_free(pbe_ciph_fetch);
    return NULL;
}

PKCS7 *PKCS12_pack_p7encdata(int pbe_nid, const char *pass, int passlen,
                             unsigned char *salt, int saltlen, int iter,
                             STACK_OF(PKCS12_SAFEBAG) *bags)
{
    return PKCS12_pack_p7encdata_ex(pbe_nid, pass, passlen, salt, saltlen,
                                    iter, bags, NULL, NULL);
}

STACK_OF(PKCS12_SAFEBAG) *PKCS12_unpack_p7encdata(PKCS7 *p7, const char *pass,
                                                  int passlen)
{
    if (!PKCS7_type_is_encrypted(p7))
        return NULL;

    if (p7->d.encrypted == NULL) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_DECODE_ERROR);
        return NULL;
    }

    return PKCS12_item_decrypt_d2i_ex(p7->d.encrypted->enc_data->algorithm,
                                   ASN1_ITEM_rptr(PKCS12_SAFEBAGS),
                                   pass, passlen,
                                   p7->d.encrypted->enc_data->enc_data, 1,
                                   p7->ctx.libctx, p7->ctx.propq);
}

PKCS8_PRIV_KEY_INFO *PKCS12_decrypt_skey_ex(const PKCS12_SAFEBAG *bag,
                                            const char *pass, int passlen,
                                            OSSL_LIB_CTX *ctx, const char *propq)
{
    return PKCS8_decrypt_ex(bag->value.shkeybag, pass, passlen, ctx, propq);
}

PKCS8_PRIV_KEY_INFO *PKCS12_decrypt_skey(const PKCS12_SAFEBAG *bag,
                                         const char *pass, int passlen)
{
    return PKCS12_decrypt_skey_ex(bag, pass, passlen, NULL, NULL);
}

int PKCS12_pack_authsafes(PKCS12 *p12, STACK_OF(PKCS7) *safes)
{
    if (ASN1_item_pack(safes, ASN1_ITEM_rptr(PKCS12_AUTHSAFES),
                       &p12->authsafes->d.data))
        return 1;
    return 0;
}

STACK_OF(PKCS7) *PKCS12_unpack_authsafes(const PKCS12 *p12)
{
    STACK_OF(PKCS7) *p7s;
    PKCS7_CTX *p7ctx;
    PKCS7 *p7;
    int i;

    if (!PKCS7_type_is_data(p12->authsafes)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_CONTENT_TYPE_NOT_DATA);
        return NULL;
    }

    if (p12->authsafes->d.data == NULL) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_DECODE_ERROR);
        return NULL;
    }

    p7ctx = &p12->authsafes->ctx;
    p7s = ASN1_item_unpack_ex(p12->authsafes->d.data,
                              ASN1_ITEM_rptr(PKCS12_AUTHSAFES),
                              ossl_pkcs7_ctx_get0_libctx(p7ctx),
                              ossl_pkcs7_ctx_get0_propq(p7ctx));
    if (p7s != NULL) {
        for (i = 0; i < sk_PKCS7_num(p7s); i++) {
            p7 = sk_PKCS7_value(p7s, i);
            if (!ossl_pkcs7_ctx_propagate(p12->authsafes, p7))
                goto err;
        }
    }
    return p7s;
err:
    sk_PKCS7_free(p7s);
    return NULL;
}
