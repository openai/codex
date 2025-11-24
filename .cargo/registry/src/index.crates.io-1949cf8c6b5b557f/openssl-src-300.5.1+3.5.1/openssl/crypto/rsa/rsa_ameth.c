/*
 * Copyright 2006-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/x509.h>
#include <openssl/bn.h>
#include <openssl/core_names.h>
#include <openssl/param_build.h>
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "crypto/rsa.h"
#include "rsa_local.h"

/* Set any parameters associated with pkey */
static int rsa_param_encode(const EVP_PKEY *pkey,
                            ASN1_STRING **pstr, int *pstrtype)
{
    const RSA *rsa = pkey->pkey.rsa;

    *pstr = NULL;
    /* If RSA it's just NULL type */
    if (RSA_test_flags(rsa, RSA_FLAG_TYPE_MASK) != RSA_FLAG_TYPE_RSASSAPSS) {
        *pstrtype = V_ASN1_NULL;
        return 1;
    }
    /* If no PSS parameters we omit parameters entirely */
    if (rsa->pss == NULL) {
        *pstrtype = V_ASN1_UNDEF;
        return 1;
    }
    /* Encode PSS parameters */
    if (ASN1_item_pack(rsa->pss, ASN1_ITEM_rptr(RSA_PSS_PARAMS), pstr) == NULL)
        return 0;

    *pstrtype = V_ASN1_SEQUENCE;
    return 1;
}
/* Decode any parameters and set them in RSA structure */
static int rsa_pub_encode(X509_PUBKEY *pk, const EVP_PKEY *pkey)
{
    unsigned char *penc = NULL;
    int penclen;
    ASN1_STRING *str;
    int strtype;

    if (!rsa_param_encode(pkey, &str, &strtype))
        return 0;
    penclen = i2d_RSAPublicKey(pkey->pkey.rsa, &penc);
    if (penclen <= 0) {
        ASN1_STRING_free(str);
        return 0;
    }
    if (X509_PUBKEY_set0_param(pk, OBJ_nid2obj(pkey->ameth->pkey_id),
                               strtype, str, penc, penclen))
        return 1;

    OPENSSL_free(penc);
    ASN1_STRING_free(str);
    return 0;
}

static int rsa_pub_decode(EVP_PKEY *pkey, const X509_PUBKEY *pubkey)
{
    const unsigned char *p;
    int pklen;
    X509_ALGOR *alg;
    RSA *rsa = NULL;

    if (!X509_PUBKEY_get0_param(NULL, &p, &pklen, &alg, pubkey))
        return 0;
    if ((rsa = d2i_RSAPublicKey(NULL, &p, pklen)) == NULL)
        return 0;
    if (!ossl_rsa_param_decode(rsa, alg)) {
        RSA_free(rsa);
        return 0;
    }

    RSA_clear_flags(rsa, RSA_FLAG_TYPE_MASK);
    switch (pkey->ameth->pkey_id) {
    case EVP_PKEY_RSA:
        RSA_set_flags(rsa, RSA_FLAG_TYPE_RSA);
        break;
    case EVP_PKEY_RSA_PSS:
        RSA_set_flags(rsa, RSA_FLAG_TYPE_RSASSAPSS);
        break;
    default:
        /* Leave the type bits zero */
        break;
    }

    if (!EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, rsa)) {
        RSA_free(rsa);
        return 0;
    }
    return 1;
}

static int rsa_pub_cmp(const EVP_PKEY *a, const EVP_PKEY *b)
{
    /*
     * Don't check the public/private key, this is mostly for smart
     * cards.
     */
    if (((RSA_flags(a->pkey.rsa) & RSA_METHOD_FLAG_NO_CHECK))
            || (RSA_flags(b->pkey.rsa) & RSA_METHOD_FLAG_NO_CHECK)) {
        return 1;
    }

    if (BN_cmp(b->pkey.rsa->n, a->pkey.rsa->n) != 0
        || BN_cmp(b->pkey.rsa->e, a->pkey.rsa->e) != 0)
        return 0;
    return 1;
}

static int old_rsa_priv_decode(EVP_PKEY *pkey,
                               const unsigned char **pder, int derlen)
{
    RSA *rsa;

    if ((rsa = d2i_RSAPrivateKey(NULL, pder, derlen)) == NULL)
        return 0;
    EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, rsa);
    return 1;
}

static int old_rsa_priv_encode(const EVP_PKEY *pkey, unsigned char **pder)
{
    return i2d_RSAPrivateKey(pkey->pkey.rsa, pder);
}

