/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/x509.h>
#include <openssl/engine.h>
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "crypto/x509.h"
#include <openssl/rsa.h>
#include <openssl/dsa.h>
#include <openssl/decoder.h>
#include <openssl/encoder.h>
#include "internal/provider.h"
#include "internal/sizes.h"

struct X509_pubkey_st {
    X509_ALGOR *algor;
    ASN1_BIT_STRING *public_key;

    EVP_PKEY *pkey;

    /* extra data for the callback, used by d2i_PUBKEY_ex */
    OSSL_LIB_CTX *libctx;
    char *propq;

    /* Flag to force legacy keys */
    unsigned int flag_force_legacy : 1;
};

static int x509_pubkey_decode(EVP_PKEY **pk, const X509_PUBKEY *key);

static int x509_pubkey_set0_libctx(X509_PUBKEY *x, OSSL_LIB_CTX *libctx,
                                   const char *propq)
{
    if (x != NULL) {
        x->libctx = libctx;
        OPENSSL_free(x->propq);
        x->propq = NULL;
        if (propq != NULL) {
            x->propq = OPENSSL_strdup(propq);
            if (x->propq == NULL)
                return 0;
        }
    }
    return 1;
}

ASN1_SEQUENCE(X509_PUBKEY_INTERNAL) = {
        ASN1_SIMPLE(X509_PUBKEY, algor, X509_ALGOR),
        ASN1_SIMPLE(X509_PUBKEY, public_key, ASN1_BIT_STRING)
} static_ASN1_SEQUENCE_END_name(X509_PUBKEY, X509_PUBKEY_INTERNAL)

X509_PUBKEY *ossl_d2i_X509_PUBKEY_INTERNAL(const unsigned char **pp,
                                           long len, OSSL_LIB_CTX *libctx,
                                           const char *propq)
{
    X509_PUBKEY *xpub = OPENSSL_zalloc(sizeof(*xpub));

    if (xpub == NULL)
        return NULL;
    return (X509_PUBKEY *)ASN1_item_d2i_ex((ASN1_VALUE **)&xpub, pp, len,
                                           ASN1_ITEM_rptr(X509_PUBKEY_INTERNAL),
                                           libctx, propq);
}

void ossl_X509_PUBKEY_INTERNAL_free(X509_PUBKEY *xpub)
{
    ASN1_item_free((ASN1_VALUE *)xpub, ASN1_ITEM_rptr(X509_PUBKEY_INTERNAL));
}

static void x509_pubkey_ex_free(ASN1_VALUE **pval, const ASN1_ITEM *it)
{
    X509_PUBKEY *pubkey;

    if (pval != NULL && (pubkey = (X509_PUBKEY *)*pval) != NULL) {
        X509_ALGOR_free(pubkey->algor);
        ASN1_BIT_STRING_free(pubkey->public_key);
        EVP_PKEY_free(pubkey->pkey);
        OPENSSL_free(pubkey->propq);
        OPENSSL_free(pubkey);
        *pval = NULL;
    }
}

static int x509_pubkey_ex_populate(ASN1_VALUE **pval, const ASN1_ITEM *it)
{
    X509_PUBKEY *pubkey = (X509_PUBKEY *)*pval;

    return (pubkey->algor != NULL
            || (pubkey->algor = X509_ALGOR_new()) != NULL)
        && (pubkey->public_key != NULL
            || (pubkey->public_key = ASN1_BIT_STRING_new()) != NULL);
}


static int x509_pubkey_ex_new_ex(ASN1_VALUE **pval, const ASN1_ITEM *it,
                                 OSSL_LIB_CTX *libctx, const char *propq)
{
    X509_PUBKEY *ret;

    if ((ret = OPENSSL_zalloc(sizeof(*ret))) == NULL)
        return 0;
    if (!x509_pubkey_ex_populate((ASN1_VALUE **)&ret, NULL)
        || !x509_pubkey_set0_libctx(ret, libctx, propq)) {
        x509_pubkey_ex_free((ASN1_VALUE **)&ret, NULL);
        ret = NULL;
        ERR_raise(ERR_LIB_ASN1, ERR_R_X509_LIB);
    } else {
        *pval = (ASN1_VALUE *)ret;
    }

    return ret != NULL;
}

