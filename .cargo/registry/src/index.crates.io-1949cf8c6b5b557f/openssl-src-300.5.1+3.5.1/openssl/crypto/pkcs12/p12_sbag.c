/*
 * Copyright 1999-2023 The OpenSSL Project Authors. All Rights Reserved.
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
#include "crypto/x509.h"

#ifndef OPENSSL_NO_DEPRECATED_1_1_0
ASN1_TYPE *PKCS12_get_attr(const PKCS12_SAFEBAG *bag, int attr_nid)
{
    return PKCS12_get_attr_gen(bag->attrib, attr_nid);
}
#endif

const ASN1_TYPE *PKCS12_SAFEBAG_get0_attr(const PKCS12_SAFEBAG *bag,
                                          int attr_nid)
{
    return PKCS12_get_attr_gen(bag->attrib, attr_nid);
}

ASN1_TYPE *PKCS8_get_attr(PKCS8_PRIV_KEY_INFO *p8, int attr_nid)
{
    return PKCS12_get_attr_gen(PKCS8_pkey_get0_attrs(p8), attr_nid);
}

const PKCS8_PRIV_KEY_INFO *PKCS12_SAFEBAG_get0_p8inf(const PKCS12_SAFEBAG *bag)
{
    if (PKCS12_SAFEBAG_get_nid(bag) != NID_keyBag)
        return NULL;
    return bag->value.keybag;
}

const X509_SIG *PKCS12_SAFEBAG_get0_pkcs8(const PKCS12_SAFEBAG *bag)
{
    if (OBJ_obj2nid(bag->type) != NID_pkcs8ShroudedKeyBag)
        return NULL;
    return bag->value.shkeybag;
}

const STACK_OF(PKCS12_SAFEBAG) *
PKCS12_SAFEBAG_get0_safes(const PKCS12_SAFEBAG *bag)
{
    if (OBJ_obj2nid(bag->type) != NID_safeContentsBag)
        return NULL;
    return bag->value.safes;
}

const ASN1_OBJECT *PKCS12_SAFEBAG_get0_type(const PKCS12_SAFEBAG *bag)
{
    return bag->type;
}

int PKCS12_SAFEBAG_get_nid(const PKCS12_SAFEBAG *bag)
{
    return OBJ_obj2nid(bag->type);
}

int PKCS12_SAFEBAG_get_bag_nid(const PKCS12_SAFEBAG *bag)
{
    int btype = PKCS12_SAFEBAG_get_nid(bag);

    if (btype != NID_certBag && btype != NID_crlBag && btype != NID_secretBag)
        return -1;
    return OBJ_obj2nid(bag->value.bag->type);
}

const ASN1_OBJECT *PKCS12_SAFEBAG_get0_bag_type(const PKCS12_SAFEBAG *bag)
{
    return bag->value.bag->type;
}

const ASN1_TYPE *PKCS12_SAFEBAG_get0_bag_obj(const PKCS12_SAFEBAG *bag)
{
    return bag->value.bag->value.other;
}

X509 *PKCS12_SAFEBAG_get1_cert(const PKCS12_SAFEBAG *bag)
{
    if (PKCS12_SAFEBAG_get_nid(bag) != NID_certBag)
        return NULL;
    if (OBJ_obj2nid(bag->value.bag->type) != NID_x509Certificate)
        return NULL;
    return ASN1_item_unpack(bag->value.bag->value.octet,
                            ASN1_ITEM_rptr(X509));
}

X509_CRL *PKCS12_SAFEBAG_get1_crl(const PKCS12_SAFEBAG *bag)
{
    if (PKCS12_SAFEBAG_get_nid(bag) != NID_crlBag)
        return NULL;
    if (OBJ_obj2nid(bag->value.bag->type) != NID_x509Crl)
        return NULL;
    return ASN1_item_unpack(bag->value.bag->value.octet,
                            ASN1_ITEM_rptr(X509_CRL));
}

X509 *PKCS12_SAFEBAG_get1_cert_ex(const PKCS12_SAFEBAG *bag,
                                  OSSL_LIB_CTX *libctx, const char *propq)
{
    X509 *ret = NULL;

    if (PKCS12_SAFEBAG_get_nid(bag) != NID_certBag)
        return NULL;
    if (OBJ_obj2nid(bag->value.bag->type) != NID_x509Certificate)
        return NULL;
    ret = ASN1_item_unpack_ex(bag->value.bag->value.octet,
                              ASN1_ITEM_rptr(X509), libctx, propq);
    if (!ossl_x509_set0_libctx(ret, libctx, propq)) {
        X509_free(ret);
        return NULL;
    }
    return ret;
}

X509_CRL *PKCS12_SAFEBAG_get1_crl_ex(const PKCS12_SAFEBAG *bag,
                                     OSSL_LIB_CTX *libctx, const char *propq)
{
    X509_CRL *ret = NULL;

    if (PKCS12_SAFEBAG_get_nid(bag) != NID_crlBag)
        return NULL;
    if (OBJ_obj2nid(bag->value.bag->type) != NID_x509Crl)
        return NULL;
    ret = ASN1_item_unpack_ex(bag->value.bag->value.octet,
                              ASN1_ITEM_rptr(X509_CRL), libctx, propq);
    if (!ossl_x509_crl_set0_libctx(ret, libctx, propq)) {
        X509_CRL_free(ret);
        return NULL;
    }
    return ret;
}

PKCS12_SAFEBAG *PKCS12_SAFEBAG_create_cert(X509 *x509)
{
    return PKCS12_item_pack_safebag(x509, ASN1_ITEM_rptr(X509),
                                    NID_x509Certificate, NID_certBag);
}

PKCS12_SAFEBAG *PKCS12_SAFEBAG_create_crl(X509_CRL *crl)
{
    return PKCS12_item_pack_safebag(crl, ASN1_ITEM_rptr(X509_CRL),
                                    NID_x509Crl, NID_crlBag);
}

PKCS12_SAFEBAG *PKCS12_SAFEBAG_create_secret(int type, int vtype, const unsigned char *value, int len)
{
    PKCS12_BAGS *bag;
    PKCS12_SAFEBAG *safebag;

    if ((bag = PKCS12_BAGS_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        return NULL;
    }
    bag->type = OBJ_nid2obj(type);

    switch (vtype) {
    case V_ASN1_OCTET_STRING:
        {
            ASN1_OCTET_STRING *strtmp = ASN1_OCTET_STRING_new();

            if (strtmp == NULL) {
                ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
                goto err;
            }
            /* Pack data into an octet string */
            if (!ASN1_OCTET_STRING_set(strtmp, value, len)) {
                ASN1_OCTET_STRING_free(strtmp);
                ERR_raise(ERR_LIB_PKCS12, PKCS12_R_ENCODE_ERROR);
                goto err;
            }
            bag->value.other = ASN1_TYPE_new();
            if (bag->value.other == NULL) {
                ASN1_OCTET_STRING_free(strtmp);
                ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
                goto err;
            }
            ASN1_TYPE_set(bag->value.other, vtype, strtmp);
        }
        break;

    default:
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_INVALID_TYPE);
        goto err;
    }

    if ((safebag = PKCS12_SAFEBAG_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        goto err;
    }
    safebag->value.bag = bag;
    safebag->type = OBJ_nid2obj(NID_secretBag);
    return safebag;

 err:
    PKCS12_BAGS_free(bag);
    return NULL;
}