static int rsa_priv_encode(PKCS8_PRIV_KEY_INFO *p8, const EVP_PKEY *pkey)
{
    unsigned char *rk = NULL;
    int rklen;
    ASN1_STRING *str;
    int strtype;

    if (!rsa_param_encode(pkey, &str, &strtype))
        return 0;
    rklen = i2d_RSAPrivateKey(pkey->pkey.rsa, &rk);

    if (rklen <= 0) {
        ERR_raise(ERR_LIB_RSA, ERR_R_ASN1_LIB);
        ASN1_STRING_free(str);
        return 0;
    }

    if (!PKCS8_pkey_set0(p8, OBJ_nid2obj(pkey->ameth->pkey_id), 0,
                         strtype, str, rk, rklen)) {
        ERR_raise(ERR_LIB_RSA, ERR_R_ASN1_LIB);
        ASN1_STRING_free(str);
        OPENSSL_clear_free(rk, rklen);
        return 0;
    }

    return 1;
}

static int rsa_priv_decode(EVP_PKEY *pkey, const PKCS8_PRIV_KEY_INFO *p8)
{
    int ret = 0;
    RSA *rsa = ossl_rsa_key_from_pkcs8(p8, NULL, NULL);

    if (rsa != NULL) {
        ret = 1;
        EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, rsa);
    }
    return ret;
}

static int int_rsa_size(const EVP_PKEY *pkey)
{
    return RSA_size(pkey->pkey.rsa);
}

static int rsa_bits(const EVP_PKEY *pkey)
{
    return BN_num_bits(pkey->pkey.rsa->n);
}

static int rsa_security_bits(const EVP_PKEY *pkey)
{
    return RSA_security_bits(pkey->pkey.rsa);
}

static void int_rsa_free(EVP_PKEY *pkey)
{
    RSA_free(pkey->pkey.rsa);
}

static int rsa_pss_param_print(BIO *bp, int pss_key, RSA_PSS_PARAMS *pss,
                               int indent)
{
    int rv = 0;
    X509_ALGOR *maskHash = NULL;

    if (!BIO_indent(bp, indent, 128))
        goto err;
    if (pss_key) {
        if (pss == NULL) {
            if (BIO_puts(bp, "No PSS parameter restrictions\n") <= 0)
                return 0;
            return 1;
        } else {
            if (BIO_puts(bp, "PSS parameter restrictions:") <= 0)
                return 0;
        }
    } else if (pss == NULL) {
        if (BIO_puts(bp, "(INVALID PSS PARAMETERS)\n") <= 0)
            return 0;
        return 1;
    }
    if (BIO_puts(bp, "\n") <= 0)
        goto err;
    if (pss_key)
        indent += 2;
    if (!BIO_indent(bp, indent, 128))
        goto err;
    if (BIO_puts(bp, "Hash Algorithm: ") <= 0)
        goto err;

    if (pss->hashAlgorithm) {
        if (i2a_ASN1_OBJECT(bp, pss->hashAlgorithm->algorithm) <= 0)
            goto err;
    } else if (BIO_puts(bp, "sha1 (default)") <= 0) {
        goto err;
    }

    if (BIO_puts(bp, "\n") <= 0)
        goto err;

    if (!BIO_indent(bp, indent, 128))
        goto err;

    if (BIO_puts(bp, "Mask Algorithm: ") <= 0)
        goto err;
    if (pss->maskGenAlgorithm) {
        if (i2a_ASN1_OBJECT(bp, pss->maskGenAlgorithm->algorithm) <= 0)
            goto err;
        if (BIO_puts(bp, " with ") <= 0)
            goto err;
        maskHash = ossl_x509_algor_mgf1_decode(pss->maskGenAlgorithm);
        if (maskHash != NULL) {
            if (i2a_ASN1_OBJECT(bp, maskHash->algorithm) <= 0)
                goto err;
        } else if (BIO_puts(bp, "INVALID") <= 0) {
            goto err;
        }
    } else if (BIO_puts(bp, "mgf1 with sha1 (default)") <= 0) {
        goto err;
    }
    BIO_puts(bp, "\n");

    if (!BIO_indent(bp, indent, 128))
        goto err;
    if (BIO_printf(bp, "%s Salt Length: 0x", pss_key ? "Minimum" : "") <= 0)
        goto err;
    if (pss->saltLength) {
        if (i2a_ASN1_INTEGER(bp, pss->saltLength) <= 0)
            goto err;
    } else if (BIO_puts(bp, "14 (default)") <= 0) {
        goto err;
    }
    BIO_puts(bp, "\n");

    if (!BIO_indent(bp, indent, 128))
        goto err;
    if (BIO_puts(bp, "Trailer Field: 0x") <= 0)
        goto err;
    if (pss->trailerField) {
        if (i2a_ASN1_INTEGER(bp, pss->trailerField) <= 0)
            goto err;
    } else if (BIO_puts(bp, "01 (default)") <= 0) {
        goto err;
    }
    BIO_puts(bp, "\n");

    rv = 1;

 err:
    X509_ALGOR_free(maskHash);
    return rv;

}

