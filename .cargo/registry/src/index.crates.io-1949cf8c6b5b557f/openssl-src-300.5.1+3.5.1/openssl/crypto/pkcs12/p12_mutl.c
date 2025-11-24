/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * HMAC low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include "crypto/evp.h"
#include <openssl/crypto.h>
#include <openssl/hmac.h>
#include <openssl/rand.h>
#include <openssl/pkcs12.h>
#include "p12_local.h"

static int pkcs12_pbmac1_pbkdf2_key_gen(const char *pass, int passlen,
                                        unsigned char *salt, int saltlen,
                                        int id, int iter, int keylen,
                                        unsigned char *out,
                                        const EVP_MD *md_type);

int PKCS12_mac_present(const PKCS12 *p12)
{
    return p12->mac ? 1 : 0;
}

void PKCS12_get0_mac(const ASN1_OCTET_STRING **pmac,
                     const X509_ALGOR **pmacalg,
                     const ASN1_OCTET_STRING **psalt,
                     const ASN1_INTEGER **piter,
                     const PKCS12 *p12)
{
    if (p12->mac) {
        X509_SIG_get0(p12->mac->dinfo, pmacalg, pmac);
        if (psalt)
            *psalt = p12->mac->salt;
        if (piter)
            *piter = p12->mac->iter;
    } else {
        if (pmac)
            *pmac = NULL;
        if (pmacalg)
            *pmacalg = NULL;
        if (psalt)
            *psalt = NULL;
        if (piter)
            *piter = NULL;
    }
}

#define TK26_MAC_KEY_LEN 32

static int pkcs12_gen_gost_mac_key(const char *pass, int passlen,
                                   const unsigned char *salt, int saltlen,
                                   int iter, int keylen, unsigned char *key,
                                   const EVP_MD *digest)
{
    unsigned char out[96];

    if (keylen != TK26_MAC_KEY_LEN) {
        return 0;
    }

    if (!PKCS5_PBKDF2_HMAC(pass, passlen, salt, saltlen, iter,
                           digest, sizeof(out), out)) {
        return 0;
    }
    memcpy(key, out + sizeof(out) - TK26_MAC_KEY_LEN, TK26_MAC_KEY_LEN);
    OPENSSL_cleanse(out, sizeof(out));
    return 1;
}

PBKDF2PARAM *PBMAC1_get1_pbkdf2_param(const X509_ALGOR *macalg)
{
    PBMAC1PARAM *param = NULL;
    PBKDF2PARAM *pbkdf2_param = NULL;
    const ASN1_OBJECT *kdf_oid;

    param = ASN1_TYPE_unpack_sequence(ASN1_ITEM_rptr(PBMAC1PARAM), macalg->parameter);
    if (param == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }

    X509_ALGOR_get0(&kdf_oid, NULL, NULL, param->keyDerivationFunc);
    if (OBJ_obj2nid(kdf_oid) != NID_id_pbkdf2) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_PASSED_INVALID_ARGUMENT);
        PBMAC1PARAM_free(param);
        return NULL;
    }

    pbkdf2_param = ASN1_TYPE_unpack_sequence(ASN1_ITEM_rptr(PBKDF2PARAM),
                                             param->keyDerivationFunc->parameter);
    PBMAC1PARAM_free(param);

    return pbkdf2_param;
}

