/*
 * Copyright 1999-2025 The OpenSSL Project Authors. All Rights Reserved.
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

static int pkcs12_add_bag(STACK_OF(PKCS12_SAFEBAG) **pbags,
                          PKCS12_SAFEBAG *bag);
static int pkcs12_remove_bag(STACK_OF(PKCS12_SAFEBAG) **pbags,
                             PKCS12_SAFEBAG *bag);
static PKCS12_SAFEBAG *pkcs12_add_cert_bag(STACK_OF(PKCS12_SAFEBAG) **pbags,
                                           X509 *cert,
                                           const char *name,
                                           int namelen,
                                           unsigned char *keyid,
                                           int keyidlen);

static int copy_bag_attr(PKCS12_SAFEBAG *bag, EVP_PKEY *pkey, int nid)
{
    int idx = EVP_PKEY_get_attr_by_NID(pkey, nid, -1);

    if (idx < 0)
        return 1;
    return X509at_add1_attr(&bag->attrib, EVP_PKEY_get_attr(pkey, idx)) != NULL;
}

PKCS12 *PKCS12_create_ex2(const char *pass, const char *name, EVP_PKEY *pkey,
                          X509 *cert, STACK_OF(X509) *ca, int nid_key, int nid_cert,
                          int iter, int mac_iter, int keytype,
                          OSSL_LIB_CTX *ctx, const char *propq,
                          PKCS12_create_cb *cb, void *cbarg)
{
    PKCS12 *p12 = NULL;
    STACK_OF(PKCS7) *safes = NULL;
    STACK_OF(PKCS12_SAFEBAG) *bags = NULL;
    PKCS12_SAFEBAG *bag = NULL;
    int i, cbret;
    unsigned char keyid[EVP_MAX_MD_SIZE];
    unsigned int keyidlen = 0;
    int namelen = -1;
    unsigned char *pkeyid = NULL;
    int pkeyidlen = -1;

    /* Set defaults */
    if (nid_cert == NID_undef)
        nid_cert = NID_aes_256_cbc;
    if (nid_key == NID_undef)
        nid_key = NID_aes_256_cbc;
    if (!iter)
        iter = PKCS12_DEFAULT_ITER;
    if (!mac_iter)
        mac_iter = PKCS12_DEFAULT_ITER;

    if (pkey == NULL && cert == NULL && ca == NULL) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_INVALID_NULL_ARGUMENT);
        return NULL;
    }

    if (pkey && cert) {
        if (!X509_check_private_key(cert, pkey))
            return NULL;
        if (!X509_digest(cert, EVP_sha1(), keyid, &keyidlen))
            return NULL;
    }

    if (cert) {
        if (name == NULL)
            name = (char *)X509_alias_get0(cert, &namelen);
        if (keyidlen > 0) {
            pkeyid = keyid;
            pkeyidlen = keyidlen;
        } else {
            pkeyid = X509_keyid_get0(cert, &pkeyidlen);
        }

        bag = pkcs12_add_cert_bag(&bags, cert, name, namelen, pkeyid, pkeyidlen);
        if (cb != NULL) {
            cbret = cb(bag, cbarg);
            if (cbret  == -1) {
                ERR_raise(ERR_LIB_PKCS12, PKCS12_R_CALLBACK_FAILED);
                goto err;
            } else if (cbret == 0) {
                pkcs12_remove_bag(&bags, bag);
            }
        }
    }

    /* Add all other certificates */
    for (i = 0; i < sk_X509_num(ca); i++) {
        if ((bag = PKCS12_add_cert(&bags, sk_X509_value(ca, i))) == NULL)
            goto err;
        if (cb != NULL) {
            cbret = cb(bag, cbarg);
            if (cbret  == -1) {
                ERR_raise(ERR_LIB_PKCS12, PKCS12_R_CALLBACK_FAILED);
                goto err;
            } else if (cbret == 0) {
                pkcs12_remove_bag(&bags, bag);
            }
        }
    }

    if (bags && !PKCS12_add_safe_ex(&safes, bags, nid_cert, iter, pass,
                                    ctx, propq))
        goto err;

    sk_PKCS12_SAFEBAG_pop_free(bags, PKCS12_SAFEBAG_free);
    bags = NULL;

    if (pkey) {
        bag = PKCS12_add_key_ex(&bags, pkey, keytype, iter, nid_key, pass,
                                ctx, propq);

        if (!bag)
            goto err;

        if (!copy_bag_attr(bag, pkey, NID_ms_csp_name))
            goto err;
        if (!copy_bag_attr(bag, pkey, NID_LocalKeySet))
            goto err;

        if (name && !PKCS12_add_friendlyname(bag, name, -1))
            goto err;
        if (keyidlen && !PKCS12_add_localkeyid(bag, keyid, keyidlen))
            goto err;
        if (cb != NULL) {
            cbret = cb(bag, cbarg);
            if (cbret  == -1) {
                ERR_raise(ERR_LIB_PKCS12, PKCS12_R_CALLBACK_FAILED);
                goto err;
            } else if (cbret == 0) {
                pkcs12_remove_bag(&bags, bag);
            }
        }
    }

    if (bags && !PKCS12_add_safe(&safes, bags, -1, 0, NULL))
        goto err;

    sk_PKCS12_SAFEBAG_pop_free(bags, PKCS12_SAFEBAG_free);
    bags = NULL;

    p12 = PKCS12_add_safes_ex(safes, 0, ctx, propq);

    if (p12 == NULL)
        goto err;

    sk_PKCS7_pop_free(safes, PKCS7_free);

    safes = NULL;

    if ((mac_iter != -1) &&
        !PKCS12_set_mac(p12, pass, -1, NULL, 0, mac_iter, NULL))
        goto err;

    return p12;

 err:
    PKCS12_free(p12);
    sk_PKCS7_pop_free(safes, PKCS7_free);
    sk_PKCS12_SAFEBAG_pop_free(bags, PKCS12_SAFEBAG_free);
    return NULL;

}

