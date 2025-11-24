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

#include "internal/constant_time.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/x509.h>
#include <openssl/rsa.h>
#include <openssl/bn.h>
#include <openssl/evp.h>
#include <openssl/x509v3.h>
#include <openssl/cms.h>
#include "crypto/evp.h"
#include "crypto/rsa.h"
#include "rsa_local.h"

/* RSA pkey context structure */

typedef struct {
    /* Key gen parameters */
    int nbits;
    BIGNUM *pub_exp;
    int primes;
    /* Keygen callback info */
    int gentmp[2];
    /* RSA padding mode */
    int pad_mode;
    /* message digest */
    const EVP_MD *md;
    /* message digest for MGF1 */
    const EVP_MD *mgf1md;
    /* PSS salt length */
    int saltlen;
    /* Minimum salt length or -1 if no PSS parameter restriction */
    int min_saltlen;
    /* Temp buffer */
    unsigned char *tbuf;
    /* OAEP label */
    unsigned char *oaep_label;
    size_t oaep_labellen;
    /* if to use implicit rejection in PKCS#1 v1.5 decryption */
    int implicit_rejection;
} RSA_PKEY_CTX;

/* True if PSS parameters are restricted */
#define rsa_pss_restricted(rctx) (rctx->min_saltlen != -1)

static int pkey_rsa_init(EVP_PKEY_CTX *ctx)
{
    RSA_PKEY_CTX *rctx = OPENSSL_zalloc(sizeof(*rctx));

    if (rctx == NULL)
        return 0;
    rctx->nbits = 2048;
    rctx->primes = RSA_DEFAULT_PRIME_NUM;
    if (pkey_ctx_is_pss(ctx))
        rctx->pad_mode = RSA_PKCS1_PSS_PADDING;
    else
        rctx->pad_mode = RSA_PKCS1_PADDING;
    /* Maximum for sign, auto for verify */
    rctx->saltlen = RSA_PSS_SALTLEN_AUTO;
    rctx->min_saltlen = -1;
    rctx->implicit_rejection = 1;
    ctx->data = rctx;
    ctx->keygen_info = rctx->gentmp;
    ctx->keygen_info_count = 2;

    return 1;
}

static int pkey_rsa_copy(EVP_PKEY_CTX *dst, const EVP_PKEY_CTX *src)
{
    RSA_PKEY_CTX *dctx, *sctx;

    if (!pkey_rsa_init(dst))
        return 0;
    sctx = src->data;
    dctx = dst->data;
    dctx->nbits = sctx->nbits;
    if (sctx->pub_exp) {
        dctx->pub_exp = BN_dup(sctx->pub_exp);
        if (!dctx->pub_exp)
            return 0;
    }
    dctx->pad_mode = sctx->pad_mode;
    dctx->md = sctx->md;
    dctx->mgf1md = sctx->mgf1md;
    dctx->saltlen = sctx->saltlen;
    dctx->implicit_rejection = sctx->implicit_rejection;
    if (sctx->oaep_label) {
        OPENSSL_free(dctx->oaep_label);
        dctx->oaep_label = OPENSSL_memdup(sctx->oaep_label, sctx->oaep_labellen);
        if (!dctx->oaep_label)
            return 0;
        dctx->oaep_labellen = sctx->oaep_labellen;
    }
    return 1;
}

static int setup_tbuf(RSA_PKEY_CTX *ctx, EVP_PKEY_CTX *pk)
{
    if (ctx->tbuf != NULL)
        return 1;
    if ((ctx->tbuf =
            OPENSSL_malloc(RSA_size(EVP_PKEY_get0_RSA(pk->pkey)))) == NULL)
        return 0;
    return 1;
}

static void pkey_rsa_cleanup(EVP_PKEY_CTX *ctx)
{
    RSA_PKEY_CTX *rctx = ctx->data;
    if (rctx) {
        BN_free(rctx->pub_exp);
        OPENSSL_free(rctx->tbuf);
        OPENSSL_free(rctx->oaep_label);
        OPENSSL_free(rctx);
    }
}