static int PBMAC1_PBKDF2_HMAC(OSSL_LIB_CTX *ctx, const char *propq,
                              const char *pass, int passlen,
                              const X509_ALGOR *macalg, unsigned char *key)
{
    PBKDF2PARAM *pbkdf2_param = NULL;
    const ASN1_OBJECT *kdf_hmac_oid;
    int kdf_hmac_nid;
    int ret = -1;
    int keylen = 0;
    EVP_MD *kdf_md = NULL;
    const ASN1_OCTET_STRING *pbkdf2_salt = NULL;

    pbkdf2_param = PBMAC1_get1_pbkdf2_param(macalg);
    if (pbkdf2_param == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_UNSUPPORTED);
        goto err;
    }
    keylen = ASN1_INTEGER_get(pbkdf2_param->keylength);
    pbkdf2_salt = pbkdf2_param->salt->value.octet_string;

    if (pbkdf2_param->prf == NULL) {
        kdf_hmac_nid = NID_hmacWithSHA1;
    } else {
        X509_ALGOR_get0(&kdf_hmac_oid, NULL, NULL, pbkdf2_param->prf);
        kdf_hmac_nid = OBJ_obj2nid(kdf_hmac_oid);
    }

    kdf_md = EVP_MD_fetch(ctx, OBJ_nid2sn(ossl_hmac2mdnid(kdf_hmac_nid)), propq);
    if (kdf_md == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_FETCH_FAILED);
        goto err;
    }

    if (PKCS5_PBKDF2_HMAC(pass, passlen, pbkdf2_salt->data, pbkdf2_salt->length,
                          ASN1_INTEGER_get(pbkdf2_param->iter), kdf_md, keylen, key) <= 0) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    ret = keylen;

 err:
    EVP_MD_free(kdf_md);
    PBKDF2PARAM_free(pbkdf2_param);

    return ret;
}

/* Generate a MAC, also used for verification */
static int pkcs12_gen_mac(PKCS12 *p12, const char *pass, int passlen,
                          unsigned char *mac, unsigned int *maclen,
                          int pbmac1_md_nid, int pbmac1_kdf_nid,
                          int (*pkcs12_key_gen)(const char *pass, int passlen,
                                                unsigned char *salt, int slen,
                                                int id, int iter, int n,
                                                unsigned char *out,
                                                const EVP_MD *md_type))
{
    int ret = 0;
    const EVP_MD *md;
    EVP_MD *md_fetch;
    HMAC_CTX *hmac = NULL;
    unsigned char key[EVP_MAX_MD_SIZE], *salt;
    int saltlen, iter;
    char md_name[80];
    int keylen = 0;
    int md_nid = NID_undef;
    const X509_ALGOR *macalg;
    const ASN1_OBJECT *macoid;

    if (!PKCS7_type_is_data(p12->authsafes)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_CONTENT_TYPE_NOT_DATA);
        return 0;
    }

    if (p12->authsafes->d.data == NULL) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_DECODE_ERROR);
        return 0;
    }

    salt = p12->mac->salt->data;
    saltlen = p12->mac->salt->length;
    if (p12->mac->iter == NULL)
        iter = 1;
    else
        iter = ASN1_INTEGER_get(p12->mac->iter);
    X509_SIG_get0(p12->mac->dinfo, &macalg, NULL);
    X509_ALGOR_get0(&macoid, NULL, NULL, macalg);
    if (OBJ_obj2nid(macoid) == NID_pbmac1) {
        if (OBJ_obj2txt(md_name, sizeof(md_name), OBJ_nid2obj(pbmac1_md_nid), 0) < 0)
            return 0;
    } else {
        if (OBJ_obj2txt(md_name, sizeof(md_name), macoid, 0) < 0)
            return 0;
    }
    (void)ERR_set_mark();
    md = md_fetch = EVP_MD_fetch(p12->authsafes->ctx.libctx, md_name,
                                 p12->authsafes->ctx.propq);
    if (md == NULL)
        md = EVP_get_digestbynid(OBJ_obj2nid(macoid));

    if (md == NULL) {
        (void)ERR_clear_last_mark();
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_UNKNOWN_DIGEST_ALGORITHM);
        return 0;
    }
    (void)ERR_pop_to_mark();

    keylen = EVP_MD_get_size(md);
    md_nid = EVP_MD_get_type(md);
    if (keylen <= 0)
        goto err;

    /* For PBMAC1 we use a special keygen callback if not provided (e.g. on verification) */
    if (pbmac1_md_nid != NID_undef && pkcs12_key_gen == NULL) {
        keylen = PBMAC1_PBKDF2_HMAC(p12->authsafes->ctx.libctx, p12->authsafes->ctx.propq,
                                    pass, passlen, macalg, key);
        if (keylen < 0)
            goto err;
    } else if ((md_nid == NID_id_GostR3411_94
                || md_nid == NID_id_GostR3411_2012_256
                || md_nid == NID_id_GostR3411_2012_512)
               && ossl_safe_getenv("LEGACY_GOST_PKCS12") == NULL) {
        keylen = TK26_MAC_KEY_LEN;
        if (!pkcs12_gen_gost_mac_key(pass, passlen, salt, saltlen, iter,
                                     keylen, key, md)) {
            ERR_raise(ERR_LIB_PKCS12, PKCS12_R_KEY_GEN_ERROR);
            goto err;
        }
    } else {
        EVP_MD *hmac_md = (EVP_MD *)md;
        int fetched = 0;

        if (pbmac1_kdf_nid != NID_undef) {
            char hmac_md_name[128];

            if (OBJ_obj2txt(hmac_md_name, sizeof(hmac_md_name), OBJ_nid2obj(pbmac1_kdf_nid), 0) < 0)
                goto err;
            hmac_md = EVP_MD_fetch(NULL, hmac_md_name, NULL);
            if (hmac_md == NULL)
                goto err;
            fetched = 1;
        }
        if (pkcs12_key_gen != NULL) {
            int res = (*pkcs12_key_gen)(pass, passlen, salt, saltlen, PKCS12_MAC_ID,
                                        iter, keylen, key, hmac_md);

            if (fetched)
                EVP_MD_free(hmac_md);
            if (res != 1) {
                ERR_raise(ERR_LIB_PKCS12, PKCS12_R_KEY_GEN_ERROR);
                goto err;
            }
        } else {
            if (fetched)
                EVP_MD_free(hmac_md);
            /* Default to UTF-8 password */
            if (!PKCS12_key_gen_utf8_ex(pass, passlen, salt, saltlen, PKCS12_MAC_ID,
                                        iter, keylen, key, md,
                                        p12->authsafes->ctx.libctx, p12->authsafes->ctx.propq)) {
                ERR_raise(ERR_LIB_PKCS12, PKCS12_R_KEY_GEN_ERROR);
                goto err;
            }
        }
    }
    if ((hmac = HMAC_CTX_new()) == NULL
        || !HMAC_Init_ex(hmac, key, keylen, md, NULL)
        || !HMAC_Update(hmac, p12->authsafes->d.data->data,
                        p12->authsafes->d.data->length)
        || !HMAC_Final(hmac, mac, maclen)) {
        goto err;
    }
    ret = 1;

