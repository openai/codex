/*
 * Copyright 1999-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include "internal/cryptlib.h"
#include <openssl/x509.h>
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/hmac.h>
#include <openssl/trace.h>
#include <openssl/core_names.h>
#include "crypto/evp.h"
#include "evp_local.h"

int ossl_pkcs5_pbkdf2_hmac_ex(const char *pass, int passlen,
                              const unsigned char *salt, int saltlen, int iter,
                              const EVP_MD *digest, int keylen, unsigned char *out,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    const char *empty = "";
    int rv = 1, mode = 1;
    EVP_KDF *kdf;
    EVP_KDF_CTX *kctx;
    const char *mdname = EVP_MD_get0_name(digest);
    OSSL_PARAM params[6], *p = params;

    /* Keep documented behaviour. */
    if (pass == NULL) {
        pass = empty;
        passlen = 0;
    } else if (passlen == -1) {
        passlen = strlen(pass);
    }
    if (salt == NULL && saltlen == 0)
        salt = (unsigned char *)empty;

    kdf = EVP_KDF_fetch(libctx, OSSL_KDF_NAME_PBKDF2, propq);
    if (kdf == NULL)
         return 0;
    kctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (kctx == NULL)
        return 0;
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_PASSWORD,
                                             (char *)pass, (size_t)passlen);
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_PKCS5, &mode);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             (unsigned char *)salt, saltlen);
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_ITER, &iter);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)mdname, 0);
    *p = OSSL_PARAM_construct_end();
    if (EVP_KDF_derive(kctx, out, keylen, params) != 1)
        rv = 0;

    EVP_KDF_CTX_free(kctx);

    OSSL_TRACE_BEGIN(PKCS5V2) {
        BIO_printf(trc_out, "Password:\n");
        BIO_hex_string(trc_out,
                       0, passlen, pass, passlen);
        BIO_printf(trc_out, "\n");
        BIO_printf(trc_out, "Salt:\n");
        BIO_hex_string(trc_out,
                       0, saltlen, salt, saltlen);
        BIO_printf(trc_out, "\n");
        BIO_printf(trc_out, "Iteration count %d\n", iter);
        BIO_printf(trc_out, "Key:\n");
        BIO_hex_string(trc_out,
                       0, keylen, out, keylen);
        BIO_printf(trc_out, "\n");
    } OSSL_TRACE_END(PKCS5V2);
    return rv;
}

int PKCS5_PBKDF2_HMAC(const char *pass, int passlen, const unsigned char *salt,
                      int saltlen, int iter, const EVP_MD *digest, int keylen,
                      unsigned char *out)
{
    return ossl_pkcs5_pbkdf2_hmac_ex(pass, passlen, salt, saltlen, iter, digest,
                                     keylen, out, NULL, NULL);
}


int PKCS5_PBKDF2_HMAC_SHA1(const char *pass, int passlen,
                           const unsigned char *salt, int saltlen, int iter,
                           int keylen, unsigned char *out)
{
    EVP_MD *digest;
    int r = 0;

    if ((digest = EVP_MD_fetch(NULL, SN_sha1, NULL)) != NULL)
        r = ossl_pkcs5_pbkdf2_hmac_ex(pass, passlen, salt, saltlen, iter,
                                      digest, keylen, out, NULL, NULL);
    EVP_MD_free(digest);
    return r;
}

/*
 * Now the key derivation function itself. This is a bit evil because it has
 * to check the ASN1 parameters are valid: and there are quite a few of
 * them...
 */

int PKCS5_v2_PBE_keyivgen_ex(EVP_CIPHER_CTX *ctx, const char *pass, int passlen,
                             ASN1_TYPE *param, const EVP_CIPHER *c,
                             const EVP_MD *md, int en_de,
                             OSSL_LIB_CTX *libctx, const char *propq)
{
    PBE2PARAM *pbe2 = NULL;
    char ciph_name[80];
    const EVP_CIPHER *cipher = NULL;
    EVP_CIPHER *cipher_fetch = NULL;
    EVP_PBE_KEYGEN_EX *kdf;

    int rv = 0;

    pbe2 = ASN1_TYPE_unpack_sequence(ASN1_ITEM_rptr(PBE2PARAM), param);
    if (pbe2 == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_DECODE_ERROR);
        goto err;
    }

    /* See if we recognise the key derivation function */
    if (!EVP_PBE_find_ex(EVP_PBE_TYPE_KDF, OBJ_obj2nid(pbe2->keyfunc->algorithm),
                         NULL, NULL, NULL, &kdf)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_KEY_DERIVATION_FUNCTION);
        goto err;
    }

    /*
     * lets see if we recognise the encryption algorithm.
     */
    if (OBJ_obj2txt(ciph_name, sizeof(ciph_name), pbe2->encryption->algorithm, 0) <= 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_CIPHER);
        goto err;
    }

    (void)ERR_set_mark();
    cipher = cipher_fetch = EVP_CIPHER_fetch(libctx, ciph_name, propq);
    /* Fallback to legacy method */
    if (cipher == NULL)
        cipher = EVP_get_cipherbyname(ciph_name);

    if (cipher == NULL) {
        (void)ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_CIPHER);
        goto err;
    }
    (void)ERR_pop_to_mark();

    /* Fixup cipher based on AlgorithmIdentifier */
    if (!EVP_CipherInit_ex(ctx, cipher, NULL, NULL, NULL, en_de))
        goto err;
    if (EVP_CIPHER_asn1_to_param(ctx, pbe2->encryption->parameter) <= 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_CIPHER_PARAMETER_ERROR);
        goto err;
    }
    rv = kdf(ctx, pass, passlen, pbe2->keyfunc->parameter, NULL, NULL, en_de, libctx, propq);
 err:
    EVP_CIPHER_free(cipher_fetch);
    PBE2PARAM_free(pbe2);
    return rv;
}

