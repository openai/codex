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
#include <openssl/evp.h>
#include <openssl/core.h>
#include <openssl/core_names.h>
#include <openssl/pkcs12.h>
#include <openssl/x509.h>
#include "crypto/evp.h"
#include "evp_local.h"

/* Password based encryption (PBE) functions */

/* Setup a cipher context from a PBE algorithm */

struct evp_pbe_st {
    int pbe_type;
    int pbe_nid;
    int cipher_nid;
    int md_nid;
    EVP_PBE_KEYGEN *keygen;
    EVP_PBE_KEYGEN_EX *keygen_ex;
};

static STACK_OF(EVP_PBE_CTL) *pbe_algs;

static const EVP_PBE_CTL builtin_pbe[] = {
    {EVP_PBE_TYPE_OUTER, NID_pbeWithMD2AndDES_CBC,
     NID_des_cbc, NID_md2, PKCS5_PBE_keyivgen, PKCS5_PBE_keyivgen_ex},
    {EVP_PBE_TYPE_OUTER, NID_pbeWithMD5AndDES_CBC,
     NID_des_cbc, NID_md5, PKCS5_PBE_keyivgen, PKCS5_PBE_keyivgen_ex},
    {EVP_PBE_TYPE_OUTER, NID_pbeWithSHA1AndRC2_CBC,
     NID_rc2_64_cbc, NID_sha1, PKCS5_PBE_keyivgen, PKCS5_PBE_keyivgen_ex},

    {EVP_PBE_TYPE_OUTER, NID_id_pbkdf2, -1, -1, PKCS5_v2_PBKDF2_keyivgen,
     PKCS5_v2_PBKDF2_keyivgen_ex},

    {EVP_PBE_TYPE_OUTER, NID_pbe_WithSHA1And128BitRC4,
     NID_rc4, NID_sha1, PKCS12_PBE_keyivgen, &PKCS12_PBE_keyivgen_ex},
    {EVP_PBE_TYPE_OUTER, NID_pbe_WithSHA1And40BitRC4,
     NID_rc4_40, NID_sha1, PKCS12_PBE_keyivgen, &PKCS12_PBE_keyivgen_ex},
    {EVP_PBE_TYPE_OUTER, NID_pbe_WithSHA1And3_Key_TripleDES_CBC,
     NID_des_ede3_cbc, NID_sha1, PKCS12_PBE_keyivgen, &PKCS12_PBE_keyivgen_ex},
    {EVP_PBE_TYPE_OUTER, NID_pbe_WithSHA1And2_Key_TripleDES_CBC,
     NID_des_ede_cbc, NID_sha1, PKCS12_PBE_keyivgen, &PKCS12_PBE_keyivgen_ex},
    {EVP_PBE_TYPE_OUTER, NID_pbe_WithSHA1And128BitRC2_CBC,
     NID_rc2_cbc, NID_sha1, PKCS12_PBE_keyivgen, &PKCS12_PBE_keyivgen_ex},
    {EVP_PBE_TYPE_OUTER, NID_pbe_WithSHA1And40BitRC2_CBC,
     NID_rc2_40_cbc, NID_sha1, PKCS12_PBE_keyivgen, &PKCS12_PBE_keyivgen_ex},

    {EVP_PBE_TYPE_OUTER, NID_pbes2, -1, -1, PKCS5_v2_PBE_keyivgen, &PKCS5_v2_PBE_keyivgen_ex},

    {EVP_PBE_TYPE_OUTER, NID_pbeWithMD2AndRC2_CBC,
     NID_rc2_64_cbc, NID_md2, PKCS5_PBE_keyivgen, PKCS5_PBE_keyivgen_ex},
    {EVP_PBE_TYPE_OUTER, NID_pbeWithMD5AndRC2_CBC,
     NID_rc2_64_cbc, NID_md5, PKCS5_PBE_keyivgen, PKCS5_PBE_keyivgen_ex},
    {EVP_PBE_TYPE_OUTER, NID_pbeWithSHA1AndDES_CBC,
     NID_des_cbc, NID_sha1, PKCS5_PBE_keyivgen, PKCS5_PBE_keyivgen_ex},

    {EVP_PBE_TYPE_PRF, NID_hmacWithSHA1, -1, NID_sha1, 0},
    {EVP_PBE_TYPE_PRF, NID_hmac_md5, -1, NID_md5, 0},
    {EVP_PBE_TYPE_PRF, NID_hmac_sha1, -1, NID_sha1, 0},
    {EVP_PBE_TYPE_PRF, NID_hmacWithMD5, -1, NID_md5, 0},
    {EVP_PBE_TYPE_PRF, NID_hmacWithSHA224, -1, NID_sha224, 0},
    {EVP_PBE_TYPE_PRF, NID_hmacWithSHA256, -1, NID_sha256, 0},
    {EVP_PBE_TYPE_PRF, NID_hmacWithSHA384, -1, NID_sha384, 0},
    {EVP_PBE_TYPE_PRF, NID_hmacWithSHA512, -1, NID_sha512, 0},
    {EVP_PBE_TYPE_PRF, NID_id_HMACGostR3411_94, -1, NID_id_GostR3411_94, 0},
    {EVP_PBE_TYPE_PRF, NID_id_tc26_hmac_gost_3411_2012_256, -1,
     NID_id_GostR3411_2012_256, 0},
    {EVP_PBE_TYPE_PRF, NID_id_tc26_hmac_gost_3411_2012_512, -1,
     NID_id_GostR3411_2012_512, 0},
    {EVP_PBE_TYPE_PRF, NID_hmac_sha3_224, -1, NID_sha3_224, 0},
    {EVP_PBE_TYPE_PRF, NID_hmac_sha3_256, -1, NID_sha3_256, 0},
    {EVP_PBE_TYPE_PRF, NID_hmac_sha3_384, -1, NID_sha3_384, 0},
    {EVP_PBE_TYPE_PRF, NID_hmac_sha3_512, -1, NID_sha3_512, 0},
    {EVP_PBE_TYPE_PRF, NID_hmacWithSHA512_224, -1, NID_sha512_224, 0},
    {EVP_PBE_TYPE_PRF, NID_hmacWithSHA512_256, -1, NID_sha512_256, 0},
#ifndef OPENSSL_NO_SM3
    {EVP_PBE_TYPE_PRF, NID_hmacWithSM3, -1, NID_sm3, 0},
#endif
    {EVP_PBE_TYPE_KDF, NID_id_pbkdf2, -1, -1, PKCS5_v2_PBKDF2_keyivgen, &PKCS5_v2_PBKDF2_keyivgen_ex},
#ifndef OPENSSL_NO_SCRYPT
    {EVP_PBE_TYPE_KDF, NID_id_scrypt, -1, -1, PKCS5_v2_scrypt_keyivgen, &PKCS5_v2_scrypt_keyivgen_ex}
#endif
};


