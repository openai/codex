/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/ec.h>
#include <openssl/rand.h>
#include <openssl/err.h>
#ifndef FIPS_MODULE
# include <openssl/x509.h>
#endif
#include "crypto/ecx.h"
#include "ecx_backend.h"

/*
 * The intention with the "backend" source file is to offer backend support
 * for legacy backends (EVP_PKEY_ASN1_METHOD and EVP_PKEY_METHOD) and provider
 * implementations alike.
 */

int ossl_ecx_public_from_private(ECX_KEY *key)
{
    switch (key->type) {
    case ECX_KEY_TYPE_X25519:
        ossl_x25519_public_from_private(key->pubkey, key->privkey);
        break;
    case ECX_KEY_TYPE_ED25519:
        if (!ossl_ed25519_public_from_private(key->libctx, key->pubkey,
                                              key->privkey, key->propq)) {
            ERR_raise(ERR_LIB_EC, EC_R_FAILED_MAKING_PUBLIC_KEY);
            return 0;
        }
        break;
    case ECX_KEY_TYPE_X448:
        ossl_x448_public_from_private(key->pubkey, key->privkey);
        break;
    case ECX_KEY_TYPE_ED448:
        if (!ossl_ed448_public_from_private(key->libctx, key->pubkey,
                                            key->privkey, key->propq)) {
            ERR_raise(ERR_LIB_EC, EC_R_FAILED_MAKING_PUBLIC_KEY);
            return 0;
        }
        break;
    }
    return 1;
}

int ossl_ecx_key_fromdata(ECX_KEY *ecx, const OSSL_PARAM params[],
                          int include_private)
{
    size_t privkeylen = 0, pubkeylen = 0;
    const OSSL_PARAM *param_priv_key = NULL, *param_pub_key;
    unsigned char *pubkey;

    if (ecx == NULL)
        return 0;

    param_pub_key = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);
    if (include_private)
        param_priv_key =
            OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);

    if (param_pub_key == NULL && param_priv_key == NULL)
        return 0;

    if (param_priv_key != NULL) {
        if (!OSSL_PARAM_get_octet_string(param_priv_key,
                                         (void **)&ecx->privkey, ecx->keylen,
                                         &privkeylen))
            return 0;
        if (privkeylen != ecx->keylen) {
            /*
             * Invalid key length. We will clear what we've received now. We
             * can't leave it to ossl_ecx_key_free() because that will call
             * OPENSSL_secure_clear_free() and assume the correct key length
             */
            OPENSSL_secure_clear_free(ecx->privkey, privkeylen);
            ecx->privkey = NULL;
            return 0;
        }
    }


    pubkey = ecx->pubkey;
    if (param_pub_key != NULL
        && !OSSL_PARAM_get_octet_string(param_pub_key,
                                        (void **)&pubkey,
                                         sizeof(ecx->pubkey), &pubkeylen))
        return 0;

    if ((param_pub_key != NULL && pubkeylen != ecx->keylen))
        return 0;

    if (param_pub_key == NULL && !ossl_ecx_public_from_private(ecx))
        return 0;

    ecx->haspubkey = 1;

    return 1;
}

ECX_KEY *ossl_ecx_key_dup(const ECX_KEY *key, int selection)
{
    ECX_KEY *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;

    ret->libctx = key->libctx;
    ret->haspubkey = 0;
    ret->keylen = key->keylen;
    ret->type = key->type;

    if (!CRYPTO_NEW_REF(&ret->references, 1))
        goto err;

    if (key->propq != NULL) {
        ret->propq = OPENSSL_strdup(key->propq);
        if (ret->propq == NULL)
            goto err;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0
        && key->haspubkey == 1) {
        memcpy(ret->pubkey, key->pubkey, sizeof(ret->pubkey));
        ret->haspubkey = 1;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0
        && key->privkey != NULL) {
        if (ossl_ecx_key_allocate_privkey(ret) == NULL) {
            ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
            goto err;
        }
        memcpy(ret->privkey, key->privkey, ret->keylen);
    }

    return ret;

err:
    CRYPTO_FREE_REF(&ret->references);
    ossl_ecx_key_free(ret);
    return NULL;
}