static int pkey_rsa_sign(EVP_PKEY_CTX *ctx, unsigned char *sig,
                         size_t *siglen, const unsigned char *tbs,
                         size_t tbslen)
{
    int ret;
    RSA_PKEY_CTX *rctx = ctx->data;
    /*
     * Discard const. Its marked as const because this may be a cached copy of
     * the "real" key. These calls don't make any modifications that need to
     * be reflected back in the "original" key.
     */
    RSA *rsa = (RSA *)EVP_PKEY_get0_RSA(ctx->pkey);
    int md_size;

    if (rctx->md) {
        md_size = EVP_MD_get_size(rctx->md);
        if (md_size <= 0) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_DIGEST_LENGTH);
            return -1;
        }

        if (tbslen != (size_t)md_size) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_DIGEST_LENGTH);
            return -1;
        }

        if (EVP_MD_get_type(rctx->md) == NID_mdc2) {
            unsigned int sltmp;
            if (rctx->pad_mode != RSA_PKCS1_PADDING)
                return -1;
            ret = RSA_sign_ASN1_OCTET_STRING(0, tbs, tbslen, sig, &sltmp, rsa);

            if (ret <= 0)
                return ret;
            ret = sltmp;
        } else if (rctx->pad_mode == RSA_X931_PADDING) {
            if ((size_t)RSA_size(rsa) < tbslen + 1) {
                ERR_raise(ERR_LIB_RSA, RSA_R_KEY_SIZE_TOO_SMALL);
                return -1;
            }
            if (!setup_tbuf(rctx, ctx)) {
                ERR_raise(ERR_LIB_RSA, ERR_R_RSA_LIB);
                return -1;
            }
            memcpy(rctx->tbuf, tbs, tbslen);
            rctx->tbuf[tbslen] = RSA_X931_hash_id(EVP_MD_get_type(rctx->md));
            ret = RSA_private_encrypt(tbslen + 1, rctx->tbuf,
                                      sig, rsa, RSA_X931_PADDING);
        } else if (rctx->pad_mode == RSA_PKCS1_PADDING) {
            unsigned int sltmp;
            ret = RSA_sign(EVP_MD_get_type(rctx->md),
                           tbs, tbslen, sig, &sltmp, rsa);
            if (ret <= 0)
                return ret;
            ret = sltmp;
        } else if (rctx->pad_mode == RSA_PKCS1_PSS_PADDING) {
            if (!setup_tbuf(rctx, ctx))
                return -1;
            if (!RSA_padding_add_PKCS1_PSS_mgf1(rsa,
                                                rctx->tbuf, tbs,
                                                rctx->md, rctx->mgf1md,
                                                rctx->saltlen))
                return -1;
            ret = RSA_private_encrypt(RSA_size(rsa), rctx->tbuf,
                                      sig, rsa, RSA_NO_PADDING);
        } else {
            return -1;
        }
    } else {
        ret = RSA_private_encrypt(tbslen, tbs, sig, rsa, rctx->pad_mode);
    }
    if (ret < 0)
        return ret;
    *siglen = ret;
    return 1;
}

static int pkey_rsa_verifyrecover(EVP_PKEY_CTX *ctx,
                                  unsigned char *rout, size_t *routlen,
                                  const unsigned char *sig, size_t siglen)
{
    int ret;
    RSA_PKEY_CTX *rctx = ctx->data;
    /*
     * Discard const. Its marked as const because this may be a cached copy of
     * the "real" key. These calls don't make any modifications that need to
     * be reflected back in the "original" key.
     */
    RSA *rsa = (RSA *)EVP_PKEY_get0_RSA(ctx->pkey);

    if (rctx->md) {
        if (rctx->pad_mode == RSA_X931_PADDING) {
            if (!setup_tbuf(rctx, ctx))
                return -1;
            ret = RSA_public_decrypt(siglen, sig, rctx->tbuf, rsa,
                                     RSA_X931_PADDING);
            if (ret < 1)
                return 0;
            ret--;
            if (rctx->tbuf[ret] != RSA_X931_hash_id(EVP_MD_get_type(rctx->md))) {
                ERR_raise(ERR_LIB_RSA, RSA_R_ALGORITHM_MISMATCH);
                return 0;
            }
            if (ret != EVP_MD_get_size(rctx->md)) {
                ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_DIGEST_LENGTH);
                return 0;
            }
            if (rout)
                memcpy(rout, rctx->tbuf, ret);
        } else if (rctx->pad_mode == RSA_PKCS1_PADDING) {
            size_t sltmp;
            ret = ossl_rsa_verify(EVP_MD_get_type(rctx->md),
                                  NULL, 0, rout, &sltmp,
                                  sig, siglen, rsa);
            if (ret <= 0)
                return 0;
            ret = sltmp;
        } else {
            return -1;
        }
    } else {
        ret = RSA_public_decrypt(siglen, sig, rout, rsa, rctx->pad_mode);
    }
    if (ret < 0)
        return ret;
    *routlen = ret;
    return 1;
}

