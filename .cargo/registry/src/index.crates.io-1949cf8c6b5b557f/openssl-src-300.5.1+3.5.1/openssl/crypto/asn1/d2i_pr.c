/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/decoder.h>
#include <openssl/engine.h>
#include <openssl/x509.h>
#include <openssl/asn1.h>
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "crypto/x509.h"
#include "internal/asn1.h"
#include "internal/sizes.h"

static EVP_PKEY *
d2i_PrivateKey_decoder(int keytype, EVP_PKEY **a, const unsigned char **pp,
                       long length, OSSL_LIB_CTX *libctx, const char *propq)
{
    OSSL_DECODER_CTX *dctx = NULL;
    size_t len = length;
    EVP_PKEY *pkey = NULL, *bak_a = NULL;
    EVP_PKEY **ppkey = &pkey;
    const char *key_name = NULL;
    char keytypebuf[OSSL_MAX_NAME_SIZE];
    int ret;
    const unsigned char *p = *pp;
    const char *structure;
    PKCS8_PRIV_KEY_INFO *p8info;
    const ASN1_OBJECT *algoid;

    if (keytype != EVP_PKEY_NONE) {
        key_name = evp_pkey_type2name(keytype);
        if (key_name == NULL)
            return NULL;
    }

    /* This is just a probe. It might fail, so we ignore errors */
    ERR_set_mark();
    p8info = d2i_PKCS8_PRIV_KEY_INFO(NULL, pp, len);
    ERR_pop_to_mark();
    if (p8info != NULL) {
        int64_t v;

        /* ascertain version is 0 or 1 as per RFC5958 */
        if (!ASN1_INTEGER_get_int64(&v, p8info->version)
            || (v != 0 && v != 1)) {
            *pp = p;
            ERR_raise(ERR_LIB_ASN1, ASN1_R_ASN1_PARSE_ERROR);
            PKCS8_PRIV_KEY_INFO_free(p8info);
            return NULL;
        }
        if (key_name == NULL
                && PKCS8_pkey_get0(&algoid, NULL, NULL, NULL, p8info)
                && OBJ_obj2txt(keytypebuf, sizeof(keytypebuf), algoid, 0))
            key_name = keytypebuf;
        structure = "PrivateKeyInfo";
        PKCS8_PRIV_KEY_INFO_free(p8info);
    } else {
        structure = "type-specific";
    }
    *pp = p;

    if (a != NULL && (bak_a = *a) != NULL)
        ppkey = a;
    dctx = OSSL_DECODER_CTX_new_for_pkey(ppkey, "DER", structure, key_name,
                                         EVP_PKEY_KEYPAIR, libctx, propq);
    if (a != NULL)
        *a = bak_a;
    if (dctx == NULL)
        goto err;

    ret = OSSL_DECODER_from_data(dctx, pp, &len);
    OSSL_DECODER_CTX_free(dctx);
    if (ret
        && *ppkey != NULL
        && evp_keymgmt_util_has(*ppkey, OSSL_KEYMGMT_SELECT_PRIVATE_KEY)) {
        if (a != NULL)
            *a = *ppkey;
        return *ppkey;
    }

 err:
    if (ppkey != a)
        EVP_PKEY_free(*ppkey);
    return NULL;
}

EVP_PKEY *
ossl_d2i_PrivateKey_legacy(int keytype, EVP_PKEY **a, const unsigned char **pp,
                           long length, OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_PKEY *ret;
    const unsigned char *p = *pp;

    if (a == NULL || *a == NULL) {
        if ((ret = EVP_PKEY_new()) == NULL) {
            ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
            return NULL;
        }
    } else {
        ret = *a;
#ifndef OPENSSL_NO_ENGINE
        ENGINE_finish(ret->engine);
        ret->engine = NULL;
#endif
    }

    if (!EVP_PKEY_set_type(ret, keytype)) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_UNKNOWN_PUBLIC_KEY_TYPE);
        goto err;
    }

    ERR_set_mark();
    if (!ret->ameth->old_priv_decode ||
        !ret->ameth->old_priv_decode(ret, &p, length)) {
        if (ret->ameth->priv_decode != NULL
                || ret->ameth->priv_decode_ex != NULL) {
            EVP_PKEY *tmp;
            PKCS8_PRIV_KEY_INFO *p8 = NULL;
            p8 = d2i_PKCS8_PRIV_KEY_INFO(NULL, &p, length);
            if (p8 == NULL) {
                ERR_clear_last_mark();
                goto err;
            }
            tmp = evp_pkcs82pkey_legacy(p8, libctx, propq);
            PKCS8_PRIV_KEY_INFO_free(p8);
            if (tmp == NULL) {
                ERR_clear_last_mark();
                goto err;
            }
            EVP_PKEY_free(ret);
            ret = tmp;
            ERR_pop_to_mark();
            if (EVP_PKEY_type(keytype) != EVP_PKEY_get_base_id(ret))
                goto err;
        } else {
            ERR_clear_last_mark();
            ERR_raise(ERR_LIB_ASN1, ERR_R_ASN1_LIB);
            goto err;
        }
    } else {
      ERR_clear_last_mark();
    }
    *pp = p;
    if (a != NULL)
        *a = ret;
    return ret;
 err:
    if (a == NULL || *a != ret)
        EVP_PKEY_free(ret);
    return NULL;
}