static int x509_pubkey_ex_d2i_ex(ASN1_VALUE **pval,
                                 const unsigned char **in, long len,
                                 const ASN1_ITEM *it, int tag, int aclass,
                                 char opt, ASN1_TLC *ctx, OSSL_LIB_CTX *libctx,
                                 const char *propq)
{
    const unsigned char *in_saved = *in;
    size_t publen;
    X509_PUBKEY *pubkey;
    int ret;
    OSSL_DECODER_CTX *dctx = NULL;
    unsigned char *tmpbuf = NULL;

    if (*pval == NULL && !x509_pubkey_ex_new_ex(pval, it, libctx, propq))
        return 0;
    if (!x509_pubkey_ex_populate(pval, NULL)) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_X509_LIB);
        return 0;
    }

    /* This ensures that |*in| advances properly no matter what */
    if ((ret = ASN1_item_ex_d2i(pval, in, len,
                                ASN1_ITEM_rptr(X509_PUBKEY_INTERNAL),
                                tag, aclass, opt, ctx)) <= 0)
        return ret;

    publen = *in - in_saved;
    if (!ossl_assert(publen > 0)) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    pubkey = (X509_PUBKEY *)*pval;
    EVP_PKEY_free(pubkey->pkey);
    pubkey->pkey = NULL;

    /*
     * Opportunistically decode the key but remove any non fatal errors
     * from the queue. Subsequent explicit attempts to decode/use the key
     * will return an appropriate error.
     */
    ERR_set_mark();

    /*
     * Try to decode with legacy method first.  This ensures that engines
     * aren't overridden by providers.
     */
    if ((ret = x509_pubkey_decode(&pubkey->pkey, pubkey)) == -1) {
        /* -1 indicates a fatal error, like malloc failure */
        ERR_clear_last_mark();
        goto end;
    }

    /* Try to decode it into an EVP_PKEY with OSSL_DECODER */
    if (ret <= 0 && !pubkey->flag_force_legacy) {
        const unsigned char *p;
        char txtoidname[OSSL_MAX_NAME_SIZE];
        size_t slen = publen;

        /*
        * The decoders don't know how to handle anything other than Universal
        * class so we modify the data accordingly.
        */
        if (aclass != V_ASN1_UNIVERSAL) {
            tmpbuf = OPENSSL_memdup(in_saved, publen);
            if (tmpbuf == NULL)
                return 0;
            in_saved = tmpbuf;
            *tmpbuf = V_ASN1_CONSTRUCTED | V_ASN1_SEQUENCE;
        }
        p = in_saved;

        if (OBJ_obj2txt(txtoidname, sizeof(txtoidname),
                        pubkey->algor->algorithm, 0) <= 0) {
            ERR_clear_last_mark();
            goto end;
        }
        if ((dctx =
             OSSL_DECODER_CTX_new_for_pkey(&pubkey->pkey,
                                           "DER", "SubjectPublicKeyInfo",
                                           txtoidname, EVP_PKEY_PUBLIC_KEY,
                                           pubkey->libctx,
                                           pubkey->propq)) != NULL)
            /*
             * As said higher up, we're being opportunistic.  In other words,
             * we don't care if we fail.
             */
            if (OSSL_DECODER_from_data(dctx, &p, &slen)) {
                if (slen != 0) {
                    /*
                     * If we successfully decoded then we *must* consume all the
                     * bytes.
                     */
                    ERR_clear_last_mark();
                    ERR_raise(ERR_LIB_ASN1, EVP_R_DECODE_ERROR);
                    goto end;
                }
            }
    }

    ERR_pop_to_mark();
    ret = 1;
 end:
    OSSL_DECODER_CTX_free(dctx);
    OPENSSL_free(tmpbuf);
    return ret;
}

static int x509_pubkey_ex_i2d(const ASN1_VALUE **pval, unsigned char **out,
                              const ASN1_ITEM *it, int tag, int aclass)
{
    return ASN1_item_ex_i2d(pval, out, ASN1_ITEM_rptr(X509_PUBKEY_INTERNAL),
                            tag, aclass);
}

static int x509_pubkey_ex_print(BIO *out, const ASN1_VALUE **pval, int indent,
                                const char *fname, const ASN1_PCTX *pctx)
{
    return ASN1_item_print(out, *pval, indent,
                           ASN1_ITEM_rptr(X509_PUBKEY_INTERNAL), pctx);
}

static const ASN1_EXTERN_FUNCS x509_pubkey_ff = {
    NULL,
    NULL,
    x509_pubkey_ex_free,
    0,                          /* Default clear behaviour is OK */
    NULL,
    x509_pubkey_ex_i2d,
    x509_pubkey_ex_print,
    x509_pubkey_ex_new_ex,
    x509_pubkey_ex_d2i_ex,
};