err:
    OPENSSL_cleanse(key, sizeof(key));
    HMAC_CTX_free(hmac);
    EVP_MD_free(md_fetch);
    return ret;
}

int PKCS12_gen_mac(PKCS12 *p12, const char *pass, int passlen,
                   unsigned char *mac, unsigned int *maclen)
{
    return pkcs12_gen_mac(p12, pass, passlen, mac, maclen, NID_undef, NID_undef, NULL);
}

/* Verify the mac */
int PKCS12_verify_mac(PKCS12 *p12, const char *pass, int passlen)
{
    unsigned char mac[EVP_MAX_MD_SIZE];
    unsigned int maclen;
    const ASN1_OCTET_STRING *macoct;
    const X509_ALGOR *macalg;
    const ASN1_OBJECT *macoid;

    if (p12->mac == NULL) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_ABSENT);
        return 0;
    }

    X509_SIG_get0(p12->mac->dinfo, &macalg, NULL);
    X509_ALGOR_get0(&macoid, NULL, NULL, macalg);
    if (OBJ_obj2nid(macoid) == NID_pbmac1) {
        PBMAC1PARAM *param = NULL;
        const ASN1_OBJECT *hmac_oid;
        int md_nid = NID_undef;

        param = ASN1_TYPE_unpack_sequence(ASN1_ITEM_rptr(PBMAC1PARAM), macalg->parameter);
        if (param == NULL) {
            ERR_raise(ERR_LIB_PKCS12, ERR_R_UNSUPPORTED);
            return 0;
        }
        X509_ALGOR_get0(&hmac_oid, NULL, NULL, param->messageAuthScheme);
        md_nid = ossl_hmac2mdnid(OBJ_obj2nid(hmac_oid));

        if (!pkcs12_gen_mac(p12, pass, passlen, mac, &maclen, md_nid, NID_undef, NULL)) {
            ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_GENERATION_ERROR);
            PBMAC1PARAM_free(param);
            return 0;
        }
        PBMAC1PARAM_free(param);
    } else {
        if (!pkcs12_gen_mac(p12, pass, passlen, mac, &maclen, NID_undef, NID_undef, NULL)) {
            ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_GENERATION_ERROR);
            return 0;
        }
    }
    X509_SIG_get0(p12->mac->dinfo, NULL, &macoct);
    if ((maclen != (unsigned int)ASN1_STRING_length(macoct))
        || CRYPTO_memcmp(mac, ASN1_STRING_get0_data(macoct), maclen) != 0)
        return 0;

    return 1;
}