static int pkey_rsa_verify(EVP_PKEY_CTX *ctx,
                           const unsigned char *sig, size_t siglen,
                           const unsigned char *tbs, size_t tbslen)
{
    RSA_PKEY_CTX *rctx = ctx->data;
    /*
     * Discard const. Its marked as const because this may be a cached copy of
     * the "real" key. These calls don't make any modifications that need to
     * be reflected back in the "original" key.
     */
    RSA *rsa = (RSA *)EVP_PKEY_get0_RSA(ctx->pkey);
    size_t rslen;
    int md_size;

    if (rctx->md) {
        if (rctx->pad_mode == RSA_PKCS1_PADDING)
            return RSA_verify(EVP_MD_get_type(rctx->md), tbs, tbslen,
                              sig, siglen, rsa);
        md_size = EVP_MD_get_size(rctx->md);
        if (md_size <= 0) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_DIGEST_LENGTH);
            return -1;
        }
        if (tbslen != (size_t)md_size) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_DIGEST_LENGTH);
            return -1;
        }
        if (rctx->pad_mode == RSA_X931_PADDING) {
            if (pkey_rsa_verifyrecover(ctx, NULL, &rslen, sig, siglen) <= 0)
                return 0;
        } else if (rctx->pad_mode == RSA_PKCS1_PSS_PADDING) {
            int ret;
            if (!setup_tbuf(rctx, ctx))
                return -1;
            ret = RSA_public_decrypt(siglen, sig, rctx->tbuf,
                                     rsa, RSA_NO_PADDING);
            if (ret <= 0)
                return 0;
            ret = RSA_verify_PKCS1_PSS_mgf1(rsa, tbs,
                                            rctx->md, rctx->mgf1md,
                                            rctx->tbuf, rctx->saltlen);
            if (ret <= 0)
                return 0;
            return 1;
        } else {
            return -1;
        }
    } else {
        if (!setup_tbuf(rctx, ctx))
            return -1;
        rslen = RSA_public_decrypt(siglen, sig, rctx->tbuf,
                                   rsa, rctx->pad_mode);
        if (rslen == 0)
            return 0;
    }

    if ((rslen != tbslen) || memcmp(tbs, rctx->tbuf, rslen))
        return 0;

    return 1;

}

static int pkey_rsa_encrypt(EVP_PKEY_CTX *ctx,
                            unsigned char *out, size_t *outlen,
                            const unsigned char *in, size_t inlen)
{
    int ret;
    RSA_PKEY_CTX *rctx = ctx->data;
    /*
     * Discard const. Its marked as const because this may be a cached copy of
     * the "real" key. These calls don't make any modifications that need to
     * be reflected back in the "original" key.
     */
    RSA *rsa = (RSA *)EVP_PKEY_get0_RSA(ctx->pkey);

    if (rctx->pad_mode == RSA_PKCS1_OAEP_PADDING) {
        int klen = RSA_size(rsa);
        if (!setup_tbuf(rctx, ctx))
            return -1;
        if (!RSA_padding_add_PKCS1_OAEP_mgf1(rctx->tbuf, klen,
                                             in, inlen,
                                             rctx->oaep_label,
                                             rctx->oaep_labellen,
                                             rctx->md, rctx->mgf1md))
            return -1;
        ret = RSA_public_encrypt(klen, rctx->tbuf, out, rsa, RSA_NO_PADDING);
    } else {
        ret = RSA_public_encrypt(inlen, in, out, rsa, rctx->pad_mode);
    }
    if (ret < 0)
        return ret;
    *outlen = ret;
    return 1;
}