static int pkey_rsa_print(BIO *bp, const EVP_PKEY *pkey, int off, int priv)
{
    const RSA *x = pkey->pkey.rsa;
    char *str;
    const char *s;
    int ret = 0, mod_len = 0, ex_primes;

    if (x->n != NULL)
        mod_len = BN_num_bits(x->n);
    ex_primes = sk_RSA_PRIME_INFO_num(x->prime_infos);

    if (!BIO_indent(bp, off, 128))
        goto err;

    if (BIO_printf(bp, "%s ", pkey_is_pss(pkey) ?  "RSA-PSS" : "RSA") <= 0)
        goto err;

    if (priv && x->d) {
        if (BIO_printf(bp, "Private-Key: (%d bit, %d primes)\n",
                       mod_len, ex_primes <= 0 ? 2 : ex_primes + 2) <= 0)
            goto err;
        str = "modulus:";
        s = "publicExponent:";
    } else {
        if (BIO_printf(bp, "Public-Key: (%d bit)\n", mod_len) <= 0)
            goto err;
        str = "Modulus:";
        s = "Exponent:";
    }
    if (!ASN1_bn_print(bp, str, x->n, NULL, off))
        goto err;
    if (!ASN1_bn_print(bp, s, x->e, NULL, off))
        goto err;
    if (priv) {
        int i;

        if (!ASN1_bn_print(bp, "privateExponent:", x->d, NULL, off))
            goto err;
        if (!ASN1_bn_print(bp, "prime1:", x->p, NULL, off))
            goto err;
        if (!ASN1_bn_print(bp, "prime2:", x->q, NULL, off))
            goto err;
        if (!ASN1_bn_print(bp, "exponent1:", x->dmp1, NULL, off))
            goto err;
        if (!ASN1_bn_print(bp, "exponent2:", x->dmq1, NULL, off))
            goto err;
        if (!ASN1_bn_print(bp, "coefficient:", x->iqmp, NULL, off))
            goto err;
        for (i = 0; i < sk_RSA_PRIME_INFO_num(x->prime_infos); i++) {
            /* print multi-prime info */
            BIGNUM *bn = NULL;
            RSA_PRIME_INFO *pinfo;
            int j;

            pinfo = sk_RSA_PRIME_INFO_value(x->prime_infos, i);
            for (j = 0; j < 3; j++) {
                if (!BIO_indent(bp, off, 128))
                    goto err;
                switch (j) {
                case 0:
                    if (BIO_printf(bp, "prime%d:", i + 3) <= 0)
                        goto err;
                    bn = pinfo->r;
                    break;
                case 1:
                    if (BIO_printf(bp, "exponent%d:", i + 3) <= 0)
                        goto err;
                    bn = pinfo->d;
                    break;
                case 2:
                    if (BIO_printf(bp, "coefficient%d:", i + 3) <= 0)
                        goto err;
                    bn = pinfo->t;
                    break;
                default:
                    break;
                }
                if (!ASN1_bn_print(bp, "", bn, NULL, off))
                    goto err;
            }
        }
    }
    if (pkey_is_pss(pkey) && !rsa_pss_param_print(bp, 1, x->pss, off))
        goto err;
    ret = 1;
 err:
    return ret;
}

static int rsa_pub_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                         ASN1_PCTX *ctx)
{
    return pkey_rsa_print(bp, pkey, indent, 0);
}

static int rsa_priv_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                          ASN1_PCTX *ctx)
{
    return pkey_rsa_print(bp, pkey, indent, 1);
}

static int rsa_sig_print(BIO *bp, const X509_ALGOR *sigalg,
                         const ASN1_STRING *sig, int indent, ASN1_PCTX *pctx)
{
    if (OBJ_obj2nid(sigalg->algorithm) == EVP_PKEY_RSA_PSS) {
        int rv;
        RSA_PSS_PARAMS *pss = ossl_rsa_pss_decode(sigalg);

        rv = rsa_pss_param_print(bp, 0, pss, indent);
        RSA_PSS_PARAMS_free(pss);
        if (!rv)
            return 0;
    } else if (BIO_puts(bp, "\n") <= 0) {
        return 0;
    }
    if (sig)
        return X509_signature_dump(bp, sig, indent);
    return 1;
}