/* Set a mac */
int PKCS12_set_mac(PKCS12 *p12, const char *pass, int passlen,
                   unsigned char *salt, int saltlen, int iter,
                   const EVP_MD *md_type)
{
    unsigned char mac[EVP_MAX_MD_SIZE];
    unsigned int maclen;
    ASN1_OCTET_STRING *macoct;

    if (md_type == NULL)
        /* No need to do a fetch as the md_type is used only to get a NID */
        md_type = EVP_sha256();
    if (!iter)
        iter = PKCS12_DEFAULT_ITER;
    if (PKCS12_setup_mac(p12, iter, salt, saltlen, md_type) == PKCS12_ERROR) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_SETUP_ERROR);
        return 0;
    }
    /*
     * Note that output mac is forced to UTF-8...
     */
    if (!pkcs12_gen_mac(p12, pass, passlen, mac, &maclen, NID_undef, NID_undef, NULL)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_GENERATION_ERROR);
        return 0;
    }
    X509_SIG_getm(p12->mac->dinfo, NULL, &macoct);
    if (!ASN1_OCTET_STRING_set(macoct, mac, maclen)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_STRING_SET_ERROR);
        return 0;
    }
    return 1;
}

static int pkcs12_pbmac1_pbkdf2_key_gen(const char *pass, int passlen,
                                        unsigned char *salt, int saltlen,
                                        int id, int iter, int keylen,
                                        unsigned char *out,
                                        const EVP_MD *md_type)
{
    return PKCS5_PBKDF2_HMAC(pass, passlen, salt, saltlen, iter,
                             md_type, keylen, out);
}

static int pkcs12_setup_mac(PKCS12 *p12, int iter, unsigned char *salt, int saltlen,
                            int nid)
{
    X509_ALGOR *macalg;

    PKCS12_MAC_DATA_free(p12->mac);
    p12->mac = NULL;

    if ((p12->mac = PKCS12_MAC_DATA_new()) == NULL)
        return PKCS12_ERROR;
    if (iter > 1) {
        if ((p12->mac->iter = ASN1_INTEGER_new()) == NULL) {
            ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
            return 0;
        }
        if (!ASN1_INTEGER_set(p12->mac->iter, iter)) {
            ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
            return 0;
        }
    }
    if (saltlen == 0)
        saltlen = PKCS12_SALT_LEN;
    else if (saltlen < 0)
        return 0;
    if ((p12->mac->salt->data = OPENSSL_malloc(saltlen)) == NULL)
        return 0;
    p12->mac->salt->length = saltlen;
    if (salt == NULL) {
        if (RAND_bytes_ex(p12->authsafes->ctx.libctx, p12->mac->salt->data,
                          (size_t)saltlen, 0) <= 0)
            return 0;
    } else {
        memcpy(p12->mac->salt->data, salt, saltlen);
    }
    X509_SIG_getm(p12->mac->dinfo, &macalg, NULL);
    if (!X509_ALGOR_set0(macalg, OBJ_nid2obj(nid), V_ASN1_NULL, NULL)) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        return 0;
    }

    return 1;
}