PKCS12 *PKCS12_create_ex(const char *pass, const char *name, EVP_PKEY *pkey, X509 *cert,
                        STACK_OF(X509) *ca, int nid_key, int nid_cert, int iter,
                        int mac_iter, int keytype,
                        OSSL_LIB_CTX *ctx, const char *propq)
{
    return PKCS12_create_ex2(pass, name, pkey, cert, ca, nid_key, nid_cert,
                             iter, mac_iter, keytype, ctx, propq,
                             NULL, NULL);
}

PKCS12 *PKCS12_create(const char *pass, const char *name, EVP_PKEY *pkey, X509 *cert,
                      STACK_OF(X509) *ca, int nid_key, int nid_cert, int iter,
                      int mac_iter, int keytype)
{
    return PKCS12_create_ex(pass, name, pkey, cert, ca, nid_key, nid_cert,
                            iter, mac_iter, keytype, NULL, NULL);
}

static PKCS12_SAFEBAG *pkcs12_add_cert_bag(STACK_OF(PKCS12_SAFEBAG) **pbags,
                                           X509 *cert,
                                           const char *name,
                                           int namelen,
                                           unsigned char *keyid,
                                           int keyidlen)
{
    PKCS12_SAFEBAG *bag = NULL;

    /* Add user certificate */
    if ((bag = PKCS12_SAFEBAG_create_cert(cert)) == NULL)
        goto err;

    if (name != NULL && !PKCS12_add_friendlyname(bag, name, namelen))
        goto err;

    if (keyid != NULL && !PKCS12_add_localkeyid(bag, keyid, keyidlen))
        goto err;

    if (!pkcs12_add_bag(pbags, bag))
        goto err;

    return bag;

 err:
    PKCS12_SAFEBAG_free(bag);
    return NULL;
}

PKCS12_SAFEBAG *PKCS12_add_cert(STACK_OF(PKCS12_SAFEBAG) **pbags, X509 *cert)
{
    char *name = NULL;
    int namelen = -1;
    unsigned char *keyid = NULL;
    int keyidlen = -1;

    /*
     * Use friendlyName and localKeyID in certificate. (if present)
     */
    name = (char *)X509_alias_get0(cert, &namelen);
    keyid = X509_keyid_get0(cert, &keyidlen);

    return pkcs12_add_cert_bag(pbags, cert, name, namelen, keyid, keyidlen);
}

PKCS12_SAFEBAG *PKCS12_add_key_ex(STACK_OF(PKCS12_SAFEBAG) **pbags,
                                  EVP_PKEY *key, int key_usage, int iter,
                                  int nid_key, const char *pass,
                                  OSSL_LIB_CTX *ctx, const char *propq)
{

    PKCS12_SAFEBAG *bag = NULL;
    PKCS8_PRIV_KEY_INFO *p8 = NULL;

    /* Make a PKCS#8 structure */
    if ((p8 = EVP_PKEY2PKCS8(key)) == NULL)
        goto err;
    if (key_usage && !PKCS8_add_keyusage(p8, key_usage)) {
        PKCS8_PRIV_KEY_INFO_free(p8);
        goto err;
    }
    if (nid_key != -1) {
        /* This call does not take ownership of p8 */
        bag = PKCS12_SAFEBAG_create_pkcs8_encrypt_ex(nid_key, pass, -1, NULL, 0,
                                                     iter, p8, ctx, propq);
    } else {
        bag = PKCS12_SAFEBAG_create0_p8inf(p8);
        if (bag != NULL)
           p8 = NULL; /* bag takes ownership of p8 */
    }
    /* This does not need to be in the error path */
    if (p8 != NULL)
        PKCS8_PRIV_KEY_INFO_free(p8);

    if (bag == NULL || !pkcs12_add_bag(pbags, bag))
        goto err;

    return bag;

 err:
    PKCS12_SAFEBAG_free(bag);
    return NULL;

}