EVP_PKEY *d2i_PrivateKey_ex(int keytype, EVP_PKEY **a, const unsigned char **pp,
                            long length, OSSL_LIB_CTX *libctx,
                            const char *propq)
{
    EVP_PKEY *ret;

    ret = d2i_PrivateKey_decoder(keytype, a, pp, length, libctx, propq);
    /* try the legacy path if the decoder failed */
    if (ret == NULL)
        ret = ossl_d2i_PrivateKey_legacy(keytype, a, pp, length, libctx, propq);
    return ret;
}

EVP_PKEY *d2i_PrivateKey(int type, EVP_PKEY **a, const unsigned char **pp,
                         long length)
{
    return d2i_PrivateKey_ex(type, a, pp, length, NULL, NULL);
}

static EVP_PKEY *d2i_AutoPrivateKey_legacy(EVP_PKEY **a,
                                           const unsigned char **pp,
                                           long length,
                                           OSSL_LIB_CTX *libctx,
                                           const char *propq)
{
    STACK_OF(ASN1_TYPE) *inkey;
    const unsigned char *p;
    int keytype;

    p = *pp;
    /*
     * Dirty trick: read in the ASN1 data into a STACK_OF(ASN1_TYPE): by
     * analyzing it we can determine the passed structure: this assumes the
     * input is surrounded by an ASN1 SEQUENCE.
     */
    inkey = d2i_ASN1_SEQUENCE_ANY(NULL, &p, length);
    p = *pp;
    /*
     * Since we only need to discern "traditional format" RSA and DSA keys we
     * can just count the elements.
     */
    if (sk_ASN1_TYPE_num(inkey) == 6) {
        keytype = EVP_PKEY_DSA;
    } else if (sk_ASN1_TYPE_num(inkey) == 4) {
        keytype = EVP_PKEY_EC;
    } else if (sk_ASN1_TYPE_num(inkey) == 3) { /* This seems to be PKCS8, not
                                              * traditional format */
        PKCS8_PRIV_KEY_INFO *p8 = d2i_PKCS8_PRIV_KEY_INFO(NULL, &p, length);
        EVP_PKEY *ret;

        sk_ASN1_TYPE_pop_free(inkey, ASN1_TYPE_free);
        if (p8 == NULL) {
            ERR_raise(ERR_LIB_ASN1, ASN1_R_UNSUPPORTED_PUBLIC_KEY_TYPE);
            return NULL;
        }
        ret = evp_pkcs82pkey_legacy(p8, libctx, propq);
        PKCS8_PRIV_KEY_INFO_free(p8);
        if (ret == NULL)
            return NULL;
        *pp = p;
        if (a != NULL) {
            *a = ret;
        }
        return ret;
    } else {
        keytype = EVP_PKEY_RSA;
    }
    sk_ASN1_TYPE_pop_free(inkey, ASN1_TYPE_free);
    return ossl_d2i_PrivateKey_legacy(keytype, a, pp, length, libctx, propq);
}

/*
 * This works like d2i_PrivateKey() except it passes the keytype as
 * EVP_PKEY_NONE, which then figures out the type during decoding.
 */
EVP_PKEY *d2i_AutoPrivateKey_ex(EVP_PKEY **a, const unsigned char **pp,
                                long length, OSSL_LIB_CTX *libctx,
                                const char *propq)
{
    EVP_PKEY *ret;

    ret = d2i_PrivateKey_decoder(EVP_PKEY_NONE, a, pp, length, libctx, propq);
    /* try the legacy path if the decoder failed */
    if (ret == NULL)
        ret = d2i_AutoPrivateKey_legacy(a, pp, length, libctx, propq);
    return ret;
}

EVP_PKEY *d2i_AutoPrivateKey(EVP_PKEY **a, const unsigned char **pp,
                             long length)
{
    return d2i_AutoPrivateKey_ex(a, pp, length, NULL, NULL);
}