int EVP_PBE_CipherInit_ex(ASN1_OBJECT *pbe_obj, const char *pass, int passlen,
                          ASN1_TYPE *param, EVP_CIPHER_CTX *ctx, int en_de,
                          OSSL_LIB_CTX *libctx, const char *propq)
{
    const EVP_CIPHER *cipher = NULL;
    EVP_CIPHER *cipher_fetch = NULL;
    const EVP_MD *md = NULL;
    EVP_MD *md_fetch = NULL;
    int ret = 0, cipher_nid, md_nid;
    EVP_PBE_KEYGEN_EX *keygen_ex;
    EVP_PBE_KEYGEN *keygen;

    if (!EVP_PBE_find_ex(EVP_PBE_TYPE_OUTER, OBJ_obj2nid(pbe_obj),
                         &cipher_nid, &md_nid, &keygen, &keygen_ex)) {
        char obj_tmp[80];

        if (pbe_obj == NULL)
            OPENSSL_strlcpy(obj_tmp, "NULL", sizeof(obj_tmp));
        else
            i2t_ASN1_OBJECT(obj_tmp, sizeof(obj_tmp), pbe_obj);
        ERR_raise_data(ERR_LIB_EVP, EVP_R_UNKNOWN_PBE_ALGORITHM,
                       "TYPE=%s", obj_tmp);
        goto err;
    }

    if (pass == NULL)
        passlen = 0;
    else if (passlen == -1)
        passlen = strlen(pass);

    if (cipher_nid != -1) {
        (void)ERR_set_mark();
        cipher = cipher_fetch = EVP_CIPHER_fetch(libctx, OBJ_nid2sn(cipher_nid), propq);
        /* Fallback to legacy method */
        if (cipher == NULL)
            cipher = EVP_get_cipherbynid(cipher_nid);
        if (cipher == NULL) {
            (void)ERR_clear_last_mark();
            ERR_raise_data(ERR_LIB_EVP, EVP_R_UNKNOWN_CIPHER,
                           OBJ_nid2sn(cipher_nid));
            goto err;
        }
        (void)ERR_pop_to_mark();
    }

    if (md_nid != -1) {
        (void)ERR_set_mark();
        md = md_fetch = EVP_MD_fetch(libctx, OBJ_nid2sn(md_nid), propq);
        /* Fallback to legacy method */
        if (md == NULL)
            md = EVP_get_digestbynid(md_nid);

        if (md == NULL) {
            (void)ERR_clear_last_mark();
            ERR_raise(ERR_LIB_EVP, EVP_R_UNKNOWN_DIGEST);
            goto err;
        }
        (void)ERR_pop_to_mark();
    }

    /* Try extended keygen with libctx/propq first, fall back to legacy keygen */
    if (keygen_ex != NULL)
        ret = keygen_ex(ctx, pass, passlen, param, cipher, md, en_de, libctx, propq);
    else
        ret = keygen(ctx, pass, passlen, param, cipher, md, en_de);

err:
    EVP_CIPHER_free(cipher_fetch);
    EVP_MD_free(md_fetch);

    return ret;
}

int EVP_PBE_CipherInit(ASN1_OBJECT *pbe_obj, const char *pass, int passlen,
                       ASN1_TYPE *param, EVP_CIPHER_CTX *ctx, int en_de)
{
    return EVP_PBE_CipherInit_ex(pbe_obj, pass, passlen, param, ctx, en_de, NULL, NULL);
}