/* Turn PKCS8 object into a keybag */

PKCS12_SAFEBAG *PKCS12_SAFEBAG_create0_p8inf(PKCS8_PRIV_KEY_INFO *p8)
{
    PKCS12_SAFEBAG *bag = PKCS12_SAFEBAG_new();

    if (bag == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        return NULL;
    }
    bag->type = OBJ_nid2obj(NID_keyBag);
    bag->value.keybag = p8;
    return bag;
}

/* Turn PKCS8 object into a shrouded keybag */

PKCS12_SAFEBAG *PKCS12_SAFEBAG_create0_pkcs8(X509_SIG *p8)
{
    PKCS12_SAFEBAG *bag = PKCS12_SAFEBAG_new();

    /* Set up the safe bag */
    if (bag == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        return NULL;
    }
    bag->type = OBJ_nid2obj(NID_pkcs8ShroudedKeyBag);
    bag->value.shkeybag = p8;
    return bag;
}

PKCS12_SAFEBAG *PKCS12_SAFEBAG_create_pkcs8_encrypt_ex(int pbe_nid,
                                                       const char *pass,
                                                       int passlen,
                                                       unsigned char *salt,
                                                       int saltlen, int iter,
                                                       PKCS8_PRIV_KEY_INFO *p8inf,
                                                       OSSL_LIB_CTX *ctx,
                                                       const char *propq)
{
    PKCS12_SAFEBAG *bag = NULL;
    const EVP_CIPHER *pbe_ciph = NULL;
    EVP_CIPHER *pbe_ciph_fetch = NULL;
    X509_SIG *p8;

    ERR_set_mark();
    pbe_ciph = pbe_ciph_fetch = EVP_CIPHER_fetch(ctx, OBJ_nid2sn(pbe_nid), propq);
    if (pbe_ciph == NULL)
        pbe_ciph = EVP_get_cipherbynid(pbe_nid);
    ERR_pop_to_mark();

    if (pbe_ciph != NULL)
        pbe_nid = -1;

    p8 = PKCS8_encrypt_ex(pbe_nid, pbe_ciph, pass, passlen, salt, saltlen, iter,
                          p8inf, ctx, propq);
    if (p8 == NULL)
        goto err;

    bag = PKCS12_SAFEBAG_create0_pkcs8(p8);
    if (bag == NULL)
        X509_SIG_free(p8);

err:
    EVP_CIPHER_free(pbe_ciph_fetch);
    return bag;
}

PKCS12_SAFEBAG *PKCS12_SAFEBAG_create_pkcs8_encrypt(int pbe_nid,
                                                    const char *pass,
                                                    int passlen,
                                                    unsigned char *salt,
                                                    int saltlen, int iter,
                                                    PKCS8_PRIV_KEY_INFO *p8inf)
{
    return PKCS12_SAFEBAG_create_pkcs8_encrypt_ex(pbe_nid, pass, passlen,
                                                  salt, saltlen, iter, p8inf,
                                                  NULL, NULL);
}