IMPLEMENT_EXTERN_ASN1(X509_PUBKEY, V_ASN1_SEQUENCE, x509_pubkey_ff)
IMPLEMENT_ASN1_FUNCTIONS(X509_PUBKEY)

X509_PUBKEY *X509_PUBKEY_new_ex(OSSL_LIB_CTX *libctx, const char *propq)
{
    X509_PUBKEY *pubkey = NULL;

    pubkey = (X509_PUBKEY *)ASN1_item_new_ex(X509_PUBKEY_it(), libctx, propq);
    if (!x509_pubkey_set0_libctx(pubkey, libctx, propq)) {
        X509_PUBKEY_free(pubkey);
        pubkey = NULL;
    }
    return pubkey;
}

/*
 * X509_PUBKEY_dup() must be implemented manually, because there is no
 * support for it in ASN1_EXTERN_FUNCS.
 */
X509_PUBKEY *X509_PUBKEY_dup(const X509_PUBKEY *a)
{
    X509_PUBKEY *pubkey = OPENSSL_zalloc(sizeof(*pubkey));

    if (pubkey == NULL)
        return NULL;
    if (!x509_pubkey_set0_libctx(pubkey, a->libctx, a->propq)) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        x509_pubkey_ex_free((ASN1_VALUE **)&pubkey,
                            ASN1_ITEM_rptr(X509_PUBKEY_INTERNAL));
        return NULL;
    }
    if ((pubkey->algor = X509_ALGOR_dup(a->algor)) == NULL
        || (pubkey->public_key = ASN1_BIT_STRING_new()) == NULL
        || !ASN1_BIT_STRING_set(pubkey->public_key,
                                a->public_key->data,
                                a->public_key->length)) {
        x509_pubkey_ex_free((ASN1_VALUE **)&pubkey,
                            ASN1_ITEM_rptr(X509_PUBKEY_INTERNAL));
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        return NULL;
    }

    if (a->pkey != NULL) {
        ERR_set_mark();
        pubkey->pkey = EVP_PKEY_dup(a->pkey);
        if (pubkey->pkey == NULL) {
            pubkey->flag_force_legacy = 1;
            if (x509_pubkey_decode(&pubkey->pkey, pubkey) <= 0) {
                x509_pubkey_ex_free((ASN1_VALUE **)&pubkey,
                                    ASN1_ITEM_rptr(X509_PUBKEY_INTERNAL));
                ERR_clear_last_mark();
                return NULL;
            }
        }
        ERR_pop_to_mark();
    }
    return pubkey;
}