#ifndef FIPS_MODULE
ECX_KEY *ossl_ecx_key_op(const X509_ALGOR *palg,
                         const unsigned char *p, int plen,
                         int id, ecx_key_op_t op,
                         OSSL_LIB_CTX *libctx, const char *propq)
{
    ECX_KEY *key = NULL;
    unsigned char *privkey, *pubkey;

    if (op != KEY_OP_KEYGEN) {
        if (palg != NULL) {
            int ptype;

            /* Algorithm parameters must be absent */
            X509_ALGOR_get0(NULL, &ptype, NULL, palg);
            if (ptype != V_ASN1_UNDEF) {
                ERR_raise(ERR_LIB_EC, EC_R_INVALID_ENCODING);
                return 0;
            }
            if (id == EVP_PKEY_NONE)
                id = OBJ_obj2nid(palg->algorithm);
            else if (id != OBJ_obj2nid(palg->algorithm)) {
                ERR_raise(ERR_LIB_EC, EC_R_INVALID_ENCODING);
                return 0;
            }
        }

        if (p == NULL || id == EVP_PKEY_NONE || plen != KEYLENID(id)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_ENCODING);
            return 0;
        }
    }

    key = ossl_ecx_key_new(libctx, KEYNID2TYPE(id), 1, propq);
    if (key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        return 0;
    }
    pubkey = key->pubkey;

    if (op == KEY_OP_PUBLIC) {
        memcpy(pubkey, p, plen);
    } else {
        privkey = ossl_ecx_key_allocate_privkey(key);
        if (privkey == NULL) {
            ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
            goto err;
        }
        if (op == KEY_OP_KEYGEN) {
            if (id != EVP_PKEY_NONE) {
                if (RAND_priv_bytes_ex(libctx, privkey, KEYLENID(id), 0) <= 0)
                    goto err;
                if (id == EVP_PKEY_X25519) {
                    privkey[0] &= 248;
                    privkey[X25519_KEYLEN - 1] &= 127;
                    privkey[X25519_KEYLEN - 1] |= 64;
                } else if (id == EVP_PKEY_X448) {
                    privkey[0] &= 252;
                    privkey[X448_KEYLEN - 1] |= 128;
                }
            }
        } else {
            memcpy(privkey, p, KEYLENID(id));
        }
        if (!ossl_ecx_public_from_private(key)) {
            ERR_raise(ERR_LIB_EC, EC_R_FAILED_MAKING_PUBLIC_KEY);
            goto err;
        }
    }

    return key;
 err:
    ossl_ecx_key_free(key);
    return NULL;
}

ECX_KEY *ossl_ecx_key_from_pkcs8(const PKCS8_PRIV_KEY_INFO *p8inf,
                                 OSSL_LIB_CTX *libctx, const char *propq)
{
    ECX_KEY *ecx = NULL;
    const unsigned char *p;
    int plen;
    ASN1_OCTET_STRING *oct = NULL;
    const X509_ALGOR *palg;

    if (!PKCS8_pkey_get0(NULL, &p, &plen, &palg, p8inf))
        return 0;

    oct = d2i_ASN1_OCTET_STRING(NULL, &p, plen);
    if (oct == NULL) {
        p = NULL;
        plen = 0;
    } else {
        p = ASN1_STRING_get0_data(oct);
        plen = ASN1_STRING_length(oct);
    }

    /*
     * EVP_PKEY_NONE means that ecx_key_op() has to figure out the key type
     * on its own.
     */
    ecx = ossl_ecx_key_op(palg, p, plen, EVP_PKEY_NONE, KEY_OP_PRIVATE,
                          libctx, propq);
    ASN1_OCTET_STRING_free(oct);
    return ecx;
}
#endif