/* Set up a mac structure */
int PKCS12_setup_mac(PKCS12 *p12, int iter, unsigned char *salt, int saltlen,
                     const EVP_MD *md_type)
{
    return pkcs12_setup_mac(p12, iter, salt, saltlen, EVP_MD_get_type(md_type));
}

int PKCS12_set_pbmac1_pbkdf2(PKCS12 *p12, const char *pass, int passlen,
                             unsigned char *salt, int saltlen, int iter,
                             const EVP_MD *md_type, const char *prf_md_name)
{
    unsigned char mac[EVP_MAX_MD_SIZE];
    unsigned int maclen;
    ASN1_OCTET_STRING *macoct;
    X509_ALGOR *alg = NULL;
    int ret = 0;
    int prf_md_nid = NID_undef, prf_nid = NID_undef, hmac_nid;
    unsigned char *known_salt = NULL;
    int keylen = 0;
    PBMAC1PARAM *param = NULL;
    X509_ALGOR  *hmac_alg = NULL, *macalg = NULL;

    if (md_type == NULL)
        /* No need to do a fetch as the md_type is used only to get a NID */
        md_type = EVP_sha256();

    if (prf_md_name == NULL)
        prf_md_nid = EVP_MD_get_type(md_type);
    else
        prf_md_nid = OBJ_txt2nid(prf_md_name);

    if (iter == 0)
        iter = PKCS12_DEFAULT_ITER;

    keylen = EVP_MD_get_size(md_type);

    prf_nid  = ossl_md2hmacnid(prf_md_nid);
    hmac_nid = ossl_md2hmacnid(EVP_MD_get_type(md_type));

    if (prf_nid == NID_undef || hmac_nid == NID_undef) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_UNKNOWN_DIGEST_ALGORITHM);
        goto err;
    }

    if (salt == NULL) {
        known_salt = OPENSSL_malloc(saltlen);
        if (known_salt == NULL)
            goto err;

        if (RAND_bytes_ex(NULL, known_salt, saltlen, 0) <= 0) {
            ERR_raise(ERR_LIB_PKCS12, ERR_R_RAND_LIB);
            goto err;
        }
    }

    param = PBMAC1PARAM_new();
    hmac_alg = X509_ALGOR_new();
    alg = PKCS5_pbkdf2_set(iter, salt ? salt : known_salt, saltlen, prf_nid, keylen);
    if (param == NULL || hmac_alg == NULL || alg == NULL)
        goto err;

    if (pkcs12_setup_mac(p12, iter, salt ? salt : known_salt, saltlen,
                         NID_pbmac1) == PKCS12_ERROR) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_SETUP_ERROR);
        goto err;
    }

    if (!X509_ALGOR_set0(hmac_alg, OBJ_nid2obj(hmac_nid), V_ASN1_NULL, NULL)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_SETUP_ERROR);
        goto err;
    }

    X509_ALGOR_free(param->keyDerivationFunc);
    X509_ALGOR_free(param->messageAuthScheme);
    param->keyDerivationFunc = alg;
    param->messageAuthScheme = hmac_alg;

    X509_SIG_getm(p12->mac->dinfo, &macalg, &macoct);
    if (!ASN1_TYPE_pack_sequence(ASN1_ITEM_rptr(PBMAC1PARAM), param, &macalg->parameter))
        goto err;

    /*
     * Note that output mac is forced to UTF-8...
     */
    if (!pkcs12_gen_mac(p12, pass, passlen, mac, &maclen,
                        EVP_MD_get_type(md_type), prf_md_nid,
                        pkcs12_pbmac1_pbkdf2_key_gen)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_GENERATION_ERROR);
        goto err;
    }
    if (!ASN1_OCTET_STRING_set(macoct, mac, maclen)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_MAC_STRING_SET_ERROR);
        goto err;
    }
    ret = 1;

 err:
    PBMAC1PARAM_free(param);
    OPENSSL_free(known_salt);
    return ret;
}