int X509_PUBKEY_set(X509_PUBKEY **x, EVP_PKEY *pkey)
{
    X509_PUBKEY *pk = NULL;

    if (x == NULL || pkey == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (pkey->ameth != NULL) {
        if ((pk = X509_PUBKEY_new()) == NULL) {
            ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
            goto error;
        }
        if (pkey->ameth->pub_encode != NULL) {
            if (!pkey->ameth->pub_encode(pk, pkey)) {
                ERR_raise(ERR_LIB_X509, X509_R_PUBLIC_KEY_ENCODE_ERROR);
                goto error;
            }
        } else {
            ERR_raise(ERR_LIB_X509, X509_R_METHOD_NOT_SUPPORTED);
            goto error;
        }
    } else if (evp_pkey_is_provided(pkey)) {
        unsigned char *der = NULL;
        size_t derlen = 0;
        OSSL_ENCODER_CTX *ectx =
            OSSL_ENCODER_CTX_new_for_pkey(pkey, EVP_PKEY_PUBLIC_KEY,
                                          "DER", "SubjectPublicKeyInfo",
                                          NULL);

        if (OSSL_ENCODER_to_data(ectx, &der, &derlen)) {
            const unsigned char *pder = der;

            pk = d2i_X509_PUBKEY(NULL, &pder, (long)derlen);
        }

        OSSL_ENCODER_CTX_free(ectx);
        OPENSSL_free(der);
    }

    if (pk == NULL) {
        ERR_raise(ERR_LIB_X509, X509_R_UNSUPPORTED_ALGORITHM);
        goto error;
    }

    X509_PUBKEY_free(*x);
    if (!EVP_PKEY_up_ref(pkey)) {
        ERR_raise(ERR_LIB_X509, ERR_R_INTERNAL_ERROR);
        goto error;
    }
    *x = pk;

    /*
     * pk->pkey is NULL when using the legacy routine, but is non-NULL when
     * going through the encoder, and for all intents and purposes, it's
     * a perfect copy of the public key portions of |pkey|, just not the same
     * instance.  If that's all there was to pkey then we could simply return
     * early, right here. However, some application might very well depend on
     * the passed |pkey| being used and none other, so we spend a few more
     * cycles throwing away the newly created |pk->pkey| and replace it with
     * |pkey|.
     */
    if (pk->pkey != NULL)
        EVP_PKEY_free(pk->pkey);

    pk->pkey = pkey;
    return 1;

 error:
    X509_PUBKEY_free(pk);
    return 0;
}

/*
 * Attempt to decode a public key.
 * Returns 1 on success, 0 for a decode failure and -1 for a fatal
 * error e.g. malloc failure.
 *
 * This function is #legacy.
 */
static int x509_pubkey_decode(EVP_PKEY **ppkey, const X509_PUBKEY *key)
{
    EVP_PKEY *pkey;
    int nid;

    nid = OBJ_obj2nid(key->algor->algorithm);
    if (!key->flag_force_legacy) {
#ifndef OPENSSL_NO_ENGINE
        ENGINE *e = NULL;

        e = ENGINE_get_pkey_meth_engine(nid);
        if (e == NULL)
            return 0;
        ENGINE_finish(e);
#else
        return 0;
#endif
    }

    pkey = EVP_PKEY_new();
    if (pkey == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_EVP_LIB);
        return -1;
    }

    if (!EVP_PKEY_set_type(pkey, nid)) {
        ERR_raise(ERR_LIB_X509, X509_R_UNSUPPORTED_ALGORITHM);
        goto error;
    }

    if (pkey->ameth->pub_decode) {
        /*
         * Treat any failure of pub_decode as a decode error. In
         * future we could have different return codes for decode
         * errors and fatal errors such as malloc failure.
         */
        if (!pkey->ameth->pub_decode(pkey, key))
            goto error;
    } else {
        ERR_raise(ERR_LIB_X509, X509_R_METHOD_NOT_SUPPORTED);
        goto error;
    }

    *ppkey = pkey;
    return 1;

 error:
    EVP_PKEY_free(pkey);
    return 0;
}