static int rsa_pkey_ctrl(EVP_PKEY *pkey, int op, long arg1, void *arg2)
{
    const EVP_MD *md;
    const EVP_MD *mgf1md;
    int min_saltlen;

    switch (op) {
    case ASN1_PKEY_CTRL_DEFAULT_MD_NID:
        if (pkey->pkey.rsa->pss != NULL) {
            if (!ossl_rsa_pss_get_param(pkey->pkey.rsa->pss, &md, &mgf1md,
                                        &min_saltlen)) {
                ERR_raise(ERR_LIB_RSA, ERR_R_INTERNAL_ERROR);
                return 0;
            }
            *(int *)arg2 = EVP_MD_get_type(md);
            /* Return of 2 indicates this MD is mandatory */
            return 2;
        }
        *(int *)arg2 = NID_sha256;
        return 1;

    default:
        return -2;
    }
}

/*
 * Convert EVP_PKEY_CTX in PSS mode into corresponding algorithm parameter,
 * suitable for setting an AlgorithmIdentifier.
 */

static RSA_PSS_PARAMS *rsa_ctx_to_pss(EVP_PKEY_CTX *pkctx)
{
    const EVP_MD *sigmd, *mgf1md;
    EVP_PKEY *pk = EVP_PKEY_CTX_get0_pkey(pkctx);
    int saltlen;
    int saltlenMax = -1;
    int md_size;

    if (EVP_PKEY_CTX_get_signature_md(pkctx, &sigmd) <= 0)
        return NULL;
    md_size = EVP_MD_get_size(sigmd);
    if (md_size <= 0)
        return NULL;
    if (EVP_PKEY_CTX_get_rsa_mgf1_md(pkctx, &mgf1md) <= 0)
        return NULL;
    if (EVP_PKEY_CTX_get_rsa_pss_saltlen(pkctx, &saltlen) <= 0)
        return NULL;
    if (saltlen == RSA_PSS_SALTLEN_DIGEST) {
        saltlen = md_size;
    } else if (saltlen == RSA_PSS_SALTLEN_AUTO_DIGEST_MAX) {
        /* FIPS 186-4 section 5 "The RSA Digital Signature Algorithm",
         * subsection 5.5 "PKCS #1" says: "For RSASSA-PSS […] the length (in
         * bytes) of the salt (sLen) shall satisfy 0 <= sLen <= hLen, where
         * hLen is the length of the hash function output block (in bytes)."
         *
         * Provide a way to use at most the digest length, so that the default
         * does not violate FIPS 186-4. */
        saltlen = RSA_PSS_SALTLEN_MAX;
        saltlenMax = md_size;
    }
    if (saltlen == RSA_PSS_SALTLEN_MAX || saltlen == RSA_PSS_SALTLEN_AUTO) {
        saltlen = EVP_PKEY_get_size(pk) - md_size - 2;
        if ((EVP_PKEY_get_bits(pk) & 0x7) == 1)
            saltlen--;
        if (saltlen < 0)
            return NULL;
        if (saltlenMax >= 0 && saltlen > saltlenMax)
            saltlen = saltlenMax;
    }

    return ossl_rsa_pss_params_create(sigmd, mgf1md, saltlen);
}

RSA_PSS_PARAMS *ossl_rsa_pss_params_create(const EVP_MD *sigmd,
                                           const EVP_MD *mgf1md, int saltlen)
{
    RSA_PSS_PARAMS *pss = RSA_PSS_PARAMS_new();

    if (pss == NULL)
        goto err;
    if (saltlen != 20) {
        pss->saltLength = ASN1_INTEGER_new();
        if (pss->saltLength == NULL)
            goto err;
        if (!ASN1_INTEGER_set(pss->saltLength, saltlen))
            goto err;
    }
    if (!ossl_x509_algor_new_from_md(&pss->hashAlgorithm, sigmd))
        goto err;
    if (mgf1md == NULL)
        mgf1md = sigmd;
    if (!ossl_x509_algor_md_to_mgf1(&pss->maskGenAlgorithm, mgf1md))
        goto err;
    if (!ossl_x509_algor_new_from_md(&pss->maskHash, mgf1md))
        goto err;
    return pss;
 err:
    RSA_PSS_PARAMS_free(pss);
    return NULL;
}

ASN1_STRING *ossl_rsa_ctx_to_pss_string(EVP_PKEY_CTX *pkctx)
{
    RSA_PSS_PARAMS *pss = rsa_ctx_to_pss(pkctx);
    ASN1_STRING *os;

    if (pss == NULL)
        return NULL;

    os = ASN1_item_pack(pss, ASN1_ITEM_rptr(RSA_PSS_PARAMS), NULL);
    RSA_PSS_PARAMS_free(pss);
    return os;
}

/*
 * From PSS AlgorithmIdentifier set public key parameters. If pkey isn't NULL
 * then the EVP_MD_CTX is setup and initialised. If it is NULL parameters are
 * passed to pkctx instead.
 */