static int pkey_rsa_decrypt(EVP_PKEY_CTX *ctx,
                            unsigned char *out, size_t *outlen,
                            const unsigned char *in, size_t inlen)
{
    int ret;
    int pad_mode;
    RSA_PKEY_CTX *rctx = ctx->data;
    /*
     * Discard const. Its marked as const because this may be a cached copy of
     * the "real" key. These calls don't make any modifications that need to
     * be reflected back in the "original" key.
     */
    RSA *rsa = (RSA *)EVP_PKEY_get0_RSA(ctx->pkey);

    if (rctx->pad_mode == RSA_PKCS1_OAEP_PADDING) {
        if (!setup_tbuf(rctx, ctx))
            return -1;
        ret = RSA_private_decrypt(inlen, in, rctx->tbuf, rsa, RSA_NO_PADDING);
        if (ret <= 0)
            return ret;
        ret = RSA_padding_check_PKCS1_OAEP_mgf1(out, ret, rctx->tbuf,
                                                ret, ret,
                                                rctx->oaep_label,
                                                rctx->oaep_labellen,
                                                rctx->md, rctx->mgf1md);
    } else {
        if (rctx->pad_mode == RSA_PKCS1_PADDING &&
              rctx->implicit_rejection == 0)
            pad_mode = RSA_PKCS1_NO_IMPLICIT_REJECT_PADDING;
        else
            pad_mode = rctx->pad_mode;
        ret = RSA_private_decrypt(inlen, in, out, rsa, pad_mode);
    }
    *outlen = constant_time_select_s(constant_time_msb_s(ret), *outlen, ret);
    ret = constant_time_select_int(constant_time_msb(ret), ret, 1);
    return ret;
}

static int check_padding_md(const EVP_MD *md, int padding)
{
    int mdnid;

    if (!md)
        return 1;

    mdnid = EVP_MD_get_type(md);

    if (padding == RSA_NO_PADDING) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PADDING_MODE);
        return 0;
    }

    if (padding == RSA_X931_PADDING) {
        if (RSA_X931_hash_id(mdnid) == -1) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_X931_DIGEST);
            return 0;
        }
    } else {
        switch (mdnid) {
        /* List of all supported RSA digests */
        case NID_sha1:
        case NID_sha224:
        case NID_sha256:
        case NID_sha384:
        case NID_sha512:
        case NID_sha512_224:
        case NID_sha512_256:
        case NID_md5:
        case NID_md5_sha1:
        case NID_md2:
        case NID_md4:
        case NID_mdc2:
        case NID_ripemd160:
        case NID_sha3_224:
        case NID_sha3_256:
        case NID_sha3_384:
        case NID_sha3_512:
            return 1;

        default:
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_DIGEST);
            return 0;

        }
    }

    return 1;
}