DECLARE_OBJ_BSEARCH_CMP_FN(EVP_PBE_CTL, EVP_PBE_CTL, pbe2);

static int pbe2_cmp(const EVP_PBE_CTL *pbe1, const EVP_PBE_CTL *pbe2)
{
    int ret = pbe1->pbe_type - pbe2->pbe_type;
    if (ret)
        return ret;
    else
        return pbe1->pbe_nid - pbe2->pbe_nid;
}

IMPLEMENT_OBJ_BSEARCH_CMP_FN(EVP_PBE_CTL, EVP_PBE_CTL, pbe2);

static int pbe_cmp(const EVP_PBE_CTL *const *a, const EVP_PBE_CTL *const *b)
{
    int ret = (*a)->pbe_type - (*b)->pbe_type;
    if (ret)
        return ret;
    else
        return (*a)->pbe_nid - (*b)->pbe_nid;
}

/* Add a PBE algorithm */

int EVP_PBE_alg_add_type(int pbe_type, int pbe_nid, int cipher_nid,
                         int md_nid, EVP_PBE_KEYGEN *keygen)
{
    EVP_PBE_CTL *pbe_tmp = NULL;

    if (pbe_algs == NULL) {
        pbe_algs = sk_EVP_PBE_CTL_new(pbe_cmp);
        if (pbe_algs == NULL) {
            ERR_raise(ERR_LIB_EVP, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    if ((pbe_tmp = OPENSSL_zalloc(sizeof(*pbe_tmp))) == NULL)
        goto err;

    pbe_tmp->pbe_type = pbe_type;
    pbe_tmp->pbe_nid = pbe_nid;
    pbe_tmp->cipher_nid = cipher_nid;
    pbe_tmp->md_nid = md_nid;
    pbe_tmp->keygen = keygen;

    if (!sk_EVP_PBE_CTL_push(pbe_algs, pbe_tmp)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_CRYPTO_LIB);
        goto err;
    }
    return 1;

 err:
    OPENSSL_free(pbe_tmp);
    return 0;
}

int EVP_PBE_alg_add(int nid, const EVP_CIPHER *cipher, const EVP_MD *md,
                    EVP_PBE_KEYGEN *keygen)
{
    int cipher_nid, md_nid;

    if (cipher)
        cipher_nid = EVP_CIPHER_get_nid(cipher);
    else
        cipher_nid = -1;
    if (md)
        md_nid = EVP_MD_get_type(md);
    else
        md_nid = -1;

    return EVP_PBE_alg_add_type(EVP_PBE_TYPE_OUTER, nid,
                                cipher_nid, md_nid, keygen);
}

int EVP_PBE_find_ex(int type, int pbe_nid, int *pcnid, int *pmnid,
                    EVP_PBE_KEYGEN **pkeygen, EVP_PBE_KEYGEN_EX **pkeygen_ex)
{
    EVP_PBE_CTL *pbetmp = NULL, pbelu;
    int i;
    if (pbe_nid == NID_undef)
        return 0;

    pbelu.pbe_type = type;
    pbelu.pbe_nid = pbe_nid;

    if (pbe_algs != NULL) {
        /* Ideally, this would be done under lock */
        sk_EVP_PBE_CTL_sort(pbe_algs);
        i = sk_EVP_PBE_CTL_find(pbe_algs, &pbelu);
        pbetmp = sk_EVP_PBE_CTL_value(pbe_algs, i);
    }
    if (pbetmp == NULL) {
        pbetmp = OBJ_bsearch_pbe2(&pbelu, builtin_pbe, OSSL_NELEM(builtin_pbe));
    }
    if (pbetmp == NULL)
        return 0;
    if (pcnid != NULL)
        *pcnid = pbetmp->cipher_nid;
    if (pmnid != NULL)
        *pmnid = pbetmp->md_nid;
    if (pkeygen != NULL)
        *pkeygen = pbetmp->keygen;
    if (pkeygen_ex != NULL)
        *pkeygen_ex = pbetmp->keygen_ex;
    return 1;
}

int EVP_PBE_find(int type, int pbe_nid,
                 int *pcnid, int *pmnid, EVP_PBE_KEYGEN **pkeygen)
{
    return EVP_PBE_find_ex(type, pbe_nid, pcnid, pmnid, pkeygen, NULL);
}

static void free_evp_pbe_ctl(EVP_PBE_CTL *pbe)
{
    OPENSSL_free(pbe);
}

void EVP_PBE_cleanup(void)
{
    sk_EVP_PBE_CTL_pop_free(pbe_algs, free_evp_pbe_ctl);
    pbe_algs = NULL;
}

int EVP_PBE_get(int *ptype, int *ppbe_nid, size_t num)
{
    const EVP_PBE_CTL *tpbe;

    if (num >= OSSL_NELEM(builtin_pbe))
        return 0;

    tpbe = builtin_pbe + num;
    if (ptype)
        *ptype = tpbe->pbe_type;
    if (ppbe_nid)
        *ppbe_nid = tpbe->pbe_nid;
    return 1;
}