int ossl_rsa_pss_to_ctx(EVP_MD_CTX *ctx, EVP_PKEY_CTX *pkctx,
                        const X509_ALGOR *sigalg, EVP_PKEY *pkey)
{
    int rv = -1;
    int saltlen;
    const EVP_MD *mgf1md = NULL, *md = NULL;
    RSA_PSS_PARAMS *pss;

    /* Sanity check: make sure it is PSS */
    if (OBJ_obj2nid(sigalg->algorithm) != EVP_PKEY_RSA_PSS) {
        ERR_raise(ERR_LIB_RSA, RSA_R_UNSUPPORTED_SIGNATURE_TYPE);
        return -1;
    }
    /* Decode PSS parameters */
    pss = ossl_rsa_pss_decode(sigalg);

    if (!ossl_rsa_pss_get_param(pss, &md, &mgf1md, &saltlen)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PSS_PARAMETERS);
        goto err;
    }

    /* We have all parameters now set up context */
    if (pkey) {
        if (!EVP_DigestVerifyInit(ctx, &pkctx, md, NULL, pkey))
            goto err;
    } else {
        const EVP_MD *checkmd;
        if (EVP_PKEY_CTX_get_signature_md(pkctx, &checkmd) <= 0)
            goto err;
        if (EVP_MD_get_type(md) != EVP_MD_get_type(checkmd)) {
            ERR_raise(ERR_LIB_RSA, RSA_R_DIGEST_DOES_NOT_MATCH);
            goto err;
        }
    }

    if (EVP_PKEY_CTX_set_rsa_padding(pkctx, RSA_PKCS1_PSS_PADDING) <= 0)
        goto err;

    if (EVP_PKEY_CTX_set_rsa_pss_saltlen(pkctx, saltlen) <= 0)
        goto err;

    if (EVP_PKEY_CTX_set_rsa_mgf1_md(pkctx, mgf1md) <= 0)
        goto err;
    /* Carry on */
    rv = 1;

 err:
    RSA_PSS_PARAMS_free(pss);
    return rv;
}

static int rsa_pss_verify_param(const EVP_MD **pmd, const EVP_MD **pmgf1md,
                                int *psaltlen, int *ptrailerField)
{
    if (psaltlen != NULL && *psaltlen < 0) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_SALT_LENGTH);
        return 0;
    }
    /*
     * low-level routines support only trailer field 0xbc (value 1) and
     * PKCS#1 says we should reject any other value anyway.
     */
    if (ptrailerField != NULL && *ptrailerField != 1) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_TRAILER);
        return 0;
    }
    return 1;
}

int ossl_rsa_pss_get_param(const RSA_PSS_PARAMS *pss, const EVP_MD **pmd,
                           const EVP_MD **pmgf1md, int *psaltlen)
{
    /*
     * Callers do not care about the trailer field, and yet, we must
     * pass it from get_param to verify_param, since the latter checks
     * its value.
     *
     * When callers start caring, it's a simple thing to add another
     * argument to this function.
     */
    int trailerField = 0;

    return ossl_rsa_pss_get_param_unverified(pss, pmd, pmgf1md, psaltlen,
                                             &trailerField)
        && rsa_pss_verify_param(pmd, pmgf1md, psaltlen, &trailerField);
}

/*
 * Customised RSA item verification routine. This is called when a signature
 * is encountered requiring special handling. We currently only handle PSS.
 */

static int rsa_item_verify(EVP_MD_CTX *ctx, const ASN1_ITEM *it,
                           const void *asn, const X509_ALGOR *sigalg,
                           const ASN1_BIT_STRING *sig, EVP_PKEY *pkey)
{
    /* Sanity check: make sure it is PSS */
    if (OBJ_obj2nid(sigalg->algorithm) != EVP_PKEY_RSA_PSS) {
        ERR_raise(ERR_LIB_RSA, RSA_R_UNSUPPORTED_SIGNATURE_TYPE);
        return -1;
    }
    if (ossl_rsa_pss_to_ctx(ctx, NULL, sigalg, pkey) > 0) {
        /* Carry on */
        return 2;
    }
    return -1;
}