static int pkey_rsa_ctrl(EVP_PKEY_CTX *ctx, int type, int p1, void *p2)
{
    RSA_PKEY_CTX *rctx = ctx->data;
    int md_size;

    switch (type) {
    case EVP_PKEY_CTRL_RSA_PADDING:
        if ((p1 >= RSA_PKCS1_PADDING) && (p1 <= RSA_PKCS1_PSS_PADDING)) {
            if (!check_padding_md(rctx->md, p1))
                return 0;
            if (p1 == RSA_PKCS1_PSS_PADDING) {
                if (!(ctx->operation &
                      (EVP_PKEY_OP_SIGN | EVP_PKEY_OP_VERIFY)))
                    goto bad_pad;
                if (!rctx->md)
                    rctx->md = EVP_sha1();
            } else if (pkey_ctx_is_pss(ctx)) {
                goto bad_pad;
            }
            if (p1 == RSA_PKCS1_OAEP_PADDING) {
                if (!(ctx->operation & EVP_PKEY_OP_TYPE_CRYPT))
                    goto bad_pad;
                if (!rctx->md)
                    rctx->md = EVP_sha1();
            }
            rctx->pad_mode = p1;
            return 1;
        }
 bad_pad:
        ERR_raise(ERR_LIB_RSA, RSA_R_ILLEGAL_OR_UNSUPPORTED_PADDING_MODE);
        return -2;

    case EVP_PKEY_CTRL_GET_RSA_PADDING:
        *(int *)p2 = rctx->pad_mode;
        return 1;

    case EVP_PKEY_CTRL_RSA_PSS_SALTLEN:
    case EVP_PKEY_CTRL_GET_RSA_PSS_SALTLEN:
        if (rctx->pad_mode != RSA_PKCS1_PSS_PADDING) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PSS_SALTLEN);
            return -2;
        }
        if (type == EVP_PKEY_CTRL_GET_RSA_PSS_SALTLEN) {
            *(int *)p2 = rctx->saltlen;
        } else {
            if (p1 < RSA_PSS_SALTLEN_MAX)
                return -2;
            if (rsa_pss_restricted(rctx)) {
                if (p1 == RSA_PSS_SALTLEN_AUTO
                    && ctx->operation == EVP_PKEY_OP_VERIFY) {
                    ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PSS_SALTLEN);
                    return -2;
                }
                md_size = EVP_MD_get_size(rctx->md);
                if (md_size <= 0) {
                    ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_DIGEST_LENGTH);
                    return -2;
                }
                if ((p1 == RSA_PSS_SALTLEN_DIGEST
                     && rctx->min_saltlen > md_size)
                    || (p1 >= 0 && p1 < rctx->min_saltlen)) {
                    ERR_raise(ERR_LIB_RSA, RSA_R_PSS_SALTLEN_TOO_SMALL);
                    return 0;
                }
            }
            rctx->saltlen = p1;
        }
        return 1;

    case EVP_PKEY_CTRL_RSA_KEYGEN_BITS:
        if (p1 < RSA_MIN_MODULUS_BITS) {
            ERR_raise(ERR_LIB_RSA, RSA_R_KEY_SIZE_TOO_SMALL);
            return -2;
        }
        rctx->nbits = p1;
        return 1;

    case EVP_PKEY_CTRL_RSA_KEYGEN_PUBEXP:
        if (p2 == NULL || !BN_is_odd((BIGNUM *)p2) || BN_is_one((BIGNUM *)p2)) {
            ERR_raise(ERR_LIB_RSA, RSA_R_BAD_E_VALUE);
            return -2;
        }
        BN_free(rctx->pub_exp);
        rctx->pub_exp = p2;
        return 1;

    case EVP_PKEY_CTRL_RSA_KEYGEN_PRIMES:
        if (p1 < RSA_DEFAULT_PRIME_NUM || p1 > RSA_MAX_PRIME_NUM) {
            ERR_raise(ERR_LIB_RSA, RSA_R_KEY_PRIME_NUM_INVALID);
            return -2;
        }
        rctx->primes = p1;
        return 1;

    case EVP_PKEY_CTRL_RSA_OAEP_MD:
    case EVP_PKEY_CTRL_GET_RSA_OAEP_MD:
        if (rctx->pad_mode != RSA_PKCS1_OAEP_PADDING) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PADDING_MODE);
            return -2;
        }
        if (type == EVP_PKEY_CTRL_GET_RSA_OAEP_MD)
            *(const EVP_MD **)p2 = rctx->md;
        else
            rctx->md = p2;
        return 1;

    case EVP_PKEY_CTRL_MD:
        if (!check_padding_md(p2, rctx->pad_mode))
            return 0;
        if (rsa_pss_restricted(rctx)) {
            if (EVP_MD_get_type(rctx->md) == EVP_MD_get_type(p2))
                return 1;
            ERR_raise(ERR_LIB_RSA, RSA_R_DIGEST_NOT_ALLOWED);
            return 0;
        }
        rctx->md = p2;
        return 1;

    case EVP_PKEY_CTRL_GET_MD:
        *(const EVP_MD **)p2 = rctx->md;
        return 1;

    case EVP_PKEY_CTRL_RSA_MGF1_MD:
    case EVP_PKEY_CTRL_GET_RSA_MGF1_MD:
        if (rctx->pad_mode != RSA_PKCS1_PSS_PADDING
            && rctx->pad_mode != RSA_PKCS1_OAEP_PADDING) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_MGF1_MD);
            return -2;
        }
        if (type == EVP_PKEY_CTRL_GET_RSA_MGF1_MD) {
            if (rctx->mgf1md)
                *(const EVP_MD **)p2 = rctx->mgf1md;
            else
                *(const EVP_MD **)p2 = rctx->md;
        } else {
            if (rsa_pss_restricted(rctx)) {
                if (EVP_MD_get_type(rctx->mgf1md) == EVP_MD_get_type(p2))
                    return 1;
                ERR_raise(ERR_LIB_RSA, RSA_R_MGF1_DIGEST_NOT_ALLOWED);
                return 0;
            }
            rctx->mgf1md = p2;
        }
        return 1;

    case EVP_PKEY_CTRL_RSA_OAEP_LABEL:
        if (rctx->pad_mode != RSA_PKCS1_OAEP_PADDING) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PADDING_MODE);
            return -2;
        }
        OPENSSL_free(rctx->oaep_label);
        if (p2 && p1 > 0) {
            rctx->oaep_label = p2;
            rctx->oaep_labellen = p1;
        } else {
            rctx->oaep_label = NULL;
            rctx->oaep_labellen = 0;
        }
        return 1;

    case EVP_PKEY_CTRL_GET_RSA_OAEP_LABEL:
        if (rctx->pad_mode != RSA_PKCS1_OAEP_PADDING) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PADDING_MODE);
            return -2;
        }
        if (p2 == NULL) {
            ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
            return 0;
        }
        *(unsigned char **)p2 = rctx->oaep_label;
        return rctx->oaep_labellen;

    case EVP_PKEY_CTRL_RSA_IMPLICIT_REJECTION:
        if (rctx->pad_mode != RSA_PKCS1_PADDING) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PADDING_MODE);
            return -2;
        }
        rctx->implicit_rejection = p1;
        return 1;

    case EVP_PKEY_CTRL_DIGESTINIT:
    case EVP_PKEY_CTRL_PKCS7_SIGN:
#ifndef OPENSSL_NO_CMS
    case EVP_PKEY_CTRL_CMS_SIGN:
#endif
    return 1;

    case EVP_PKEY_CTRL_PKCS7_ENCRYPT:
    case EVP_PKEY_CTRL_PKCS7_DECRYPT:
#ifndef OPENSSL_NO_CMS
    case EVP_PKEY_CTRL_CMS_DECRYPT:
    case EVP_PKEY_CTRL_CMS_ENCRYPT:
#endif
    if (!pkey_ctx_is_pss(ctx))
        return 1;
    /* fall through */
    case EVP_PKEY_CTRL_PEER_KEY:
        ERR_raise(ERR_LIB_RSA, RSA_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;

    default:
        return -2;

    }
}

static int pkey_rsa_ctrl_str(EVP_PKEY_CTX *ctx,
                             const char *type, const char *value)
{
    if (value == NULL) {
        ERR_raise(ERR_LIB_RSA, RSA_R_VALUE_MISSING);
        return 0;
    }
    if (strcmp(type, "rsa_padding_mode") == 0) {
        int pm;

        if (strcmp(value, "pkcs1") == 0) {
            pm = RSA_PKCS1_PADDING;
        } else if (strcmp(value, "none") == 0) {
            pm = RSA_NO_PADDING;
        } else if (strcmp(value, "oeap") == 0) {
            pm = RSA_PKCS1_OAEP_PADDING;
        } else if (strcmp(value, "oaep") == 0) {
            pm = RSA_PKCS1_OAEP_PADDING;
        } else if (strcmp(value, "x931") == 0) {
            pm = RSA_X931_PADDING;
        } else if (strcmp(value, "pss") == 0) {
            pm = RSA_PKCS1_PSS_PADDING;
        } else {
            ERR_raise(ERR_LIB_RSA, RSA_R_UNKNOWN_PADDING_TYPE);
            return -2;
        }
        return EVP_PKEY_CTX_set_rsa_padding(ctx, pm);
    }

    if (strcmp(type, "rsa_pss_saltlen") == 0) {
        int saltlen;

        if (!strcmp(value, "digest"))
            saltlen = RSA_PSS_SALTLEN_DIGEST;
        else if (!strcmp(value, "max"))
            saltlen = RSA_PSS_SALTLEN_MAX;
        else if (!strcmp(value, "auto"))
            saltlen = RSA_PSS_SALTLEN_AUTO;
        else
            saltlen = atoi(value);
        return EVP_PKEY_CTX_set_rsa_pss_saltlen(ctx, saltlen);
    }

    if (strcmp(type, "rsa_keygen_bits") == 0) {
        int nbits = atoi(value);

        return EVP_PKEY_CTX_set_rsa_keygen_bits(ctx, nbits);
    }

    if (strcmp(type, "rsa_keygen_pubexp") == 0) {
        int ret;

        BIGNUM *pubexp = NULL;
        if (!BN_asc2bn(&pubexp, value))
            return 0;
        ret = EVP_PKEY_CTX_set1_rsa_keygen_pubexp(ctx, pubexp);
        BN_free(pubexp);
        return ret;
    }

    if (strcmp(type, "rsa_keygen_primes") == 0) {
        int nprimes = atoi(value);

        return EVP_PKEY_CTX_set_rsa_keygen_primes(ctx, nprimes);
    }

    if (strcmp(type, "rsa_mgf1_md") == 0)
        return EVP_PKEY_CTX_md(ctx,
                               EVP_PKEY_OP_TYPE_SIG | EVP_PKEY_OP_TYPE_CRYPT,
                               EVP_PKEY_CTRL_RSA_MGF1_MD, value);

    if (pkey_ctx_is_pss(ctx)) {

        if (strcmp(type, "rsa_pss_keygen_mgf1_md") == 0)
            return EVP_PKEY_CTX_md(ctx, EVP_PKEY_OP_KEYGEN,
                                   EVP_PKEY_CTRL_RSA_MGF1_MD, value);

        if (strcmp(type, "rsa_pss_keygen_md") == 0)
            return EVP_PKEY_CTX_md(ctx, EVP_PKEY_OP_KEYGEN,
                                   EVP_PKEY_CTRL_MD, value);

        if (strcmp(type, "rsa_pss_keygen_saltlen") == 0) {
            int saltlen = atoi(value);

            return EVP_PKEY_CTX_set_rsa_pss_keygen_saltlen(ctx, saltlen);
        }
    }

    if (strcmp(type, "rsa_oaep_md") == 0)
        return EVP_PKEY_CTX_md(ctx, EVP_PKEY_OP_TYPE_CRYPT,
                               EVP_PKEY_CTRL_RSA_OAEP_MD, value);

    if (strcmp(type, "rsa_oaep_label") == 0) {
        unsigned char *lab;
        long lablen;
        int ret;

        lab = OPENSSL_hexstr2buf(value, &lablen);
        if (!lab)
            return 0;
        ret = EVP_PKEY_CTX_set0_rsa_oaep_label(ctx, lab, lablen);
        if (ret <= 0)
            OPENSSL_free(lab);
        return ret;
    }

    return -2;
}

