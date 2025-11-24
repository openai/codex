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

#include <openssl/bn.h>
#include "internal/cryptlib.h"
#include "dsa_local.h"
#include "crypto/asn1_dsa.h"
#include "crypto/dsa.h"

DSA_SIG *DSA_do_sign(const unsigned char *dgst, int dlen, DSA *dsa)
{
    return dsa->meth->dsa_do_sign(dgst, dlen, dsa);
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int DSA_sign_setup(DSA *dsa, BN_CTX *ctx_in, BIGNUM **kinvp, BIGNUM **rp)
{
    return dsa->meth->dsa_sign_setup(dsa, ctx_in, kinvp, rp);
}
#endif

DSA_SIG *DSA_SIG_new(void)
{
    DSA_SIG *sig = OPENSSL_zalloc(sizeof(*sig));

    return sig;
}

void DSA_SIG_free(DSA_SIG *sig)
{
    if (sig == NULL)
        return;
    BN_clear_free(sig->r);
    BN_clear_free(sig->s);
    OPENSSL_free(sig);
}

DSA_SIG *d2i_DSA_SIG(DSA_SIG **psig, const unsigned char **ppin, long len)
{
    DSA_SIG *sig;

    if (len < 0)
        return NULL;
    if (psig != NULL && *psig != NULL) {
        sig = *psig;
    } else {
        sig = DSA_SIG_new();
        if (sig == NULL)
            return NULL;
    }
    if (sig->r == NULL)
        sig->r = BN_new();
    if (sig->s == NULL)
        sig->s = BN_new();
    if (sig->r == NULL || sig->s == NULL
        || ossl_decode_der_dsa_sig(sig->r, sig->s, ppin, (size_t)len) == 0) {
        if (psig == NULL || *psig == NULL)
            DSA_SIG_free(sig);
        return NULL;
    }
    if (psig != NULL && *psig == NULL)
        *psig = sig;
    return sig;
}

int i2d_DSA_SIG(const DSA_SIG *sig, unsigned char **ppout)
{
    BUF_MEM *buf = NULL;
    size_t encoded_len;
    WPACKET pkt;

    if (ppout == NULL) {
        if (!WPACKET_init_null(&pkt, 0))
            return -1;
    } else if (*ppout == NULL) {
        if ((buf = BUF_MEM_new()) == NULL
                || !WPACKET_init_len(&pkt, buf, 0)) {
            BUF_MEM_free(buf);
            return -1;
        }
    } else {
        if (!WPACKET_init_static_len(&pkt, *ppout, SIZE_MAX, 0))
            return -1;
    }

    if (!ossl_encode_der_dsa_sig(&pkt, sig->r, sig->s)
            || !WPACKET_get_total_written(&pkt, &encoded_len)
            || !WPACKET_finish(&pkt)) {
        BUF_MEM_free(buf);
        WPACKET_cleanup(&pkt);
        return -1;
    }

    if (ppout != NULL) {
        if (*ppout == NULL) {
            *ppout = (unsigned char *)buf->data;
            buf->data = NULL;
            BUF_MEM_free(buf);
        } else {
            *ppout += encoded_len;
        }
    }

    return (int)encoded_len;
}

int DSA_size(const DSA *dsa)
{
    int ret = -1;
    DSA_SIG sig;

    if (dsa->params.q != NULL) {
        sig.r = sig.s = dsa->params.q;
        ret = i2d_DSA_SIG(&sig, NULL);

        if (ret < 0)
            ret = 0;
    }
    return ret;
}

void DSA_SIG_get0(const DSA_SIG *sig, const BIGNUM **pr, const BIGNUM **ps)
{
    if (pr != NULL)
        *pr = sig->r;
    if (ps != NULL)
        *ps = sig->s;
}

int DSA_SIG_set0(DSA_SIG *sig, BIGNUM *r, BIGNUM *s)
{
    if (r == NULL || s == NULL)
        return 0;
    BN_clear_free(sig->r);
    BN_clear_free(sig->s);
    sig->r = r;
    sig->s = s;
    return 1;
}

int ossl_dsa_sign_int(int type, const unsigned char *dgst, int dlen,
                      unsigned char *sig, unsigned int *siglen, DSA *dsa,
                      unsigned int nonce_type, const char *digestname,
                      OSSL_LIB_CTX *libctx, const char *propq)
{
    DSA_SIG *s;

    if (sig == NULL) {
        *siglen = DSA_size(dsa);
        return 1;
    }

    /* legacy case uses the method table */
    if (dsa->libctx == NULL || dsa->meth != DSA_get_default_method())
        s = DSA_do_sign(dgst, dlen, dsa);
    else
        s = ossl_dsa_do_sign_int(dgst, dlen, dsa,
                                 nonce_type, digestname, libctx, propq);
    if (s == NULL) {
        *siglen = 0;
        return 0;
    }
    *siglen = i2d_DSA_SIG(s, &sig);
    DSA_SIG_free(s);
    return 1;
}

int DSA_sign(int type, const unsigned char *dgst, int dlen,
             unsigned char *sig, unsigned int *siglen, DSA *dsa)
{
    return ossl_dsa_sign_int(type, dgst, dlen, sig, siglen, dsa,
                             0, NULL, NULL, NULL);
}

/* data has already been hashed (probably with SHA or SHA-1). */
/*-
 * returns
 *      1: correct signature
 *      0: incorrect signature
 *     -1: error
 */
int DSA_verify(int type, const unsigned char *dgst, int dgst_len,
               const unsigned char *sigbuf, int siglen, DSA *dsa)
{
    DSA_SIG *s;
    const unsigned char *p = sigbuf;
    unsigned char *der = NULL;
    int derlen = -1;
    int ret = -1;

    s = DSA_SIG_new();
    if (s == NULL)
        return ret;
    if (d2i_DSA_SIG(&s, &p, siglen) == NULL)
        goto err;
    /* Ensure signature uses DER and doesn't have trailing garbage */
    derlen = i2d_DSA_SIG(s, &der);
    if (derlen != siglen || memcmp(sigbuf, der, derlen))
        goto err;
    ret = DSA_do_verify(dgst, dgst_len, s, dsa);
 err:
    OPENSSL_clear_free(der, derlen);
    DSA_SIG_free(s);
    return ret;
}