static int rsa_item_sign(EVP_MD_CTX *ctx, const ASN1_ITEM *it, const void *asn,
                         X509_ALGOR *alg1, X509_ALGOR *alg2,
                         ASN1_BIT_STRING *sig)
{
    int pad_mode;
    EVP_PKEY_CTX *pkctx = EVP_MD_CTX_get_pkey_ctx(ctx);

    if (EVP_PKEY_CTX_get_rsa_padding(pkctx, &pad_mode) <= 0)
        return 0;
    if (pad_mode == RSA_PKCS1_PADDING)
        return 2;
    if (pad_mode == RSA_PKCS1_PSS_PADDING) {
        unsigned char aid[128];
        size_t aid_len = 0;
        OSSL_PARAM params[2];

        if (evp_pkey_ctx_is_legacy(pkctx)) {
            /* No provider -> we cannot query it for algorithm ID. */
            ASN1_STRING *os1 = NULL;

            os1 = ossl_rsa_ctx_to_pss_string(pkctx);
            if (os1 == NULL)
                return 0;
            /* Duplicate parameters if we have to */
            if (alg2 != NULL) {
                ASN1_STRING *os2 = ASN1_STRING_dup(os1);

                if (os2 == NULL) {
                    ASN1_STRING_free(os1);
                    return 0;
                }
                if (!X509_ALGOR_set0(alg2, OBJ_nid2obj(EVP_PKEY_RSA_PSS),
                                     V_ASN1_SEQUENCE, os2)) {
                    ASN1_STRING_free(os1);
                    ASN1_STRING_free(os2);
                    return 0;
                }
            }
            if (!X509_ALGOR_set0(alg1, OBJ_nid2obj(EVP_PKEY_RSA_PSS),
                                 V_ASN1_SEQUENCE, os1)) {
                    ASN1_STRING_free(os1);
                    return 0;
            }
            return 3;
        }

        params[0] = OSSL_PARAM_construct_octet_string(
            OSSL_SIGNATURE_PARAM_ALGORITHM_ID, aid, sizeof(aid));
        params[1] = OSSL_PARAM_construct_end();

        if (EVP_PKEY_CTX_get_params(pkctx, params) <= 0)
            return 0;
        if ((aid_len = params[0].return_size) == 0)
            return 0;

        if (alg1 != NULL) {
            const unsigned char *pp = aid;

            if (d2i_X509_ALGOR(&alg1, &pp, aid_len) == NULL)
                return 0;
        }
        if (alg2 != NULL) {
            const unsigned char *pp = aid;

            if (d2i_X509_ALGOR(&alg2, &pp, aid_len) == NULL)
                return 0;
        }

        return 3;
    }
    return 2;
}

static int rsa_sig_info_set(X509_SIG_INFO *siginf, const X509_ALGOR *sigalg,
                            const ASN1_STRING *sig)
{
    int rv = 0;
    int mdnid, saltlen, md_size;
    uint32_t flags;
    const EVP_MD *mgf1md = NULL, *md = NULL;
    RSA_PSS_PARAMS *pss;
    int secbits;

    /* Sanity check: make sure it is PSS */
    if (OBJ_obj2nid(sigalg->algorithm) != EVP_PKEY_RSA_PSS)
        return 0;
    /* Decode PSS parameters */
    pss = ossl_rsa_pss_decode(sigalg);
    if (!ossl_rsa_pss_get_param(pss, &md, &mgf1md, &saltlen))
        goto err;
    md_size = EVP_MD_get_size(md);
    if (md_size <= 0)
        goto err;
    mdnid = EVP_MD_get_type(md);
    /*
     * For TLS need SHA256, SHA384 or SHA512, digest and MGF1 digest must
     * match and salt length must equal digest size
     */
    if ((mdnid == NID_sha256 || mdnid == NID_sha384 || mdnid == NID_sha512)
            && mdnid == EVP_MD_get_type(mgf1md)
            && saltlen == md_size)
        flags = X509_SIG_INFO_TLS;
    else
        flags = 0;
    /* Note: security bits half number of digest bits */
    secbits = md_size * 4;
    /*
     * SHA1 and MD5 are known to be broken. Reduce security bits so that
     * they're no longer accepted at security level 1. The real values don't
     * really matter as long as they're lower than 80, which is our security
     * level 1.
     * https://eprint.iacr.org/2020/014 puts a chosen-prefix attack for SHA1 at
     * 2^63.4
     * https://documents.epfl.ch/users/l/le/lenstra/public/papers/lat.pdf
     * puts a chosen-prefix attack for MD5 at 2^39.
     */
    if (mdnid == NID_sha1)
        secbits = 64;
    else if (mdnid == NID_md5_sha1)
        secbits = 68;
    else if (mdnid == NID_md5)
        secbits = 39;
    X509_SIG_INFO_set(siginf, mdnid, EVP_PKEY_RSA_PSS, secbits,
                      flags);
    rv = 1;
    err:
    RSA_PSS_PARAMS_free(pss);
    return rv;
}

static int rsa_pkey_check(const EVP_PKEY *pkey)
{
    return RSA_check_key_ex(pkey->pkey.rsa, NULL);
}

static size_t rsa_pkey_dirty_cnt(const EVP_PKEY *pkey)
{
    return pkey->pkey.rsa->dirty_cnt;
}