/* Set PSS parameters when generating a key, if necessary */
static int rsa_set_pss_param(RSA *rsa, EVP_PKEY_CTX *ctx)
{
    RSA_PKEY_CTX *rctx = ctx->data;

    if (!pkey_ctx_is_pss(ctx))
        return 1;
    /* If all parameters are default values don't set pss */
    if (rctx->md == NULL && rctx->mgf1md == NULL && rctx->saltlen == -2)
        return 1;
    rsa->pss = ossl_rsa_pss_params_create(rctx->md, rctx->mgf1md,
                                          rctx->saltlen == -2
                                          ? 0 : rctx->saltlen);
    if (rsa->pss == NULL)
        return 0;
    return 1;
}

static int pkey_rsa_keygen(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey)
{
    RSA *rsa = NULL;
    RSA_PKEY_CTX *rctx = ctx->data;
    BN_GENCB *pcb;
    int ret;

    if (rctx->pub_exp == NULL) {
        rctx->pub_exp = BN_new();
        if (rctx->pub_exp == NULL || !BN_set_word(rctx->pub_exp, RSA_F4))
            return 0;
    }
    rsa = RSA_new();
    if (rsa == NULL)
        return 0;
    if (ctx->pkey_gencb) {
        pcb = BN_GENCB_new();
        if (pcb == NULL) {
            RSA_free(rsa);
            return 0;
        }
        evp_pkey_set_cb_translate(pcb, ctx);
    } else {
        pcb = NULL;
    }
    ret = RSA_generate_multi_prime_key(rsa, rctx->nbits, rctx->primes,
                                       rctx->pub_exp, pcb);
    BN_GENCB_free(pcb);
    if (ret > 0 && !rsa_set_pss_param(rsa, ctx)) {
        RSA_free(rsa);
        return 0;
    }
    if (ret > 0)
        EVP_PKEY_assign(pkey, ctx->pmeth->pkey_id, rsa);
    else
        RSA_free(rsa);
    return ret;
}