EVP_PKEY *X509_PUBKEY_get0(const X509_PUBKEY *key)
{
    if (key == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if (key->pkey == NULL) {
        /* We failed to decode the key when we loaded it, or it was never set */
        ERR_raise(ERR_LIB_EVP, EVP_R_DECODE_ERROR);
        return NULL;
    }

    return key->pkey;
}

EVP_PKEY *X509_PUBKEY_get(const X509_PUBKEY *key)
{
    EVP_PKEY *ret = X509_PUBKEY_get0(key);

    if (ret != NULL && !EVP_PKEY_up_ref(ret)) {
        ERR_raise(ERR_LIB_X509, ERR_R_INTERNAL_ERROR);
        ret = NULL;
    }
    return ret;
}

/*
 * Now three pseudo ASN1 routines that take an EVP_PKEY structure and encode
 * or decode as X509_PUBKEY
 */
static EVP_PKEY *d2i_PUBKEY_int(EVP_PKEY **a,
                                const unsigned char **pp, long length,
                                OSSL_LIB_CTX *libctx, const char *propq,
                                unsigned int force_legacy,
                                X509_PUBKEY *
                                (*d2i_x509_pubkey)(X509_PUBKEY **a,
                                                   const unsigned char **in,
                                                   long len))
{
    X509_PUBKEY *xpk, *xpk2 = NULL, **pxpk = NULL;
    EVP_PKEY *pktmp = NULL;
    const unsigned char *q;

    q = *pp;

    /*
     * If libctx or propq are non-NULL, we take advantage of the reuse
     * feature.  It's not generally recommended, but is safe enough for
     * newly created structures.
     */
    if (libctx != NULL || propq != NULL || force_legacy) {
        xpk2 = OPENSSL_zalloc(sizeof(*xpk2));
        if (xpk2 == NULL)
            return NULL;
        if (!x509_pubkey_set0_libctx(xpk2, libctx, propq))
            goto end;
        xpk2->flag_force_legacy = !!force_legacy;
        pxpk = &xpk2;
    }
    xpk = d2i_x509_pubkey(pxpk, &q, length);
    if (xpk == NULL)
        goto end;
    pktmp = X509_PUBKEY_get(xpk);
    X509_PUBKEY_free(xpk);
    xpk2 = NULL;                 /* We know that xpk == xpk2 */
    if (pktmp == NULL)
        goto end;
    *pp = q;
    if (a != NULL) {
        EVP_PKEY_free(*a);
        *a = pktmp;
    }
 end:
    X509_PUBKEY_free(xpk2);
    return pktmp;
}

/* For the algorithm specific d2i functions further down */
EVP_PKEY *ossl_d2i_PUBKEY_legacy(EVP_PKEY **a, const unsigned char **pp,
                                 long length)
{
    return d2i_PUBKEY_int(a, pp, length, NULL, NULL, 1, d2i_X509_PUBKEY);
}

EVP_PKEY *d2i_PUBKEY_ex(EVP_PKEY **a, const unsigned char **pp, long length,
                        OSSL_LIB_CTX *libctx, const char *propq)
{
    return d2i_PUBKEY_int(a, pp, length, libctx, propq, 0, d2i_X509_PUBKEY);
}

EVP_PKEY *d2i_PUBKEY(EVP_PKEY **a, const unsigned char **pp, long length)
{
    return d2i_PUBKEY_ex(a, pp, length, NULL, NULL);
}

int i2d_PUBKEY(const EVP_PKEY *a, unsigned char **pp)
{
    int ret = -1;

    if (a == NULL)
        return 0;
    if (a->ameth != NULL) {
        X509_PUBKEY *xpk = NULL;

        if ((xpk = X509_PUBKEY_new()) == NULL)
            return -1;

        /* pub_encode() only encode parameters, not the key itself */
        if (a->ameth->pub_encode != NULL && a->ameth->pub_encode(xpk, a)) {
            xpk->pkey = (EVP_PKEY *)a;
            ret = i2d_X509_PUBKEY(xpk, pp);
            xpk->pkey = NULL;
        }
        X509_PUBKEY_free(xpk);
    } else if (a->keymgmt != NULL) {
        OSSL_ENCODER_CTX *ctx =
            OSSL_ENCODER_CTX_new_for_pkey(a, EVP_PKEY_PUBLIC_KEY,
                                          "DER", "SubjectPublicKeyInfo",
                                          NULL);
        BIO *out = BIO_new(BIO_s_mem());
        BUF_MEM *buf = NULL;

        if (OSSL_ENCODER_CTX_get_num_encoders(ctx) != 0
            && out != NULL
            && OSSL_ENCODER_to_bio(ctx, out)
            && BIO_get_mem_ptr(out, &buf) > 0) {
            ret = buf->length;

            if (pp != NULL) {
                if (*pp == NULL) {
                    *pp = (unsigned char *)buf->data;
                    buf->length = 0;
                    buf->data = NULL;
                } else {
                    memcpy(*pp, buf->data, ret);
                    *pp += ret;
                }
            }
        }
        BIO_free(out);
        OSSL_ENCODER_CTX_free(ctx);
    }

    return ret;
}

/*
 * The following are equivalents but which return RSA and DSA keys
 */
RSA *d2i_RSA_PUBKEY(RSA **a, const unsigned char **pp, long length)
{
    EVP_PKEY *pkey;
    RSA *key = NULL;
    const unsigned char *q;

    q = *pp;
    pkey = ossl_d2i_PUBKEY_legacy(NULL, &q, length);
    if (pkey == NULL)
        return NULL;
    key = EVP_PKEY_get1_RSA(pkey);
    EVP_PKEY_free(pkey);
    if (key == NULL)
        return NULL;
    *pp = q;
    if (a != NULL) {
        RSA_free(*a);
        *a = key;
    }
    return key;
}

int i2d_RSA_PUBKEY(const RSA *a, unsigned char **pp)
{
    EVP_PKEY *pktmp;
    int ret;
    if (!a)
        return 0;
    pktmp = EVP_PKEY_new();
    if (pktmp == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return -1;
    }
    (void)EVP_PKEY_assign_RSA(pktmp, (RSA *)a);
    ret = i2d_PUBKEY(pktmp, pp);
    pktmp->pkey.ptr = NULL;
    EVP_PKEY_free(pktmp);
    return ret;
}

#ifndef OPENSSL_NO_DH
DH *ossl_d2i_DH_PUBKEY(DH **a, const unsigned char **pp, long length)
{
    EVP_PKEY *pkey;
    DH *key = NULL;
    const unsigned char *q;

    q = *pp;
    pkey = ossl_d2i_PUBKEY_legacy(NULL, &q, length);
    if (pkey == NULL)
        return NULL;
    if (EVP_PKEY_get_id(pkey) == EVP_PKEY_DH)
        key = EVP_PKEY_get1_DH(pkey);
    EVP_PKEY_free(pkey);
    if (key == NULL)
        return NULL;
    *pp = q;
    if (a != NULL) {
        DH_free(*a);
        *a = key;
    }
    return key;
}

int ossl_i2d_DH_PUBKEY(const DH *a, unsigned char **pp)
{
    EVP_PKEY *pktmp;
    int ret;
    if (!a)
        return 0;
    pktmp = EVP_PKEY_new();
    if (pktmp == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return -1;
    }
    (void)EVP_PKEY_assign_DH(pktmp, (DH *)a);
    ret = i2d_PUBKEY(pktmp, pp);
    pktmp->pkey.ptr = NULL;
    EVP_PKEY_free(pktmp);
    return ret;
}

DH *ossl_d2i_DHx_PUBKEY(DH **a, const unsigned char **pp, long length)
{
    EVP_PKEY *pkey;
    DH *key = NULL;
    const unsigned char *q;

    q = *pp;
    pkey = ossl_d2i_PUBKEY_legacy(NULL, &q, length);
    if (pkey == NULL)
        return NULL;
    if (EVP_PKEY_get_id(pkey) == EVP_PKEY_DHX)
        key = EVP_PKEY_get1_DH(pkey);
    EVP_PKEY_free(pkey);
    if (key == NULL)
        return NULL;
    *pp = q;
    if (a != NULL) {
        DH_free(*a);
        *a = key;
    }
    return key;
}

int ossl_i2d_DHx_PUBKEY(const DH *a, unsigned char **pp)
{
    EVP_PKEY *pktmp;
    int ret;
    if (!a)
        return 0;
    pktmp = EVP_PKEY_new();
    if (pktmp == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return -1;
    }
    (void)EVP_PKEY_assign(pktmp, EVP_PKEY_DHX, (DH *)a);
    ret = i2d_PUBKEY(pktmp, pp);
    pktmp->pkey.ptr = NULL;
    EVP_PKEY_free(pktmp);
    return ret;
}
#endif

#ifndef OPENSSL_NO_DSA
DSA *d2i_DSA_PUBKEY(DSA **a, const unsigned char **pp, long length)
{
    EVP_PKEY *pkey;
    DSA *key = NULL;
    const unsigned char *q;

    q = *pp;
    pkey = ossl_d2i_PUBKEY_legacy(NULL, &q, length);
    if (pkey == NULL)
        return NULL;
    key = EVP_PKEY_get1_DSA(pkey);
    EVP_PKEY_free(pkey);
    if (key == NULL)
        return NULL;
    *pp = q;
    if (a != NULL) {
        DSA_free(*a);
        *a = key;
    }
    return key;
}

/* Called from decoders; disallows provided DSA keys without parameters. */
DSA *ossl_d2i_DSA_PUBKEY(DSA **a, const unsigned char **pp, long length)
{
    DSA *key = NULL;
    const unsigned char *data;
    const BIGNUM *p, *q, *g;

    data = *pp;
    key = d2i_DSA_PUBKEY(NULL, &data, length);
    if (key == NULL)
        return NULL;
    DSA_get0_pqg(key, &p, &q, &g);
    if (p == NULL || q == NULL || g == NULL) {
        DSA_free(key);
        return NULL;
    }
    *pp = data;
    if (a != NULL) {
        DSA_free(*a);
        *a = key;
    }
    return key;
}

int i2d_DSA_PUBKEY(const DSA *a, unsigned char **pp)
{
    EVP_PKEY *pktmp;
    int ret;
    if (!a)
        return 0;
    pktmp = EVP_PKEY_new();
    if (pktmp == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return -1;
    }
    (void)EVP_PKEY_assign_DSA(pktmp, (DSA *)a);
    ret = i2d_PUBKEY(pktmp, pp);
    pktmp->pkey.ptr = NULL;
    EVP_PKEY_free(pktmp);
    return ret;
}
#endif

#ifndef OPENSSL_NO_EC
EC_KEY *d2i_EC_PUBKEY(EC_KEY **a, const unsigned char **pp, long length)
{
    EVP_PKEY *pkey;
    EC_KEY *key = NULL;
    const unsigned char *q;
    int type;

    q = *pp;
    pkey = ossl_d2i_PUBKEY_legacy(NULL, &q, length);
    if (pkey == NULL)
        return NULL;
    type = EVP_PKEY_get_id(pkey);
    if (type == EVP_PKEY_EC || type == EVP_PKEY_SM2)
        key = EVP_PKEY_get1_EC_KEY(pkey);
    EVP_PKEY_free(pkey);
    if (key == NULL)
        return NULL;
    *pp = q;
    if (a != NULL) {
        EC_KEY_free(*a);
        *a = key;
    }
    return key;
}

int i2d_EC_PUBKEY(const EC_KEY *a, unsigned char **pp)
{
    EVP_PKEY *pktmp;
    int ret;

    if (a == NULL)
        return 0;
    if ((pktmp = EVP_PKEY_new()) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return -1;
    }
    (void)EVP_PKEY_assign_EC_KEY(pktmp, (EC_KEY *)a);
    ret = i2d_PUBKEY(pktmp, pp);
    pktmp->pkey.ptr = NULL;
    EVP_PKEY_free(pktmp);
    return ret;
}

# ifndef OPENSSL_NO_ECX
ECX_KEY *ossl_d2i_ED25519_PUBKEY(ECX_KEY **a,
                                 const unsigned char **pp, long length)
{
    EVP_PKEY *pkey;
    ECX_KEY *key = NULL;
    const unsigned char *q;

    q = *pp;
    pkey = ossl_d2i_PUBKEY_legacy(NULL, &q, length);
    if (pkey == NULL)
        return NULL;
    key = ossl_evp_pkey_get1_ED25519(pkey);
    EVP_PKEY_free(pkey);
    if (key == NULL)
        return NULL;
    *pp = q;
    if (a != NULL) {
        ossl_ecx_key_free(*a);
        *a = key;
    }
    return key;
}

int ossl_i2d_ED25519_PUBKEY(const ECX_KEY *a, unsigned char **pp)
{
    EVP_PKEY *pktmp;
    int ret;

    if (a == NULL)
        return 0;
    if ((pktmp = EVP_PKEY_new()) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return -1;
    }
    (void)EVP_PKEY_assign(pktmp, EVP_PKEY_ED25519, (ECX_KEY *)a);
    ret = i2d_PUBKEY(pktmp, pp);
    pktmp->pkey.ptr = NULL;
    EVP_PKEY_free(pktmp);
    return ret;
}

ECX_KEY *ossl_d2i_ED448_PUBKEY(ECX_KEY **a,
                               const unsigned char **pp, long length)
{
    EVP_PKEY *pkey;
    ECX_KEY *key = NULL;
    const unsigned char *q;

    q = *pp;
    pkey = ossl_d2i_PUBKEY_legacy(NULL, &q, length);
    if (pkey == NULL)
        return NULL;
    if (EVP_PKEY_get_id(pkey) == EVP_PKEY_ED448)
        key = ossl_evp_pkey_get1_ED448(pkey);
    EVP_PKEY_free(pkey);
    if (key == NULL)
        return NULL;
    *pp = q;
    if (a != NULL) {
        ossl_ecx_key_free(*a);
        *a = key;
    }
    return key;
}

int ossl_i2d_ED448_PUBKEY(const ECX_KEY *a, unsigned char **pp)
{
    EVP_PKEY *pktmp;
    int ret;

    if (a == NULL)
        return 0;
    if ((pktmp = EVP_PKEY_new()) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return -1;
    }
    (void)EVP_PKEY_assign(pktmp, EVP_PKEY_ED448, (ECX_KEY *)a);
    ret = i2d_PUBKEY(pktmp, pp);
    pktmp->pkey.ptr = NULL;
    EVP_PKEY_free(pktmp);
    return ret;
}

ECX_KEY *ossl_d2i_X25519_PUBKEY(ECX_KEY **a,
                                const unsigned char **pp, long length)
{
    EVP_PKEY *pkey;
    ECX_KEY *key = NULL;
    const unsigned char *q;

    q = *pp;
    pkey = ossl_d2i_PUBKEY_legacy(NULL, &q, length);
    if (pkey == NULL)
        return NULL;
    if (EVP_PKEY_get_id(pkey) == EVP_PKEY_X25519)
        key = ossl_evp_pkey_get1_X25519(pkey);
    EVP_PKEY_free(pkey);
    if (key == NULL)
        return NULL;
    *pp = q;
    if (a != NULL) {
        ossl_ecx_key_free(*a);
        *a = key;
    }
    return key;
}

int ossl_i2d_X25519_PUBKEY(const ECX_KEY *a, unsigned char **pp)
{
    EVP_PKEY *pktmp;
    int ret;

    if (a == NULL)
        return 0;
    if ((pktmp = EVP_PKEY_new()) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return -1;
    }
    (void)EVP_PKEY_assign(pktmp, EVP_PKEY_X25519, (ECX_KEY *)a);
    ret = i2d_PUBKEY(pktmp, pp);
    pktmp->pkey.ptr = NULL;
    EVP_PKEY_free(pktmp);
    return ret;
}

ECX_KEY *ossl_d2i_X448_PUBKEY(ECX_KEY **a,
                              const unsigned char **pp, long length)
{
    EVP_PKEY *pkey;
    ECX_KEY *key = NULL;
    const unsigned char *q;

    q = *pp;
    pkey = ossl_d2i_PUBKEY_legacy(NULL, &q, length);
    if (pkey == NULL)
        return NULL;
    if (EVP_PKEY_get_id(pkey) == EVP_PKEY_X448)
        key = ossl_evp_pkey_get1_X448(pkey);
    EVP_PKEY_free(pkey);
    if (key == NULL)
        return NULL;
    *pp = q;
    if (a != NULL) {
        ossl_ecx_key_free(*a);
        *a = key;
    }
    return key;
}

int ossl_i2d_X448_PUBKEY(const ECX_KEY *a, unsigned char **pp)
{
    EVP_PKEY *pktmp;
    int ret;

    if (a == NULL)
        return 0;
    if ((pktmp = EVP_PKEY_new()) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return -1;
    }
    (void)EVP_PKEY_assign(pktmp, EVP_PKEY_X448, (ECX_KEY *)a);
    ret = i2d_PUBKEY(pktmp, pp);
    pktmp->pkey.ptr = NULL;
    EVP_PKEY_free(pktmp);
    return ret;
}

# endif /* OPENSSL_NO_ECX */
#endif

void X509_PUBKEY_set0_public_key(X509_PUBKEY *pub,
                                 unsigned char *penc, int penclen)
{
    ASN1_STRING_set0(pub->public_key, penc, penclen);
    ossl_asn1_string_set_bits_left(pub->public_key, 0);
}

int X509_PUBKEY_set0_param(X509_PUBKEY *pub, ASN1_OBJECT *aobj,
                           int ptype, void *pval,
                           unsigned char *penc, int penclen)
{
    if (!X509_ALGOR_set0(pub->algor, aobj, ptype, pval))
        return 0;
    if (penc != NULL)
        X509_PUBKEY_set0_public_key(pub, penc, penclen);
    return 1;
}

int X509_PUBKEY_get0_param(ASN1_OBJECT **ppkalg,
                           const unsigned char **pk, int *ppklen,
                           X509_ALGOR **pa, const X509_PUBKEY *pub)
{
    if (ppkalg)
        *ppkalg = pub->algor->algorithm;
    if (pk) {
        *pk = pub->public_key->data;
        *ppklen = pub->public_key->length;
    }
    if (pa)
        *pa = pub->algor;
    return 1;
}

ASN1_BIT_STRING *X509_get0_pubkey_bitstr(const X509 *x)
{
    if (x == NULL)
        return NULL;
    return x->cert_info.key->public_key;
}

/* Returns 1 for equal, 0, for non-equal, < 0 on error */
int X509_PUBKEY_eq(const X509_PUBKEY *a, const X509_PUBKEY *b)
{
    X509_ALGOR *algA, *algB;
    EVP_PKEY *pA, *pB;

    if (a == b)
        return 1;
    if (a == NULL || b == NULL)
        return 0;
    if (!X509_PUBKEY_get0_param(NULL, NULL, NULL, &algA, a) || algA == NULL
        || !X509_PUBKEY_get0_param(NULL, NULL, NULL, &algB, b) || algB == NULL)
        return -2;
    if (X509_ALGOR_cmp(algA, algB) != 0)
        return 0;
    if ((pA = X509_PUBKEY_get0(a)) == NULL
        || (pB = X509_PUBKEY_get0(b)) == NULL)
        return -2;
    return EVP_PKEY_eq(pA, pB);
}

int ossl_x509_PUBKEY_get0_libctx(OSSL_LIB_CTX **plibctx, const char **ppropq,
                                 const X509_PUBKEY *key)
{
    if (plibctx)
        *plibctx = key->libctx;
    if (ppropq)
        *ppropq = key->propq;
    return 1;
}
