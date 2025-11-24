/*-
 * Copyright 2007-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * CRMF implementation by Martin Peylo, Miikka Viljanen, and David von Oheimb.
 */

#include <string.h>

#include <openssl/rand.h>
#include <openssl/evp.h>
#include <openssl/hmac.h>

/* explicit #includes not strictly needed since implied by the above: */
#include <openssl/asn1t.h>
#include <openssl/crmf.h>
#include <openssl/err.h>
#include <openssl/params.h>
#include <openssl/core_names.h>

#include "internal/sizes.h"

#include "crmf_local.h"

/*-
 * creates and initializes OSSL_CRMF_PBMPARAMETER (section 4.4)
 * |slen| SHOULD be at least 8 (16 is common)
 * |owfnid| e.g., NID_sha256
 * |itercnt| MUST be >= 100 (e.g., 500) and <= OSSL_CRMF_PBM_MAX_ITERATION_COUNT
 * |macnid| e.g., NID_hmac_sha1
 * returns pointer to OSSL_CRMF_PBMPARAMETER on success, NULL on error
 */
OSSL_CRMF_PBMPARAMETER *OSSL_CRMF_pbmp_new(OSSL_LIB_CTX *libctx, size_t slen,
                                           int owfnid, size_t itercnt,
                                           int macnid)
{
    OSSL_CRMF_PBMPARAMETER *pbm = NULL;
    unsigned char *salt = NULL;

    if ((pbm = OSSL_CRMF_PBMPARAMETER_new()) == NULL)
        goto err;

    /*
     * salt contains a randomly generated value used in computing the key
     * of the MAC process.  The salt SHOULD be at least 8 octets (64
     * bits) long.
     */
    if ((salt = OPENSSL_malloc(slen)) == NULL)
        goto err;
    if (RAND_bytes_ex(libctx, salt, slen, 0) <= 0) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_FAILURE_OBTAINING_RANDOM);
        goto err;
    }
    if (!ASN1_OCTET_STRING_set(pbm->salt, salt, (int)slen))
        goto err;

    /*
     * owf identifies the hash algorithm and associated parameters used to
     * compute the key used in the MAC process.  All implementations MUST
     * support SHA-1.
     */
    if (!X509_ALGOR_set0(pbm->owf, OBJ_nid2obj(owfnid), V_ASN1_UNDEF, NULL)) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_SETTING_OWF_ALGOR_FAILURE);
        goto err;
    }

    /*
     * iterationCount identifies the number of times the hash is applied
     * during the key computation process.  The iterationCount MUST be a
     * minimum of 100. Many people suggest using values as high as 1000
     * iterations as the minimum value.  The trade off here is between
     * protection of the password from attacks and the time spent by the
     * server processing all of the different iterations in deriving
     * passwords.  Hashing is generally considered a cheap operation but
     * this may not be true with all hash functions in the future.
     */
    if (itercnt < 100) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_ITERATIONCOUNT_BELOW_100);
        goto err;
    }
    if (itercnt > OSSL_CRMF_PBM_MAX_ITERATION_COUNT) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_BAD_PBM_ITERATIONCOUNT);
        goto err;
    }

    if (!ASN1_INTEGER_set(pbm->iterationCount, itercnt)) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_CRMFERROR);
        goto err;
    }

    /*
     * mac identifies the algorithm and associated parameters of the MAC
     * function to be used.  All implementations MUST support HMAC-SHA1 [HMAC].
     * All implementations SHOULD support DES-MAC and Triple-DES-MAC [PKCS11].
     */
    if (!X509_ALGOR_set0(pbm->mac, OBJ_nid2obj(macnid), V_ASN1_UNDEF, NULL)) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_SETTING_MAC_ALGOR_FAILURE);
        goto err;
    }

    OPENSSL_free(salt);
    return pbm;
 err:
    OPENSSL_free(salt);
    OSSL_CRMF_PBMPARAMETER_free(pbm);
    return NULL;
}

/*-
 * calculates the PBM based on the settings of the given OSSL_CRMF_PBMPARAMETER
 * |pbmp| identifies the algorithms, salt to use
 * |msg| message to apply the PBM for
 * |msglen| length of the message
 * |sec| key to use
 * |seclen| length of the key
 * |out| pointer to the computed mac, will be set on success
 * |outlen| if not NULL, will set variable to the length of the mac on success
 * returns 1 on success, 0 on error
 */