/*
 * There is no need to do RSA_test_flags(rsa, RSA_FLAG_TYPE_RSASSAPSS)
 * checks in this method since the caller tests EVP_KEYMGMT_is_a() first.
 */
static int rsa_int_export_to(const EVP_PKEY *from, int rsa_type,
                             void *to_keydata,
                             OSSL_FUNC_keymgmt_import_fn *importer,
                             OSSL_LIB_CTX *libctx, const char *propq)
{
    RSA *rsa = from->pkey.rsa;
    OSSL_PARAM_BLD *tmpl = OSSL_PARAM_BLD_new();
    OSSL_PARAM *params = NULL;
    int selection = 0;
    int rv = 0;

    if (tmpl == NULL)
        return 0;
    /* Public parameters must always be present */
    if (RSA_get0_n(rsa) == NULL || RSA_get0_e(rsa) == NULL)
        goto err;

    if (!ossl_rsa_todata(rsa, tmpl, NULL, 1))
        goto err;

    selection |= OSSL_KEYMGMT_SELECT_PUBLIC_KEY;
    if (RSA_get0_d(rsa) != NULL)
        selection |= OSSL_KEYMGMT_SELECT_PRIVATE_KEY;

    if (rsa->pss != NULL) {
        const EVP_MD *md = NULL, *mgf1md = NULL;
        int md_nid, mgf1md_nid, saltlen, trailerfield;
        RSA_PSS_PARAMS_30 pss_params;

        if (!ossl_rsa_pss_get_param_unverified(rsa->pss, &md, &mgf1md,
                                               &saltlen, &trailerfield))
            goto err;
        md_nid = EVP_MD_get_type(md);
        mgf1md_nid = EVP_MD_get_type(mgf1md);
        if (!ossl_rsa_pss_params_30_set_defaults(&pss_params)
            || !ossl_rsa_pss_params_30_set_hashalg(&pss_params, md_nid)
            || !ossl_rsa_pss_params_30_set_maskgenhashalg(&pss_params,
                                                          mgf1md_nid)
            || !ossl_rsa_pss_params_30_set_saltlen(&pss_params, saltlen)
            || !ossl_rsa_pss_params_30_todata(&pss_params, tmpl, NULL))
            goto err;
        selection |= OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS;
    }

    if ((params = OSSL_PARAM_BLD_to_param(tmpl)) == NULL)
        goto err;

    /* We export, the provider imports */
    rv = importer(to_keydata, selection, params);

 err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(tmpl);
    return rv;
}

static int rsa_int_import_from(const OSSL_PARAM params[], void *vpctx,
                               int rsa_type)
{
    EVP_PKEY_CTX *pctx = vpctx;
    EVP_PKEY *pkey = EVP_PKEY_CTX_get0_pkey(pctx);
    RSA *rsa = ossl_rsa_new_with_ctx(pctx->libctx);
    RSA_PSS_PARAMS_30 rsa_pss_params = { 0, };
    int pss_defaults_set = 0;
    int ok = 0;

    if (rsa == NULL) {
        ERR_raise(ERR_LIB_DH, ERR_R_RSA_LIB);
        return 0;
    }

    RSA_clear_flags(rsa, RSA_FLAG_TYPE_MASK);
    RSA_set_flags(rsa, rsa_type);

    if (!ossl_rsa_pss_params_30_fromdata(&rsa_pss_params, &pss_defaults_set,
                                         params, pctx->libctx))
        goto err;

    switch (rsa_type) {
    case RSA_FLAG_TYPE_RSA:
        /*
         * Were PSS parameters filled in?
         * In that case, something's wrong
         */
        if (!ossl_rsa_pss_params_30_is_unrestricted(&rsa_pss_params))
            goto err;
        break;
    case RSA_FLAG_TYPE_RSASSAPSS:
        /*
         * Were PSS parameters filled in?  In that case, create the old
         * RSA_PSS_PARAMS structure.  Otherwise, this is an unrestricted key.
         */
        if (!ossl_rsa_pss_params_30_is_unrestricted(&rsa_pss_params)) {
            /* Create the older RSA_PSS_PARAMS from RSA_PSS_PARAMS_30 data */
            int mdnid = ossl_rsa_pss_params_30_hashalg(&rsa_pss_params);
            int mgf1mdnid = ossl_rsa_pss_params_30_maskgenhashalg(&rsa_pss_params);
            int saltlen = ossl_rsa_pss_params_30_saltlen(&rsa_pss_params);
            const EVP_MD *md = EVP_get_digestbynid(mdnid);
            const EVP_MD *mgf1md = EVP_get_digestbynid(mgf1mdnid);

            if ((rsa->pss = ossl_rsa_pss_params_create(md, mgf1md,
                                                       saltlen)) == NULL)
                goto err;
        }
        break;
    default:
        /* RSA key sub-types we don't know how to handle yet */
        goto err;
    }

    if (!ossl_rsa_fromdata(rsa, params, 1))
        goto err;

    switch (rsa_type) {
    case RSA_FLAG_TYPE_RSA:
        ok = EVP_PKEY_assign_RSA(pkey, rsa);
        break;
    case RSA_FLAG_TYPE_RSASSAPSS:
        ok = EVP_PKEY_assign(pkey, EVP_PKEY_RSA_PSS, rsa);
        break;
    }

 err:
    if (!ok)
        RSA_free(rsa);
    return ok;
}