PKCS12_SAFEBAG *PKCS12_add_key(STACK_OF(PKCS12_SAFEBAG) **pbags,
                               EVP_PKEY *key, int key_usage, int iter,
                               int nid_key, const char *pass)
{
    return PKCS12_add_key_ex(pbags, key, key_usage, iter, nid_key, pass,
                             NULL, NULL);
}

PKCS12_SAFEBAG *PKCS12_add_secret(STACK_OF(PKCS12_SAFEBAG) **pbags,
                                  int nid_type, const unsigned char *value, int len)
{
    PKCS12_SAFEBAG *bag = NULL;

    /* Add secret, storing the value as an octet string */
    if ((bag = PKCS12_SAFEBAG_create_secret(nid_type, V_ASN1_OCTET_STRING, value, len)) == NULL)
        goto err;

    if (!pkcs12_add_bag(pbags, bag))
        goto err;

    return bag;
 err:
    PKCS12_SAFEBAG_free(bag);
    return NULL;
}

int PKCS12_add_safe_ex(STACK_OF(PKCS7) **psafes, STACK_OF(PKCS12_SAFEBAG) *bags,
                       int nid_safe, int iter, const char *pass,
                       OSSL_LIB_CTX *ctx, const char *propq)
{
    PKCS7 *p7 = NULL;
    int free_safes = 0;

    if (*psafes == NULL) {
        *psafes = sk_PKCS7_new_null();
        if (*psafes == NULL)
            return 0;
        free_safes = 1;
    }

    if (nid_safe == 0)
#ifdef OPENSSL_NO_RC2
        nid_safe = NID_pbe_WithSHA1And3_Key_TripleDES_CBC;
#else
        nid_safe = NID_pbe_WithSHA1And40BitRC2_CBC;
#endif

    if (nid_safe == -1)
        p7 = PKCS12_pack_p7data(bags);
    else
        p7 = PKCS12_pack_p7encdata_ex(nid_safe, pass, -1, NULL, 0, iter, bags, ctx, propq);
    if (p7 == NULL)
        goto err;

    if (!sk_PKCS7_push(*psafes, p7))
        goto err;

    return 1;

 err:
    if (free_safes) {
        sk_PKCS7_free(*psafes);
        *psafes = NULL;
    }
    PKCS7_free(p7);
    return 0;
}

int PKCS12_add_safe(STACK_OF(PKCS7) **psafes, STACK_OF(PKCS12_SAFEBAG) *bags,
                    int nid_safe, int iter, const char *pass)
{
    return PKCS12_add_safe_ex(psafes, bags, nid_safe, iter, pass, NULL, NULL);
}


static int pkcs12_remove_bag(STACK_OF(PKCS12_SAFEBAG) **pbags,
                             PKCS12_SAFEBAG *bag)
{
    PKCS12_SAFEBAG *tmp;

    if (pbags == NULL || bag == NULL)
        return 1;

    if ((tmp = sk_PKCS12_SAFEBAG_delete_ptr(*pbags, bag)) == NULL)
        return 0;

    PKCS12_SAFEBAG_free(tmp);
    return 1;
}

static int pkcs12_add_bag(STACK_OF(PKCS12_SAFEBAG) **pbags,
                          PKCS12_SAFEBAG *bag)
{
    int free_bags = 0;

    if (pbags == NULL)
        return 1;
    if (*pbags == NULL) {
        *pbags = sk_PKCS12_SAFEBAG_new_null();
        if (*pbags == NULL)
            return 0;
        free_bags = 1;
    }

    if (!sk_PKCS12_SAFEBAG_push(*pbags, bag)) {
        if (free_bags) {
            sk_PKCS12_SAFEBAG_free(*pbags);
            *pbags = NULL;
        }
        return 0;
    }

    return 1;

}

PKCS12 *PKCS12_add_safes_ex(STACK_OF(PKCS7) *safes, int nid_p7,
                            OSSL_LIB_CTX *ctx, const char *propq)
{
    PKCS12 *p12;

    if (nid_p7 <= 0)
        nid_p7 = NID_pkcs7_data;
    p12 = PKCS12_init_ex(nid_p7, ctx, propq);
    if (p12 == NULL)
        return NULL;

    if (!PKCS12_pack_authsafes(p12, safes)) {
        PKCS12_free(p12);
        return NULL;
    }

    return p12;

}

PKCS12 *PKCS12_add_safes(STACK_OF(PKCS7) *safes, int nid_p7)
{
    return PKCS12_add_safes_ex(safes, nid_p7, NULL, NULL);
}