/* could be combined with other MAC calculations in the library */
int OSSL_CRMF_pbm_new(OSSL_LIB_CTX *libctx, const char *propq,
                      const OSSL_CRMF_PBMPARAMETER *pbmp,
                      const unsigned char *msg, size_t msglen,
                      const unsigned char *sec, size_t seclen,
                      unsigned char **out, size_t *outlen)
{
    int mac_nid, hmac_md_nid = NID_undef;
    char mdname[OSSL_MAX_NAME_SIZE];
    char hmac_mdname[OSSL_MAX_NAME_SIZE];
    EVP_MD *owf = NULL;
    EVP_MD_CTX *ctx = NULL;
    unsigned char basekey[EVP_MAX_MD_SIZE];
    unsigned int bklen = EVP_MAX_MD_SIZE;
    int64_t iterations;
    unsigned char *mac_res = 0;
    int ok = 0;

    if (out == NULL || pbmp == NULL || pbmp->mac == NULL
            || pbmp->mac->algorithm == NULL || msg == NULL || sec == NULL) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_NULL_ARGUMENT);
        goto err;
    }
    if ((mac_res = OPENSSL_malloc(EVP_MAX_MD_SIZE)) == NULL)
        goto err;

    /*
     * owf identifies the hash algorithm and associated parameters used to
     * compute the key used in the MAC process.  All implementations MUST
     * support SHA-1.
     */
    OBJ_obj2txt(mdname, sizeof(mdname), pbmp->owf->algorithm, 0);
    if ((owf = EVP_MD_fetch(libctx, mdname, propq)) == NULL) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_UNSUPPORTED_ALGORITHM);
        goto err;
    }

    if ((ctx = EVP_MD_CTX_new()) == NULL)
        goto err;

    /* compute the basekey of the salted secret */
    if (!EVP_DigestInit_ex(ctx, owf, NULL))
        goto err;
    /* first the secret */
    if (!EVP_DigestUpdate(ctx, sec, seclen))
        goto err;
    /* then the salt */
    if (!EVP_DigestUpdate(ctx, pbmp->salt->data, pbmp->salt->length))
        goto err;
    if (!EVP_DigestFinal_ex(ctx, basekey, &bklen))
        goto err;
    if (!ASN1_INTEGER_get_int64(&iterations, pbmp->iterationCount)
            || iterations < 100 /* min from RFC */
            || iterations > OSSL_CRMF_PBM_MAX_ITERATION_COUNT) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_BAD_PBM_ITERATIONCOUNT);
        goto err;
    }

    /* the first iteration was already done above */
    while (--iterations > 0) {
        if (!EVP_DigestInit_ex(ctx, owf, NULL))
            goto err;
        if (!EVP_DigestUpdate(ctx, basekey, bklen))
            goto err;
        if (!EVP_DigestFinal_ex(ctx, basekey, &bklen))
            goto err;
    }

    /*
     * mac identifies the algorithm and associated parameters of the MAC
     * function to be used.  All implementations MUST support HMAC-SHA1 [HMAC].
     * All implementations SHOULD support DES-MAC and Triple-DES-MAC [PKCS11].
     */
    mac_nid = OBJ_obj2nid(pbmp->mac->algorithm);

    if (!EVP_PBE_find(EVP_PBE_TYPE_PRF, mac_nid, NULL, &hmac_md_nid, NULL)
        || OBJ_obj2txt(hmac_mdname, sizeof(hmac_mdname),
                       OBJ_nid2obj(hmac_md_nid), 0) <= 0) {
        ERR_raise(ERR_LIB_CRMF, CRMF_R_UNSUPPORTED_ALGORITHM);
        goto err;
    }
    /* could be generalized to allow non-HMAC: */
    if (EVP_Q_mac(libctx, "HMAC", propq, hmac_mdname, NULL, basekey, bklen,
                  msg, msglen, mac_res, EVP_MAX_MD_SIZE, outlen) == NULL)
        goto err;

    ok = 1;

 err:
    OPENSSL_cleanse(basekey, bklen);
    EVP_MD_free(owf);
    EVP_MD_CTX_free(ctx);

    if (ok == 1) {
        *out = mac_res;
        return 1;
    }

    OPENSSL_free(mac_res);

    if (pbmp != NULL && pbmp->mac != NULL) {
        char buf[128];

        if (OBJ_obj2txt(buf, sizeof(buf), pbmp->mac->algorithm, 0))
            ERR_add_error_data(1, buf);
    }
    return 0;
}