int PKCS5_v2_PBE_keyivgen(EVP_CIPHER_CTX *ctx, const char *pass, int passlen,
                          ASN1_TYPE *param, const EVP_CIPHER *c,
                          const EVP_MD *md, int en_de)
{
    return PKCS5_v2_PBE_keyivgen_ex(ctx, pass, passlen, param, c, md, en_de, NULL, NULL);
}

int PKCS5_v2_PBKDF2_keyivgen_ex(EVP_CIPHER_CTX *ctx, const char *pass,
                                int passlen, ASN1_TYPE *param,
                                const EVP_CIPHER *c, const EVP_MD *md, int en_de,
                                OSSL_LIB_CTX *libctx, const char *propq)
{
    unsigned char *salt, key[EVP_MAX_KEY_LENGTH];
    int saltlen, iter, t;
    int rv = 0;
    unsigned int keylen = 0;
    int prf_nid, hmac_md_nid;
    PBKDF2PARAM *kdf = NULL;
    const EVP_MD *prfmd = NULL;
    EVP_MD *prfmd_fetch = NULL;

    if (EVP_CIPHER_CTX_get0_cipher(ctx) == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        goto err;
    }
    keylen = EVP_CIPHER_CTX_get_key_length(ctx);
    OPENSSL_assert(keylen <= sizeof(key));

    /* Decode parameter */

    kdf = ASN1_TYPE_unpack_sequence(ASN1_ITEM_rptr(PBKDF2PARAM), param);

    if (kdf == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_DECODE_ERROR);
        goto err;
    }

    t = EVP_CIPHER_CTX_get_key_length(ctx);
    if (t < 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_KEY_LENGTH);
        goto err;
    }
    keylen = t;

    /* Now check the parameters of the kdf */

    if (kdf->keylength && (ASN1_INTEGER_get(kdf->keylength) != (int)keylen)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_KEYLENGTH);
        goto err;
    }

    if (kdf->prf)
        prf_nid = OBJ_obj2nid(kdf->prf->algorithm);
    else
        prf_nid = NID_hmacWithSHA1;

    if (!EVP_PBE_find(EVP_PBE_TYPE_PRF, prf_nid, NULL, &hmac_md_nid, 0)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_PRF);
        goto err;
    }

    (void)ERR_set_mark();
    prfmd = prfmd_fetch = EVP_MD_fetch(libctx, OBJ_nid2sn(hmac_md_nid), propq);
    if (prfmd == NULL)
        prfmd = EVP_get_digestbynid(hmac_md_nid);
    if (prfmd == NULL) {
        (void)ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_PRF);
        goto err;
    }
    (void)ERR_pop_to_mark();

    if (kdf->salt->type != V_ASN1_OCTET_STRING) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UNSUPPORTED_SALT_TYPE);
        goto err;
    }

    /* it seems that its all OK */
    salt = kdf->salt->value.octet_string->data;
    saltlen = kdf->salt->value.octet_string->length;
    iter = ASN1_INTEGER_get(kdf->iter);
    if (!ossl_pkcs5_pbkdf2_hmac_ex(pass, passlen, salt, saltlen, iter, prfmd,
                                   keylen, key, libctx, propq))
        goto err;
    rv = EVP_CipherInit_ex(ctx, NULL, NULL, key, NULL, en_de);
 err:
    OPENSSL_cleanse(key, keylen);
    PBKDF2PARAM_free(kdf);
    EVP_MD_free(prfmd_fetch);
    return rv;
}

int PKCS5_v2_PBKDF2_keyivgen(EVP_CIPHER_CTX *ctx, const char *pass,
                             int passlen, ASN1_TYPE *param,
                             const EVP_CIPHER *c, const EVP_MD *md, int en_de)
{
    return PKCS5_v2_PBKDF2_keyivgen_ex(ctx, pass, passlen, param, c, md, en_de,
                                       NULL, NULL);
}