static const EVP_PKEY_METHOD rsa_pkey_meth = {
    EVP_PKEY_RSA,
    EVP_PKEY_FLAG_AUTOARGLEN,
    pkey_rsa_init,
    pkey_rsa_copy,
    pkey_rsa_cleanup,

    0, 0,

    0,
    pkey_rsa_keygen,

    0,
    pkey_rsa_sign,

    0,
    pkey_rsa_verify,

    0,
    pkey_rsa_verifyrecover,

    0, 0, 0, 0,

    0,
    pkey_rsa_encrypt,

    0,
    pkey_rsa_decrypt,

    0, 0,

    pkey_rsa_ctrl,
    pkey_rsa_ctrl_str
};

const EVP_PKEY_METHOD *ossl_rsa_pkey_method(void)
{
    return &rsa_pkey_meth;
}

/*
 * Called for PSS sign or verify initialisation: checks PSS parameter
 * sanity and sets any restrictions on key usage.
 */

static int pkey_pss_init(EVP_PKEY_CTX *ctx)
{
    const RSA *rsa;
    RSA_PKEY_CTX *rctx = ctx->data;
    const EVP_MD *md;
    const EVP_MD *mgf1md;
    int min_saltlen, max_saltlen, md_size;

    /* Should never happen */
    if (!pkey_ctx_is_pss(ctx))
        return 0;
    rsa = EVP_PKEY_get0_RSA(ctx->pkey);
    /* If no restrictions just return */
    if (rsa->pss == NULL)
        return 1;
    /* Get and check parameters */
    if (!ossl_rsa_pss_get_param(rsa->pss, &md, &mgf1md, &min_saltlen))
        return 0;

    /* See if minimum salt length exceeds maximum possible */
    md_size = EVP_MD_get_size(md);
    if (md_size <= 0) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_DIGEST_LENGTH);
        return 0;
    }
    max_saltlen = RSA_size(rsa) - md_size;
    if ((RSA_bits(rsa) & 0x7) == 1)
        max_saltlen--;
    if (min_saltlen > max_saltlen) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_SALT_LENGTH);
        return 0;
    }

    rctx->min_saltlen = min_saltlen;

    /*
     * Set PSS restrictions as defaults: we can then block any attempt to
     * use invalid values in pkey_rsa_ctrl
     */

    rctx->md = md;
    rctx->mgf1md = mgf1md;
    rctx->saltlen = min_saltlen;

    return 1;
}

static const EVP_PKEY_METHOD rsa_pss_pkey_meth = {
    EVP_PKEY_RSA_PSS,
    EVP_PKEY_FLAG_AUTOARGLEN,
    pkey_rsa_init,
    pkey_rsa_copy,
    pkey_rsa_cleanup,

    0, 0,

    0,
    pkey_rsa_keygen,

    pkey_pss_init,
    pkey_rsa_sign,

    pkey_pss_init,
    pkey_rsa_verify,

    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,

    pkey_rsa_ctrl,
    pkey_rsa_ctrl_str
};

const EVP_PKEY_METHOD *ossl_rsa_pss_pkey_method(void)
{
    return &rsa_pss_pkey_meth;
}