static int rsa_pkey_export_to(const EVP_PKEY *from, void *to_keydata,
                              OSSL_FUNC_keymgmt_import_fn *importer,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    return rsa_int_export_to(from, RSA_FLAG_TYPE_RSA, to_keydata,
                             importer, libctx, propq);
}

static int rsa_pss_pkey_export_to(const EVP_PKEY *from, void *to_keydata,
                                  OSSL_FUNC_keymgmt_import_fn *importer,
                                  OSSL_LIB_CTX *libctx, const char *propq)
{
    return rsa_int_export_to(from, RSA_FLAG_TYPE_RSASSAPSS, to_keydata,
                             importer, libctx, propq);
}

static int rsa_pkey_import_from(const OSSL_PARAM params[], void *vpctx)
{
    return rsa_int_import_from(params, vpctx, RSA_FLAG_TYPE_RSA);
}

static int rsa_pss_pkey_import_from(const OSSL_PARAM params[], void *vpctx)
{
    return rsa_int_import_from(params, vpctx, RSA_FLAG_TYPE_RSASSAPSS);
}

static int rsa_pkey_copy(EVP_PKEY *to, EVP_PKEY *from)
{
    RSA *rsa = from->pkey.rsa;
    RSA *dupkey = NULL;
    int ret;

    if (rsa != NULL) {
        dupkey = ossl_rsa_dup(rsa, OSSL_KEYMGMT_SELECT_ALL);
        if (dupkey == NULL)
            return 0;
    }

    ret = EVP_PKEY_assign(to, from->type, dupkey);
    if (!ret)
        RSA_free(dupkey);
    return ret;
}

const EVP_PKEY_ASN1_METHOD ossl_rsa_asn1_meths[2] = {
    {
     EVP_PKEY_RSA,
     EVP_PKEY_RSA,
     ASN1_PKEY_SIGPARAM_NULL,

     "RSA",
     "OpenSSL RSA method",

     rsa_pub_decode,
     rsa_pub_encode,
     rsa_pub_cmp,
     rsa_pub_print,

     rsa_priv_decode,
     rsa_priv_encode,
     rsa_priv_print,

     int_rsa_size,
     rsa_bits,
     rsa_security_bits,

     0, 0, 0, 0, 0, 0,

     rsa_sig_print,
     int_rsa_free,
     rsa_pkey_ctrl,
     old_rsa_priv_decode,
     old_rsa_priv_encode,
     rsa_item_verify,
     rsa_item_sign,
     rsa_sig_info_set,
     rsa_pkey_check,

     0, 0,
     0, 0, 0, 0,

     rsa_pkey_dirty_cnt,
     rsa_pkey_export_to,
     rsa_pkey_import_from,
     rsa_pkey_copy
    },

    {
     EVP_PKEY_RSA2,
     EVP_PKEY_RSA,
     ASN1_PKEY_ALIAS}
};

const EVP_PKEY_ASN1_METHOD ossl_rsa_pss_asn1_meth = {
     EVP_PKEY_RSA_PSS,
     EVP_PKEY_RSA_PSS,
     ASN1_PKEY_SIGPARAM_NULL,

     "RSA-PSS",
     "OpenSSL RSA-PSS method",

     rsa_pub_decode,
     rsa_pub_encode,
     rsa_pub_cmp,
     rsa_pub_print,

     rsa_priv_decode,
     rsa_priv_encode,
     rsa_priv_print,

     int_rsa_size,
     rsa_bits,
     rsa_security_bits,

     0, 0, 0, 0, 0, 0,

     rsa_sig_print,
     int_rsa_free,
     rsa_pkey_ctrl,
     0, 0,
     rsa_item_verify,
     rsa_item_sign,
     rsa_sig_info_set,
     rsa_pkey_check,

     0, 0,
     0, 0, 0, 0,

     rsa_pkey_dirty_cnt,
     rsa_pss_pkey_export_to,
     rsa_pss_pkey_import_from,
     rsa_pkey_copy
};
