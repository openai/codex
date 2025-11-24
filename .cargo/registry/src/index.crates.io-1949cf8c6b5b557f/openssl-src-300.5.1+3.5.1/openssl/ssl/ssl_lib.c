/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 * Copyright 2005 Nokia. All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include "internal/e_winsock.h"
#include "ssl_local.h"

#include <openssl/objects.h>
#include <openssl/x509v3.h>
#include <openssl/rand.h>
#include <openssl/ocsp.h>
#include <openssl/dh.h>
#include <openssl/engine.h>
#include <openssl/async.h>
#include <openssl/ct.h>
#include <openssl/trace.h>
#include <openssl/core_names.h>
#include <openssl/provider.h>
#include "internal/cryptlib.h"
#include "internal/nelem.h"
#include "internal/refcount.h"
#include "internal/thread_once.h"
#include "internal/ktls.h"
#include "internal/to_hex.h"
#include "internal/ssl_unwrap.h"
#include "quic/quic_local.h"

#ifndef OPENSSL_NO_SSLKEYLOG
# include <sys/stat.h>
# include <fcntl.h>
#endif

static int ssl_undefined_function_3(SSL_CONNECTION *sc, unsigned char *r,
                                    unsigned char *s, size_t t, size_t *u)
{
    return ssl_undefined_function(SSL_CONNECTION_GET_SSL(sc));
}

static int ssl_undefined_function_4(SSL_CONNECTION *sc, int r)
{
    return ssl_undefined_function(SSL_CONNECTION_GET_SSL(sc));
}

static size_t ssl_undefined_function_5(SSL_CONNECTION *sc, const char *r,
                                       size_t s, unsigned char *t)
{
    return ssl_undefined_function(SSL_CONNECTION_GET_SSL(sc));
}

static int ssl_undefined_function_6(int r)
{
    return ssl_undefined_function(NULL);
}

static int ssl_undefined_function_7(SSL_CONNECTION *sc, unsigned char *r,
                                    size_t s, const char *t, size_t u,
                                    const unsigned char *v, size_t w, int x)
{
    return ssl_undefined_function(SSL_CONNECTION_GET_SSL(sc));
}

static int ssl_undefined_function_8(SSL_CONNECTION *sc)
{
    return ssl_undefined_function(SSL_CONNECTION_GET_SSL(sc));
}

const SSL3_ENC_METHOD ssl3_undef_enc_method = {
    ssl_undefined_function_8,
    ssl_undefined_function_3,
    ssl_undefined_function_4,
    ssl_undefined_function_5,
    NULL,                       /* client_finished_label */
    0,                          /* client_finished_label_len */
    NULL,                       /* server_finished_label */
    0,                          /* server_finished_label_len */
    ssl_undefined_function_6,
    ssl_undefined_function_7,
};

struct ssl_async_args {
    SSL *s;
    void *buf;
    size_t num;
    enum { READFUNC, WRITEFUNC, OTHERFUNC } type;
    union {
        int (*func_read) (SSL *, void *, size_t, size_t *);
        int (*func_write) (SSL *, const void *, size_t, size_t *);
        int (*func_other) (SSL *);
    } f;
};

static const struct {
    uint8_t mtype;
    uint8_t ord;
    int nid;
} dane_mds[] = {
    {
        DANETLS_MATCHING_FULL, 0, NID_undef
    },
    {
        DANETLS_MATCHING_2256, 1, NID_sha256
    },
    {
        DANETLS_MATCHING_2512, 2, NID_sha512
    },
};

static int dane_ctx_enable(struct dane_ctx_st *dctx)
{
    const EVP_MD **mdevp;
    uint8_t *mdord;
    uint8_t mdmax = DANETLS_MATCHING_LAST;
    int n = ((int)mdmax) + 1;   /* int to handle PrivMatch(255) */
    size_t i;

    if (dctx->mdevp != NULL)
        return 1;

    mdevp = OPENSSL_zalloc(n * sizeof(*mdevp));
    mdord = OPENSSL_zalloc(n * sizeof(*mdord));

    if (mdord == NULL || mdevp == NULL) {
        OPENSSL_free(mdord);
        OPENSSL_free(mdevp);
        return 0;
    }

    /* Install default entries */
    for (i = 0; i < OSSL_NELEM(dane_mds); ++i) {
        const EVP_MD *md;

        if (dane_mds[i].nid == NID_undef ||
            (md = EVP_get_digestbynid(dane_mds[i].nid)) == NULL)
            continue;
        mdevp[dane_mds[i].mtype] = md;
        mdord[dane_mds[i].mtype] = dane_mds[i].ord;
    }

    dctx->mdevp = mdevp;
    dctx->mdord = mdord;
    dctx->mdmax = mdmax;

    return 1;
}

static void dane_ctx_final(struct dane_ctx_st *dctx)
{
    OPENSSL_free(dctx->mdevp);
    dctx->mdevp = NULL;

    OPENSSL_free(dctx->mdord);
    dctx->mdord = NULL;
    dctx->mdmax = 0;
}

static void tlsa_free(danetls_record *t)
{
    if (t == NULL)
        return;
    OPENSSL_free(t->data);
    EVP_PKEY_free(t->spki);
    OPENSSL_free(t);
}

static void dane_final(SSL_DANE *dane)
{
    sk_danetls_record_pop_free(dane->trecs, tlsa_free);
    dane->trecs = NULL;

    OSSL_STACK_OF_X509_free(dane->certs);
    dane->certs = NULL;

    X509_free(dane->mcert);
    dane->mcert = NULL;
    dane->mtlsa = NULL;
    dane->mdpth = -1;
    dane->pdpth = -1;
}

/*
 * dane_copy - Copy dane configuration, sans verification state.
 */
static int ssl_dane_dup(SSL_CONNECTION *to, SSL_CONNECTION *from)
{
    int num;
    int i;

    if (!DANETLS_ENABLED(&from->dane))
        return 1;

    num = sk_danetls_record_num(from->dane.trecs);
    dane_final(&to->dane);
    to->dane.flags = from->dane.flags;
    to->dane.dctx = &SSL_CONNECTION_GET_CTX(to)->dane;
    to->dane.trecs = sk_danetls_record_new_reserve(NULL, num);

    if (to->dane.trecs == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        return 0;
    }

    for (i = 0; i < num; ++i) {
        danetls_record *t = sk_danetls_record_value(from->dane.trecs, i);

        if (SSL_dane_tlsa_add(SSL_CONNECTION_GET_SSL(to), t->usage,
                              t->selector, t->mtype, t->data, t->dlen) <= 0)
            return 0;
    }
    return 1;
}

static int dane_mtype_set(struct dane_ctx_st *dctx,
                          const EVP_MD *md, uint8_t mtype, uint8_t ord)
{
    int i;

    if (mtype == DANETLS_MATCHING_FULL && md != NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DANE_CANNOT_OVERRIDE_MTYPE_FULL);
        return 0;
    }

    if (mtype > dctx->mdmax) {
        const EVP_MD **mdevp;
        uint8_t *mdord;
        int n = ((int)mtype) + 1;

        mdevp = OPENSSL_realloc(dctx->mdevp, n * sizeof(*mdevp));
        if (mdevp == NULL)
            return -1;
        dctx->mdevp = mdevp;

        mdord = OPENSSL_realloc(dctx->mdord, n * sizeof(*mdord));
        if (mdord == NULL)
            return -1;
        dctx->mdord = mdord;

        /* Zero-fill any gaps */
        for (i = dctx->mdmax + 1; i < mtype; ++i) {
            mdevp[i] = NULL;
            mdord[i] = 0;
        }

        dctx->mdmax = mtype;
    }

    dctx->mdevp[mtype] = md;
    /* Coerce ordinal of disabled matching types to 0 */
    dctx->mdord[mtype] = (md == NULL) ? 0 : ord;

    return 1;
}

static const EVP_MD *tlsa_md_get(SSL_DANE *dane, uint8_t mtype)
{
    if (mtype > dane->dctx->mdmax)
        return NULL;
    return dane->dctx->mdevp[mtype];
}

static int dane_tlsa_add(SSL_DANE *dane,
                         uint8_t usage,
                         uint8_t selector,
                         uint8_t mtype, const unsigned char *data, size_t dlen)
{
    danetls_record *t;
    const EVP_MD *md = NULL;
    int ilen = (int)dlen;
    int i;
    int num;
    int mdsize;

    if (dane->trecs == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DANE_NOT_ENABLED);
        return -1;
    }

    if (ilen < 0 || dlen != (size_t)ilen) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DANE_TLSA_BAD_DATA_LENGTH);
        return 0;
    }

    if (usage > DANETLS_USAGE_LAST) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DANE_TLSA_BAD_CERTIFICATE_USAGE);
        return 0;
    }

    if (selector > DANETLS_SELECTOR_LAST) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DANE_TLSA_BAD_SELECTOR);
        return 0;
    }

    if (mtype != DANETLS_MATCHING_FULL) {
        md = tlsa_md_get(dane, mtype);
        if (md == NULL) {
            ERR_raise(ERR_LIB_SSL, SSL_R_DANE_TLSA_BAD_MATCHING_TYPE);
            return 0;
        }
    }

    if (md != NULL) {
        mdsize = EVP_MD_get_size(md);
        if (mdsize <= 0 || dlen != (size_t)mdsize) {
            ERR_raise(ERR_LIB_SSL, SSL_R_DANE_TLSA_BAD_DIGEST_LENGTH);
            return 0;
        }
    }
    if (!data) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DANE_TLSA_NULL_DATA);
        return 0;
    }

    if ((t = OPENSSL_zalloc(sizeof(*t))) == NULL)
        return -1;

    t->usage = usage;
    t->selector = selector;
    t->mtype = mtype;
    t->data = OPENSSL_malloc(dlen);
    if (t->data == NULL) {
        tlsa_free(t);
        return -1;
    }
    memcpy(t->data, data, dlen);
    t->dlen = dlen;

    /* Validate and cache full certificate or public key */
    if (mtype == DANETLS_MATCHING_FULL) {
        const unsigned char *p = data;
        X509 *cert = NULL;
        EVP_PKEY *pkey = NULL;

        switch (selector) {
        case DANETLS_SELECTOR_CERT:
            if (!d2i_X509(&cert, &p, ilen) || p < data ||
                dlen != (size_t)(p - data)) {
                X509_free(cert);
                tlsa_free(t);
                ERR_raise(ERR_LIB_SSL, SSL_R_DANE_TLSA_BAD_CERTIFICATE);
                return 0;
            }
            if (X509_get0_pubkey(cert) == NULL) {
                X509_free(cert);
                tlsa_free(t);
                ERR_raise(ERR_LIB_SSL, SSL_R_DANE_TLSA_BAD_CERTIFICATE);
                return 0;
            }

            if ((DANETLS_USAGE_BIT(usage) & DANETLS_TA_MASK) == 0) {
                /*
                 * The Full(0) certificate decodes to a seemingly valid X.509
                 * object with a plausible key, so the TLSA record is well
                 * formed.  However, we don't actually need the certificate for
                 * usages PKIX-EE(1) or DANE-EE(3), because at least the EE
                 * certificate is always presented by the peer.  We discard the
                 * certificate, and just use the TLSA data as an opaque blob
                 * for matching the raw presented DER octets.
                 *
                 * DO NOT FREE `t` here, it will be added to the TLSA record
                 * list below!
                 */
                X509_free(cert);
                break;
            }

            /*
             * For usage DANE-TA(2), we support authentication via "2 0 0" TLSA
             * records that contain full certificates of trust-anchors that are
             * not present in the wire chain.  For usage PKIX-TA(0), we augment
             * the chain with untrusted Full(0) certificates from DNS, in case
             * they are missing from the chain.
             */
            if ((dane->certs == NULL &&
                 (dane->certs = sk_X509_new_null()) == NULL) ||
                !sk_X509_push(dane->certs, cert)) {
                ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
                X509_free(cert);
                tlsa_free(t);
                return -1;
            }
            break;

        case DANETLS_SELECTOR_SPKI:
            if (!d2i_PUBKEY(&pkey, &p, ilen) || p < data ||
                dlen != (size_t)(p - data)) {
                EVP_PKEY_free(pkey);
                tlsa_free(t);
                ERR_raise(ERR_LIB_SSL, SSL_R_DANE_TLSA_BAD_PUBLIC_KEY);
                return 0;
            }

            /*
             * For usage DANE-TA(2), we support authentication via "2 1 0" TLSA
             * records that contain full bare keys of trust-anchors that are
             * not present in the wire chain.
             */
            if (usage == DANETLS_USAGE_DANE_TA)
                t->spki = pkey;
            else
                EVP_PKEY_free(pkey);
            break;
        }
    }

    /*-
     * Find the right insertion point for the new record.
     *
     * See crypto/x509/x509_vfy.c.  We sort DANE-EE(3) records first, so that
     * they can be processed first, as they require no chain building, and no
     * expiration or hostname checks.  Because DANE-EE(3) is numerically
     * largest, this is accomplished via descending sort by "usage".
     *
     * We also sort in descending order by matching ordinal to simplify
     * the implementation of digest agility in the verification code.
     *
     * The choice of order for the selector is not significant, so we
     * use the same descending order for consistency.
     */
    num = sk_danetls_record_num(dane->trecs);
    for (i = 0; i < num; ++i) {
        danetls_record *rec = sk_danetls_record_value(dane->trecs, i);

        if (rec->usage > usage)
            continue;
        if (rec->usage < usage)
            break;
        if (rec->selector > selector)
            continue;
        if (rec->selector < selector)
            break;
        if (dane->dctx->mdord[rec->mtype] > dane->dctx->mdord[mtype])
            continue;
        break;
    }

    if (!sk_danetls_record_insert(dane->trecs, t, i)) {
        tlsa_free(t);
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        return -1;
    }
    dane->umask |= DANETLS_USAGE_BIT(usage);

    return 1;
}

/*
 * Return 0 if there is only one version configured and it was disabled
 * at configure time.  Return 1 otherwise.
 */
static int ssl_check_allowed_versions(int min_version, int max_version)
{
    int minisdtls = 0, maxisdtls = 0;

    /* Figure out if we're doing DTLS versions or TLS versions */
    if (min_version == DTLS1_BAD_VER
        || min_version >> 8 == DTLS1_VERSION_MAJOR)
        minisdtls = 1;
    if (max_version == DTLS1_BAD_VER
        || max_version >> 8 == DTLS1_VERSION_MAJOR)
        maxisdtls = 1;
    /* A wildcard version of 0 could be DTLS or TLS. */
    if ((minisdtls && !maxisdtls && max_version != 0)
        || (maxisdtls && !minisdtls && min_version != 0)) {
        /* Mixing DTLS and TLS versions will lead to sadness; deny it. */
        return 0;
    }

    if (minisdtls || maxisdtls) {
        /* Do DTLS version checks. */
        if (min_version == 0)
            /* Ignore DTLS1_BAD_VER */
            min_version = DTLS1_VERSION;
        if (max_version == 0)
            max_version = DTLS1_2_VERSION;
#ifdef OPENSSL_NO_DTLS1_2
        if (max_version == DTLS1_2_VERSION)
            max_version = DTLS1_VERSION;
#endif
#ifdef OPENSSL_NO_DTLS1
        if (min_version == DTLS1_VERSION)
            min_version = DTLS1_2_VERSION;
#endif
        /* Done massaging versions; do the check. */
        if (0
#ifdef OPENSSL_NO_DTLS1
            || (DTLS_VERSION_GE(min_version, DTLS1_VERSION)
                && DTLS_VERSION_GE(DTLS1_VERSION, max_version))
#endif
#ifdef OPENSSL_NO_DTLS1_2
            || (DTLS_VERSION_GE(min_version, DTLS1_2_VERSION)
                && DTLS_VERSION_GE(DTLS1_2_VERSION, max_version))
#endif
            )
            return 0;
    } else {
        /* Regular TLS version checks. */
        if (min_version == 0)
            min_version = SSL3_VERSION;
        if (max_version == 0)
            max_version = TLS1_3_VERSION;
#ifdef OPENSSL_NO_TLS1_3
        if (max_version == TLS1_3_VERSION)
            max_version = TLS1_2_VERSION;
#endif
#ifdef OPENSSL_NO_TLS1_2
        if (max_version == TLS1_2_VERSION)
            max_version = TLS1_1_VERSION;
#endif
#ifdef OPENSSL_NO_TLS1_1
        if (max_version == TLS1_1_VERSION)
            max_version = TLS1_VERSION;
#endif
#ifdef OPENSSL_NO_TLS1
        if (max_version == TLS1_VERSION)
            max_version = SSL3_VERSION;
#endif
#ifdef OPENSSL_NO_SSL3
        if (min_version == SSL3_VERSION)
            min_version = TLS1_VERSION;
#endif
#ifdef OPENSSL_NO_TLS1
        if (min_version == TLS1_VERSION)
            min_version = TLS1_1_VERSION;
#endif
#ifdef OPENSSL_NO_TLS1_1
        if (min_version == TLS1_1_VERSION)
            min_version = TLS1_2_VERSION;
#endif
#ifdef OPENSSL_NO_TLS1_2
        if (min_version == TLS1_2_VERSION)
            min_version = TLS1_3_VERSION;
#endif
        /* Done massaging versions; do the check. */
        if (0
#ifdef OPENSSL_NO_SSL3
            || (min_version <= SSL3_VERSION && SSL3_VERSION <= max_version)
#endif
#ifdef OPENSSL_NO_TLS1
            || (min_version <= TLS1_VERSION && TLS1_VERSION <= max_version)
#endif
#ifdef OPENSSL_NO_TLS1_1
            || (min_version <= TLS1_1_VERSION && TLS1_1_VERSION <= max_version)
#endif
#ifdef OPENSSL_NO_TLS1_2
            || (min_version <= TLS1_2_VERSION && TLS1_2_VERSION <= max_version)
#endif
#ifdef OPENSSL_NO_TLS1_3
            || (min_version <= TLS1_3_VERSION && TLS1_3_VERSION <= max_version)
#endif
            )
            return 0;
    }
    return 1;
}

#if defined(__TANDEM) && defined(OPENSSL_VPROC)
/*
 * Define a VPROC function for HP NonStop build ssl library.
 * This is used by platform version identification tools.
 * Do not inline this procedure or make it static.
 */
# define OPENSSL_VPROC_STRING_(x)    x##_SSL
# define OPENSSL_VPROC_STRING(x)     OPENSSL_VPROC_STRING_(x)
# define OPENSSL_VPROC_FUNC          OPENSSL_VPROC_STRING(OPENSSL_VPROC)
void OPENSSL_VPROC_FUNC(void) {}
#endif

int SSL_clear(SSL *s)
{
    if (s->method == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_METHOD_SPECIFIED);
        return 0;
    }

    return s->method->ssl_reset(s);
}

int ossl_ssl_connection_reset(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (ssl_clear_bad_session(sc)) {
        SSL_SESSION_free(sc->session);
        sc->session = NULL;
    }
    SSL_SESSION_free(sc->psksession);
    sc->psksession = NULL;
    OPENSSL_free(sc->psksession_id);
    sc->psksession_id = NULL;
    sc->psksession_id_len = 0;
    sc->hello_retry_request = SSL_HRR_NONE;
    sc->sent_tickets = 0;

    sc->error = 0;
    sc->hit = 0;
    sc->shutdown = 0;

    if (sc->renegotiate) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    ossl_statem_clear(sc);

    sc->version = s->method->version;
    sc->client_version = sc->version;
    sc->rwstate = SSL_NOTHING;

    BUF_MEM_free(sc->init_buf);
    sc->init_buf = NULL;
    sc->first_packet = 0;

    sc->key_update = SSL_KEY_UPDATE_NONE;
    memset(sc->ext.compress_certificate_from_peer, 0,
           sizeof(sc->ext.compress_certificate_from_peer));
    sc->ext.compress_certificate_sent = 0;

    EVP_MD_CTX_free(sc->pha_dgst);
    sc->pha_dgst = NULL;

    /* Reset DANE verification result state */
    sc->dane.mdpth = -1;
    sc->dane.pdpth = -1;
    X509_free(sc->dane.mcert);
    sc->dane.mcert = NULL;
    sc->dane.mtlsa = NULL;

    /* Clear the verification result peername */
    X509_VERIFY_PARAM_move_peername(sc->param, NULL);

    /* Clear any shared connection state */
    OPENSSL_free(sc->shared_sigalgs);
    sc->shared_sigalgs = NULL;
    sc->shared_sigalgslen = 0;

    /*
     * Check to see if we were changed into a different method, if so, revert
     * back.
     */
    if (s->method != s->defltmeth) {
        s->method->ssl_deinit(s);
        s->method = s->defltmeth;
        if (!s->method->ssl_init(s))
            return 0;
    } else {
        if (!s->method->ssl_clear(s))
            return 0;
    }

    ossl_quic_tls_clear(sc->qtls);

    if (!RECORD_LAYER_reset(&sc->rlayer))
        return 0;

    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
/** Used to change an SSL_CTXs default SSL method type */
int SSL_CTX_set_ssl_version(SSL_CTX *ctx, const SSL_METHOD *meth)
{
    STACK_OF(SSL_CIPHER) *sk;

    if (IS_QUIC_CTX(ctx)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_WRONG_SSL_VERSION);
        return 0;
    }

    ctx->method = meth;

    if (!SSL_CTX_set_ciphersuites(ctx, OSSL_default_ciphersuites())) {
        ERR_raise(ERR_LIB_SSL, SSL_R_SSL_LIBRARY_HAS_NO_CIPHERS);
        return 0;
    }
    sk = ssl_create_cipher_list(ctx,
                                ctx->tls13_ciphersuites,
                                &(ctx->cipher_list),
                                &(ctx->cipher_list_by_id),
                                OSSL_default_cipher_list(), ctx->cert);
    if ((sk == NULL) || (sk_SSL_CIPHER_num(sk) <= 0)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_SSL_LIBRARY_HAS_NO_CIPHERS);
        return 0;
    }
    return 1;
}
#endif

SSL *SSL_new(SSL_CTX *ctx)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NULL_SSL_CTX);
        return NULL;
    }
    if (ctx->method == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_SSL_CTX_HAS_NO_DEFAULT_SSL_VERSION);
        return NULL;
    }
    return ctx->method->ssl_new(ctx);
}

int ossl_ssl_init(SSL *ssl, SSL_CTX *ctx, const SSL_METHOD *method, int type)
{
    if (!SSL_CTX_up_ref(ctx))
        return 0;

    ssl->lock = CRYPTO_THREAD_lock_new();

    if (ssl->lock == NULL || !CRYPTO_NEW_REF(&ssl->references, 1))
        goto err;

    if (!CRYPTO_new_ex_data(CRYPTO_EX_INDEX_SSL, ssl, &ssl->ex_data)) {
        CRYPTO_FREE_REF(&ssl->references);
        goto err;
    }

    ssl->ctx = ctx;
    ssl->type = type;
    ssl->defltmeth = ssl->method = method;

    return 1;

err:
    CRYPTO_THREAD_lock_free(ssl->lock);
    ssl->lock = NULL;
    SSL_CTX_free(ctx);
    return 0;
}

SSL *ossl_ssl_connection_new_int(SSL_CTX *ctx, SSL *user_ssl,
                                 const SSL_METHOD *method)
{
    SSL_CONNECTION *s;
    SSL *ssl;

    s = OPENSSL_zalloc(sizeof(*s));
    if (s == NULL)
        return NULL;

    ssl = &s->ssl;
    s->user_ssl = (user_ssl == NULL) ? ssl : user_ssl;

    if (!ossl_ssl_init(ssl, ctx, method, SSL_TYPE_SSL_CONNECTION)) {
        OPENSSL_free(s);
        s = NULL;
        ssl = NULL;
        goto sslerr;
    }

    RECORD_LAYER_init(&s->rlayer, s);

    s->options = ctx->options;

    s->dane.flags = ctx->dane.flags;
    if (method->version == ctx->method->version) {
        s->min_proto_version = ctx->min_proto_version;
        s->max_proto_version = ctx->max_proto_version;
    }

    s->mode = ctx->mode;
    s->max_cert_list = ctx->max_cert_list;
    s->max_early_data = ctx->max_early_data;
    s->recv_max_early_data = ctx->recv_max_early_data;

    s->num_tickets = ctx->num_tickets;
    s->pha_enabled = ctx->pha_enabled;

    /* Shallow copy of the ciphersuites stack */
    s->tls13_ciphersuites = sk_SSL_CIPHER_dup(ctx->tls13_ciphersuites);
    if (s->tls13_ciphersuites == NULL)
        goto cerr;

    /*
     * Earlier library versions used to copy the pointer to the CERT, not
     * its contents; only when setting new parameters for the per-SSL
     * copy, ssl_cert_new would be called (and the direct reference to
     * the per-SSL_CTX settings would be lost, but those still were
     * indirectly accessed for various purposes, and for that reason they
     * used to be known as s->ctx->default_cert). Now we don't look at the
     * SSL_CTX's CERT after having duplicated it once.
     */
    s->cert = ssl_cert_dup(ctx->cert);
    if (s->cert == NULL)
        goto sslerr;

    RECORD_LAYER_set_read_ahead(&s->rlayer, ctx->read_ahead);
    s->msg_callback = ctx->msg_callback;
    s->msg_callback_arg = ctx->msg_callback_arg;
    s->verify_mode = ctx->verify_mode;
    s->not_resumable_session_cb = ctx->not_resumable_session_cb;
    s->rlayer.record_padding_cb = ctx->record_padding_cb;
    s->rlayer.record_padding_arg = ctx->record_padding_arg;
    s->rlayer.block_padding = ctx->block_padding;
    s->rlayer.hs_padding = ctx->hs_padding;
    s->sid_ctx_length = ctx->sid_ctx_length;
    if (!ossl_assert(s->sid_ctx_length <= sizeof(s->sid_ctx)))
        goto err;
    memcpy(&s->sid_ctx, &ctx->sid_ctx, sizeof(s->sid_ctx));
    s->verify_callback = ctx->default_verify_callback;
    s->generate_session_id = ctx->generate_session_id;

    s->param = X509_VERIFY_PARAM_new();
    if (s->param == NULL)
        goto asn1err;
    X509_VERIFY_PARAM_inherit(s->param, ctx->param);
    s->quiet_shutdown = IS_QUIC_CTX(ctx) ? 0 : ctx->quiet_shutdown;

    if (!IS_QUIC_CTX(ctx))
        s->ext.max_fragment_len_mode = ctx->ext.max_fragment_len_mode;

    s->max_send_fragment = ctx->max_send_fragment;
    s->split_send_fragment = ctx->split_send_fragment;
    s->max_pipelines = ctx->max_pipelines;
    s->rlayer.default_read_buf_len = ctx->default_read_buf_len;

    s->ext.debug_cb = 0;
    s->ext.debug_arg = NULL;
    s->ext.ticket_expected = 0;
    s->ext.status_type = ctx->ext.status_type;
    s->ext.status_expected = 0;
    s->ext.ocsp.ids = NULL;
    s->ext.ocsp.exts = NULL;
    s->ext.ocsp.resp = NULL;
    s->ext.ocsp.resp_len = 0;

    if (!SSL_CTX_up_ref(ctx))
        goto err;

    s->session_ctx = ctx;
    if (ctx->ext.ecpointformats != NULL) {
        s->ext.ecpointformats =
            OPENSSL_memdup(ctx->ext.ecpointformats,
                           ctx->ext.ecpointformats_len);
        if (s->ext.ecpointformats == NULL) {
            s->ext.ecpointformats_len = 0;
            goto err;
        }
        s->ext.ecpointformats_len =
            ctx->ext.ecpointformats_len;
    }
    if (ctx->ext.supportedgroups != NULL) {
        size_t add = 0;

        if (ctx->ext.supportedgroups_len == 0)
            /* Add 1 so allocation won't fail */
            add = 1;
        s->ext.supportedgroups =
            OPENSSL_memdup(ctx->ext.supportedgroups,
                           (ctx->ext.supportedgroups_len + add)
                           * sizeof(*ctx->ext.supportedgroups));
        if (s->ext.supportedgroups == NULL) {
            s->ext.supportedgroups_len = 0;
            goto err;
        }
        s->ext.supportedgroups_len = ctx->ext.supportedgroups_len;
    }
    if (ctx->ext.keyshares != NULL) {
        size_t add = 0;

        if (ctx->ext.keyshares_len == 0)
            /* Add 1 so allocation won't fail */
            add = 1;
        s->ext.keyshares =
            OPENSSL_memdup(ctx->ext.keyshares,
                           (ctx->ext.keyshares_len + add)
                           * sizeof(*ctx->ext.keyshares));
        if (s->ext.keyshares == NULL) {
            s->ext.keyshares_len = 0;
            goto err;
        }
        s->ext.keyshares_len = ctx->ext.keyshares_len;
    }
    if (ctx->ext.tuples != NULL) {
        size_t add = 0;

        if (ctx->ext.tuples_len == 0)
            /* Add 1 so allocation won't fail */
            add = 1;
        s->ext.tuples =
            OPENSSL_memdup(ctx->ext.tuples,
                           (ctx->ext.tuples_len + add)
                           * sizeof(*ctx->ext.tuples));
        if (s->ext.tuples == NULL) {
            s->ext.tuples_len = 0;
            goto err;
        }
        s->ext.tuples_len = ctx->ext.tuples_len;
    }

#ifndef OPENSSL_NO_NEXTPROTONEG
    s->ext.npn = NULL;
#endif

    if (ctx->ext.alpn != NULL) {
        s->ext.alpn = OPENSSL_malloc(ctx->ext.alpn_len);
        if (s->ext.alpn == NULL) {
            s->ext.alpn_len = 0;
            goto err;
        }
        memcpy(s->ext.alpn, ctx->ext.alpn, ctx->ext.alpn_len);
        s->ext.alpn_len = ctx->ext.alpn_len;
    }

    s->verified_chain = NULL;
    s->verify_result = X509_V_OK;

    s->default_passwd_callback = ctx->default_passwd_callback;
    s->default_passwd_callback_userdata = ctx->default_passwd_callback_userdata;

    s->key_update = SSL_KEY_UPDATE_NONE;

    if (!IS_QUIC_CTX(ctx)) {
        s->allow_early_data_cb = ctx->allow_early_data_cb;
        s->allow_early_data_cb_data = ctx->allow_early_data_cb_data;
    }

    if (!method->ssl_init(ssl))
        goto sslerr;

    s->server = (method->ssl_accept == ssl_undefined_function) ? 0 : 1;

    if (!method->ssl_reset(ssl))
        goto sslerr;

#ifndef OPENSSL_NO_PSK
    s->psk_client_callback = ctx->psk_client_callback;
    s->psk_server_callback = ctx->psk_server_callback;
#endif
    s->psk_find_session_cb = ctx->psk_find_session_cb;
    s->psk_use_session_cb = ctx->psk_use_session_cb;

    s->async_cb = ctx->async_cb;
    s->async_cb_arg = ctx->async_cb_arg;

    s->job = NULL;

#ifndef OPENSSL_NO_COMP_ALG
    memcpy(s->cert_comp_prefs, ctx->cert_comp_prefs, sizeof(s->cert_comp_prefs));
#endif
    if (ctx->client_cert_type != NULL) {
        s->client_cert_type = OPENSSL_memdup(ctx->client_cert_type,
                                             ctx->client_cert_type_len);
        if (s->client_cert_type == NULL)
            goto sslerr;
        s->client_cert_type_len = ctx->client_cert_type_len;
    }
    if (ctx->server_cert_type != NULL) {
        s->server_cert_type = OPENSSL_memdup(ctx->server_cert_type,
                                             ctx->server_cert_type_len);
        if (s->server_cert_type == NULL)
            goto sslerr;
        s->server_cert_type_len = ctx->server_cert_type_len;
    }

#ifndef OPENSSL_NO_CT
    if (!SSL_set_ct_validation_callback(ssl, ctx->ct_validation_callback,
                                        ctx->ct_validation_callback_arg))
        goto sslerr;
#endif

    s->ssl_pkey_num = SSL_PKEY_NUM + ctx->sigalg_list_len;
    return ssl;
 cerr:
    ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
    goto err;
 asn1err:
    ERR_raise(ERR_LIB_SSL, ERR_R_ASN1_LIB);
    goto err;
 sslerr:
    ERR_raise(ERR_LIB_SSL, ERR_R_SSL_LIB);
 err:
    SSL_free(ssl);
    return NULL;
}

SSL *ossl_ssl_connection_new(SSL_CTX *ctx)
{
    return ossl_ssl_connection_new_int(ctx, NULL, ctx->method);
}

int SSL_is_dtls(const SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (s->type == SSL_TYPE_QUIC_CONNECTION || s->type == SSL_TYPE_QUIC_XSO)
        return 0;
#endif

    if (sc == NULL)
        return 0;

    return SSL_CONNECTION_IS_DTLS(sc) ? 1 : 0;
}

int SSL_is_tls(const SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (s->type == SSL_TYPE_QUIC_CONNECTION || s->type == SSL_TYPE_QUIC_XSO)
        return 0;
#endif

    if (sc == NULL)
        return 0;

    return SSL_CONNECTION_IS_DTLS(sc) ? 0 : 1;
}

int SSL_is_quic(const SSL *s)
{
    return IS_QUIC(s);
}

int SSL_up_ref(SSL *s)
{
    int i;

    if (CRYPTO_UP_REF(&s->references, &i) <= 0)
        return 0;

    REF_PRINT_COUNT("SSL", i, s);
    REF_ASSERT_ISNT(i < 2);
    return ((i > 1) ? 1 : 0);
}

int SSL_CTX_set_session_id_context(SSL_CTX *ctx, const unsigned char *sid_ctx,
                                   unsigned int sid_ctx_len)
{
    if (sid_ctx_len > SSL_MAX_SID_CTX_LENGTH) {
        ERR_raise(ERR_LIB_SSL, SSL_R_SSL_SESSION_ID_CONTEXT_TOO_LONG);
        return 0;
    }
    ctx->sid_ctx_length = sid_ctx_len;
    memcpy(ctx->sid_ctx, sid_ctx, sid_ctx_len);

    return 1;
}

int SSL_set_session_id_context(SSL *ssl, const unsigned char *sid_ctx,
                               unsigned int sid_ctx_len)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return 0;

    if (sid_ctx_len > SSL_MAX_SID_CTX_LENGTH) {
        ERR_raise(ERR_LIB_SSL, SSL_R_SSL_SESSION_ID_CONTEXT_TOO_LONG);
        return 0;
    }
    sc->sid_ctx_length = sid_ctx_len;
    memcpy(sc->sid_ctx, sid_ctx, sid_ctx_len);

    return 1;
}

int SSL_CTX_set_generate_session_id(SSL_CTX *ctx, GEN_SESSION_CB cb)
{
    if (!CRYPTO_THREAD_write_lock(ctx->lock))
        return 0;
    ctx->generate_session_id = cb;
    CRYPTO_THREAD_unlock(ctx->lock);
    return 1;
}

int SSL_set_generate_session_id(SSL *ssl, GEN_SESSION_CB cb)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL || !CRYPTO_THREAD_write_lock(ssl->lock))
        return 0;
    sc->generate_session_id = cb;
    CRYPTO_THREAD_unlock(ssl->lock);
    return 1;
}

int SSL_has_matching_session_id(const SSL *ssl, const unsigned char *id,
                                unsigned int id_len)
{
    /*
     * A quick examination of SSL_SESSION_hash and SSL_SESSION_cmp shows how
     * we can "construct" a session to give us the desired check - i.e. to
     * find if there's a session in the hash table that would conflict with
     * any new session built out of this id/id_len and the ssl_version in use
     * by this SSL.
     */
    SSL_SESSION r, *p;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(ssl);

    if (sc == NULL || id_len > sizeof(r.session_id))
        return 0;

    r.ssl_version = sc->version;
    r.session_id_length = id_len;
    memcpy(r.session_id, id, id_len);

    if (!CRYPTO_THREAD_read_lock(sc->session_ctx->lock))
        return 0;
    p = lh_SSL_SESSION_retrieve(sc->session_ctx->sessions, &r);
    CRYPTO_THREAD_unlock(sc->session_ctx->lock);
    return (p != NULL);
}

int SSL_CTX_set_purpose(SSL_CTX *s, int purpose)
{
    return X509_VERIFY_PARAM_set_purpose(s->param, purpose);
}

int SSL_set_purpose(SSL *s, int purpose)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    return X509_VERIFY_PARAM_set_purpose(sc->param, purpose);
}

int SSL_CTX_set_trust(SSL_CTX *s, int trust)
{
    return X509_VERIFY_PARAM_set_trust(s->param, trust);
}

int SSL_set_trust(SSL *s, int trust)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    return X509_VERIFY_PARAM_set_trust(sc->param, trust);
}

int SSL_set1_host(SSL *s, const char *host)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    /* clear hostname(s) and IP address in any case, also if host parses as an IP address */
    (void)X509_VERIFY_PARAM_set1_host(sc->param, NULL, 0);
    (void)X509_VERIFY_PARAM_set1_ip(sc->param, NULL, 0);
    if (host == NULL)
        return 1;

    /* If a host is provided and parses as an IP address, treat it as such. */
    return X509_VERIFY_PARAM_set1_ip_asc(sc->param, host)
        || X509_VERIFY_PARAM_set1_host(sc->param, host, 0);
}

int SSL_add1_host(SSL *s, const char *host)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    /* If a host is provided and parses as an IP address, treat it as such. */
    if (host != NULL) {
        ASN1_OCTET_STRING *ip;
        char *old_ip;

        ip = a2i_IPADDRESS(host);
        if (ip != NULL) {
            /* We didn't want it; only to check if it *is* an IP address */
            ASN1_OCTET_STRING_free(ip);

            old_ip = X509_VERIFY_PARAM_get1_ip_asc(sc->param);
            if (old_ip != NULL) {
                OPENSSL_free(old_ip);
                /* There can be only one IP address */
                ERR_raise_data(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT,
                               "IP address was already set");
                return 0;
            }

            return X509_VERIFY_PARAM_set1_ip_asc(sc->param, host);
        }
    }

    return X509_VERIFY_PARAM_add1_host(sc->param, host, 0);
}

void SSL_set_hostflags(SSL *s, unsigned int flags)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    X509_VERIFY_PARAM_set_hostflags(sc->param, flags);
}

const char *SSL_get0_peername(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    return X509_VERIFY_PARAM_get0_peername(sc->param);
}

int SSL_CTX_dane_enable(SSL_CTX *ctx)
{
    return dane_ctx_enable(&ctx->dane);
}

unsigned long SSL_CTX_dane_set_flags(SSL_CTX *ctx, unsigned long flags)
{
    unsigned long orig = ctx->dane.flags;

    ctx->dane.flags |= flags;
    return orig;
}

unsigned long SSL_CTX_dane_clear_flags(SSL_CTX *ctx, unsigned long flags)
{
    unsigned long orig = ctx->dane.flags;

    ctx->dane.flags &= ~flags;
    return orig;
}

int SSL_dane_enable(SSL *s, const char *basedomain)
{
    SSL_DANE *dane;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    dane = &sc->dane;
    if (s->ctx->dane.mdmax == 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_CONTEXT_NOT_DANE_ENABLED);
        return 0;
    }
    if (dane->trecs != NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DANE_ALREADY_ENABLED);
        return 0;
    }

    /*
     * Default SNI name.  This rejects empty names, while set1_host below
     * accepts them and disables hostname checks.  To avoid side-effects with
     * invalid input, set the SNI name first.
     */
    if (sc->ext.hostname == NULL) {
        if (!SSL_set_tlsext_host_name(s, basedomain)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_ERROR_SETTING_TLSA_BASE_DOMAIN);
            return -1;
        }
    }

    /* Primary RFC6125 reference identifier */
    if (!X509_VERIFY_PARAM_set1_host(sc->param, basedomain, 0)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_ERROR_SETTING_TLSA_BASE_DOMAIN);
        return -1;
    }

    dane->mdpth = -1;
    dane->pdpth = -1;
    dane->dctx = &s->ctx->dane;
    dane->trecs = sk_danetls_record_new_null();

    if (dane->trecs == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        return -1;
    }
    return 1;
}

unsigned long SSL_dane_set_flags(SSL *ssl, unsigned long flags)
{
    unsigned long orig;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return 0;

    orig = sc->dane.flags;

    sc->dane.flags |= flags;
    return orig;
}

unsigned long SSL_dane_clear_flags(SSL *ssl, unsigned long flags)
{
    unsigned long orig;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return 0;

    orig = sc->dane.flags;

    sc->dane.flags &= ~flags;
    return orig;
}

int SSL_get0_dane_authority(SSL *s, X509 **mcert, EVP_PKEY **mspki)
{
    SSL_DANE *dane;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return -1;

    dane = &sc->dane;

    if (!DANETLS_ENABLED(dane) || sc->verify_result != X509_V_OK)
        return -1;
    if (dane->mtlsa) {
        if (mcert)
            *mcert = dane->mcert;
        if (mspki)
            *mspki = (dane->mcert == NULL) ? dane->mtlsa->spki : NULL;
    }
    return dane->mdpth;
}

int SSL_get0_dane_tlsa(SSL *s, uint8_t *usage, uint8_t *selector,
                       uint8_t *mtype, const unsigned char **data, size_t *dlen)
{
    SSL_DANE *dane;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return -1;

    dane = &sc->dane;

    if (!DANETLS_ENABLED(dane) || sc->verify_result != X509_V_OK)
        return -1;
    if (dane->mtlsa) {
        if (usage)
            *usage = dane->mtlsa->usage;
        if (selector)
            *selector = dane->mtlsa->selector;
        if (mtype)
            *mtype = dane->mtlsa->mtype;
        if (data)
            *data = dane->mtlsa->data;
        if (dlen)
            *dlen = dane->mtlsa->dlen;
    }
    return dane->mdpth;
}

SSL_DANE *SSL_get0_dane(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    return &sc->dane;
}

int SSL_dane_tlsa_add(SSL *s, uint8_t usage, uint8_t selector,
                      uint8_t mtype, const unsigned char *data, size_t dlen)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    return dane_tlsa_add(&sc->dane, usage, selector, mtype, data, dlen);
}

int SSL_CTX_dane_mtype_set(SSL_CTX *ctx, const EVP_MD *md, uint8_t mtype,
                           uint8_t ord)
{
    return dane_mtype_set(&ctx->dane, md, mtype, ord);
}

int SSL_CTX_set1_param(SSL_CTX *ctx, X509_VERIFY_PARAM *vpm)
{
    return X509_VERIFY_PARAM_set1(ctx->param, vpm);
}

int SSL_set1_param(SSL *ssl, X509_VERIFY_PARAM *vpm)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return 0;

    return X509_VERIFY_PARAM_set1(sc->param, vpm);
}

X509_VERIFY_PARAM *SSL_CTX_get0_param(SSL_CTX *ctx)
{
    return ctx->param;
}

X509_VERIFY_PARAM *SSL_get0_param(SSL *ssl)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return NULL;

    return sc->param;
}

void SSL_certs_clear(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    ssl_cert_clear_certs(sc->cert);
}

void SSL_free(SSL *s)
{
    int i;

    if (s == NULL)
        return;
    CRYPTO_DOWN_REF(&s->references, &i);
    REF_PRINT_COUNT("SSL", i, s);
    if (i > 0)
        return;
    REF_ASSERT_ISNT(i < 0);

    if (s->method != NULL)
        s->method->ssl_free(s);

    CRYPTO_free_ex_data(CRYPTO_EX_INDEX_SSL, s, &s->ex_data);
    SSL_CTX_free(s->ctx);
    CRYPTO_THREAD_lock_free(s->lock);
    CRYPTO_FREE_REF(&s->references);

    OPENSSL_free(s);
}

void ossl_ssl_connection_free(SSL *ssl)
{
    SSL_CONNECTION *s;

    s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);
    if (s == NULL)
        return;

    /*
     * Ignore return values. This could result in user callbacks being called
     * e.g. for the QUIC TLS record layer. So we do this early before we have
     * freed other things.
     */
    ssl_free_wbio_buffer(s);
    RECORD_LAYER_clear(&s->rlayer);

    X509_VERIFY_PARAM_free(s->param);
    dane_final(&s->dane);

    BUF_MEM_free(s->init_buf);

    /* add extra stuff */
    sk_SSL_CIPHER_free(s->cipher_list);
    sk_SSL_CIPHER_free(s->cipher_list_by_id);
    sk_SSL_CIPHER_free(s->tls13_ciphersuites);
    sk_SSL_CIPHER_free(s->peer_ciphers);

    /* Make the next call work :-) */
    if (s->session != NULL) {
        ssl_clear_bad_session(s);
        SSL_SESSION_free(s->session);
    }
    SSL_SESSION_free(s->psksession);
    OPENSSL_free(s->psksession_id);

    ssl_cert_free(s->cert);
    OPENSSL_free(s->shared_sigalgs);
    /* Free up if allocated */

    OPENSSL_free(s->ext.hostname);
    SSL_CTX_free(s->session_ctx);
    OPENSSL_free(s->ext.ecpointformats);
    OPENSSL_free(s->ext.peer_ecpointformats);
    OPENSSL_free(s->ext.supportedgroups);
    OPENSSL_free(s->ext.keyshares);
    OPENSSL_free(s->ext.tuples);
    OPENSSL_free(s->ext.peer_supportedgroups);
    sk_X509_EXTENSION_pop_free(s->ext.ocsp.exts, X509_EXTENSION_free);
#ifndef OPENSSL_NO_OCSP
    sk_OCSP_RESPID_pop_free(s->ext.ocsp.ids, OCSP_RESPID_free);
#endif
#ifndef OPENSSL_NO_CT
    SCT_LIST_free(s->scts);
    OPENSSL_free(s->ext.scts);
#endif
    OPENSSL_free(s->ext.ocsp.resp);
    OPENSSL_free(s->ext.alpn);
    OPENSSL_free(s->ext.tls13_cookie);
    if (s->clienthello != NULL)
        OPENSSL_free(s->clienthello->pre_proc_exts);
    OPENSSL_free(s->clienthello);
    OPENSSL_free(s->pha_context);
    EVP_MD_CTX_free(s->pha_dgst);

    sk_X509_NAME_pop_free(s->ca_names, X509_NAME_free);
    sk_X509_NAME_pop_free(s->client_ca_names, X509_NAME_free);

    OPENSSL_free(s->client_cert_type);
    OPENSSL_free(s->server_cert_type);

    OSSL_STACK_OF_X509_free(s->verified_chain);

    if (ssl->method != NULL)
        ssl->method->ssl_deinit(ssl);

    ASYNC_WAIT_CTX_free(s->waitctx);

#if !defined(OPENSSL_NO_NEXTPROTONEG)
    OPENSSL_free(s->ext.npn);
#endif

#ifndef OPENSSL_NO_SRTP
    sk_SRTP_PROTECTION_PROFILE_free(s->srtp_profiles);
#endif

    /*
     * We do this late. We want to ensure that any other references we held to
     * these BIOs are freed first *before* we call BIO_free_all(), because
     * BIO_free_all() will only free each BIO in the chain if the number of
     * references to the first BIO have dropped to 0
     */
    BIO_free_all(s->wbio);
    s->wbio = NULL;
    BIO_free_all(s->rbio);
    s->rbio = NULL;
    OPENSSL_free(s->s3.tmp.valid_flags);
}

void SSL_set0_rbio(SSL *s, BIO *rbio)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s)) {
        ossl_quic_conn_set0_net_rbio(s, rbio);
        return;
    }
#endif

    if (sc == NULL)
        return;

    BIO_free_all(sc->rbio);
    sc->rbio = rbio;
    sc->rlayer.rrlmethod->set1_bio(sc->rlayer.rrl, sc->rbio);
}

void SSL_set0_wbio(SSL *s, BIO *wbio)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s)) {
        ossl_quic_conn_set0_net_wbio(s, wbio);
        return;
    }
#endif

    if (sc == NULL)
        return;

    /*
     * If the output buffering BIO is still in place, remove it
     */
    if (sc->bbio != NULL)
        sc->wbio = BIO_pop(sc->wbio);

    BIO_free_all(sc->wbio);
    sc->wbio = wbio;

    /* Re-attach |bbio| to the new |wbio|. */
    if (sc->bbio != NULL)
        sc->wbio = BIO_push(sc->bbio, sc->wbio);

    sc->rlayer.wrlmethod->set1_bio(sc->rlayer.wrl, sc->wbio);
}

void SSL_set_bio(SSL *s, BIO *rbio, BIO *wbio)
{
    /*
     * For historical reasons, this function has many different cases in
     * ownership handling.
     */

    /* If nothing has changed, do nothing */
    if (rbio == SSL_get_rbio(s) && wbio == SSL_get_wbio(s))
        return;

    /*
     * If the two arguments are equal then one fewer reference is granted by the
     * caller than we want to take
     */
    if (rbio != NULL && rbio == wbio) {
        if (!BIO_up_ref(rbio))
            return;
    }

    /*
     * If only the wbio is changed only adopt one reference.
     */
    if (rbio == SSL_get_rbio(s)) {
        SSL_set0_wbio(s, wbio);
        return;
    }
    /*
     * There is an asymmetry here for historical reasons. If only the rbio is
     * changed AND the rbio and wbio were originally different, then we only
     * adopt one reference.
     */
    if (wbio == SSL_get_wbio(s) && SSL_get_rbio(s) != SSL_get_wbio(s)) {
        SSL_set0_rbio(s, rbio);
        return;
    }

    /* Otherwise, adopt both references. */
    SSL_set0_rbio(s, rbio);
    SSL_set0_wbio(s, wbio);
}

BIO *SSL_get_rbio(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_conn_get_net_rbio(s);
#endif

    if (sc == NULL)
        return NULL;

    return sc->rbio;
}

BIO *SSL_get_wbio(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_conn_get_net_wbio(s);
#endif

    if (sc == NULL)
        return NULL;

    if (sc->bbio != NULL) {
        /*
         * If |bbio| is active, the true caller-configured BIO is its
         * |next_bio|.
         */
        return BIO_next(sc->bbio);
    }
    return sc->wbio;
}

int SSL_get_fd(const SSL *s)
{
    return SSL_get_rfd(s);
}

int SSL_get_rfd(const SSL *s)
{
    int ret = -1;
    BIO *b, *r;

    b = SSL_get_rbio(s);
    r = BIO_find_type(b, BIO_TYPE_DESCRIPTOR);
    if (r != NULL)
        BIO_get_fd(r, &ret);
    return ret;
}

int SSL_get_wfd(const SSL *s)
{
    int ret = -1;
    BIO *b, *r;

    b = SSL_get_wbio(s);
    r = BIO_find_type(b, BIO_TYPE_DESCRIPTOR);
    if (r != NULL)
        BIO_get_fd(r, &ret);
    return ret;
}

#ifndef OPENSSL_NO_SOCK
static const BIO_METHOD *fd_method(SSL *s)
{
#ifndef OPENSSL_NO_DGRAM
    if (IS_QUIC(s))
        return BIO_s_datagram();
#endif

    return BIO_s_socket();
}

int SSL_set_fd(SSL *s, int fd)
{
    int ret = 0;
    BIO *bio = NULL;

    if (s->type == SSL_TYPE_QUIC_XSO) {
        ERR_raise(ERR_LIB_SSL, SSL_R_CONN_USE_ONLY);
        goto err;
    }

    bio = BIO_new(fd_method(s));

    if (bio == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_BUF_LIB);
        goto err;
    }
    BIO_set_fd(bio, fd, BIO_NOCLOSE);
    SSL_set_bio(s, bio, bio);
#ifndef OPENSSL_NO_KTLS
    /*
     * The new socket is created successfully regardless of ktls_enable.
     * ktls_enable doesn't change any functionality of the socket, except
     * changing the setsockopt to enable the processing of ktls_start.
     * Thus, it is not a problem to call it for non-TLS sockets.
     */
    ktls_enable(fd);
#endif /* OPENSSL_NO_KTLS */
    ret = 1;
 err:
    return ret;
}

int SSL_set_wfd(SSL *s, int fd)
{
    BIO *rbio = SSL_get_rbio(s);
    int desired_type = IS_QUIC(s) ? BIO_TYPE_DGRAM : BIO_TYPE_SOCKET;

    if (s->type == SSL_TYPE_QUIC_XSO) {
        ERR_raise(ERR_LIB_SSL, SSL_R_CONN_USE_ONLY);
        return 0;
    }

    if (rbio == NULL || BIO_method_type(rbio) != desired_type
        || (int)BIO_get_fd(rbio, NULL) != fd) {
        BIO *bio = BIO_new(fd_method(s));

        if (bio == NULL) {
            ERR_raise(ERR_LIB_SSL, ERR_R_BUF_LIB);
            return 0;
        }
        BIO_set_fd(bio, fd, BIO_NOCLOSE);
        SSL_set0_wbio(s, bio);
#ifndef OPENSSL_NO_KTLS
        /*
         * The new socket is created successfully regardless of ktls_enable.
         * ktls_enable doesn't change any functionality of the socket, except
         * changing the setsockopt to enable the processing of ktls_start.
         * Thus, it is not a problem to call it for non-TLS sockets.
         */
        ktls_enable(fd);
#endif /* OPENSSL_NO_KTLS */
    } else {
        if (!BIO_up_ref(rbio))
            return 0;
        SSL_set0_wbio(s, rbio);
    }
    return 1;
}

int SSL_set_rfd(SSL *s, int fd)
{
    BIO *wbio = SSL_get_wbio(s);
    int desired_type = IS_QUIC(s) ? BIO_TYPE_DGRAM : BIO_TYPE_SOCKET;

    if (s->type == SSL_TYPE_QUIC_XSO) {
        ERR_raise(ERR_LIB_SSL, SSL_R_CONN_USE_ONLY);
        return 0;
    }

    if (wbio == NULL || BIO_method_type(wbio) != desired_type
        || ((int)BIO_get_fd(wbio, NULL) != fd)) {
        BIO *bio = BIO_new(fd_method(s));

        if (bio == NULL) {
            ERR_raise(ERR_LIB_SSL, ERR_R_BUF_LIB);
            return 0;
        }
        BIO_set_fd(bio, fd, BIO_NOCLOSE);
        SSL_set0_rbio(s, bio);
    } else {
        if (!BIO_up_ref(wbio))
            return 0;
        SSL_set0_rbio(s, wbio);
    }

    return 1;
}
#endif

/* return length of latest Finished message we sent, copy to 'buf' */
size_t SSL_get_finished(const SSL *s, void *buf, size_t count)
{
    size_t ret = 0;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    ret = sc->s3.tmp.finish_md_len;
    if (count > ret)
        count = ret;
    memcpy(buf, sc->s3.tmp.finish_md, count);
    return ret;
}

/* return length of latest Finished message we expected, copy to 'buf' */
size_t SSL_get_peer_finished(const SSL *s, void *buf, size_t count)
{
    size_t ret = 0;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    ret = sc->s3.tmp.peer_finish_md_len;
    if (count > ret)
        count = ret;
    memcpy(buf, sc->s3.tmp.peer_finish_md, count);
    return ret;
}

int SSL_get_verify_mode(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->verify_mode;
}

int SSL_get_verify_depth(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    return X509_VERIFY_PARAM_get_depth(sc->param);
}

int (*SSL_get_verify_callback(const SSL *s)) (int, X509_STORE_CTX *) {
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    return sc->verify_callback;
}

int SSL_CTX_get_verify_mode(const SSL_CTX *ctx)
{
    return ctx->verify_mode;
}

int SSL_CTX_get_verify_depth(const SSL_CTX *ctx)
{
    return X509_VERIFY_PARAM_get_depth(ctx->param);
}

int (*SSL_CTX_get_verify_callback(const SSL_CTX *ctx)) (int, X509_STORE_CTX *) {
    return ctx->default_verify_callback;
}

void SSL_set_verify(SSL *s, int mode,
                    int (*callback) (int ok, X509_STORE_CTX *ctx))
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->verify_mode = mode;
    if (callback != NULL)
        sc->verify_callback = callback;
}

void SSL_set_verify_depth(SSL *s, int depth)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    X509_VERIFY_PARAM_set_depth(sc->param, depth);
}

void SSL_set_read_ahead(SSL *s, int yes)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);
    OSSL_PARAM options[2], *opts = options;

    if (sc == NULL)
        return;

    RECORD_LAYER_set_read_ahead(&sc->rlayer, yes);

    *opts++ = OSSL_PARAM_construct_int(OSSL_LIBSSL_RECORD_LAYER_PARAM_READ_AHEAD,
                                       &sc->rlayer.read_ahead);
    *opts = OSSL_PARAM_construct_end();

    /* Ignore return value */
    sc->rlayer.rrlmethod->set_options(sc->rlayer.rrl, options);
}

int SSL_get_read_ahead(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL_ONLY(s);

    if (sc == NULL)
        return 0;

    return RECORD_LAYER_get_read_ahead(&sc->rlayer);
}

int SSL_pending(const SSL *s)
{
    size_t pending = s->method->ssl_pending(s);

    /*
     * SSL_pending cannot work properly if read-ahead is enabled
     * (SSL_[CTX_]ctrl(..., SSL_CTRL_SET_READ_AHEAD, 1, NULL)), and it is
     * impossible to fix since SSL_pending cannot report errors that may be
     * observed while scanning the new data. (Note that SSL_pending() is
     * often used as a boolean value, so we'd better not return -1.)
     *
     * SSL_pending also cannot work properly if the value >INT_MAX. In that case
     * we just return INT_MAX.
     */
    return pending < INT_MAX ? (int)pending : INT_MAX;
}

int SSL_has_pending(const SSL *s)
{
    /*
     * Similar to SSL_pending() but returns a 1 to indicate that we have
     * processed or unprocessed data available or 0 otherwise (as opposed to the
     * number of bytes available). Unlike SSL_pending() this will take into
     * account read_ahead data. A 1 return simply indicates that we have data.
     * That data may not result in any application data, or we may fail to parse
     * the records for some reason.
     */
    const SSL_CONNECTION *sc;

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_has_pending(s);
#endif

    sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    /* Check buffered app data if any first */
    if (SSL_CONNECTION_IS_DTLS(sc)) {
        TLS_RECORD *rdata;
        pitem *item, *iter;

        iter = pqueue_iterator(sc->rlayer.d->buffered_app_data);
        while ((item = pqueue_next(&iter)) != NULL) {
            rdata = item->data;
            if (rdata->length > 0)
                return 1;
        }
    }

    if (RECORD_LAYER_processed_read_pending(&sc->rlayer))
        return 1;

    return RECORD_LAYER_read_pending(&sc->rlayer);
}

X509 *SSL_get1_peer_certificate(const SSL *s)
{
    X509 *r = SSL_get0_peer_certificate(s);

    if (r != NULL && !X509_up_ref(r))
        return NULL;

    return r;
}

X509 *SSL_get0_peer_certificate(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    if (sc->session == NULL)
        return NULL;
    else
        return sc->session->peer;
}

STACK_OF(X509) *SSL_get_peer_cert_chain(const SSL *s)
{
    STACK_OF(X509) *r;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    if (sc->session == NULL)
        r = NULL;
    else
        r = sc->session->peer_chain;

    /*
     * If we are a client, cert_chain includes the peer's own certificate; if
     * we are a server, it does not.
     */

    return r;
}

/*
 * Now in theory, since the calling process own 't' it should be safe to
 * modify.  We need to be able to read f without being hassled
 */
int SSL_copy_session_id(SSL *t, const SSL *f)
{
    int i;
    /* TODO(QUIC FUTURE): Not allowed for QUIC currently. */
    SSL_CONNECTION *tsc = SSL_CONNECTION_FROM_SSL_ONLY(t);
    const SSL_CONNECTION *fsc = SSL_CONNECTION_FROM_CONST_SSL_ONLY(f);

    if (tsc == NULL || fsc == NULL)
        return 0;

    /* Do we need to do SSL locking? */
    if (!SSL_set_session(t, SSL_get_session(f))) {
        return 0;
    }

    /*
     * what if we are setup for one protocol version but want to talk another
     */
    if (t->method != f->method) {
        t->method->ssl_deinit(t);
        t->method = f->method;
        if (t->method->ssl_init(t) == 0)
            return 0;
    }

    CRYPTO_UP_REF(&fsc->cert->references, &i);
    ssl_cert_free(tsc->cert);
    tsc->cert = fsc->cert;
    if (!SSL_set_session_id_context(t, fsc->sid_ctx, (int)fsc->sid_ctx_length)) {
        return 0;
    }

    return 1;
}

/* Fix this so it checks all the valid key/cert options */
int SSL_CTX_check_private_key(const SSL_CTX *ctx)
{
    if ((ctx == NULL) || (ctx->cert->key->x509 == NULL)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_CERTIFICATE_ASSIGNED);
        return 0;
    }
    if (ctx->cert->key->privatekey == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_PRIVATE_KEY_ASSIGNED);
        return 0;
    }
    return X509_check_private_key
            (ctx->cert->key->x509, ctx->cert->key->privatekey);
}

/* Fix this function so that it takes an optional type parameter */
int SSL_check_private_key(const SSL *ssl)
{
    const SSL_CONNECTION *sc;

    if ((sc = SSL_CONNECTION_FROM_CONST_SSL(ssl)) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (sc->cert->key->x509 == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_CERTIFICATE_ASSIGNED);
        return 0;
    }
    if (sc->cert->key->privatekey == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_PRIVATE_KEY_ASSIGNED);
        return 0;
    }
    return X509_check_private_key(sc->cert->key->x509,
                                   sc->cert->key->privatekey);
}

int SSL_waiting_for_async(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->job)
        return 1;

    return 0;
}

int SSL_get_all_async_fds(SSL *s, OSSL_ASYNC_FD *fds, size_t *numfds)
{
    ASYNC_WAIT_CTX *ctx;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if ((ctx = sc->waitctx) == NULL)
        return 0;
    return ASYNC_WAIT_CTX_get_all_fds(ctx, fds, numfds);
}

int SSL_get_changed_async_fds(SSL *s, OSSL_ASYNC_FD *addfd, size_t *numaddfds,
                              OSSL_ASYNC_FD *delfd, size_t *numdelfds)
{
    ASYNC_WAIT_CTX *ctx;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if ((ctx = sc->waitctx) == NULL)
        return 0;
    return ASYNC_WAIT_CTX_get_changed_fds(ctx, addfd, numaddfds, delfd,
                                          numdelfds);
}

int SSL_CTX_set_async_callback(SSL_CTX *ctx, SSL_async_callback_fn callback)
{
    ctx->async_cb = callback;
    return 1;
}

int SSL_CTX_set_async_callback_arg(SSL_CTX *ctx, void *arg)
{
    ctx->async_cb_arg = arg;
    return 1;
}

int SSL_set_async_callback(SSL *s, SSL_async_callback_fn callback)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    sc->async_cb = callback;
    return 1;
}

int SSL_set_async_callback_arg(SSL *s, void *arg)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    sc->async_cb_arg = arg;
    return 1;
}

int SSL_get_async_status(SSL *s, int *status)
{
    ASYNC_WAIT_CTX *ctx;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if ((ctx = sc->waitctx) == NULL)
        return 0;
    *status = ASYNC_WAIT_CTX_get_status(ctx);
    return 1;
}

int SSL_accept(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return s->method->ssl_accept(s);
#endif

    if (sc == NULL)
        return 0;

    if (sc->handshake_func == NULL) {
        /* Not properly initialized yet */
        SSL_set_accept_state(s);
    }

    return SSL_do_handshake(s);
}

int SSL_connect(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return s->method->ssl_connect(s);
#endif

    if (sc == NULL)
        return 0;

    if (sc->handshake_func == NULL) {
        /* Not properly initialized yet */
        SSL_set_connect_state(s);
    }

    return SSL_do_handshake(s);
}

long SSL_get_default_timeout(const SSL *s)
{
    return (long int)ossl_time2seconds(s->method->get_timeout());
}

static int ssl_async_wait_ctx_cb(void *arg)
{
    SSL *s = (SSL *)arg;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->async_cb(s, sc->async_cb_arg);
}

static int ssl_start_async_job(SSL *s, struct ssl_async_args *args,
                               int (*func) (void *))
{
    int ret;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->waitctx == NULL) {
        sc->waitctx = ASYNC_WAIT_CTX_new();
        if (sc->waitctx == NULL)
            return -1;
        if (sc->async_cb != NULL
            && !ASYNC_WAIT_CTX_set_callback
                 (sc->waitctx, ssl_async_wait_ctx_cb, s))
            return -1;
    }

    sc->rwstate = SSL_NOTHING;
    switch (ASYNC_start_job(&sc->job, sc->waitctx, &ret, func, args,
                            sizeof(struct ssl_async_args))) {
    case ASYNC_ERR:
        sc->rwstate = SSL_NOTHING;
        ERR_raise(ERR_LIB_SSL, SSL_R_FAILED_TO_INIT_ASYNC);
        return -1;
    case ASYNC_PAUSE:
        sc->rwstate = SSL_ASYNC_PAUSED;
        return -1;
    case ASYNC_NO_JOBS:
        sc->rwstate = SSL_ASYNC_NO_JOBS;
        return -1;
    case ASYNC_FINISH:
        sc->job = NULL;
        return ret;
    default:
        sc->rwstate = SSL_NOTHING;
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        /* Shouldn't happen */
        return -1;
    }
}

static int ssl_io_intern(void *vargs)
{
    struct ssl_async_args *args;
    SSL *s;
    void *buf;
    size_t num;
    SSL_CONNECTION *sc;

    args = (struct ssl_async_args *)vargs;
    s = args->s;
    buf = args->buf;
    num = args->num;
    if ((sc = SSL_CONNECTION_FROM_SSL(s)) == NULL)
        return -1;

    switch (args->type) {
    case READFUNC:
        return args->f.func_read(s, buf, num, &sc->asyncrw);
    case WRITEFUNC:
        return args->f.func_write(s, buf, num, &sc->asyncrw);
    case OTHERFUNC:
        return args->f.func_other(s);
    }
    return -1;
}

int ssl_read_internal(SSL *s, void *buf, size_t num, size_t *readbytes)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return s->method->ssl_read(s, buf, num, readbytes);
#endif

    if (sc == NULL)
        return -1;

    if (sc->handshake_func == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_UNINITIALIZED);
        return -1;
    }

    if (sc->shutdown & SSL_RECEIVED_SHUTDOWN) {
        sc->rwstate = SSL_NOTHING;
        return 0;
    }

    if (sc->early_data_state == SSL_EARLY_DATA_CONNECT_RETRY
                || sc->early_data_state == SSL_EARLY_DATA_ACCEPT_RETRY) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    /*
     * If we are a client and haven't received the ServerHello etc then we
     * better do that
     */
    if (!ossl_statem_check_finish_init(sc, 0))
        return -1;

    if ((sc->mode & SSL_MODE_ASYNC) && ASYNC_get_current_job() == NULL) {
        struct ssl_async_args args;
        int ret;

        args.s = s;
        args.buf = buf;
        args.num = num;
        args.type = READFUNC;
        args.f.func_read = s->method->ssl_read;

        ret = ssl_start_async_job(s, &args, ssl_io_intern);
        *readbytes = sc->asyncrw;
        return ret;
    } else {
        return s->method->ssl_read(s, buf, num, readbytes);
    }
}

int SSL_read(SSL *s, void *buf, int num)
{
    int ret;
    size_t readbytes;

    if (num < 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_BAD_LENGTH);
        return -1;
    }

    ret = ssl_read_internal(s, buf, (size_t)num, &readbytes);

    /*
     * The cast is safe here because ret should be <= INT_MAX because num is
     * <= INT_MAX
     */
    if (ret > 0)
        ret = (int)readbytes;

    return ret;
}

int SSL_read_ex(SSL *s, void *buf, size_t num, size_t *readbytes)
{
    int ret = ssl_read_internal(s, buf, num, readbytes);

    if (ret < 0)
        ret = 0;
    return ret;
}

int SSL_read_early_data(SSL *s, void *buf, size_t num, size_t *readbytes)
{
    int ret;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    /* TODO(QUIC 0RTT): 0-RTT support */
    if (sc == NULL || !sc->server) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return SSL_READ_EARLY_DATA_ERROR;
    }

    switch (sc->early_data_state) {
    case SSL_EARLY_DATA_NONE:
        if (!SSL_in_before(s)) {
            ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
            return SSL_READ_EARLY_DATA_ERROR;
        }
        /* fall through */

    case SSL_EARLY_DATA_ACCEPT_RETRY:
        sc->early_data_state = SSL_EARLY_DATA_ACCEPTING;
        ret = SSL_accept(s);
        if (ret <= 0) {
            /* NBIO or error */
            sc->early_data_state = SSL_EARLY_DATA_ACCEPT_RETRY;
            return SSL_READ_EARLY_DATA_ERROR;
        }
        /* fall through */

    case SSL_EARLY_DATA_READ_RETRY:
        if (sc->ext.early_data == SSL_EARLY_DATA_ACCEPTED) {
            sc->early_data_state = SSL_EARLY_DATA_READING;
            ret = SSL_read_ex(s, buf, num, readbytes);
            /*
             * State machine will update early_data_state to
             * SSL_EARLY_DATA_FINISHED_READING if we get an EndOfEarlyData
             * message
             */
            if (ret > 0 || (ret <= 0 && sc->early_data_state
                                        != SSL_EARLY_DATA_FINISHED_READING)) {
                sc->early_data_state = SSL_EARLY_DATA_READ_RETRY;
                return ret > 0 ? SSL_READ_EARLY_DATA_SUCCESS
                               : SSL_READ_EARLY_DATA_ERROR;
            }
        } else {
            sc->early_data_state = SSL_EARLY_DATA_FINISHED_READING;
        }
        *readbytes = 0;
        return SSL_READ_EARLY_DATA_FINISH;

    default:
        ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return SSL_READ_EARLY_DATA_ERROR;
    }
}

int SSL_get_early_data_status(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL_ONLY(s);

    /* TODO(QUIC 0RTT): 0-RTT support */
    if (sc == NULL)
        return 0;

    return sc->ext.early_data;
}

static int ssl_peek_internal(SSL *s, void *buf, size_t num, size_t *readbytes)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return s->method->ssl_peek(s, buf, num, readbytes);
#endif

    if (sc == NULL)
        return 0;

    if (sc->handshake_func == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_UNINITIALIZED);
        return -1;
    }

    if (sc->shutdown & SSL_RECEIVED_SHUTDOWN) {
        return 0;
    }
    if ((sc->mode & SSL_MODE_ASYNC) && ASYNC_get_current_job() == NULL) {
        struct ssl_async_args args;
        int ret;

        args.s = s;
        args.buf = buf;
        args.num = num;
        args.type = READFUNC;
        args.f.func_read = s->method->ssl_peek;

        ret = ssl_start_async_job(s, &args, ssl_io_intern);
        *readbytes = sc->asyncrw;
        return ret;
    } else {
        return s->method->ssl_peek(s, buf, num, readbytes);
    }
}

int SSL_peek(SSL *s, void *buf, int num)
{
    int ret;
    size_t readbytes;

    if (num < 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_BAD_LENGTH);
        return -1;
    }

    ret = ssl_peek_internal(s, buf, (size_t)num, &readbytes);

    /*
     * The cast is safe here because ret should be <= INT_MAX because num is
     * <= INT_MAX
     */
    if (ret > 0)
        ret = (int)readbytes;

    return ret;
}


int SSL_peek_ex(SSL *s, void *buf, size_t num, size_t *readbytes)
{
    int ret = ssl_peek_internal(s, buf, num, readbytes);

    if (ret < 0)
        ret = 0;
    return ret;
}

int ssl_write_internal(SSL *s, const void *buf, size_t num,
                       uint64_t flags, size_t *written)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_write_flags(s, buf, num, flags, written);
#endif

    if (sc == NULL)
        return 0;

    if (sc->handshake_func == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_UNINITIALIZED);
        return -1;
    }

    if (sc->shutdown & SSL_SENT_SHUTDOWN) {
        sc->rwstate = SSL_NOTHING;
        ERR_raise(ERR_LIB_SSL, SSL_R_PROTOCOL_IS_SHUTDOWN);
        return -1;
    }

    if (flags != 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_UNSUPPORTED_WRITE_FLAG);
        return -1;
    }

    if (sc->early_data_state == SSL_EARLY_DATA_CONNECT_RETRY
                || sc->early_data_state == SSL_EARLY_DATA_ACCEPT_RETRY
                || sc->early_data_state == SSL_EARLY_DATA_READ_RETRY) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    /* If we are a client and haven't sent the Finished we better do that */
    if (!ossl_statem_check_finish_init(sc, 1))
        return -1;

    if ((sc->mode & SSL_MODE_ASYNC) && ASYNC_get_current_job() == NULL) {
        int ret;
        struct ssl_async_args args;

        args.s = s;
        args.buf = (void *)buf;
        args.num = num;
        args.type = WRITEFUNC;
        args.f.func_write = s->method->ssl_write;

        ret = ssl_start_async_job(s, &args, ssl_io_intern);
        *written = sc->asyncrw;
        return ret;
    } else {
        return s->method->ssl_write(s, buf, num, written);
    }
}

ossl_ssize_t SSL_sendfile(SSL *s, int fd, off_t offset, size_t size, int flags)
{
    ossl_ssize_t ret;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return 0;

    if (sc->handshake_func == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_UNINITIALIZED);
        return -1;
    }

    if (sc->shutdown & SSL_SENT_SHUTDOWN) {
        sc->rwstate = SSL_NOTHING;
        ERR_raise(ERR_LIB_SSL, SSL_R_PROTOCOL_IS_SHUTDOWN);
        return -1;
    }

    if (!BIO_get_ktls_send(sc->wbio)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_UNINITIALIZED);
        return -1;
    }

    /* If we have an alert to send, lets send it */
    if (sc->s3.alert_dispatch > 0) {
        ret = (ossl_ssize_t)s->method->ssl_dispatch_alert(s);
        if (ret <= 0) {
            /* SSLfatal() already called if appropriate */
            return ret;
        }
        /* if it went, fall through and send more stuff */
    }

    sc->rwstate = SSL_WRITING;
    if (BIO_flush(sc->wbio) <= 0) {
        if (!BIO_should_retry(sc->wbio)) {
            sc->rwstate = SSL_NOTHING;
        } else {
#ifdef EAGAIN
            set_sys_error(EAGAIN);
#endif
        }
        return -1;
    }

#ifdef OPENSSL_NO_KTLS
    ERR_raise_data(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR,
                   "can't call ktls_sendfile(), ktls disabled");
    return -1;
#else
    ret = ktls_sendfile(SSL_get_wfd(s), fd, offset, size, flags);
    if (ret < 0) {
#if defined(EAGAIN) && defined(EINTR) && defined(EBUSY)
        if ((get_last_sys_error() == EAGAIN) ||
            (get_last_sys_error() == EINTR) ||
            (get_last_sys_error() == EBUSY))
            BIO_set_retry_write(sc->wbio);
        else
#endif
            ERR_raise_data(ERR_LIB_SYS, get_last_sys_error(),
                           "ktls_sendfile failure");
        return ret;
    }
    sc->rwstate = SSL_NOTHING;
    return ret;
#endif
}

int SSL_write(SSL *s, const void *buf, int num)
{
    int ret;
    size_t written;

    if (num < 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_BAD_LENGTH);
        return -1;
    }

    ret = ssl_write_internal(s, buf, (size_t)num, 0, &written);

    /*
     * The cast is safe here because ret should be <= INT_MAX because num is
     * <= INT_MAX
     */
    if (ret > 0)
        ret = (int)written;

    return ret;
}

int SSL_write_ex(SSL *s, const void *buf, size_t num, size_t *written)
{
    return SSL_write_ex2(s, buf, num, 0, written);
}

int SSL_write_ex2(SSL *s, const void *buf, size_t num, uint64_t flags,
                  size_t *written)
{
    int ret = ssl_write_internal(s, buf, num, flags, written);

    if (ret < 0)
        ret = 0;
    return ret;
}

int SSL_write_early_data(SSL *s, const void *buf, size_t num, size_t *written)
{
    int ret, early_data_state;
    size_t writtmp;
    uint32_t partialwrite;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    /* TODO(QUIC 0RTT): This will need special handling for QUIC */
    if (sc == NULL)
        return 0;

    switch (sc->early_data_state) {
    case SSL_EARLY_DATA_NONE:
        if (sc->server
                || !SSL_in_before(s)
                || ((sc->session == NULL || sc->session->ext.max_early_data == 0)
                     && (sc->psk_use_session_cb == NULL))) {
            ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
            return 0;
        }
        /* fall through */

    case SSL_EARLY_DATA_CONNECT_RETRY:
        sc->early_data_state = SSL_EARLY_DATA_CONNECTING;
        ret = SSL_connect(s);
        if (ret <= 0) {
            /* NBIO or error */
            sc->early_data_state = SSL_EARLY_DATA_CONNECT_RETRY;
            return 0;
        }
        /* fall through */

    case SSL_EARLY_DATA_WRITE_RETRY:
        sc->early_data_state = SSL_EARLY_DATA_WRITING;
        /*
         * We disable partial write for early data because we don't keep track
         * of how many bytes we've written between the SSL_write_ex() call and
         * the flush if the flush needs to be retried)
         */
        partialwrite = sc->mode & SSL_MODE_ENABLE_PARTIAL_WRITE;
        sc->mode &= ~SSL_MODE_ENABLE_PARTIAL_WRITE;
        ret = SSL_write_ex(s, buf, num, &writtmp);
        sc->mode |= partialwrite;
        if (!ret) {
            sc->early_data_state = SSL_EARLY_DATA_WRITE_RETRY;
            return ret;
        }
        sc->early_data_state = SSL_EARLY_DATA_WRITE_FLUSH;
        /* fall through */

    case SSL_EARLY_DATA_WRITE_FLUSH:
        /* The buffering BIO is still in place so we need to flush it */
        if (statem_flush(sc) != 1)
            return 0;
        *written = num;
        sc->early_data_state = SSL_EARLY_DATA_WRITE_RETRY;
        return 1;

    case SSL_EARLY_DATA_FINISHED_READING:
    case SSL_EARLY_DATA_READ_RETRY:
        early_data_state = sc->early_data_state;
        /* We are a server writing to an unauthenticated client */
        sc->early_data_state = SSL_EARLY_DATA_UNAUTH_WRITING;
        ret = SSL_write_ex(s, buf, num, written);
        /* The buffering BIO is still in place */
        if (ret)
            (void)BIO_flush(sc->wbio);
        sc->early_data_state = early_data_state;
        return ret;

    default:
        ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
}

int SSL_shutdown(SSL *s)
{
    /*
     * Note that this function behaves differently from what one might
     * expect.  Return values are 0 for no success (yet), 1 for success; but
     * calling it once is usually not enough, even if blocking I/O is used
     * (see ssl3_shutdown).
     */
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_conn_shutdown(s, 0, NULL, 0);
#endif

    if (sc == NULL)
        return -1;

    if (sc->handshake_func == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_UNINITIALIZED);
        return -1;
    }

    if (!SSL_in_init(s)) {
        if ((sc->mode & SSL_MODE_ASYNC) && ASYNC_get_current_job() == NULL) {
            struct ssl_async_args args;

            memset(&args, 0, sizeof(args));
            args.s = s;
            args.type = OTHERFUNC;
            args.f.func_other = s->method->ssl_shutdown;

            return ssl_start_async_job(s, &args, ssl_io_intern);
        } else {
            return s->method->ssl_shutdown(s);
        }
    } else {
        ERR_raise(ERR_LIB_SSL, SSL_R_SHUTDOWN_WHILE_IN_INIT);
        return -1;
    }
}

int SSL_key_update(SSL *s, int updatetype)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_key_update(s, updatetype);
#endif

    if (sc == NULL)
        return 0;

    if (!SSL_CONNECTION_IS_TLS13(sc)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_WRONG_SSL_VERSION);
        return 0;
    }

    if (updatetype != SSL_KEY_UPDATE_NOT_REQUESTED
            && updatetype != SSL_KEY_UPDATE_REQUESTED) {
        ERR_raise(ERR_LIB_SSL, SSL_R_INVALID_KEY_UPDATE_TYPE);
        return 0;
    }

    if (!SSL_is_init_finished(s)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_STILL_IN_INIT);
        return 0;
    }

    if (RECORD_LAYER_write_pending(&sc->rlayer)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_BAD_WRITE_RETRY);
        return 0;
    }

    ossl_statem_set_in_init(sc, 1);
    sc->key_update = updatetype;
    return 1;
}

int SSL_get_key_update_type(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_get_key_update_type(s);
#endif

    if (sc == NULL)
        return 0;

    return sc->key_update;
}

/*
 * Can we accept a renegotiation request?  If yes, set the flag and
 * return 1 if yes. If not, raise error and return 0.
 */
static int can_renegotiate(const SSL_CONNECTION *sc)
{
    if (SSL_CONNECTION_IS_TLS13(sc)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_WRONG_SSL_VERSION);
        return 0;
    }

    if ((sc->options & SSL_OP_NO_RENEGOTIATION) != 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_RENEGOTIATION);
        return 0;
    }

    return 1;
}

int SSL_renegotiate(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return 0;

    if (!can_renegotiate(sc))
        return 0;

    sc->renegotiate = 1;
    sc->new_session = 1;
    return s->method->ssl_renegotiate(s);
}

int SSL_renegotiate_abbreviated(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return 0;

    if (!can_renegotiate(sc))
        return 0;

    sc->renegotiate = 1;
    sc->new_session = 0;
    return s->method->ssl_renegotiate(s);
}

int SSL_renegotiate_pending(const SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return 0;

    /*
     * becomes true when negotiation is requested; false again once a
     * handshake has finished
     */
    return (sc->renegotiate != 0);
}

int SSL_new_session_ticket(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    /* If we are in init because we're sending tickets, okay to send more. */
    if ((SSL_in_init(s) && sc->ext.extra_tickets_expected == 0)
            || SSL_IS_FIRST_HANDSHAKE(sc) || !sc->server
            || !SSL_CONNECTION_IS_TLS13(sc))
        return 0;
    sc->ext.extra_tickets_expected++;
    if (!RECORD_LAYER_write_pending(&sc->rlayer) && !SSL_in_init(s))
        ossl_statem_set_in_init(sc, 1);
    return 1;
}

long SSL_ctrl(SSL *s, int cmd, long larg, void *parg)
{
    return ossl_ctrl_internal(s, cmd, larg, parg, /*no_quic=*/0);
}

long ossl_ctrl_internal(SSL *s, int cmd, long larg, void *parg, int no_quic)
{
    long l;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    /*
     * Routing of ctrl calls for QUIC is a little counterintuitive:
     *
     *   - Firstly (no_quic=0), we pass the ctrl directly to our QUIC
     *     implementation in case it wants to handle the ctrl specially.
     *
     *   - If our QUIC implementation does not care about the ctrl, it
     *     will reenter this function with no_quic=1 and we will try to handle
     *     it directly using the QCSO SSL object stub (not the handshake layer
     *     SSL object). This is important for e.g. the version configuration
     *     ctrls below, which must use s->defltmeth (and not sc->defltmeth).
     *
     *   - If we don't handle a ctrl here specially, then processing is
     *     redirected to the handshake layer SSL object.
     */
    if (!no_quic && IS_QUIC(s))
        return s->method->ssl_ctrl(s, cmd, larg, parg);

    if (sc == NULL)
        return 0;

    switch (cmd) {
    case SSL_CTRL_GET_READ_AHEAD:
        return RECORD_LAYER_get_read_ahead(&sc->rlayer);
    case SSL_CTRL_SET_READ_AHEAD:
        l = RECORD_LAYER_get_read_ahead(&sc->rlayer);
        RECORD_LAYER_set_read_ahead(&sc->rlayer, larg);
        return l;

    case SSL_CTRL_MODE:
    {
        OSSL_PARAM options[2], *opts = options;

        sc->mode |= larg;

        *opts++ = OSSL_PARAM_construct_uint32(OSSL_LIBSSL_RECORD_LAYER_PARAM_MODE,
                                              &sc->mode);
        *opts = OSSL_PARAM_construct_end();

        /* Ignore return value */
        sc->rlayer.rrlmethod->set_options(sc->rlayer.rrl, options);

        return sc->mode;
    }
    case SSL_CTRL_CLEAR_MODE:
        return (sc->mode &= ~larg);
    case SSL_CTRL_GET_MAX_CERT_LIST:
        return (long)sc->max_cert_list;
    case SSL_CTRL_SET_MAX_CERT_LIST:
        if (larg < 0)
            return 0;
        l = (long)sc->max_cert_list;
        sc->max_cert_list = (size_t)larg;
        return l;
    case SSL_CTRL_SET_MAX_SEND_FRAGMENT:
        if (larg < 512 || larg > SSL3_RT_MAX_PLAIN_LENGTH)
            return 0;
#ifndef OPENSSL_NO_KTLS
        if (sc->wbio != NULL && BIO_get_ktls_send(sc->wbio))
            return 0;
#endif /* OPENSSL_NO_KTLS */
        sc->max_send_fragment = larg;
        if (sc->max_send_fragment < sc->split_send_fragment)
            sc->split_send_fragment = sc->max_send_fragment;
        sc->rlayer.wrlmethod->set_max_frag_len(sc->rlayer.wrl, larg);
        return 1;
    case SSL_CTRL_SET_SPLIT_SEND_FRAGMENT:
        if ((size_t)larg > sc->max_send_fragment || larg == 0)
            return 0;
        sc->split_send_fragment = larg;
        return 1;
    case SSL_CTRL_SET_MAX_PIPELINES:
        if (larg < 1 || larg > SSL_MAX_PIPELINES)
            return 0;
        sc->max_pipelines = larg;
        if (sc->rlayer.rrlmethod->set_max_pipelines != NULL)
            sc->rlayer.rrlmethod->set_max_pipelines(sc->rlayer.rrl, (size_t)larg);
        return 1;
    case SSL_CTRL_GET_RI_SUPPORT:
        return sc->s3.send_connection_binding;
    case SSL_CTRL_SET_RETRY_VERIFY:
        sc->rwstate = SSL_RETRY_VERIFY;
        return 1;
    case SSL_CTRL_CERT_FLAGS:
        return (sc->cert->cert_flags |= larg);
    case SSL_CTRL_CLEAR_CERT_FLAGS:
        return (sc->cert->cert_flags &= ~larg);

    case SSL_CTRL_GET_RAW_CIPHERLIST:
        if (parg) {
            if (sc->s3.tmp.ciphers_raw == NULL)
                return 0;
            *(unsigned char **)parg = sc->s3.tmp.ciphers_raw;
            return (int)sc->s3.tmp.ciphers_rawlen;
        } else {
            return TLS_CIPHER_LEN;
        }
    case SSL_CTRL_GET_EXTMS_SUPPORT:
        if (!sc->session || SSL_in_init(s) || ossl_statem_get_in_handshake(sc))
            return -1;
        if (sc->session->flags & SSL_SESS_FLAG_EXTMS)
            return 1;
        else
            return 0;
    case SSL_CTRL_SET_MIN_PROTO_VERSION:
        return ssl_check_allowed_versions(larg, sc->max_proto_version)
               && ssl_set_version_bound(s->defltmeth->version, (int)larg,
                                        &sc->min_proto_version);
    case SSL_CTRL_GET_MIN_PROTO_VERSION:
        return sc->min_proto_version;
    case SSL_CTRL_SET_MAX_PROTO_VERSION:
        return ssl_check_allowed_versions(sc->min_proto_version, larg)
               && ssl_set_version_bound(s->defltmeth->version, (int)larg,
                                        &sc->max_proto_version);
    case SSL_CTRL_GET_MAX_PROTO_VERSION:
        return sc->max_proto_version;
    default:
        if (IS_QUIC(s))
            return SSL_ctrl((SSL *)sc, cmd, larg, parg);
        else
            return s->method->ssl_ctrl(s, cmd, larg, parg);
    }
}

long SSL_callback_ctrl(SSL *s, int cmd, void (*fp) (void))
{
    return s->method->ssl_callback_ctrl(s, cmd, fp);
}

LHASH_OF(SSL_SESSION) *SSL_CTX_sessions(SSL_CTX *ctx)
{
    return ctx->sessions;
}

static int ssl_tsan_load(SSL_CTX *ctx, TSAN_QUALIFIER int *stat)
{
    int res = 0;

    if (ssl_tsan_lock(ctx)) {
        res = tsan_load(stat);
        ssl_tsan_unlock(ctx);
    }
    return res;
}

long SSL_CTX_ctrl(SSL_CTX *ctx, int cmd, long larg, void *parg)
{
    long l;

    /* For some cases with ctx == NULL or larg == 1 perform syntax checks */
    if (cmd == SSL_CTRL_SET_GROUPS_LIST && larg == 1)
        return tls1_set_groups_list(ctx, NULL, NULL, NULL, NULL, NULL, NULL, parg);
    if (ctx == NULL) {
        switch (cmd) {
        case SSL_CTRL_SET_SIGALGS_LIST:
        case SSL_CTRL_SET_CLIENT_SIGALGS_LIST:
            return tls1_set_sigalgs_list(ctx, NULL, parg, 0);
        default:
            return 0;
        }
    }

    switch (cmd) {
    case SSL_CTRL_GET_READ_AHEAD:
        return ctx->read_ahead;
    case SSL_CTRL_SET_READ_AHEAD:
        l = ctx->read_ahead;
        ctx->read_ahead = larg;
        return l;

    case SSL_CTRL_SET_MSG_CALLBACK_ARG:
        ctx->msg_callback_arg = parg;
        return 1;

    case SSL_CTRL_GET_MAX_CERT_LIST:
        return (long)ctx->max_cert_list;
    case SSL_CTRL_SET_MAX_CERT_LIST:
        if (larg < 0)
            return 0;
        l = (long)ctx->max_cert_list;
        ctx->max_cert_list = (size_t)larg;
        return l;

    case SSL_CTRL_SET_SESS_CACHE_SIZE:
        if (larg < 0)
            return 0;
        l = (long)ctx->session_cache_size;
        ctx->session_cache_size = (size_t)larg;
        return l;
    case SSL_CTRL_GET_SESS_CACHE_SIZE:
        return (long)ctx->session_cache_size;
    case SSL_CTRL_SET_SESS_CACHE_MODE:
        l = ctx->session_cache_mode;
        ctx->session_cache_mode = larg;
        return l;
    case SSL_CTRL_GET_SESS_CACHE_MODE:
        return ctx->session_cache_mode;

    case SSL_CTRL_SESS_NUMBER:
        return lh_SSL_SESSION_num_items(ctx->sessions);
    case SSL_CTRL_SESS_CONNECT:
        return ssl_tsan_load(ctx, &ctx->stats.sess_connect);
    case SSL_CTRL_SESS_CONNECT_GOOD:
        return ssl_tsan_load(ctx, &ctx->stats.sess_connect_good);
    case SSL_CTRL_SESS_CONNECT_RENEGOTIATE:
        return ssl_tsan_load(ctx, &ctx->stats.sess_connect_renegotiate);
    case SSL_CTRL_SESS_ACCEPT:
        return ssl_tsan_load(ctx, &ctx->stats.sess_accept);
    case SSL_CTRL_SESS_ACCEPT_GOOD:
        return ssl_tsan_load(ctx, &ctx->stats.sess_accept_good);
    case SSL_CTRL_SESS_ACCEPT_RENEGOTIATE:
        return ssl_tsan_load(ctx, &ctx->stats.sess_accept_renegotiate);
    case SSL_CTRL_SESS_HIT:
        return ssl_tsan_load(ctx, &ctx->stats.sess_hit);
    case SSL_CTRL_SESS_CB_HIT:
        return ssl_tsan_load(ctx, &ctx->stats.sess_cb_hit);
    case SSL_CTRL_SESS_MISSES:
        return ssl_tsan_load(ctx, &ctx->stats.sess_miss);
    case SSL_CTRL_SESS_TIMEOUTS:
        return ssl_tsan_load(ctx, &ctx->stats.sess_timeout);
    case SSL_CTRL_SESS_CACHE_FULL:
        return ssl_tsan_load(ctx, &ctx->stats.sess_cache_full);
    case SSL_CTRL_MODE:
        return (ctx->mode |= larg);
    case SSL_CTRL_CLEAR_MODE:
        return (ctx->mode &= ~larg);
    case SSL_CTRL_SET_MAX_SEND_FRAGMENT:
        if (larg < 512 || larg > SSL3_RT_MAX_PLAIN_LENGTH)
            return 0;
        ctx->max_send_fragment = larg;
        if (ctx->max_send_fragment < ctx->split_send_fragment)
            ctx->split_send_fragment = ctx->max_send_fragment;
        return 1;
    case SSL_CTRL_SET_SPLIT_SEND_FRAGMENT:
        if ((size_t)larg > ctx->max_send_fragment || larg == 0)
            return 0;
        ctx->split_send_fragment = larg;
        return 1;
    case SSL_CTRL_SET_MAX_PIPELINES:
        if (larg < 1 || larg > SSL_MAX_PIPELINES)
            return 0;
        ctx->max_pipelines = larg;
        return 1;
    case SSL_CTRL_CERT_FLAGS:
        return (ctx->cert->cert_flags |= larg);
    case SSL_CTRL_CLEAR_CERT_FLAGS:
        return (ctx->cert->cert_flags &= ~larg);
    case SSL_CTRL_SET_MIN_PROTO_VERSION:
        return ssl_check_allowed_versions(larg, ctx->max_proto_version)
               && ssl_set_version_bound(ctx->method->version, (int)larg,
                                        &ctx->min_proto_version);
    case SSL_CTRL_GET_MIN_PROTO_VERSION:
        return ctx->min_proto_version;
    case SSL_CTRL_SET_MAX_PROTO_VERSION:
        return ssl_check_allowed_versions(ctx->min_proto_version, larg)
               && ssl_set_version_bound(ctx->method->version, (int)larg,
                                        &ctx->max_proto_version);
    case SSL_CTRL_GET_MAX_PROTO_VERSION:
        return ctx->max_proto_version;
    default:
        return ctx->method->ssl_ctx_ctrl(ctx, cmd, larg, parg);
    }
}

long SSL_CTX_callback_ctrl(SSL_CTX *ctx, int cmd, void (*fp) (void))
{
    switch (cmd) {
    case SSL_CTRL_SET_MSG_CALLBACK:
        ctx->msg_callback = (void (*)
                             (int write_p, int version, int content_type,
                              const void *buf, size_t len, SSL *ssl,
                              void *arg))(fp);
        return 1;

    default:
        return ctx->method->ssl_ctx_callback_ctrl(ctx, cmd, fp);
    }
}

int ssl_cipher_id_cmp(const SSL_CIPHER *a, const SSL_CIPHER *b)
{
    if (a->id > b->id)
        return 1;
    if (a->id < b->id)
        return -1;
    return 0;
}

int ssl_cipher_ptr_id_cmp(const SSL_CIPHER *const *ap,
                          const SSL_CIPHER *const *bp)
{
    if ((*ap)->id > (*bp)->id)
        return 1;
    if ((*ap)->id < (*bp)->id)
        return -1;
    return 0;
}

/*
 * return a STACK of the ciphers available for the SSL and in order of
 * preference
 */
STACK_OF(SSL_CIPHER) *SSL_get_ciphers(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc != NULL) {
        if (sc->cipher_list != NULL) {
            return sc->cipher_list;
        } else if ((s->ctx != NULL) && (s->ctx->cipher_list != NULL)) {
            return s->ctx->cipher_list;
        }
    }
    return NULL;
}

STACK_OF(SSL_CIPHER) *SSL_get_client_ciphers(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL || !sc->server)
        return NULL;
    return sc->peer_ciphers;
}

STACK_OF(SSL_CIPHER) *SSL_get1_supported_ciphers(SSL *s)
{
    STACK_OF(SSL_CIPHER) *sk = NULL, *ciphers;
    int i;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    ciphers = SSL_get_ciphers(s);
    if (!ciphers)
        return NULL;
    if (!ssl_set_client_disabled(sc))
        return NULL;
    for (i = 0; i < sk_SSL_CIPHER_num(ciphers); i++) {
        const SSL_CIPHER *c = sk_SSL_CIPHER_value(ciphers, i);
        if (!ssl_cipher_disabled(sc, c, SSL_SECOP_CIPHER_SUPPORTED, 0)) {
            if (!sk)
                sk = sk_SSL_CIPHER_new_null();
            if (!sk)
                return NULL;
            if (!sk_SSL_CIPHER_push(sk, c)) {
                sk_SSL_CIPHER_free(sk);
                return NULL;
            }
        }
    }
    return sk;
}

/** return a STACK of the ciphers available for the SSL and in order of
 * algorithm id */
STACK_OF(SSL_CIPHER) *ssl_get_ciphers_by_id(SSL_CONNECTION *s)
{
    if (s != NULL) {
        if (s->cipher_list_by_id != NULL)
            return s->cipher_list_by_id;
        else if (s->ssl.ctx != NULL
                 && s->ssl.ctx->cipher_list_by_id != NULL)
            return s->ssl.ctx->cipher_list_by_id;
    }
    return NULL;
}

/** The old interface to get the same thing as SSL_get_ciphers() */
const char *SSL_get_cipher_list(const SSL *s, int n)
{
    const SSL_CIPHER *c;
    STACK_OF(SSL_CIPHER) *sk;

    if (s == NULL)
        return NULL;
    sk = SSL_get_ciphers(s);
    if ((sk == NULL) || (sk_SSL_CIPHER_num(sk) <= n))
        return NULL;
    c = sk_SSL_CIPHER_value(sk, n);
    if (c == NULL)
        return NULL;
    return c->name;
}

/** return a STACK of the ciphers available for the SSL_CTX and in order of
 * preference */
STACK_OF(SSL_CIPHER) *SSL_CTX_get_ciphers(const SSL_CTX *ctx)
{
    if (ctx != NULL)
        return ctx->cipher_list;
    return NULL;
}

/*
 * Distinguish between ciphers controlled by set_ciphersuite() and
 * set_cipher_list() when counting.
 */
static int cipher_list_tls12_num(STACK_OF(SSL_CIPHER) *sk)
{
    int i, num = 0;
    const SSL_CIPHER *c;

    if (sk == NULL)
        return 0;
    for (i = 0; i < sk_SSL_CIPHER_num(sk); ++i) {
        c = sk_SSL_CIPHER_value(sk, i);
        if (c->min_tls >= TLS1_3_VERSION)
            continue;
        num++;
    }
    return num;
}

/** specify the ciphers to be used by default by the SSL_CTX */
int SSL_CTX_set_cipher_list(SSL_CTX *ctx, const char *str)
{
    STACK_OF(SSL_CIPHER) *sk;

    sk = ssl_create_cipher_list(ctx, ctx->tls13_ciphersuites,
                                &ctx->cipher_list, &ctx->cipher_list_by_id, str,
                                ctx->cert);
    /*
     * ssl_create_cipher_list may return an empty stack if it was unable to
     * find a cipher matching the given rule string (for example if the rule
     * string specifies a cipher which has been disabled). This is not an
     * error as far as ssl_create_cipher_list is concerned, and hence
     * ctx->cipher_list and ctx->cipher_list_by_id has been updated.
     */
    if (sk == NULL)
        return 0;
    if (ctx->method->num_ciphers() > 0 && cipher_list_tls12_num(sk) == 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_CIPHER_MATCH);
        return 0;
    }
    return 1;
}

/** specify the ciphers to be used by the SSL */
int SSL_set_cipher_list(SSL *s, const char *str)
{
    STACK_OF(SSL_CIPHER) *sk;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);
    SSL_CTX *ctx;

    if (sc == NULL)
        return 0;

    ctx = s->ctx;
    sk = ssl_create_cipher_list(ctx, sc->tls13_ciphersuites,
                                &sc->cipher_list, &sc->cipher_list_by_id, str,
                                sc->cert);
    /* see comment in SSL_CTX_set_cipher_list */
    if (sk == NULL)
        return 0;
    if (ctx->method->num_ciphers() > 0 && cipher_list_tls12_num(sk) == 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_CIPHER_MATCH);
        return 0;
    }
    return 1;
}

char *SSL_get_shared_ciphers(const SSL *s, char *buf, int size)
{
    char *p;
    STACK_OF(SSL_CIPHER) *clntsk, *srvrsk;
    const SSL_CIPHER *c;
    int i;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    if (!sc->server
            || sc->peer_ciphers == NULL
            || size < 2)
        return NULL;

    p = buf;
    clntsk = sc->peer_ciphers;
    srvrsk = SSL_get_ciphers(s);
    if (clntsk == NULL || srvrsk == NULL)
        return NULL;

    if (sk_SSL_CIPHER_num(clntsk) == 0 || sk_SSL_CIPHER_num(srvrsk) == 0)
        return NULL;

    for (i = 0; i < sk_SSL_CIPHER_num(clntsk); i++) {
        int n;

        c = sk_SSL_CIPHER_value(clntsk, i);
        if (sk_SSL_CIPHER_find(srvrsk, c) < 0)
            continue;

        n = OPENSSL_strnlen(c->name, size);
        if (n >= size) {
            if (p != buf)
                --p;
            *p = '\0';
            return buf;
        }
        memcpy(p, c->name, n);
        p += n;
        *(p++) = ':';
        size -= n + 1;
    }
    p[-1] = '\0';
    return buf;
}

/**
 * Return the requested servername (SNI) value. Note that the behaviour varies
 * depending on:
 * - whether this is called by the client or the server,
 * - if we are before or during/after the handshake,
 * - if a resumption or normal handshake is being attempted/has occurred
 * - whether we have negotiated TLSv1.2 (or below) or TLSv1.3
 *
 * Note that only the host_name type is defined (RFC 3546).
 */
const char *SSL_get_servername(const SSL *s, const int type)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);
    int server;

    if (sc == NULL)
        return NULL;

    /*
     * If we don't know if we are the client or the server yet then we assume
     * client.
     */
    server = sc->handshake_func == NULL ? 0 : sc->server;

    if (type != TLSEXT_NAMETYPE_host_name)
        return NULL;

    if (server) {
        /**
         * Server side
         * In TLSv1.3 on the server SNI is not associated with the session
         * but in TLSv1.2 or below it is.
         *
         * Before the handshake:
         *  - return NULL
         *
         * During/after the handshake (TLSv1.2 or below resumption occurred):
         * - If a servername was accepted by the server in the original
         *   handshake then it will return that servername, or NULL otherwise.
         *
         * During/after the handshake (TLSv1.2 or below resumption did not occur):
         * - The function will return the servername requested by the client in
         *   this handshake or NULL if none was requested.
         */
         if (sc->hit && !SSL_CONNECTION_IS_TLS13(sc))
            return sc->session->ext.hostname;
    } else {
        /**
         * Client side
         *
         * Before the handshake:
         *  - If a servername has been set via a call to
         *    SSL_set_tlsext_host_name() then it will return that servername
         *  - If one has not been set, but a TLSv1.2 resumption is being
         *    attempted and the session from the original handshake had a
         *    servername accepted by the server then it will return that
         *    servername
         *  - Otherwise it returns NULL
         *
         * During/after the handshake (TLSv1.2 or below resumption occurred):
         * - If the session from the original handshake had a servername accepted
         *   by the server then it will return that servername.
         * - Otherwise it returns the servername set via
         *   SSL_set_tlsext_host_name() (or NULL if it was not called).
         *
         * During/after the handshake (TLSv1.2 or below resumption did not occur):
         * - It will return the servername set via SSL_set_tlsext_host_name()
         *   (or NULL if it was not called).
         */
        if (SSL_in_before(s)) {
            if (sc->ext.hostname == NULL
                    && sc->session != NULL
                    && sc->session->ssl_version != TLS1_3_VERSION)
                return sc->session->ext.hostname;
        } else {
            if (!SSL_CONNECTION_IS_TLS13(sc) && sc->hit
                && sc->session->ext.hostname != NULL)
                return sc->session->ext.hostname;
        }
    }

    return sc->ext.hostname;
}

int SSL_get_servername_type(const SSL *s)
{
    if (SSL_get_servername(s, TLSEXT_NAMETYPE_host_name) != NULL)
        return TLSEXT_NAMETYPE_host_name;
    return -1;
}

/*
 * SSL_select_next_proto implements the standard protocol selection. It is
 * expected that this function is called from the callback set by
 * SSL_CTX_set_next_proto_select_cb. The protocol data is assumed to be a
 * vector of 8-bit, length prefixed byte strings. The length byte itself is
 * not included in the length. A byte string of length 0 is invalid. No byte
 * string may be truncated. The current, but experimental algorithm for
 * selecting the protocol is: 1) If the server doesn't support NPN then this
 * is indicated to the callback. In this case, the client application has to
 * abort the connection or have a default application level protocol. 2) If
 * the server supports NPN, but advertises an empty list then the client
 * selects the first protocol in its list, but indicates via the API that this
 * fallback case was enacted. 3) Otherwise, the client finds the first
 * protocol in the server's list that it supports and selects this protocol.
 * This is because it's assumed that the server has better information about
 * which protocol a client should use. 4) If the client doesn't support any
 * of the server's advertised protocols, then this is treated the same as
 * case 2. It returns either OPENSSL_NPN_NEGOTIATED if a common protocol was
 * found, or OPENSSL_NPN_NO_OVERLAP if the fallback case was reached.
 */
int SSL_select_next_proto(unsigned char **out, unsigned char *outlen,
                          const unsigned char *server,
                          unsigned int server_len,
                          const unsigned char *client, unsigned int client_len)
{
    PACKET cpkt, csubpkt, spkt, ssubpkt;

    if (!PACKET_buf_init(&cpkt, client, client_len)
            || !PACKET_get_length_prefixed_1(&cpkt, &csubpkt)
            || PACKET_remaining(&csubpkt) == 0) {
        *out = NULL;
        *outlen = 0;
        return OPENSSL_NPN_NO_OVERLAP;
    }

    /*
     * Set the default opportunistic protocol. Will be overwritten if we find
     * a match.
     */
    *out = (unsigned char *)PACKET_data(&csubpkt);
    *outlen = (unsigned char)PACKET_remaining(&csubpkt);

    /*
     * For each protocol in server preference order, see if we support it.
     */
    if (PACKET_buf_init(&spkt, server, server_len)) {
        while (PACKET_get_length_prefixed_1(&spkt, &ssubpkt)) {
            if (PACKET_remaining(&ssubpkt) == 0)
                continue; /* Invalid - ignore it */
            if (PACKET_buf_init(&cpkt, client, client_len)) {
                while (PACKET_get_length_prefixed_1(&cpkt, &csubpkt)) {
                    if (PACKET_equal(&csubpkt, PACKET_data(&ssubpkt),
                                     PACKET_remaining(&ssubpkt))) {
                        /* We found a match */
                        *out = (unsigned char *)PACKET_data(&ssubpkt);
                        *outlen = (unsigned char)PACKET_remaining(&ssubpkt);
                        return OPENSSL_NPN_NEGOTIATED;
                    }
                }
                /* Ignore spurious trailing bytes in the client list */
            } else {
                /* This should never happen */
                return OPENSSL_NPN_NO_OVERLAP;
            }
        }
        /* Ignore spurious trailing bytes in the server list */
    }

    /*
     * There's no overlap between our protocols and the server's list. We use
     * the default opportunistic protocol selected earlier
     */
    return OPENSSL_NPN_NO_OVERLAP;
}

#ifndef OPENSSL_NO_NEXTPROTONEG
/*
 * SSL_get0_next_proto_negotiated sets *data and *len to point to the
 * client's requested protocol for this connection and returns 0. If the
 * client didn't request any protocol, then *data is set to NULL. Note that
 * the client can request any protocol it chooses. The value returned from
 * this function need not be a member of the list of supported protocols
 * provided by the callback.
 */
void SSL_get0_next_proto_negotiated(const SSL *s, const unsigned char **data,
                                    unsigned *len)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL) {
        /* We have no other way to indicate error */
        *data = NULL;
        *len = 0;
        return;
    }

    *data = sc->ext.npn;
    if (*data == NULL) {
        *len = 0;
    } else {
        *len = (unsigned int)sc->ext.npn_len;
    }
}

/*
 * SSL_CTX_set_npn_advertised_cb sets a callback that is called when
 * a TLS server needs a list of supported protocols for Next Protocol
 * Negotiation. The returned list must be in wire format.  The list is
 * returned by setting |out| to point to it and |outlen| to its length. This
 * memory will not be modified, but one should assume that the SSL* keeps a
 * reference to it. The callback should return SSL_TLSEXT_ERR_OK if it
 * wishes to advertise. Otherwise, no such extension will be included in the
 * ServerHello.
 */
void SSL_CTX_set_npn_advertised_cb(SSL_CTX *ctx,
                                   SSL_CTX_npn_advertised_cb_func cb,
                                   void *arg)
{
    if (IS_QUIC_CTX(ctx))
        /* NPN not allowed for QUIC */
        return;

    ctx->ext.npn_advertised_cb = cb;
    ctx->ext.npn_advertised_cb_arg = arg;
}

/*
 * SSL_CTX_set_next_proto_select_cb sets a callback that is called when a
 * client needs to select a protocol from the server's provided list. |out|
 * must be set to point to the selected protocol (which may be within |in|).
 * The length of the protocol name must be written into |outlen|. The
 * server's advertised protocols are provided in |in| and |inlen|. The
 * callback can assume that |in| is syntactically valid. The client must
 * select a protocol. It is fatal to the connection if this callback returns
 * a value other than SSL_TLSEXT_ERR_OK.
 */
void SSL_CTX_set_npn_select_cb(SSL_CTX *ctx,
                               SSL_CTX_npn_select_cb_func cb,
                               void *arg)
{
    if (IS_QUIC_CTX(ctx))
        /* NPN not allowed for QUIC */
        return;

    ctx->ext.npn_select_cb = cb;
    ctx->ext.npn_select_cb_arg = arg;
}
#endif

static int alpn_value_ok(const unsigned char *protos, unsigned int protos_len)
{
    unsigned int idx;

    if (protos_len < 2 || protos == NULL)
        return 0;

    for (idx = 0; idx < protos_len; idx += protos[idx] + 1) {
        if (protos[idx] == 0)
            return 0;
    }
    return idx == protos_len;
}
/*
 * SSL_CTX_set_alpn_protos sets the ALPN protocol list on |ctx| to |protos|.
 * |protos| must be in wire-format (i.e. a series of non-empty, 8-bit
 * length-prefixed strings). Returns 0 on success.
 */
int SSL_CTX_set_alpn_protos(SSL_CTX *ctx, const unsigned char *protos,
                            unsigned int protos_len)
{
    unsigned char *alpn;

    if (protos_len == 0 || protos == NULL) {
        OPENSSL_free(ctx->ext.alpn);
        ctx->ext.alpn = NULL;
        ctx->ext.alpn_len = 0;
        return 0;
    }
    /* Not valid per RFC */
    if (!alpn_value_ok(protos, protos_len))
        return 1;

    alpn = OPENSSL_memdup(protos, protos_len);
    if (alpn == NULL)
        return 1;
    OPENSSL_free(ctx->ext.alpn);
    ctx->ext.alpn = alpn;
    ctx->ext.alpn_len = protos_len;

    return 0;
}

/*
 * SSL_set_alpn_protos sets the ALPN protocol list on |ssl| to |protos|.
 * |protos| must be in wire-format (i.e. a series of non-empty, 8-bit
 * length-prefixed strings). Returns 0 on success.
 */
int SSL_set_alpn_protos(SSL *ssl, const unsigned char *protos,
                        unsigned int protos_len)
{
    unsigned char *alpn;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return 1;

    if (protos_len == 0 || protos == NULL) {
        OPENSSL_free(sc->ext.alpn);
        sc->ext.alpn = NULL;
        sc->ext.alpn_len = 0;
        return 0;
    }
    /* Not valid per RFC */
    if (!alpn_value_ok(protos, protos_len))
        return 1;

    alpn = OPENSSL_memdup(protos, protos_len);
    if (alpn == NULL)
        return 1;
    OPENSSL_free(sc->ext.alpn);
    sc->ext.alpn = alpn;
    sc->ext.alpn_len = protos_len;

    return 0;
}

/*
 * SSL_CTX_set_alpn_select_cb sets a callback function on |ctx| that is
 * called during ClientHello processing in order to select an ALPN protocol
 * from the client's list of offered protocols.
 */
void SSL_CTX_set_alpn_select_cb(SSL_CTX *ctx,
                                SSL_CTX_alpn_select_cb_func cb,
                                void *arg)
{
    ctx->ext.alpn_select_cb = cb;
    ctx->ext.alpn_select_cb_arg = arg;
}

/*
 * SSL_get0_alpn_selected gets the selected ALPN protocol (if any) from |ssl|.
 * On return it sets |*data| to point to |*len| bytes of protocol name
 * (not including the leading length-prefix byte). If the server didn't
 * respond with a negotiated protocol then |*len| will be zero.
 */
void SSL_get0_alpn_selected(const SSL *ssl, const unsigned char **data,
                            unsigned int *len)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(ssl);

    if (sc == NULL) {
        /* We have no other way to indicate error */
        *data = NULL;
        *len = 0;
        return;
    }

    *data = sc->s3.alpn_selected;
    if (*data == NULL)
        *len = 0;
    else
        *len = (unsigned int)sc->s3.alpn_selected_len;
}

int SSL_export_keying_material(SSL *s, unsigned char *out, size_t olen,
                               const char *label, size_t llen,
                               const unsigned char *context, size_t contextlen,
                               int use_context)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return -1;

    if (sc->session == NULL
        || (sc->version < TLS1_VERSION && sc->version != DTLS1_BAD_VER))
        return -1;

    return sc->ssl.method->ssl3_enc->export_keying_material(sc, out, olen, label,
                                                            llen, context,
                                                            contextlen,
                                                            use_context);
}

int SSL_export_keying_material_early(SSL *s, unsigned char *out, size_t olen,
                                     const char *label, size_t llen,
                                     const unsigned char *context,
                                     size_t contextlen)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return -1;

    if (sc->version != TLS1_3_VERSION)
        return 0;

    return tls13_export_keying_material_early(sc, out, olen, label, llen,
                                              context, contextlen);
}

static unsigned long ssl_session_hash(const SSL_SESSION *a)
{
    const unsigned char *session_id = a->session_id;
    unsigned long l;
    unsigned char tmp_storage[4];

    if (a->session_id_length < sizeof(tmp_storage)) {
        memset(tmp_storage, 0, sizeof(tmp_storage));
        memcpy(tmp_storage, a->session_id, a->session_id_length);
        session_id = tmp_storage;
    }

    l = (unsigned long)
        ((unsigned long)session_id[0]) |
        ((unsigned long)session_id[1] << 8L) |
        ((unsigned long)session_id[2] << 16L) |
        ((unsigned long)session_id[3] << 24L);
    return l;
}

/*
 * NB: If this function (or indeed the hash function which uses a sort of
 * coarser function than this one) is changed, ensure
 * SSL_CTX_has_matching_session_id() is checked accordingly. It relies on
 * being able to construct an SSL_SESSION that will collide with any existing
 * session with a matching session ID.
 */
static int ssl_session_cmp(const SSL_SESSION *a, const SSL_SESSION *b)
{
    if (a->ssl_version != b->ssl_version)
        return 1;
    if (a->session_id_length != b->session_id_length)
        return 1;
    return memcmp(a->session_id, b->session_id, a->session_id_length);
}

#ifndef OPENSSL_NO_SSLKEYLOG
/**
 * @brief Static initialization for a one-time action to initialize the SSL key log.
 */
static CRYPTO_ONCE ssl_keylog_once = CRYPTO_ONCE_STATIC_INIT;

/**
 * @brief Pointer to a read-write lock used to protect access to the key log.
 */
static CRYPTO_RWLOCK *keylog_lock = NULL;

/**
 * @brief Pointer to a BIO structure used for writing the key log information.
 */
static BIO *keylog_bio = NULL;

/**
 * @brief Initializes the SSLKEYLOGFILE lock.
 *
 * @return 1 on success, 0 on failure.
 */
DEFINE_RUN_ONCE_STATIC(ssl_keylog_init)
{
    keylog_lock = CRYPTO_THREAD_lock_new();
    if (keylog_lock == NULL)
        return 0;
    return 1;
}

/**
 * @brief checks when a BIO refcount has reached zero, and sets
 * keylog_cb to NULL if it has
 *
 * @returns 1 always
 */
static long check_keylog_bio_free(BIO *b, int oper, const char *argp,
                                  size_t len, int argi, long argl, int ret,
                                  size_t *processed)
{

    /*
     * Note we _dont_ take the keylog_lock here
     * This is intentional, because we only free the keylog lock
     * During SSL_CTX_free, in which we already posess the lock, so
     * Theres no need to grab it again here
     */
    if (oper == BIO_CB_FREE)
        keylog_bio = NULL;
    return ret;
}

/**
 * @brief records ssl secrets to a file
 */
static void do_sslkeylogfile(const SSL *ssl, const char *line)
{
    if (keylog_lock == NULL)
        return;

    if (!CRYPTO_THREAD_write_lock(keylog_lock))
        return;
    if (keylog_bio != NULL) {
        BIO_printf(keylog_bio, "%s\n", line);
        (void)BIO_flush(keylog_bio);
    }
    CRYPTO_THREAD_unlock(keylog_lock);
}
#endif

/*
 * These wrapper functions should remain rather than redeclaring
 * SSL_SESSION_hash and SSL_SESSION_cmp for void* types and casting each
 * variable. The reason is that the functions aren't static, they're exposed
 * via ssl.h.
 */

#ifndef OPENSSL_NO_SSLKEYLOG
static BIO *get_sslkeylog_bio(const char *keylogfile)
{
# ifdef _POSIX_C_SOURCE
    BIO *b;
    int fdno = -1;
    FILE *fp = NULL;

    fdno = open(keylogfile, O_WRONLY | O_CREAT | O_APPEND,  0600);
    if (fdno < 0)
        return NULL;

    fp = fdopen(fdno, "a");
    if (fp == NULL) {
        close(fdno);
        return NULL;
    }

    if ((b = BIO_new_fp(fp, BIO_CLOSE)) == NULL)
        fclose(fp);
    return b;
# else
    return BIO_new_file(keylogfile, "a");
# endif
}
#endif

SSL_CTX *SSL_CTX_new_ex(OSSL_LIB_CTX *libctx, const char *propq,
                        const SSL_METHOD *meth)
{
    SSL_CTX *ret = NULL;
#ifndef OPENSSL_NO_SSLKEYLOG
    const char *keylogfile = ossl_safe_getenv("SSLKEYLOGFILE");
#endif
#ifndef OPENSSL_NO_COMP_ALG
    int i;
#endif

    if (meth == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NULL_SSL_METHOD_PASSED);
        return NULL;
    }

    if (!OPENSSL_init_ssl(OPENSSL_INIT_LOAD_SSL_STRINGS, NULL))
        return NULL;

    /* Doing this for the run once effect */
    if (SSL_get_ex_data_X509_STORE_CTX_idx() < 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_X509_VERIFICATION_SETUP_PROBLEMS);
        goto err;
    }

    ret = OPENSSL_zalloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;

    /* Init the reference counting before any call to SSL_CTX_free */
    if (!CRYPTO_NEW_REF(&ret->references, 1)) {
        OPENSSL_free(ret);
        return NULL;
    }

    ret->lock = CRYPTO_THREAD_lock_new();
    if (ret->lock == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

#ifdef TSAN_REQUIRES_LOCKING
    ret->tsan_lock = CRYPTO_THREAD_lock_new();
    if (ret->tsan_lock == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }
#endif

    ret->libctx = libctx;
    if (propq != NULL) {
        ret->propq = OPENSSL_strdup(propq);
        if (ret->propq == NULL)
            goto err;
    }

    ret->method = meth;
    ret->min_proto_version = 0;
    ret->max_proto_version = 0;
    ret->mode = SSL_MODE_AUTO_RETRY;
    ret->session_cache_mode = SSL_SESS_CACHE_SERVER;
    ret->session_cache_size = SSL_SESSION_CACHE_MAX_SIZE_DEFAULT;
    /* We take the system default. */
    ret->session_timeout = meth->get_timeout();
    ret->max_cert_list = SSL_MAX_CERT_LIST_DEFAULT;
    ret->verify_mode = SSL_VERIFY_NONE;

    ret->sessions = lh_SSL_SESSION_new(ssl_session_hash, ssl_session_cmp);
    if (ret->sessions == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }
    ret->cert_store = X509_STORE_new();
    if (ret->cert_store == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
        goto err;
    }
#ifndef OPENSSL_NO_CT
    ret->ctlog_store = CTLOG_STORE_new_ex(libctx, propq);
    if (ret->ctlog_store == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CT_LIB);
        goto err;
    }
#endif

    /* initialize cipher/digest methods table */
    if (!ssl_load_ciphers(ret)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SSL_LIB);
        goto err;
    }

    if (!ssl_load_groups(ret)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SSL_LIB);
        goto err;
    }

    /* load provider sigalgs */
    if (!ssl_load_sigalgs(ret)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SSL_LIB);
        goto err;
    }

    /* initialise sig algs */
    if (!ssl_setup_sigalgs(ret)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SSL_LIB);
        goto err;
    }

    if (!SSL_CTX_set_ciphersuites(ret, OSSL_default_ciphersuites())) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SSL_LIB);
        goto err;
    }

    if ((ret->cert = ssl_cert_new(SSL_PKEY_NUM + ret->sigalg_list_len)) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SSL_LIB);
        goto err;
    }

    if (!ssl_create_cipher_list(ret,
                                ret->tls13_ciphersuites,
                                &ret->cipher_list, &ret->cipher_list_by_id,
                                OSSL_default_cipher_list(), ret->cert)
        || sk_SSL_CIPHER_num(ret->cipher_list) <= 0) {
        ERR_raise(ERR_LIB_SSL, SSL_R_LIBRARY_HAS_NO_CIPHERS);
        goto err;
    }

    ret->param = X509_VERIFY_PARAM_new();
    if (ret->param == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
        goto err;
    }

    /*
     * If these aren't available from the provider we'll get NULL returns.
     * That's fine but will cause errors later if SSLv3 is negotiated
     */
    ret->md5 = ssl_evp_md_fetch(libctx, NID_md5, propq);
    ret->sha1 = ssl_evp_md_fetch(libctx, NID_sha1, propq);

    if ((ret->ca_names = sk_X509_NAME_new_null()) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if ((ret->client_ca_names = sk_X509_NAME_new_null()) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if (!CRYPTO_new_ex_data(CRYPTO_EX_INDEX_SSL_CTX, ret, &ret->ex_data)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if ((ret->ext.secure = OPENSSL_secure_zalloc(sizeof(*ret->ext.secure))) == NULL)
        goto err;

    /* No compression for DTLS */
    if (!(meth->ssl3_enc->enc_flags & SSL_ENC_FLAG_DTLS))
        ret->comp_methods = SSL_COMP_get_compression_methods();

    ret->max_send_fragment = SSL3_RT_MAX_PLAIN_LENGTH;
    ret->split_send_fragment = SSL3_RT_MAX_PLAIN_LENGTH;

    /* Setup RFC5077 ticket keys */
    if ((RAND_bytes_ex(libctx, ret->ext.tick_key_name,
                       sizeof(ret->ext.tick_key_name), 0) <= 0)
        || (RAND_priv_bytes_ex(libctx, ret->ext.secure->tick_hmac_key,
                               sizeof(ret->ext.secure->tick_hmac_key), 0) <= 0)
        || (RAND_priv_bytes_ex(libctx, ret->ext.secure->tick_aes_key,
                               sizeof(ret->ext.secure->tick_aes_key), 0) <= 0))
        ret->options |= SSL_OP_NO_TICKET;

    if (RAND_priv_bytes_ex(libctx, ret->ext.cookie_hmac_key,
                           sizeof(ret->ext.cookie_hmac_key), 0) <= 0) {
        ERR_raise(ERR_LIB_SSL, ERR_R_RAND_LIB);
        goto err;
    }

#ifndef OPENSSL_NO_SRP
    if (!ssl_ctx_srp_ctx_init_intern(ret)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SSL_LIB);
        goto err;
    }
#endif
#ifndef OPENSSL_NO_ENGINE
# ifdef OPENSSL_SSL_CLIENT_ENGINE_AUTO
#  define eng_strx(x)     #x
#  define eng_str(x)      eng_strx(x)
    /* Use specific client engine automatically... ignore errors */
    {
        ENGINE *eng;
        eng = ENGINE_by_id(eng_str(OPENSSL_SSL_CLIENT_ENGINE_AUTO));
        if (!eng) {
            ERR_clear_error();
            ENGINE_load_builtin_engines();
            eng = ENGINE_by_id(eng_str(OPENSSL_SSL_CLIENT_ENGINE_AUTO));
        }
        if (!eng || !SSL_CTX_set_client_cert_engine(ret, eng))
            ERR_clear_error();
    }
# endif
#endif

#ifndef OPENSSL_NO_COMP_ALG
    /*
     * Set the default order: brotli, zlib, zstd
     * Including only those enabled algorithms
     */
    memset(ret->cert_comp_prefs, 0, sizeof(ret->cert_comp_prefs));
    i = 0;
    if (ossl_comp_has_alg(TLSEXT_comp_cert_brotli))
        ret->cert_comp_prefs[i++] = TLSEXT_comp_cert_brotli;
    if (ossl_comp_has_alg(TLSEXT_comp_cert_zlib))
        ret->cert_comp_prefs[i++] = TLSEXT_comp_cert_zlib;
    if (ossl_comp_has_alg(TLSEXT_comp_cert_zstd))
        ret->cert_comp_prefs[i++] = TLSEXT_comp_cert_zstd;
#endif
    /*
     * Disable compression by default to prevent CRIME. Applications can
     * re-enable compression by configuring
     * SSL_CTX_clear_options(ctx, SSL_OP_NO_COMPRESSION);
     * or by using the SSL_CONF library. Similarly we also enable TLSv1.3
     * middlebox compatibility by default. This may be disabled by default in
     * a later OpenSSL version.
     */
    ret->options |= SSL_OP_NO_COMPRESSION | SSL_OP_ENABLE_MIDDLEBOX_COMPAT;

    ret->ext.status_type = TLSEXT_STATUSTYPE_nothing;

    /*
     * We cannot usefully set a default max_early_data here (which gets
     * propagated in SSL_new(), for the following reason: setting the
     * SSL field causes tls_construct_stoc_early_data() to tell the
     * client that early data will be accepted when constructing a TLS 1.3
     * session ticket, and the client will accordingly send us early data
     * when using that ticket (if the client has early data to send).
     * However, in order for the early data to actually be consumed by
     * the application, the application must also have calls to
     * SSL_read_early_data(); otherwise we'll just skip past the early data
     * and ignore it.  So, since the application must add calls to
     * SSL_read_early_data(), we also require them to add
     * calls to SSL_CTX_set_max_early_data() in order to use early data,
     * eliminating the bandwidth-wasting early data in the case described
     * above.
     */
    ret->max_early_data = 0;

    /*
     * Default recv_max_early_data is a fully loaded single record. Could be
     * split across multiple records in practice. We set this differently to
     * max_early_data so that, in the default case, we do not advertise any
     * support for early_data, but if a client were to send us some (e.g.
     * because of an old, stale ticket) then we will tolerate it and skip over
     * it.
     */
    ret->recv_max_early_data = SSL3_RT_MAX_PLAIN_LENGTH;

    /* By default we send two session tickets automatically in TLSv1.3 */
    ret->num_tickets = 2;

# ifndef OPENSSL_NO_QUIC
    /* only create a cache for client CTX-es */
    if (meth == OSSL_QUIC_client_method())
        if ((ret->tokencache = ossl_quic_new_token_store()) == NULL)
            goto err;
    ret->domain_flags = 0;
    if (IS_QUIC_METHOD(meth)) {
#  if defined(OPENSSL_THREADS)
        if (meth == OSSL_QUIC_client_thread_method())
            ret->domain_flags
                = SSL_DOMAIN_FLAG_MULTI_THREAD
                | SSL_DOMAIN_FLAG_THREAD_ASSISTED
                | SSL_DOMAIN_FLAG_BLOCKING;
        else
            ret->domain_flags
                = SSL_DOMAIN_FLAG_MULTI_THREAD
                | SSL_DOMAIN_FLAG_LEGACY_BLOCKING;
#  else
        ret->domain_flags
            = SSL_DOMAIN_FLAG_SINGLE_THREAD
            | SSL_DOMAIN_FLAG_LEGACY_BLOCKING;
#  endif
    }
# endif

    if (!ssl_ctx_system_config(ret)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_ERROR_IN_SYSTEM_DEFAULT_CONFIG);
        goto err;
    }

#ifndef OPENSSL_NO_SSLKEYLOG
    if (keylogfile != NULL && strlen(keylogfile) != 0) {
        /* Make sure we have a global lock allocated */
        if (!RUN_ONCE(&ssl_keylog_once, ssl_keylog_init)) {
            /* use a trace message as a warning */
            OSSL_TRACE(TLS, "Unable to initalize keylog data\n");
            goto out;
        }

        /* Grab our global lock */
        if (!CRYPTO_THREAD_write_lock(keylog_lock)) {
            OSSL_TRACE(TLS, "Unable to acquire keylog write lock\n");
            goto out;
        } else {
            /*
             * If the bio for the requested keylog file hasn't been
             * created yet, go ahead and create it, and set it to append
             * if its already there.
             */
            if (keylog_bio == NULL) {
                keylog_bio = get_sslkeylog_bio(keylogfile);
                if (keylog_bio == NULL) {
                    OSSL_TRACE(TLS, "Unable to create keylog bio\n");
                    goto out;
                }
                BIO_set_callback_ex(keylog_bio, check_keylog_bio_free);
            } else {
                /* up our refcount for the already-created case */
                BIO_up_ref(keylog_bio);
            }
            /* If we have a bio now, assign the callback handler */
            if (keylog_bio != NULL)
                ret->do_sslkeylog = 1;
            /* unlock, and we're done */
            CRYPTO_THREAD_unlock(keylog_lock);
        }
    }
out:
#endif
    return ret;
 err:
    SSL_CTX_free(ret);
#ifndef OPENSSL_NO_SSLKEYLOG
    BIO_free(keylog_bio);
#endif
    return NULL;
}

SSL_CTX *SSL_CTX_new(const SSL_METHOD *meth)
{
    return SSL_CTX_new_ex(NULL, NULL, meth);
}

int SSL_CTX_up_ref(SSL_CTX *ctx)
{
    int i;

    if (CRYPTO_UP_REF(&ctx->references, &i) <= 0)
        return 0;

    REF_PRINT_COUNT("SSL_CTX", i, ctx);
    REF_ASSERT_ISNT(i < 2);
    return ((i > 1) ? 1 : 0);
}

void SSL_CTX_free(SSL_CTX *a)
{
    int i;
    size_t j;

    if (a == NULL)
        return;

    CRYPTO_DOWN_REF(&a->references, &i);
    REF_PRINT_COUNT("SSL_CTX", i, a);
    if (i > 0)
        return;
    REF_ASSERT_ISNT(i < 0);

#ifndef OPENSSL_NO_SSLKEYLOG
    if (keylog_lock != NULL && CRYPTO_THREAD_write_lock(keylog_lock)) {
        if (a->do_sslkeylog == 1)
            BIO_free(keylog_bio);
        a->do_sslkeylog = 0;
        CRYPTO_THREAD_unlock(keylog_lock);
    }
#endif

    X509_VERIFY_PARAM_free(a->param);
    dane_ctx_final(&a->dane);

    /*
     * Free internal session cache. However: the remove_cb() may reference
     * the ex_data of SSL_CTX, thus the ex_data store can only be removed
     * after the sessions were flushed.
     * As the ex_data handling routines might also touch the session cache,
     * the most secure solution seems to be: empty (flush) the cache, then
     * free ex_data, then finally free the cache.
     * (See ticket [openssl.org #212].)
     */
    if (a->sessions != NULL)
        SSL_CTX_flush_sessions_ex(a, 0);

    CRYPTO_free_ex_data(CRYPTO_EX_INDEX_SSL_CTX, a, &a->ex_data);
    lh_SSL_SESSION_free(a->sessions);
    X509_STORE_free(a->cert_store);
#ifndef OPENSSL_NO_CT
    CTLOG_STORE_free(a->ctlog_store);
#endif
    sk_SSL_CIPHER_free(a->cipher_list);
    sk_SSL_CIPHER_free(a->cipher_list_by_id);
    sk_SSL_CIPHER_free(a->tls13_ciphersuites);
    ssl_cert_free(a->cert);
    sk_X509_NAME_pop_free(a->ca_names, X509_NAME_free);
    sk_X509_NAME_pop_free(a->client_ca_names, X509_NAME_free);
    OSSL_STACK_OF_X509_free(a->extra_certs);
    a->comp_methods = NULL;
#ifndef OPENSSL_NO_SRTP
    sk_SRTP_PROTECTION_PROFILE_free(a->srtp_profiles);
#endif
#ifndef OPENSSL_NO_SRP
    ssl_ctx_srp_ctx_free_intern(a);
#endif
#ifndef OPENSSL_NO_ENGINE
    tls_engine_finish(a->client_cert_engine);
#endif

    OPENSSL_free(a->ext.ecpointformats);
    OPENSSL_free(a->ext.supportedgroups);
    OPENSSL_free(a->ext.keyshares);
    OPENSSL_free(a->ext.tuples);
    OPENSSL_free(a->ext.alpn);
    OPENSSL_secure_free(a->ext.secure);

    ssl_evp_md_free(a->md5);
    ssl_evp_md_free(a->sha1);

    for (j = 0; j < SSL_ENC_NUM_IDX; j++)
        ssl_evp_cipher_free(a->ssl_cipher_methods[j]);
    for (j = 0; j < SSL_MD_NUM_IDX; j++)
        ssl_evp_md_free(a->ssl_digest_methods[j]);
    for (j = 0; j < a->group_list_len; j++) {
        OPENSSL_free(a->group_list[j].tlsname);
        OPENSSL_free(a->group_list[j].realname);
        OPENSSL_free(a->group_list[j].algorithm);
    }
    OPENSSL_free(a->group_list);
    for (j = 0; j < a->sigalg_list_len; j++) {
        OPENSSL_free(a->sigalg_list[j].name);
        OPENSSL_free(a->sigalg_list[j].sigalg_name);
        OPENSSL_free(a->sigalg_list[j].sigalg_oid);
        OPENSSL_free(a->sigalg_list[j].sig_name);
        OPENSSL_free(a->sigalg_list[j].sig_oid);
        OPENSSL_free(a->sigalg_list[j].hash_name);
        OPENSSL_free(a->sigalg_list[j].hash_oid);
        OPENSSL_free(a->sigalg_list[j].keytype);
        OPENSSL_free(a->sigalg_list[j].keytype_oid);
    }
    OPENSSL_free(a->sigalg_list);
    OPENSSL_free(a->ssl_cert_info);

    OPENSSL_free(a->sigalg_lookup_cache);
    OPENSSL_free(a->tls12_sigalgs);

    OPENSSL_free(a->client_cert_type);
    OPENSSL_free(a->server_cert_type);

    CRYPTO_THREAD_lock_free(a->lock);
    CRYPTO_FREE_REF(&a->references);
#ifdef TSAN_REQUIRES_LOCKING
    CRYPTO_THREAD_lock_free(a->tsan_lock);
#endif

    OPENSSL_free(a->propq);
#ifndef OPENSSL_NO_QLOG
    OPENSSL_free(a->qlog_title);
#endif

#ifndef OPENSSL_NO_QUIC
    ossl_quic_free_token_store(a->tokencache);
#endif

    OPENSSL_free(a);
}

void SSL_CTX_set_default_passwd_cb(SSL_CTX *ctx, pem_password_cb *cb)
{
    ctx->default_passwd_callback = cb;
}

void SSL_CTX_set_default_passwd_cb_userdata(SSL_CTX *ctx, void *u)
{
    ctx->default_passwd_callback_userdata = u;
}

pem_password_cb *SSL_CTX_get_default_passwd_cb(SSL_CTX *ctx)
{
    return ctx->default_passwd_callback;
}

void *SSL_CTX_get_default_passwd_cb_userdata(SSL_CTX *ctx)
{
    return ctx->default_passwd_callback_userdata;
}

void SSL_set_default_passwd_cb(SSL *s, pem_password_cb *cb)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->default_passwd_callback = cb;
}

void SSL_set_default_passwd_cb_userdata(SSL *s, void *u)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->default_passwd_callback_userdata = u;
}

pem_password_cb *SSL_get_default_passwd_cb(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    return sc->default_passwd_callback;
}

void *SSL_get_default_passwd_cb_userdata(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    return sc->default_passwd_callback_userdata;
}

void SSL_CTX_set_cert_verify_callback(SSL_CTX *ctx,
                                      int (*cb) (X509_STORE_CTX *, void *),
                                      void *arg)
{
    ctx->app_verify_callback = cb;
    ctx->app_verify_arg = arg;
}

void SSL_CTX_set_verify(SSL_CTX *ctx, int mode,
                        int (*cb) (int, X509_STORE_CTX *))
{
    ctx->verify_mode = mode;
    ctx->default_verify_callback = cb;
}

void SSL_CTX_set_verify_depth(SSL_CTX *ctx, int depth)
{
    X509_VERIFY_PARAM_set_depth(ctx->param, depth);
}

void SSL_CTX_set_cert_cb(SSL_CTX *c, int (*cb) (SSL *ssl, void *arg), void *arg)
{
    ssl_cert_set_cert_cb(c->cert, cb, arg);
}

void SSL_set_cert_cb(SSL *s, int (*cb) (SSL *ssl, void *arg), void *arg)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    ssl_cert_set_cert_cb(sc->cert, cb, arg);
}

void ssl_set_masks(SSL_CONNECTION *s)
{
    CERT *c = s->cert;
    uint32_t *pvalid = s->s3.tmp.valid_flags;
    int rsa_enc, rsa_sign, dh_tmp, dsa_sign;
    unsigned long mask_k, mask_a;
    int have_ecc_cert, ecdsa_ok;

    if (c == NULL)
        return;

    dh_tmp = (c->dh_tmp != NULL
              || c->dh_tmp_cb != NULL
              || c->dh_tmp_auto);

    rsa_enc = pvalid[SSL_PKEY_RSA] & CERT_PKEY_VALID;
    rsa_sign = pvalid[SSL_PKEY_RSA] & CERT_PKEY_VALID;
    dsa_sign = pvalid[SSL_PKEY_DSA_SIGN] & CERT_PKEY_VALID;
    have_ecc_cert = pvalid[SSL_PKEY_ECC] & CERT_PKEY_VALID;
    mask_k = 0;
    mask_a = 0;

    OSSL_TRACE4(TLS_CIPHER, "dh_tmp=%d rsa_enc=%d rsa_sign=%d dsa_sign=%d\n",
               dh_tmp, rsa_enc, rsa_sign, dsa_sign);

#ifndef OPENSSL_NO_GOST
    if (ssl_has_cert(s, SSL_PKEY_GOST12_512)) {
        mask_k |= SSL_kGOST | SSL_kGOST18;
        mask_a |= SSL_aGOST12;
    }
    if (ssl_has_cert(s, SSL_PKEY_GOST12_256)) {
        mask_k |= SSL_kGOST | SSL_kGOST18;
        mask_a |= SSL_aGOST12;
    }
    if (ssl_has_cert(s, SSL_PKEY_GOST01)) {
        mask_k |= SSL_kGOST;
        mask_a |= SSL_aGOST01;
    }
#endif

    if (rsa_enc)
        mask_k |= SSL_kRSA;

    if (dh_tmp)
        mask_k |= SSL_kDHE;

    /*
     * If we only have an RSA-PSS certificate allow RSA authentication
     * if TLS 1.2 and peer supports it.
     */

    if (rsa_enc || rsa_sign || (ssl_has_cert(s, SSL_PKEY_RSA_PSS_SIGN)
                && pvalid[SSL_PKEY_RSA_PSS_SIGN] & CERT_PKEY_EXPLICIT_SIGN
                && TLS1_get_version(&s->ssl) == TLS1_2_VERSION))
        mask_a |= SSL_aRSA;

    if (dsa_sign) {
        mask_a |= SSL_aDSS;
    }

    mask_a |= SSL_aNULL;

    /*
     * You can do anything with an RPK key, since there's no cert to restrict it
     * But we need to check for private keys
     */
    if (pvalid[SSL_PKEY_RSA] & CERT_PKEY_RPK) {
        mask_a |= SSL_aRSA;
        mask_k |= SSL_kRSA;
    }
    if (pvalid[SSL_PKEY_ECC] & CERT_PKEY_RPK)
        mask_a |= SSL_aECDSA;
    if (TLS1_get_version(&s->ssl) == TLS1_2_VERSION) {
        if (pvalid[SSL_PKEY_RSA_PSS_SIGN] & CERT_PKEY_RPK)
            mask_a |= SSL_aRSA;
        if (pvalid[SSL_PKEY_ED25519] & CERT_PKEY_RPK
                || pvalid[SSL_PKEY_ED448] & CERT_PKEY_RPK)
            mask_a |= SSL_aECDSA;
    }

    /*
     * An ECC certificate may be usable for ECDH and/or ECDSA cipher suites
     * depending on the key usage extension.
     */
    if (have_ecc_cert) {
        uint32_t ex_kusage;
        ex_kusage = X509_get_key_usage(c->pkeys[SSL_PKEY_ECC].x509);
        ecdsa_ok = ex_kusage & X509v3_KU_DIGITAL_SIGNATURE;
        if (!(pvalid[SSL_PKEY_ECC] & CERT_PKEY_SIGN))
            ecdsa_ok = 0;
        if (ecdsa_ok)
            mask_a |= SSL_aECDSA;
    }
    /* Allow Ed25519 for TLS 1.2 if peer supports it */
    if (!(mask_a & SSL_aECDSA) && ssl_has_cert(s, SSL_PKEY_ED25519)
            && pvalid[SSL_PKEY_ED25519] & CERT_PKEY_EXPLICIT_SIGN
            && TLS1_get_version(&s->ssl) == TLS1_2_VERSION)
            mask_a |= SSL_aECDSA;

    /* Allow Ed448 for TLS 1.2 if peer supports it */
    if (!(mask_a & SSL_aECDSA) && ssl_has_cert(s, SSL_PKEY_ED448)
            && pvalid[SSL_PKEY_ED448] & CERT_PKEY_EXPLICIT_SIGN
            && TLS1_get_version(&s->ssl) == TLS1_2_VERSION)
            mask_a |= SSL_aECDSA;

    mask_k |= SSL_kECDHE;

#ifndef OPENSSL_NO_PSK
    mask_k |= SSL_kPSK;
    mask_a |= SSL_aPSK;
    if (mask_k & SSL_kRSA)
        mask_k |= SSL_kRSAPSK;
    if (mask_k & SSL_kDHE)
        mask_k |= SSL_kDHEPSK;
    if (mask_k & SSL_kECDHE)
        mask_k |= SSL_kECDHEPSK;
#endif

    s->s3.tmp.mask_k = mask_k;
    s->s3.tmp.mask_a = mask_a;
}

int ssl_check_srvr_ecc_cert_and_alg(X509 *x, SSL_CONNECTION *s)
{
    if (s->s3.tmp.new_cipher->algorithm_auth & SSL_aECDSA) {
        /* key usage, if present, must allow signing */
        if (!(X509_get_key_usage(x) & X509v3_KU_DIGITAL_SIGNATURE)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_ECC_CERT_NOT_FOR_SIGNING);
            return 0;
        }
    }
    return 1;                   /* all checks are ok */
}

int ssl_get_server_cert_serverinfo(SSL_CONNECTION *s,
                                   const unsigned char **serverinfo,
                                   size_t *serverinfo_length)
{
    CERT_PKEY *cpk = s->s3.tmp.cert;
    *serverinfo_length = 0;

    if (cpk == NULL || cpk->serverinfo == NULL)
        return 0;

    *serverinfo = cpk->serverinfo;
    *serverinfo_length = cpk->serverinfo_length;
    return 1;
}

void ssl_update_cache(SSL_CONNECTION *s, int mode)
{
    int i;

    /*
     * If the session_id_length is 0, we are not supposed to cache it, and it
     * would be rather hard to do anyway :-). Also if the session has already
     * been marked as not_resumable we should not cache it for later reuse.
     */
    if (s->session->session_id_length == 0 || s->session->not_resumable)
        return;

    /*
     * If sid_ctx_length is 0 there is no specific application context
     * associated with this session, so when we try to resume it and
     * SSL_VERIFY_PEER is requested to verify the client identity, we have no
     * indication that this is actually a session for the proper application
     * context, and the *handshake* will fail, not just the resumption attempt.
     * Do not cache (on the server) these sessions that are not resumable
     * (clients can set SSL_VERIFY_PEER without needing a sid_ctx set).
     */
    if (s->server && s->session->sid_ctx_length == 0
            && (s->verify_mode & SSL_VERIFY_PEER) != 0)
        return;

    i = s->session_ctx->session_cache_mode;
    if ((i & mode) != 0
        && (!s->hit || SSL_CONNECTION_IS_TLS13(s))) {
        /*
         * Add the session to the internal cache. In server side TLSv1.3 we
         * normally don't do this because by default it's a full stateless ticket
         * with only a dummy session id so there is no reason to cache it,
         * unless:
         * - we are doing early_data, in which case we cache so that we can
         *   detect replays
         * - the application has set a remove_session_cb so needs to know about
         *   session timeout events
         * - SSL_OP_NO_TICKET is set in which case it is a stateful ticket
         */
        if ((i & SSL_SESS_CACHE_NO_INTERNAL_STORE) == 0
                && (!SSL_CONNECTION_IS_TLS13(s)
                    || !s->server
                    || (s->max_early_data > 0
                        && (s->options & SSL_OP_NO_ANTI_REPLAY) == 0)
                    || s->session_ctx->remove_session_cb != NULL
                    || (s->options & SSL_OP_NO_TICKET) != 0))
            SSL_CTX_add_session(s->session_ctx, s->session);

        /*
         * Add the session to the external cache. We do this even in server side
         * TLSv1.3 without early data because some applications just want to
         * know about the creation of a session and aren't doing a full cache.
         */
        if (s->session_ctx->new_session_cb != NULL && SSL_SESSION_up_ref(s->session)) {
            if (!s->session_ctx->new_session_cb(SSL_CONNECTION_GET_USER_SSL(s),
                                                s->session))
                SSL_SESSION_free(s->session);
        }
    }

    /* auto flush every 255 connections */
    if ((!(i & SSL_SESS_CACHE_NO_AUTO_CLEAR)) && ((i & mode) == mode)) {
        TSAN_QUALIFIER int *stat;

        if (mode & SSL_SESS_CACHE_CLIENT)
            stat = &s->session_ctx->stats.sess_connect_good;
        else
            stat = &s->session_ctx->stats.sess_accept_good;
        if ((ssl_tsan_load(s->session_ctx, stat) & 0xff) == 0xff)
            SSL_CTX_flush_sessions_ex(s->session_ctx, time(NULL));
    }
}

const SSL_METHOD *SSL_CTX_get_ssl_method(const SSL_CTX *ctx)
{
    return ctx->method;
}

const SSL_METHOD *SSL_get_ssl_method(const SSL *s)
{
    return s->method;
}

int SSL_set_ssl_method(SSL *s, const SSL_METHOD *meth)
{
    int ret = 1;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    /* Not allowed for QUIC */
    if (sc == NULL
        || (s->type != SSL_TYPE_SSL_CONNECTION && s->method != meth)
        || (s->type == SSL_TYPE_SSL_CONNECTION && IS_QUIC_METHOD(meth)))
        return 0;

    if (s->method != meth) {
        const SSL_METHOD *sm = s->method;
        int (*hf) (SSL *) = sc->handshake_func;

        if (sm->version == meth->version)
            s->method = meth;
        else {
            sm->ssl_deinit(s);
            s->method = meth;
            ret = s->method->ssl_init(s);
        }

        if (hf == sm->ssl_connect)
            sc->handshake_func = meth->ssl_connect;
        else if (hf == sm->ssl_accept)
            sc->handshake_func = meth->ssl_accept;
    }
    return ret;
}

int SSL_get_error(const SSL *s, int i)
{
    return ossl_ssl_get_error(s, i, /*check_err=*/1);
}

int ossl_ssl_get_error(const SSL *s, int i, int check_err)
{
    int reason;
    unsigned long l;
    BIO *bio;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (i > 0)
        return SSL_ERROR_NONE;

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s)) {
        reason = ossl_quic_get_error(s, i);
        if (reason != SSL_ERROR_NONE)
            return reason;
    }
#endif

    if (sc == NULL)
        return SSL_ERROR_SSL;

    /*
     * Make things return SSL_ERROR_SYSCALL when doing SSL_do_handshake etc,
     * where we do encode the error
     */
    if (check_err && (l = ERR_peek_error()) != 0) {
        if (ERR_GET_LIB(l) == ERR_LIB_SYS)
            return SSL_ERROR_SYSCALL;
        else
            return SSL_ERROR_SSL;
    }

#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
#endif
    {
        if (SSL_want_read(s)) {
            bio = SSL_get_rbio(s);
            if (BIO_should_read(bio))
                return SSL_ERROR_WANT_READ;
            else if (BIO_should_write(bio))
                /*
                 * This one doesn't make too much sense ... We never try to
                 * write to the rbio, and an application program where rbio and
                 * wbio are separate couldn't even know what it should wait for.
                 * However if we ever set s->rwstate incorrectly (so that we
                 * have SSL_want_read(s) instead of SSL_want_write(s)) and rbio
                 * and wbio *are* the same, this test works around that bug; so
                 * it might be safer to keep it.
                 */
                return SSL_ERROR_WANT_WRITE;
            else if (BIO_should_io_special(bio)) {
                reason = BIO_get_retry_reason(bio);
                if (reason == BIO_RR_CONNECT)
                    return SSL_ERROR_WANT_CONNECT;
                else if (reason == BIO_RR_ACCEPT)
                    return SSL_ERROR_WANT_ACCEPT;
                else
                    return SSL_ERROR_SYSCALL; /* unknown */
            }
        }

        if (SSL_want_write(s)) {
            /*
             * Access wbio directly - in order to use the buffered bio if
             * present
             */
            bio = sc->wbio;
            if (BIO_should_write(bio))
                return SSL_ERROR_WANT_WRITE;
            else if (BIO_should_read(bio))
                /*
                 * See above (SSL_want_read(s) with BIO_should_write(bio))
                 */
                return SSL_ERROR_WANT_READ;
            else if (BIO_should_io_special(bio)) {
                reason = BIO_get_retry_reason(bio);
                if (reason == BIO_RR_CONNECT)
                    return SSL_ERROR_WANT_CONNECT;
                else if (reason == BIO_RR_ACCEPT)
                    return SSL_ERROR_WANT_ACCEPT;
                else
                    return SSL_ERROR_SYSCALL;
            }
        }
    }

    if (SSL_want_x509_lookup(s))
        return SSL_ERROR_WANT_X509_LOOKUP;
    if (SSL_want_retry_verify(s))
        return SSL_ERROR_WANT_RETRY_VERIFY;
    if (SSL_want_async(s))
        return SSL_ERROR_WANT_ASYNC;
    if (SSL_want_async_job(s))
        return SSL_ERROR_WANT_ASYNC_JOB;
    if (SSL_want_client_hello_cb(s))
        return SSL_ERROR_WANT_CLIENT_HELLO_CB;

    if ((sc->shutdown & SSL_RECEIVED_SHUTDOWN) &&
        (sc->s3.warn_alert == SSL_AD_CLOSE_NOTIFY))
        return SSL_ERROR_ZERO_RETURN;

    return SSL_ERROR_SYSCALL;
}

static int ssl_do_handshake_intern(void *vargs)
{
    struct ssl_async_args *args = (struct ssl_async_args *)vargs;
    SSL *s = args->s;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return -1;

    return sc->handshake_func(s);
}

int SSL_do_handshake(SSL *s)
{
    int ret = 1;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_do_handshake(s);
#endif

    if (sc == NULL)
        return -1;

    if (sc->handshake_func == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_CONNECTION_TYPE_NOT_SET);
        return -1;
    }

    if (!ossl_statem_check_finish_init(sc, -1))
        return -1;

    s->method->ssl_renegotiate_check(s, 0);

    if (SSL_in_init(s) || SSL_in_before(s)) {
        if ((sc->mode & SSL_MODE_ASYNC) && ASYNC_get_current_job() == NULL) {
            struct ssl_async_args args;

            memset(&args, 0, sizeof(args));
            args.s = s;

            ret = ssl_start_async_job(s, &args, ssl_do_handshake_intern);
        } else {
            ret = sc->handshake_func(s);
        }
    }

    return ret;
}

void SSL_set_accept_state(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s)) {
        /* We suppress errors because this is a void function */
        (void)ossl_quic_set_accept_state(s, 0 /* suppress errors */);
        return;
    }
#endif

    sc->server = 1;
    sc->shutdown = 0;
    ossl_statem_clear(sc);
    sc->handshake_func = s->method->ssl_accept;
    /* Ignore return value. Its a void public API function */
    RECORD_LAYER_reset(&sc->rlayer);
}

void SSL_set_connect_state(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s)) {
        /* We suppress errors because this is a void function */
        (void)ossl_quic_set_connect_state(s, 0 /* suppress errors */);
        return;
    }
#endif

    sc->server = 0;
    sc->shutdown = 0;
    ossl_statem_clear(sc);
    sc->handshake_func = s->method->ssl_connect;
    /* Ignore return value. Its a void public API function */
    RECORD_LAYER_reset(&sc->rlayer);
}

int ssl_undefined_function(SSL *s)
{
    ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    return 0;
}

int ssl_undefined_void_function(void)
{
    ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    return 0;
}

const char *ssl_protocol_to_string(int version)
{
    switch (version) {
    case TLS1_3_VERSION:
        return "TLSv1.3";

    case TLS1_2_VERSION:
        return "TLSv1.2";

    case TLS1_1_VERSION:
        return "TLSv1.1";

    case TLS1_VERSION:
        return "TLSv1";

    case SSL3_VERSION:
        return "SSLv3";

    case DTLS1_BAD_VER:
        return "DTLSv0.9";

    case DTLS1_VERSION:
        return "DTLSv1";

    case DTLS1_2_VERSION:
        return "DTLSv1.2";

    default:
        return "unknown";
    }
}

const char *SSL_get_version(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

#ifndef OPENSSL_NO_QUIC
    /* We only support QUICv1 - so if its QUIC its QUICv1 */
    if (s->type == SSL_TYPE_QUIC_CONNECTION || s->type == SSL_TYPE_QUIC_XSO)
        return "QUICv1";
#endif

    if (sc == NULL)
        return NULL;

    return ssl_protocol_to_string(sc->version);
}

__owur int SSL_get_handshake_rtt(const SSL *s, uint64_t *rtt)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return -1;
    if (sc->ts_msg_write.t <= 0 || sc->ts_msg_read.t <= 0)
        return 0; /* data not (yet) available */
    if (sc->ts_msg_read.t < sc->ts_msg_write.t)
        return -1;

    *rtt = ossl_time2us(ossl_time_subtract(sc->ts_msg_read, sc->ts_msg_write));
    return 1;
}

static int dup_ca_names(STACK_OF(X509_NAME) **dst, STACK_OF(X509_NAME) *src)
{
    STACK_OF(X509_NAME) *sk;
    X509_NAME *xn;
    int i;

    if (src == NULL) {
        *dst = NULL;
        return 1;
    }

    if ((sk = sk_X509_NAME_new_null()) == NULL)
        return 0;
    for (i = 0; i < sk_X509_NAME_num(src); i++) {
        xn = X509_NAME_dup(sk_X509_NAME_value(src, i));
        if (xn == NULL) {
            sk_X509_NAME_pop_free(sk, X509_NAME_free);
            return 0;
        }
        if (sk_X509_NAME_insert(sk, xn, i) == 0) {
            X509_NAME_free(xn);
            sk_X509_NAME_pop_free(sk, X509_NAME_free);
            return 0;
        }
    }
    *dst = sk;

    return 1;
}

SSL *SSL_dup(SSL *s)
{
    SSL *ret;
    int i;
    /* TODO(QUIC FUTURE): Add an SSL_METHOD function for duplication */
    SSL_CONNECTION *retsc;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return NULL;

    /* If we're not quiescent, just up_ref! */
    if (!SSL_in_init(s) || !SSL_in_before(s)) {
        CRYPTO_UP_REF(&s->references, &i);
        return s;
    }

    /*
     * Otherwise, copy configuration state, and session if set.
     */
    if ((ret = SSL_new(SSL_get_SSL_CTX(s))) == NULL)
        return NULL;
    if ((retsc = SSL_CONNECTION_FROM_SSL_ONLY(ret)) == NULL)
        goto err;

    if (sc->session != NULL) {
        /*
         * Arranges to share the same session via up_ref.  This "copies"
         * session-id, SSL_METHOD, sid_ctx, and 'cert'
         */
        if (!SSL_copy_session_id(ret, s))
            goto err;
    } else {
        /*
         * No session has been established yet, so we have to expect that
         * s->cert or ret->cert will be changed later -- they should not both
         * point to the same object, and thus we can't use
         * SSL_copy_session_id.
         */
        if (!SSL_set_ssl_method(ret, s->method))
            goto err;

        if (sc->cert != NULL) {
            ssl_cert_free(retsc->cert);
            retsc->cert = ssl_cert_dup(sc->cert);
            if (retsc->cert == NULL)
                goto err;
        }

        if (!SSL_set_session_id_context(ret, sc->sid_ctx,
                                        (int)sc->sid_ctx_length))
            goto err;
    }

    if (!ssl_dane_dup(retsc, sc))
        goto err;
    retsc->version = sc->version;
    retsc->options = sc->options;
    retsc->min_proto_version = sc->min_proto_version;
    retsc->max_proto_version = sc->max_proto_version;
    retsc->mode = sc->mode;
    SSL_set_max_cert_list(ret, SSL_get_max_cert_list(s));
    SSL_set_read_ahead(ret, SSL_get_read_ahead(s));
    retsc->msg_callback = sc->msg_callback;
    retsc->msg_callback_arg = sc->msg_callback_arg;
    SSL_set_verify(ret, SSL_get_verify_mode(s), SSL_get_verify_callback(s));
    SSL_set_verify_depth(ret, SSL_get_verify_depth(s));
    retsc->generate_session_id = sc->generate_session_id;

    SSL_set_info_callback(ret, SSL_get_info_callback(s));

    /* copy app data, a little dangerous perhaps */
    if (!CRYPTO_dup_ex_data(CRYPTO_EX_INDEX_SSL, &ret->ex_data, &s->ex_data))
        goto err;

    retsc->server = sc->server;
    if (sc->handshake_func) {
        if (sc->server)
            SSL_set_accept_state(ret);
        else
            SSL_set_connect_state(ret);
    }
    retsc->shutdown = sc->shutdown;
    retsc->hit = sc->hit;

    retsc->default_passwd_callback = sc->default_passwd_callback;
    retsc->default_passwd_callback_userdata = sc->default_passwd_callback_userdata;

    X509_VERIFY_PARAM_inherit(retsc->param, sc->param);

    /* dup the cipher_list and cipher_list_by_id stacks */
    if (sc->cipher_list != NULL) {
        if ((retsc->cipher_list = sk_SSL_CIPHER_dup(sc->cipher_list)) == NULL)
            goto err;
    }
    if (sc->cipher_list_by_id != NULL)
        if ((retsc->cipher_list_by_id = sk_SSL_CIPHER_dup(sc->cipher_list_by_id))
            == NULL)
            goto err;

    /* Dup the client_CA list */
    if (!dup_ca_names(&retsc->ca_names, sc->ca_names)
            || !dup_ca_names(&retsc->client_ca_names, sc->client_ca_names))
        goto err;

    return ret;

 err:
    SSL_free(ret);
    return NULL;
}

X509 *SSL_get_certificate(const SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    if (sc->cert != NULL)
        return sc->cert->key->x509;
    else
        return NULL;
}

EVP_PKEY *SSL_get_privatekey(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    if (sc->cert != NULL)
        return sc->cert->key->privatekey;
    else
        return NULL;
}

X509 *SSL_CTX_get0_certificate(const SSL_CTX *ctx)
{
    if (ctx->cert != NULL)
        return ctx->cert->key->x509;
    else
        return NULL;
}

EVP_PKEY *SSL_CTX_get0_privatekey(const SSL_CTX *ctx)
{
    if (ctx->cert != NULL)
        return ctx->cert->key->privatekey;
    else
        return NULL;
}

const SSL_CIPHER *SSL_get_current_cipher(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    if ((sc->session != NULL) && (sc->session->cipher != NULL))
        return sc->session->cipher;
    return NULL;
}

const SSL_CIPHER *SSL_get_pending_cipher(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    return sc->s3.tmp.new_cipher;
}

const COMP_METHOD *SSL_get_current_compression(const SSL *s)
{
#ifndef OPENSSL_NO_COMP
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL_ONLY(s);

    if (sc == NULL)
        return NULL;

    return sc->rlayer.wrlmethod->get_compression(sc->rlayer.wrl);
#else
    return NULL;
#endif
}

const COMP_METHOD *SSL_get_current_expansion(const SSL *s)
{
#ifndef OPENSSL_NO_COMP
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL_ONLY(s);

    if (sc == NULL)
        return NULL;

    return sc->rlayer.rrlmethod->get_compression(sc->rlayer.rrl);
#else
    return NULL;
#endif
}

int ssl_init_wbio_buffer(SSL_CONNECTION *s)
{
    BIO *bbio;

    if (s->bbio != NULL) {
        /* Already buffered. */
        return 1;
    }

    bbio = BIO_new(BIO_f_buffer());
    if (bbio == NULL || BIO_set_read_buffer_size(bbio, 1) <= 0) {
        BIO_free(bbio);
        ERR_raise(ERR_LIB_SSL, ERR_R_BUF_LIB);
        return 0;
    }
    s->bbio = bbio;
    s->wbio = BIO_push(bbio, s->wbio);

    s->rlayer.wrlmethod->set1_bio(s->rlayer.wrl, s->wbio);

    return 1;
}

int ssl_free_wbio_buffer(SSL_CONNECTION *s)
{
    /* callers ensure s is never null */
    if (s->bbio == NULL)
        return 1;

    s->wbio = BIO_pop(s->wbio);
    s->rlayer.wrlmethod->set1_bio(s->rlayer.wrl, s->wbio);

    BIO_free(s->bbio);
    s->bbio = NULL;

    return 1;
}

void SSL_CTX_set_quiet_shutdown(SSL_CTX *ctx, int mode)
{
    ctx->quiet_shutdown = mode;
}

int SSL_CTX_get_quiet_shutdown(const SSL_CTX *ctx)
{
    return ctx->quiet_shutdown;
}

void SSL_set_quiet_shutdown(SSL *s, int mode)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    /* Not supported with QUIC */
    if (sc == NULL)
        return;

    sc->quiet_shutdown = mode;
}

int SSL_get_quiet_shutdown(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL_ONLY(s);

    /* Not supported with QUIC */
    if (sc == NULL)
        return 0;

    return sc->quiet_shutdown;
}

void SSL_set_shutdown(SSL *s, int mode)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    /* Not supported with QUIC */
    if (sc == NULL)
        return;

    sc->shutdown = mode;
}

int SSL_get_shutdown(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL_ONLY(s);

#ifndef OPENSSL_NO_QUIC
    /* QUIC: Just indicate whether the connection was shutdown cleanly. */
    if (IS_QUIC(s))
        return ossl_quic_get_shutdown(s);
#endif

    if (sc == NULL)
        return 0;

    return sc->shutdown;
}

int SSL_version(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

#ifndef OPENSSL_NO_QUIC
    /* We only support QUICv1 - so if its QUIC its QUICv1 */
    if (s->type == SSL_TYPE_QUIC_CONNECTION || s->type == SSL_TYPE_QUIC_XSO)
        return OSSL_QUIC1_VERSION;
#endif
    if (sc == NULL)
        return 0;

    return sc->version;
}

int SSL_client_version(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

#ifndef OPENSSL_NO_QUIC
    /* We only support QUICv1 - so if its QUIC its QUICv1 */
    if (s->type == SSL_TYPE_QUIC_CONNECTION || s->type == SSL_TYPE_QUIC_XSO)
        return OSSL_QUIC1_VERSION;
#endif
    if (sc == NULL)
        return 0;

    return sc->client_version;
}

SSL_CTX *SSL_get_SSL_CTX(const SSL *ssl)
{
    return ssl->ctx;
}

SSL_CTX *SSL_set_SSL_CTX(SSL *ssl, SSL_CTX *ctx)
{
    CERT *new_cert;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    /* TODO(QUIC FUTURE): Add support for QUIC */
    if (sc == NULL)
        return NULL;

    if (ssl->ctx == ctx)
        return ssl->ctx;
    if (ctx == NULL)
        ctx = sc->session_ctx;
    new_cert = ssl_cert_dup(ctx->cert);
    if (new_cert == NULL)
        goto err;
    if (!custom_exts_copy_conn(&new_cert->custext, &sc->cert->custext))
        goto err;
    if (!custom_exts_copy_flags(&new_cert->custext, &sc->cert->custext))
        goto err;

    /*
     * Program invariant: |sid_ctx| has fixed size (SSL_MAX_SID_CTX_LENGTH),
     * so setter APIs must prevent invalid lengths from entering the system.
     */
    if (!ossl_assert(sc->sid_ctx_length <= sizeof(sc->sid_ctx)))
        goto err;
    if (!SSL_CTX_up_ref(ctx))
        goto err;

    /*
     * If the session ID context matches that of the parent SSL_CTX,
     * inherit it from the new SSL_CTX as well. If however the context does
     * not match (i.e., it was set per-ssl with SSL_set_session_id_context),
     * leave it unchanged.
     */
    if ((ssl->ctx != NULL) &&
        (sc->sid_ctx_length == ssl->ctx->sid_ctx_length) &&
        (memcmp(sc->sid_ctx, ssl->ctx->sid_ctx, sc->sid_ctx_length) == 0)) {
        sc->sid_ctx_length = ctx->sid_ctx_length;
        memcpy(&sc->sid_ctx, &ctx->sid_ctx, sizeof(sc->sid_ctx));
    }

    ssl_cert_free(sc->cert);
    sc->cert = new_cert;
    SSL_CTX_free(ssl->ctx);     /* decrement reference count */
    ssl->ctx = ctx;

    return ssl->ctx;

err:
    ssl_cert_free(new_cert);
    return NULL;
}

int SSL_CTX_set_default_verify_paths(SSL_CTX *ctx)
{
    return X509_STORE_set_default_paths_ex(ctx->cert_store, ctx->libctx,
                                           ctx->propq);
}

int SSL_CTX_set_default_verify_dir(SSL_CTX *ctx)
{
    X509_LOOKUP *lookup;

    lookup = X509_STORE_add_lookup(ctx->cert_store, X509_LOOKUP_hash_dir());
    if (lookup == NULL)
        return 0;

    /* We ignore errors, in case the directory doesn't exist */
    ERR_set_mark();

    X509_LOOKUP_add_dir(lookup, NULL, X509_FILETYPE_DEFAULT);

    ERR_pop_to_mark();

    return 1;
}

int SSL_CTX_set_default_verify_file(SSL_CTX *ctx)
{
    X509_LOOKUP *lookup;

    lookup = X509_STORE_add_lookup(ctx->cert_store, X509_LOOKUP_file());
    if (lookup == NULL)
        return 0;

    /* We ignore errors, in case the file doesn't exist */
    ERR_set_mark();

    X509_LOOKUP_load_file_ex(lookup, NULL, X509_FILETYPE_DEFAULT, ctx->libctx,
                             ctx->propq);

    ERR_pop_to_mark();

    return 1;
}

int SSL_CTX_set_default_verify_store(SSL_CTX *ctx)
{
    X509_LOOKUP *lookup;

    lookup = X509_STORE_add_lookup(ctx->cert_store, X509_LOOKUP_store());
    if (lookup == NULL)
        return 0;

    /* We ignore errors, in case the directory doesn't exist */
    ERR_set_mark();

    X509_LOOKUP_add_store_ex(lookup, NULL, ctx->libctx, ctx->propq);

    ERR_pop_to_mark();

    return 1;
}

int SSL_CTX_load_verify_file(SSL_CTX *ctx, const char *CAfile)
{
    return X509_STORE_load_file_ex(ctx->cert_store, CAfile, ctx->libctx,
                                   ctx->propq);
}

int SSL_CTX_load_verify_dir(SSL_CTX *ctx, const char *CApath)
{
    return X509_STORE_load_path(ctx->cert_store, CApath);
}

int SSL_CTX_load_verify_store(SSL_CTX *ctx, const char *CAstore)
{
    return X509_STORE_load_store_ex(ctx->cert_store, CAstore, ctx->libctx,
                                    ctx->propq);
}

int SSL_CTX_load_verify_locations(SSL_CTX *ctx, const char *CAfile,
                                  const char *CApath)
{
    if (CAfile == NULL && CApath == NULL)
        return 0;
    if (CAfile != NULL && !SSL_CTX_load_verify_file(ctx, CAfile))
        return 0;
    if (CApath != NULL && !SSL_CTX_load_verify_dir(ctx, CApath))
        return 0;
    return 1;
}

void SSL_set_info_callback(SSL *ssl,
                           void (*cb) (const SSL *ssl, int type, int val))
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return;

    sc->info_callback = cb;
}

/*
 * One compiler (Diab DCC) doesn't like argument names in returned function
 * pointer.
 */
void (*SSL_get_info_callback(const SSL *ssl)) (const SSL * /* ssl */ ,
                                               int /* type */ ,
                                               int /* val */ ) {
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(ssl);

    if (sc == NULL)
        return NULL;

    return sc->info_callback;
}

void SSL_set_verify_result(SSL *ssl, long arg)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return;

    sc->verify_result = arg;
}

long SSL_get_verify_result(const SSL *ssl)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(ssl);

    if (sc == NULL)
        return 0;

    return sc->verify_result;
}

size_t SSL_get_client_random(const SSL *ssl, unsigned char *out, size_t outlen)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(ssl);

    if (sc == NULL)
        return 0;

    if (outlen == 0)
        return sizeof(sc->s3.client_random);
    if (outlen > sizeof(sc->s3.client_random))
        outlen = sizeof(sc->s3.client_random);
    memcpy(out, sc->s3.client_random, outlen);
    return outlen;
}

size_t SSL_get_server_random(const SSL *ssl, unsigned char *out, size_t outlen)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(ssl);

    if (sc == NULL)
        return 0;

    if (outlen == 0)
        return sizeof(sc->s3.server_random);
    if (outlen > sizeof(sc->s3.server_random))
        outlen = sizeof(sc->s3.server_random);
    memcpy(out, sc->s3.server_random, outlen);
    return outlen;
}

size_t SSL_SESSION_get_master_key(const SSL_SESSION *session,
                                  unsigned char *out, size_t outlen)
{
    if (outlen == 0)
        return session->master_key_length;
    if (outlen > session->master_key_length)
        outlen = session->master_key_length;
    memcpy(out, session->master_key, outlen);
    return outlen;
}

int SSL_SESSION_set1_master_key(SSL_SESSION *sess, const unsigned char *in,
                                size_t len)
{
    if (len > sizeof(sess->master_key))
        return 0;

    memcpy(sess->master_key, in, len);
    sess->master_key_length = len;
    return 1;
}


int SSL_set_ex_data(SSL *s, int idx, void *arg)
{
    return CRYPTO_set_ex_data(&s->ex_data, idx, arg);
}

void *SSL_get_ex_data(const SSL *s, int idx)
{
    return CRYPTO_get_ex_data(&s->ex_data, idx);
}

int SSL_CTX_set_ex_data(SSL_CTX *s, int idx, void *arg)
{
    return CRYPTO_set_ex_data(&s->ex_data, idx, arg);
}

void *SSL_CTX_get_ex_data(const SSL_CTX *s, int idx)
{
    return CRYPTO_get_ex_data(&s->ex_data, idx);
}

X509_STORE *SSL_CTX_get_cert_store(const SSL_CTX *ctx)
{
    return ctx->cert_store;
}

void SSL_CTX_set_cert_store(SSL_CTX *ctx, X509_STORE *store)
{
    X509_STORE_free(ctx->cert_store);
    ctx->cert_store = store;
}

void SSL_CTX_set1_cert_store(SSL_CTX *ctx, X509_STORE *store)
{
    if (store != NULL && !X509_STORE_up_ref(store))
        return;

    SSL_CTX_set_cert_store(ctx, store);
}

int SSL_want(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_want(s);
#endif

    if (sc == NULL)
        return SSL_NOTHING;

    return sc->rwstate;
}

#ifndef OPENSSL_NO_PSK
int SSL_CTX_use_psk_identity_hint(SSL_CTX *ctx, const char *identity_hint)
{
    if (identity_hint != NULL && strlen(identity_hint) > PSK_MAX_IDENTITY_LEN) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DATA_LENGTH_TOO_LONG);
        return 0;
    }
    OPENSSL_free(ctx->cert->psk_identity_hint);
    if (identity_hint != NULL) {
        ctx->cert->psk_identity_hint = OPENSSL_strdup(identity_hint);
        if (ctx->cert->psk_identity_hint == NULL)
            return 0;
    } else
        ctx->cert->psk_identity_hint = NULL;
    return 1;
}

int SSL_use_psk_identity_hint(SSL *s, const char *identity_hint)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (identity_hint != NULL && strlen(identity_hint) > PSK_MAX_IDENTITY_LEN) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DATA_LENGTH_TOO_LONG);
        return 0;
    }
    OPENSSL_free(sc->cert->psk_identity_hint);
    if (identity_hint != NULL) {
        sc->cert->psk_identity_hint = OPENSSL_strdup(identity_hint);
        if (sc->cert->psk_identity_hint == NULL)
            return 0;
    } else
        sc->cert->psk_identity_hint = NULL;
    return 1;
}

const char *SSL_get_psk_identity_hint(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL || sc->session == NULL)
        return NULL;

    return sc->session->psk_identity_hint;
}

const char *SSL_get_psk_identity(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL || sc->session == NULL)
        return NULL;

    return sc->session->psk_identity;
}

void SSL_set_psk_client_callback(SSL *s, SSL_psk_client_cb_func cb)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->psk_client_callback = cb;
}

void SSL_CTX_set_psk_client_callback(SSL_CTX *ctx, SSL_psk_client_cb_func cb)
{
    ctx->psk_client_callback = cb;
}

void SSL_set_psk_server_callback(SSL *s, SSL_psk_server_cb_func cb)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->psk_server_callback = cb;
}

void SSL_CTX_set_psk_server_callback(SSL_CTX *ctx, SSL_psk_server_cb_func cb)
{
    ctx->psk_server_callback = cb;
}
#endif

void SSL_set_psk_find_session_callback(SSL *s, SSL_psk_find_session_cb_func cb)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->psk_find_session_cb = cb;
}

void SSL_CTX_set_psk_find_session_callback(SSL_CTX *ctx,
                                           SSL_psk_find_session_cb_func cb)
{
    ctx->psk_find_session_cb = cb;
}

void SSL_set_psk_use_session_callback(SSL *s, SSL_psk_use_session_cb_func cb)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->psk_use_session_cb = cb;
}

void SSL_CTX_set_psk_use_session_callback(SSL_CTX *ctx,
                                           SSL_psk_use_session_cb_func cb)
{
    ctx->psk_use_session_cb = cb;
}

void SSL_CTX_set_msg_callback(SSL_CTX *ctx,
                              void (*cb) (int write_p, int version,
                                          int content_type, const void *buf,
                                          size_t len, SSL *ssl, void *arg))
{
    SSL_CTX_callback_ctrl(ctx, SSL_CTRL_SET_MSG_CALLBACK, (void (*)(void))cb);
}

void SSL_set_msg_callback(SSL *ssl,
                          void (*cb) (int write_p, int version,
                                      int content_type, const void *buf,
                                      size_t len, SSL *ssl, void *arg))
{
    SSL_callback_ctrl(ssl, SSL_CTRL_SET_MSG_CALLBACK, (void (*)(void))cb);
}

void SSL_CTX_set_not_resumable_session_callback(SSL_CTX *ctx,
                                                int (*cb) (SSL *ssl,
                                                           int
                                                           is_forward_secure))
{
    SSL_CTX_callback_ctrl(ctx, SSL_CTRL_SET_NOT_RESUMABLE_SESS_CB,
                          (void (*)(void))cb);
}

void SSL_set_not_resumable_session_callback(SSL *ssl,
                                            int (*cb) (SSL *ssl,
                                                       int is_forward_secure))
{
    SSL_callback_ctrl(ssl, SSL_CTRL_SET_NOT_RESUMABLE_SESS_CB,
                      (void (*)(void))cb);
}

void SSL_CTX_set_record_padding_callback(SSL_CTX *ctx,
                                         size_t (*cb) (SSL *ssl, int type,
                                                       size_t len, void *arg))
{
    ctx->record_padding_cb = cb;
}

void SSL_CTX_set_record_padding_callback_arg(SSL_CTX *ctx, void *arg)
{
    ctx->record_padding_arg = arg;
}

void *SSL_CTX_get_record_padding_callback_arg(const SSL_CTX *ctx)
{
    return ctx->record_padding_arg;
}

int SSL_CTX_set_block_padding_ex(SSL_CTX *ctx, size_t app_block_size,
                                 size_t hs_block_size)
{
    if (IS_QUIC_CTX(ctx) && (app_block_size > 1 || hs_block_size > 1))
        return 0;

    /* block size of 0 or 1 is basically no padding */
    if (app_block_size == 1) {
        ctx->block_padding = 0;
    } else if (app_block_size <= SSL3_RT_MAX_PLAIN_LENGTH) {
        ctx->block_padding = app_block_size;
    } else {
        return 0;
    }
    if (hs_block_size == 1) {
        ctx->hs_padding = 0;
    } else if (hs_block_size <= SSL3_RT_MAX_PLAIN_LENGTH) {
        ctx->hs_padding = hs_block_size;
    } else {
        return 0;
    }
    return 1;
}

int SSL_CTX_set_block_padding(SSL_CTX *ctx, size_t block_size)
{
    return SSL_CTX_set_block_padding_ex(ctx, block_size, block_size);
}

int SSL_set_record_padding_callback(SSL *ssl,
                                     size_t (*cb) (SSL *ssl, int type,
                                                   size_t len, void *arg))
{
    BIO *b;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (sc == NULL)
        return 0;

    b = SSL_get_wbio(ssl);
    if (b == NULL || !BIO_get_ktls_send(b)) {
        sc->rlayer.record_padding_cb = cb;
        return 1;
    }
    return 0;
}

void SSL_set_record_padding_callback_arg(SSL *ssl, void *arg)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return;

    sc->rlayer.record_padding_arg = arg;
}

void *SSL_get_record_padding_callback_arg(const SSL *ssl)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(ssl);

    if (sc == NULL)
        return NULL;

    return sc->rlayer.record_padding_arg;
}

int SSL_set_block_padding_ex(SSL *ssl, size_t app_block_size,
                             size_t hs_block_size)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL
        || (IS_QUIC(ssl)
            && (app_block_size > 1 || hs_block_size > 1)))
        return 0;

    /* block size of 0 or 1 is basically no padding */
    if (app_block_size == 1) {
        sc->rlayer.block_padding = 0;
    } else if (app_block_size <= SSL3_RT_MAX_PLAIN_LENGTH) {
        sc->rlayer.block_padding = app_block_size;
    } else {
        return 0;
    }
    if (hs_block_size == 1) {
        sc->rlayer.hs_padding = 0;
    } else if (hs_block_size <= SSL3_RT_MAX_PLAIN_LENGTH) {
        sc->rlayer.hs_padding = hs_block_size;
    } else {
        return 0;
    }
    return 1;
}

int SSL_set_block_padding(SSL *ssl, size_t block_size)
{
    return SSL_set_block_padding_ex(ssl, block_size, block_size);
}

int SSL_set_num_tickets(SSL *s, size_t num_tickets)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    sc->num_tickets = num_tickets;

    return 1;
}

size_t SSL_get_num_tickets(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->num_tickets;
}

int SSL_CTX_set_num_tickets(SSL_CTX *ctx, size_t num_tickets)
{
    ctx->num_tickets = num_tickets;

    return 1;
}

size_t SSL_CTX_get_num_tickets(const SSL_CTX *ctx)
{
    return ctx->num_tickets;
}

/* Retrieve handshake hashes */
int ssl_handshake_hash(SSL_CONNECTION *s,
                       unsigned char *out, size_t outlen,
                       size_t *hashlen)
{
    EVP_MD_CTX *ctx = NULL;
    EVP_MD_CTX *hdgst = s->s3.handshake_dgst;
    int hashleni = EVP_MD_CTX_get_size(hdgst);
    int ret = 0;

    if (hashleni < 0 || (size_t)hashleni > outlen) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    ctx = EVP_MD_CTX_new();
    if (ctx == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    if (!EVP_MD_CTX_copy_ex(ctx, hdgst)
        || EVP_DigestFinal_ex(ctx, out, NULL) <= 0) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    *hashlen = hashleni;

    ret = 1;
 err:
    EVP_MD_CTX_free(ctx);
    return ret;
}

int SSL_session_reused(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->hit;
}

int SSL_is_server(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->server;
}

#ifndef OPENSSL_NO_DEPRECATED_1_1_0
void SSL_set_debug(SSL *s, int debug)
{
    /* Old function was do-nothing anyway... */
    (void)s;
    (void)debug;
}
#endif

void SSL_set_security_level(SSL *s, int level)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->cert->sec_level = level;
}

int SSL_get_security_level(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->cert->sec_level;
}

void SSL_set_security_callback(SSL *s,
                               int (*cb) (const SSL *s, const SSL_CTX *ctx,
                                          int op, int bits, int nid,
                                          void *other, void *ex))
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->cert->sec_cb = cb;
}

int (*SSL_get_security_callback(const SSL *s)) (const SSL *s,
                                                const SSL_CTX *ctx, int op,
                                                int bits, int nid, void *other,
                                                void *ex) {
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    return sc->cert->sec_cb;
}

void SSL_set0_security_ex_data(SSL *s, void *ex)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    sc->cert->sec_ex = ex;
}

void *SSL_get0_security_ex_data(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    return sc->cert->sec_ex;
}

void SSL_CTX_set_security_level(SSL_CTX *ctx, int level)
{
    ctx->cert->sec_level = level;
}

int SSL_CTX_get_security_level(const SSL_CTX *ctx)
{
    return ctx->cert->sec_level;
}

void SSL_CTX_set_security_callback(SSL_CTX *ctx,
                                   int (*cb) (const SSL *s, const SSL_CTX *ctx,
                                              int op, int bits, int nid,
                                              void *other, void *ex))
{
    ctx->cert->sec_cb = cb;
}

int (*SSL_CTX_get_security_callback(const SSL_CTX *ctx)) (const SSL *s,
                                                          const SSL_CTX *ctx,
                                                          int op, int bits,
                                                          int nid,
                                                          void *other,
                                                          void *ex) {
    return ctx->cert->sec_cb;
}

void SSL_CTX_set0_security_ex_data(SSL_CTX *ctx, void *ex)
{
    ctx->cert->sec_ex = ex;
}

void *SSL_CTX_get0_security_ex_data(const SSL_CTX *ctx)
{
    return ctx->cert->sec_ex;
}

uint64_t SSL_CTX_get_options(const SSL_CTX *ctx)
{
    return ctx->options;
}

uint64_t SSL_get_options(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_get_options(s);
#endif

    if (sc == NULL)
        return 0;

    return sc->options;
}

uint64_t SSL_CTX_set_options(SSL_CTX *ctx, uint64_t op)
{
    return ctx->options |= op;
}

uint64_t SSL_set_options(SSL *s, uint64_t op)
{
    SSL_CONNECTION *sc;
    OSSL_PARAM options[2], *opts = options;

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_set_options(s, op);
#endif

    sc = SSL_CONNECTION_FROM_SSL(s);
    if (sc == NULL)
        return 0;

    sc->options |= op;

    *opts++ = OSSL_PARAM_construct_uint64(OSSL_LIBSSL_RECORD_LAYER_PARAM_OPTIONS,
                                          &sc->options);
    *opts = OSSL_PARAM_construct_end();

    /* Ignore return value */
    sc->rlayer.rrlmethod->set_options(sc->rlayer.rrl, options);
    sc->rlayer.wrlmethod->set_options(sc->rlayer.wrl, options);

    return sc->options;
}

uint64_t SSL_CTX_clear_options(SSL_CTX *ctx, uint64_t op)
{
    return ctx->options &= ~op;
}

uint64_t SSL_clear_options(SSL *s, uint64_t op)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);
    OSSL_PARAM options[2], *opts = options;

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_clear_options(s, op);
#endif

    if (sc == NULL)
        return 0;

    sc->options &= ~op;

    *opts++ = OSSL_PARAM_construct_uint64(OSSL_LIBSSL_RECORD_LAYER_PARAM_OPTIONS,
                                          &sc->options);
    *opts = OSSL_PARAM_construct_end();

    /* Ignore return value */
    sc->rlayer.rrlmethod->set_options(sc->rlayer.rrl, options);
    sc->rlayer.wrlmethod->set_options(sc->rlayer.wrl, options);

    return sc->options;
}

STACK_OF(X509) *SSL_get0_verified_chain(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    return sc->verified_chain;
}

IMPLEMENT_OBJ_BSEARCH_GLOBAL_CMP_FN(SSL_CIPHER, SSL_CIPHER, ssl_cipher_id);

#ifndef OPENSSL_NO_CT

/*
 * Moves SCTs from the |src| stack to the |dst| stack.
 * The source of each SCT will be set to |origin|.
 * If |dst| points to a NULL pointer, a new stack will be created and owned by
 * the caller.
 * Returns the number of SCTs moved, or a negative integer if an error occurs.
 * The |dst| stack is created and possibly partially populated even in case
 * of error, likewise the |src| stack may be left in an intermediate state.
 */
static int ct_move_scts(STACK_OF(SCT) **dst, STACK_OF(SCT) *src,
                        sct_source_t origin)
{
    int scts_moved = 0;
    SCT *sct = NULL;

    if (*dst == NULL) {
        *dst = sk_SCT_new_null();
        if (*dst == NULL) {
            ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    while ((sct = sk_SCT_pop(src)) != NULL) {
        if (SCT_set_source(sct, origin) != 1)
            goto err;

        if (!sk_SCT_push(*dst, sct))
            goto err;
        scts_moved += 1;
    }

    return scts_moved;
 err:
    SCT_free(sct);
    return -1;
}

/*
 * Look for data collected during ServerHello and parse if found.
 * Returns the number of SCTs extracted.
 */
static int ct_extract_tls_extension_scts(SSL_CONNECTION *s)
{
    int scts_extracted = 0;

    if (s->ext.scts != NULL) {
        const unsigned char *p = s->ext.scts;
        STACK_OF(SCT) *scts = o2i_SCT_LIST(NULL, &p, s->ext.scts_len);

        scts_extracted = ct_move_scts(&s->scts, scts, SCT_SOURCE_TLS_EXTENSION);

        SCT_LIST_free(scts);
    }

    return scts_extracted;
}

/*
 * Checks for an OCSP response and then attempts to extract any SCTs found if it
 * contains an SCT X509 extension. They will be stored in |s->scts|.
 * Returns:
 * - The number of SCTs extracted, assuming an OCSP response exists.
 * - 0 if no OCSP response exists or it contains no SCTs.
 * - A negative integer if an error occurs.
 */
static int ct_extract_ocsp_response_scts(SSL_CONNECTION *s)
{
# ifndef OPENSSL_NO_OCSP
    int scts_extracted = 0;
    const unsigned char *p;
    OCSP_BASICRESP *br = NULL;
    OCSP_RESPONSE *rsp = NULL;
    STACK_OF(SCT) *scts = NULL;
    int i;

    if (s->ext.ocsp.resp == NULL || s->ext.ocsp.resp_len == 0)
        goto err;

    p = s->ext.ocsp.resp;
    rsp = d2i_OCSP_RESPONSE(NULL, &p, (int)s->ext.ocsp.resp_len);
    if (rsp == NULL)
        goto err;

    br = OCSP_response_get1_basic(rsp);
    if (br == NULL)
        goto err;

    for (i = 0; i < OCSP_resp_count(br); ++i) {
        OCSP_SINGLERESP *single = OCSP_resp_get0(br, i);

        if (single == NULL)
            continue;

        scts =
            OCSP_SINGLERESP_get1_ext_d2i(single, NID_ct_cert_scts, NULL, NULL);
        scts_extracted =
            ct_move_scts(&s->scts, scts, SCT_SOURCE_OCSP_STAPLED_RESPONSE);
        if (scts_extracted < 0)
            goto err;
    }
 err:
    SCT_LIST_free(scts);
    OCSP_BASICRESP_free(br);
    OCSP_RESPONSE_free(rsp);
    return scts_extracted;
# else
    /* Behave as if no OCSP response exists */
    return 0;
# endif
}

/*
 * Attempts to extract SCTs from the peer certificate.
 * Return the number of SCTs extracted, or a negative integer if an error
 * occurs.
 */
static int ct_extract_x509v3_extension_scts(SSL_CONNECTION *s)
{
    int scts_extracted = 0;
    X509 *cert = s->session != NULL ? s->session->peer : NULL;

    if (cert != NULL) {
        STACK_OF(SCT) *scts =
            X509_get_ext_d2i(cert, NID_ct_precert_scts, NULL, NULL);

        scts_extracted =
            ct_move_scts(&s->scts, scts, SCT_SOURCE_X509V3_EXTENSION);

        SCT_LIST_free(scts);
    }

    return scts_extracted;
}

/*
 * Attempts to find all received SCTs by checking TLS extensions, the OCSP
 * response (if it exists) and X509v3 extensions in the certificate.
 * Returns NULL if an error occurs.
 */
const STACK_OF(SCT) *SSL_get0_peer_scts(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    if (!sc->scts_parsed) {
        if (ct_extract_tls_extension_scts(sc) < 0 ||
            ct_extract_ocsp_response_scts(sc) < 0 ||
            ct_extract_x509v3_extension_scts(sc) < 0)
            goto err;

        sc->scts_parsed = 1;
    }
    return sc->scts;
 err:
    return NULL;
}

static int ct_permissive(const CT_POLICY_EVAL_CTX *ctx,
                         const STACK_OF(SCT) *scts, void *unused_arg)
{
    return 1;
}

static int ct_strict(const CT_POLICY_EVAL_CTX *ctx,
                     const STACK_OF(SCT) *scts, void *unused_arg)
{
    int count = scts != NULL ? sk_SCT_num(scts) : 0;
    int i;

    for (i = 0; i < count; ++i) {
        SCT *sct = sk_SCT_value(scts, i);
        int status = SCT_get_validation_status(sct);

        if (status == SCT_VALIDATION_STATUS_VALID)
            return 1;
    }
    ERR_raise(ERR_LIB_SSL, SSL_R_NO_VALID_SCTS);
    return 0;
}

int SSL_set_ct_validation_callback(SSL *s, ssl_ct_validation_cb callback,
                                   void *arg)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    /*
     * Since code exists that uses the custom extension handler for CT, look
     * for this and throw an error if they have already registered to use CT.
     */
    if (callback != NULL && SSL_CTX_has_client_custom_ext(s->ctx,
                                                          TLSEXT_TYPE_signed_certificate_timestamp))
    {
        ERR_raise(ERR_LIB_SSL, SSL_R_CUSTOM_EXT_HANDLER_ALREADY_INSTALLED);
        return 0;
    }

    if (callback != NULL) {
        /*
         * If we are validating CT, then we MUST accept SCTs served via OCSP
         */
        if (!SSL_set_tlsext_status_type(s, TLSEXT_STATUSTYPE_ocsp))
            return 0;
    }

    sc->ct_validation_callback = callback;
    sc->ct_validation_callback_arg = arg;

    return 1;
}

int SSL_CTX_set_ct_validation_callback(SSL_CTX *ctx,
                                       ssl_ct_validation_cb callback, void *arg)
{
    /*
     * Since code exists that uses the custom extension handler for CT, look for
     * this and throw an error if they have already registered to use CT.
     */
    if (callback != NULL && SSL_CTX_has_client_custom_ext(ctx,
                                                          TLSEXT_TYPE_signed_certificate_timestamp))
    {
        ERR_raise(ERR_LIB_SSL, SSL_R_CUSTOM_EXT_HANDLER_ALREADY_INSTALLED);
        return 0;
    }

    ctx->ct_validation_callback = callback;
    ctx->ct_validation_callback_arg = arg;
    return 1;
}

int SSL_ct_is_enabled(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->ct_validation_callback != NULL;
}

int SSL_CTX_ct_is_enabled(const SSL_CTX *ctx)
{
    return ctx->ct_validation_callback != NULL;
}

int ssl_validate_ct(SSL_CONNECTION *s)
{
    int ret = 0;
    X509 *cert = s->session != NULL ? s->session->peer : NULL;
    X509 *issuer;
    SSL_DANE *dane = &s->dane;
    CT_POLICY_EVAL_CTX *ctx = NULL;
    const STACK_OF(SCT) *scts;

    /*
     * If no callback is set, the peer is anonymous, or its chain is invalid,
     * skip SCT validation - just return success.  Applications that continue
     * handshakes without certificates, with unverified chains, or pinned leaf
     * certificates are outside the scope of the WebPKI and CT.
     *
     * The above exclusions notwithstanding the vast majority of peers will
     * have rather ordinary certificate chains validated by typical
     * applications that perform certificate verification and therefore will
     * process SCTs when enabled.
     */
    if (s->ct_validation_callback == NULL || cert == NULL ||
        s->verify_result != X509_V_OK ||
        s->verified_chain == NULL || sk_X509_num(s->verified_chain) <= 1)
        return 1;

    /*
     * CT not applicable for chains validated via DANE-TA(2) or DANE-EE(3)
     * trust-anchors.  See https://tools.ietf.org/html/rfc7671#section-4.2
     */
    if (DANETLS_ENABLED(dane) && dane->mtlsa != NULL) {
        switch (dane->mtlsa->usage) {
        case DANETLS_USAGE_DANE_TA:
        case DANETLS_USAGE_DANE_EE:
            return 1;
        }
    }

    ctx = CT_POLICY_EVAL_CTX_new_ex(SSL_CONNECTION_GET_CTX(s)->libctx,
                                    SSL_CONNECTION_GET_CTX(s)->propq);
    if (ctx == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CT_LIB);
        goto end;
    }

    issuer = sk_X509_value(s->verified_chain, 1);
    CT_POLICY_EVAL_CTX_set1_cert(ctx, cert);
    CT_POLICY_EVAL_CTX_set1_issuer(ctx, issuer);
    CT_POLICY_EVAL_CTX_set_shared_CTLOG_STORE(ctx,
            SSL_CONNECTION_GET_CTX(s)->ctlog_store);
    CT_POLICY_EVAL_CTX_set_time(
            ctx, (uint64_t)SSL_SESSION_get_time_ex(s->session) * 1000);

    scts = SSL_get0_peer_scts(SSL_CONNECTION_GET_SSL(s));

    /*
     * This function returns success (> 0) only when all the SCTs are valid, 0
     * when some are invalid, and < 0 on various internal errors (out of
     * memory, etc.).  Having some, or even all, invalid SCTs is not sufficient
     * reason to abort the handshake, that decision is up to the callback.
     * Therefore, we error out only in the unexpected case that the return
     * value is negative.
     *
     * XXX: One might well argue that the return value of this function is an
     * unfortunate design choice.  Its job is only to determine the validation
     * status of each of the provided SCTs.  So long as it correctly separates
     * the wheat from the chaff it should return success.  Failure in this case
     * ought to correspond to an inability to carry out its duties.
     */
    if (SCT_LIST_validate(scts, ctx) < 0) {
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, SSL_R_SCT_VERIFICATION_FAILED);
        goto end;
    }

    ret = s->ct_validation_callback(ctx, scts, s->ct_validation_callback_arg);
    if (ret < 0)
        ret = 0;                /* This function returns 0 on failure */
    if (!ret)
        SSLfatal(s, SSL_AD_HANDSHAKE_FAILURE, SSL_R_CALLBACK_FAILED);

 end:
    CT_POLICY_EVAL_CTX_free(ctx);
    /*
     * With SSL_VERIFY_NONE the session may be cached and reused despite a
     * failure return code here.  Also the application may wish the complete
     * the handshake, and then disconnect cleanly at a higher layer, after
     * checking the verification status of the completed connection.
     *
     * We therefore force a certificate verification failure which will be
     * visible via SSL_get_verify_result() and cached as part of any resumed
     * session.
     *
     * Note: the permissive callback is for information gathering only, always
     * returns success, and does not affect verification status.  Only the
     * strict callback or a custom application-specified callback can trigger
     * connection failure or record a verification error.
     */
    if (ret <= 0)
        s->verify_result = X509_V_ERR_NO_VALID_SCTS;
    return ret;
}

int SSL_CTX_enable_ct(SSL_CTX *ctx, int validation_mode)
{
    switch (validation_mode) {
    default:
        ERR_raise(ERR_LIB_SSL, SSL_R_INVALID_CT_VALIDATION_TYPE);
        return 0;
    case SSL_CT_VALIDATION_PERMISSIVE:
        return SSL_CTX_set_ct_validation_callback(ctx, ct_permissive, NULL);
    case SSL_CT_VALIDATION_STRICT:
        return SSL_CTX_set_ct_validation_callback(ctx, ct_strict, NULL);
    }
}

int SSL_enable_ct(SSL *s, int validation_mode)
{
    switch (validation_mode) {
    default:
        ERR_raise(ERR_LIB_SSL, SSL_R_INVALID_CT_VALIDATION_TYPE);
        return 0;
    case SSL_CT_VALIDATION_PERMISSIVE:
        return SSL_set_ct_validation_callback(s, ct_permissive, NULL);
    case SSL_CT_VALIDATION_STRICT:
        return SSL_set_ct_validation_callback(s, ct_strict, NULL);
    }
}

int SSL_CTX_set_default_ctlog_list_file(SSL_CTX *ctx)
{
    return CTLOG_STORE_load_default_file(ctx->ctlog_store);
}

int SSL_CTX_set_ctlog_list_file(SSL_CTX *ctx, const char *path)
{
    return CTLOG_STORE_load_file(ctx->ctlog_store, path);
}

void SSL_CTX_set0_ctlog_store(SSL_CTX *ctx, CTLOG_STORE *logs)
{
    CTLOG_STORE_free(ctx->ctlog_store);
    ctx->ctlog_store = logs;
}

const CTLOG_STORE *SSL_CTX_get0_ctlog_store(const SSL_CTX *ctx)
{
    return ctx->ctlog_store;
}

#endif  /* OPENSSL_NO_CT */

void SSL_CTX_set_client_hello_cb(SSL_CTX *c, SSL_client_hello_cb_fn cb,
                                 void *arg)
{
    c->client_hello_cb = cb;
    c->client_hello_cb_arg = arg;
}

void SSL_CTX_set_new_pending_conn_cb(SSL_CTX *c, SSL_new_pending_conn_cb_fn cb,
                                     void *arg)
{
    c->new_pending_conn_cb = cb;
    c->new_pending_conn_arg = arg;
}

int SSL_client_hello_isv2(SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->clienthello == NULL)
        return 0;
    return sc->clienthello->isv2;
}

unsigned int SSL_client_hello_get0_legacy_version(SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->clienthello == NULL)
        return 0;
    return sc->clienthello->legacy_version;
}

size_t SSL_client_hello_get0_random(SSL *s, const unsigned char **out)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->clienthello == NULL)
        return 0;
    if (out != NULL)
        *out = sc->clienthello->random;
    return SSL3_RANDOM_SIZE;
}

size_t SSL_client_hello_get0_session_id(SSL *s, const unsigned char **out)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->clienthello == NULL)
        return 0;
    if (out != NULL)
        *out = sc->clienthello->session_id;
    return sc->clienthello->session_id_len;
}

size_t SSL_client_hello_get0_ciphers(SSL *s, const unsigned char **out)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->clienthello == NULL)
        return 0;
    if (out != NULL)
        *out = PACKET_data(&sc->clienthello->ciphersuites);
    return PACKET_remaining(&sc->clienthello->ciphersuites);
}

size_t SSL_client_hello_get0_compression_methods(SSL *s, const unsigned char **out)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->clienthello == NULL)
        return 0;
    if (out != NULL)
        *out = sc->clienthello->compressions;
    return sc->clienthello->compressions_len;
}

int SSL_client_hello_get1_extensions_present(SSL *s, int **out, size_t *outlen)
{
    RAW_EXTENSION *ext;
    int *present;
    size_t num = 0, i;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->clienthello == NULL || out == NULL || outlen == NULL)
        return 0;
    for (i = 0; i < sc->clienthello->pre_proc_exts_len; i++) {
        ext = sc->clienthello->pre_proc_exts + i;
        if (ext->present)
            num++;
    }
    if (num == 0) {
        *out = NULL;
        *outlen = 0;
        return 1;
    }
    if ((present = OPENSSL_malloc(sizeof(*present) * num)) == NULL)
        return 0;
    for (i = 0; i < sc->clienthello->pre_proc_exts_len; i++) {
        ext = sc->clienthello->pre_proc_exts + i;
        if (ext->present) {
            if (ext->received_order >= num)
                goto err;
            present[ext->received_order] = ext->type;
        }
    }
    *out = present;
    *outlen = num;
    return 1;
 err:
    OPENSSL_free(present);
    return 0;
}

int SSL_client_hello_get_extension_order(SSL *s, uint16_t *exts, size_t *num_exts)
{
    RAW_EXTENSION *ext;
    size_t num = 0, i;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->clienthello == NULL || num_exts == NULL)
        return 0;
    for (i = 0; i < sc->clienthello->pre_proc_exts_len; i++) {
        ext = sc->clienthello->pre_proc_exts + i;
        if (ext->present)
            num++;
    }
    if (num == 0) {
        *num_exts = 0;
        return 1;
    }
    if (exts == NULL) {
        *num_exts = num;
        return 1;
    }
    if (*num_exts < num)
        return 0;
    for (i = 0; i < sc->clienthello->pre_proc_exts_len; i++) {
        ext = sc->clienthello->pre_proc_exts + i;
        if (ext->present) {
            if (ext->received_order >= num)
                return 0;
            exts[ext->received_order] = ext->type;
        }
    }
    *num_exts = num;
    return 1;
}

int SSL_client_hello_get0_ext(SSL *s, unsigned int type, const unsigned char **out,
                       size_t *outlen)
{
    size_t i;
    RAW_EXTENSION *r;
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->clienthello == NULL)
        return 0;
    for (i = 0; i < sc->clienthello->pre_proc_exts_len; ++i) {
        r = sc->clienthello->pre_proc_exts + i;
        if (r->present && r->type == type) {
            if (out != NULL)
                *out = PACKET_data(&r->data);
            if (outlen != NULL)
                *outlen = PACKET_remaining(&r->data);
            return 1;
        }
    }
    return 0;
}

int SSL_free_buffers(SSL *ssl)
{
    RECORD_LAYER *rl;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (sc == NULL)
        return 0;

    rl = &sc->rlayer;

    return rl->rrlmethod->free_buffers(rl->rrl)
           && rl->wrlmethod->free_buffers(rl->wrl);
}

int SSL_alloc_buffers(SSL *ssl)
{
    RECORD_LAYER *rl;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return 0;

    /* QUIC always has buffers allocated. */
    if (IS_QUIC(ssl))
        return 1;

    rl = &sc->rlayer;

    return rl->rrlmethod->alloc_buffers(rl->rrl)
           && rl->wrlmethod->alloc_buffers(rl->wrl);
}

void SSL_CTX_set_keylog_callback(SSL_CTX *ctx, SSL_CTX_keylog_cb_func cb)
{
    ctx->keylog_callback = cb;
}

SSL_CTX_keylog_cb_func SSL_CTX_get_keylog_callback(const SSL_CTX *ctx)
{
    return ctx->keylog_callback;
}

static int nss_keylog_int(const char *prefix,
                          SSL_CONNECTION *sc,
                          const uint8_t *parameter_1,
                          size_t parameter_1_len,
                          const uint8_t *parameter_2,
                          size_t parameter_2_len)
{
    char *out = NULL;
    char *cursor = NULL;
    size_t out_len = 0, i, prefix_len;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(sc);

#ifndef OPENSSL_NO_SSLKEYLOG
    if (sctx->keylog_callback == NULL && sctx->do_sslkeylog == 0)
        return 1;
#else
    if (sctx->keylog_callback == NULL)
        return 1;
#endif

    /*
     * Our output buffer will contain the following strings, rendered with
     * space characters in between, terminated by a NULL character: first the
     * prefix, then the first parameter, then the second parameter. The
     * meaning of each parameter depends on the specific key material being
     * logged. Note that the first and second parameters are encoded in
     * hexadecimal, so we need a buffer that is twice their lengths.
     */
    prefix_len = strlen(prefix);
    out_len = prefix_len + (2 * parameter_1_len) + (2 * parameter_2_len) + 3;
    if ((out = cursor = OPENSSL_malloc(out_len)) == NULL)
        return 0;

    memcpy(cursor, prefix, prefix_len);
    cursor += prefix_len;
    *cursor++ = ' ';

    for (i = 0; i < parameter_1_len; ++i)
        cursor += ossl_to_lowerhex(cursor, parameter_1[i]);
    *cursor++ = ' ';

    for (i = 0; i < parameter_2_len; ++i)
        cursor += ossl_to_lowerhex(cursor, parameter_2[i]);
    *cursor = '\0';

#ifndef OPENSSL_NO_SSLKEYLOG
    if (sctx->do_sslkeylog == 1)
        do_sslkeylogfile(SSL_CONNECTION_GET_SSL(sc), (const char *)out);
#endif
    if (sctx->keylog_callback != NULL)
        sctx->keylog_callback(SSL_CONNECTION_GET_USER_SSL(sc), (const char *)out);
    OPENSSL_clear_free(out, out_len);
    return 1;
}

int ssl_log_rsa_client_key_exchange(SSL_CONNECTION *sc,
                                    const uint8_t *encrypted_premaster,
                                    size_t encrypted_premaster_len,
                                    const uint8_t *premaster,
                                    size_t premaster_len)
{
    if (encrypted_premaster_len < 8) {
        SSLfatal(sc, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    /* We only want the first 8 bytes of the encrypted premaster as a tag. */
    return nss_keylog_int("RSA",
                          sc,
                          encrypted_premaster,
                          8,
                          premaster,
                          premaster_len);
}

int ssl_log_secret(SSL_CONNECTION *sc,
                   const char *label,
                   const uint8_t *secret,
                   size_t secret_len)
{
    return nss_keylog_int(label,
                          sc,
                          sc->s3.client_random,
                          SSL3_RANDOM_SIZE,
                          secret,
                          secret_len);
}

#define SSLV2_CIPHER_LEN    3

int ssl_cache_cipherlist(SSL_CONNECTION *s, PACKET *cipher_suites, int sslv2format)
{
    int n;

    n = sslv2format ? SSLV2_CIPHER_LEN : TLS_CIPHER_LEN;

    if (PACKET_remaining(cipher_suites) == 0) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_NO_CIPHERS_SPECIFIED);
        return 0;
    }

    if (PACKET_remaining(cipher_suites) % n != 0) {
        SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_ERROR_IN_RECEIVED_CIPHER_LIST);
        return 0;
    }

    OPENSSL_free(s->s3.tmp.ciphers_raw);
    s->s3.tmp.ciphers_raw = NULL;
    s->s3.tmp.ciphers_rawlen = 0;

    if (sslv2format) {
        size_t numciphers = PACKET_remaining(cipher_suites) / n;
        PACKET sslv2ciphers = *cipher_suites;
        unsigned int leadbyte;
        unsigned char *raw;

        /*
         * We store the raw ciphers list in SSLv3+ format so we need to do some
         * preprocessing to convert the list first. If there are any SSLv2 only
         * ciphersuites with a non-zero leading byte then we are going to
         * slightly over allocate because we won't store those. But that isn't a
         * problem.
         */
        raw = OPENSSL_malloc(numciphers * TLS_CIPHER_LEN);
        s->s3.tmp.ciphers_raw = raw;
        if (raw == NULL) {
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
            return 0;
        }
        for (s->s3.tmp.ciphers_rawlen = 0;
             PACKET_remaining(&sslv2ciphers) > 0;
             raw += TLS_CIPHER_LEN) {
            if (!PACKET_get_1(&sslv2ciphers, &leadbyte)
                    || (leadbyte == 0
                        && !PACKET_copy_bytes(&sslv2ciphers, raw,
                                              TLS_CIPHER_LEN))
                    || (leadbyte != 0
                        && !PACKET_forward(&sslv2ciphers, TLS_CIPHER_LEN))) {
                SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_BAD_PACKET);
                OPENSSL_free(s->s3.tmp.ciphers_raw);
                s->s3.tmp.ciphers_raw = NULL;
                s->s3.tmp.ciphers_rawlen = 0;
                return 0;
            }
            if (leadbyte == 0)
                s->s3.tmp.ciphers_rawlen += TLS_CIPHER_LEN;
        }
    } else if (!PACKET_memdup(cipher_suites, &s->s3.tmp.ciphers_raw,
                           &s->s3.tmp.ciphers_rawlen)) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    return 1;
}

int SSL_bytes_to_cipher_list(SSL *s, const unsigned char *bytes, size_t len,
                             int isv2format, STACK_OF(SSL_CIPHER) **sk,
                             STACK_OF(SSL_CIPHER) **scsvs)
{
    PACKET pkt;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (!PACKET_buf_init(&pkt, bytes, len))
        return 0;
    return ossl_bytes_to_cipher_list(sc, &pkt, sk, scsvs, isv2format, 0);
}

int ossl_bytes_to_cipher_list(SSL_CONNECTION *s, PACKET *cipher_suites,
                              STACK_OF(SSL_CIPHER) **skp,
                              STACK_OF(SSL_CIPHER) **scsvs_out,
                              int sslv2format, int fatal)
{
    const SSL_CIPHER *c;
    STACK_OF(SSL_CIPHER) *sk = NULL;
    STACK_OF(SSL_CIPHER) *scsvs = NULL;
    int n;
    /* 3 = SSLV2_CIPHER_LEN > TLS_CIPHER_LEN = 2. */
    unsigned char cipher[SSLV2_CIPHER_LEN];

    n = sslv2format ? SSLV2_CIPHER_LEN : TLS_CIPHER_LEN;

    if (PACKET_remaining(cipher_suites) == 0) {
        if (fatal)
            SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_NO_CIPHERS_SPECIFIED);
        else
            ERR_raise(ERR_LIB_SSL, SSL_R_NO_CIPHERS_SPECIFIED);
        return 0;
    }

    if (PACKET_remaining(cipher_suites) % n != 0) {
        if (fatal)
            SSLfatal(s, SSL_AD_DECODE_ERROR,
                     SSL_R_ERROR_IN_RECEIVED_CIPHER_LIST);
        else
            ERR_raise(ERR_LIB_SSL, SSL_R_ERROR_IN_RECEIVED_CIPHER_LIST);
        return 0;
    }

    sk = sk_SSL_CIPHER_new_null();
    scsvs = sk_SSL_CIPHER_new_null();
    if (sk == NULL || scsvs == NULL) {
        if (fatal)
            SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        else
            ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

    while (PACKET_copy_bytes(cipher_suites, cipher, n)) {
        /*
         * SSLv3 ciphers wrapped in an SSLv2-compatible ClientHello have the
         * first byte set to zero, while true SSLv2 ciphers have a non-zero
         * first byte. We don't support any true SSLv2 ciphers, so skip them.
         */
        if (sslv2format && cipher[0] != '\0')
            continue;

        /* For SSLv2-compat, ignore leading 0-byte. */
        c = ssl_get_cipher_by_char(s, sslv2format ? &cipher[1] : cipher, 1);
        if (c != NULL) {
            if ((c->valid && !sk_SSL_CIPHER_push(sk, c)) ||
                (!c->valid && !sk_SSL_CIPHER_push(scsvs, c))) {
                if (fatal)
                    SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
                else
                    ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
                goto err;
            }
        }
    }
    if (PACKET_remaining(cipher_suites) > 0) {
        if (fatal)
            SSLfatal(s, SSL_AD_DECODE_ERROR, SSL_R_BAD_LENGTH);
        else
            ERR_raise(ERR_LIB_SSL, SSL_R_BAD_LENGTH);
        goto err;
    }

    if (skp != NULL)
        *skp = sk;
    else
        sk_SSL_CIPHER_free(sk);
    if (scsvs_out != NULL)
        *scsvs_out = scsvs;
    else
        sk_SSL_CIPHER_free(scsvs);
    return 1;
 err:
    sk_SSL_CIPHER_free(sk);
    sk_SSL_CIPHER_free(scsvs);
    return 0;
}

int SSL_CTX_set_max_early_data(SSL_CTX *ctx, uint32_t max_early_data)
{
    ctx->max_early_data = max_early_data;

    return 1;
}

uint32_t SSL_CTX_get_max_early_data(const SSL_CTX *ctx)
{
    return ctx->max_early_data;
}

int SSL_set_max_early_data(SSL *s, uint32_t max_early_data)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return 0;

    sc->max_early_data = max_early_data;

    return 1;
}

uint32_t SSL_get_max_early_data(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->max_early_data;
}

int SSL_CTX_set_recv_max_early_data(SSL_CTX *ctx, uint32_t recv_max_early_data)
{
    ctx->recv_max_early_data = recv_max_early_data;

    return 1;
}

uint32_t SSL_CTX_get_recv_max_early_data(const SSL_CTX *ctx)
{
    return ctx->recv_max_early_data;
}

int SSL_set_recv_max_early_data(SSL *s, uint32_t recv_max_early_data)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return 0;

    sc->recv_max_early_data = recv_max_early_data;

    return 1;
}

uint32_t SSL_get_recv_max_early_data(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->recv_max_early_data;
}

__owur unsigned int ssl_get_max_send_fragment(const SSL_CONNECTION *sc)
{
    /* Return any active Max Fragment Len extension */
    if (sc->session != NULL && USE_MAX_FRAGMENT_LENGTH_EXT(sc->session))
        return GET_MAX_FRAGMENT_LENGTH(sc->session);

    /* return current SSL connection setting */
    return sc->max_send_fragment;
}

__owur unsigned int ssl_get_split_send_fragment(const SSL_CONNECTION *sc)
{
    /* Return a value regarding an active Max Fragment Len extension */
    if (sc->session != NULL && USE_MAX_FRAGMENT_LENGTH_EXT(sc->session)
        && sc->split_send_fragment > GET_MAX_FRAGMENT_LENGTH(sc->session))
        return GET_MAX_FRAGMENT_LENGTH(sc->session);

    /* else limit |split_send_fragment| to current |max_send_fragment| */
    if (sc->split_send_fragment > sc->max_send_fragment)
        return sc->max_send_fragment;

    /* return current SSL connection setting */
    return sc->split_send_fragment;
}

int SSL_stateless(SSL *s)
{
    int ret;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return 0;

    /* Ensure there is no state left over from a previous invocation */
    if (!SSL_clear(s))
        return 0;

    ERR_clear_error();

    sc->s3.flags |= TLS1_FLAGS_STATELESS;
    ret = SSL_accept(s);
    sc->s3.flags &= ~TLS1_FLAGS_STATELESS;

    if (ret > 0 && sc->ext.cookieok)
        return 1;

    if (sc->hello_retry_request == SSL_HRR_PENDING && !ossl_statem_in_error(sc))
        return 0;

    return -1;
}

void SSL_CTX_set_post_handshake_auth(SSL_CTX *ctx, int val)
{
    ctx->pha_enabled = val;
}

void SSL_set_post_handshake_auth(SSL *ssl, int val)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (sc == NULL)
        return;

    sc->pha_enabled = val;
}

int SSL_verify_client_post_handshake(SSL *ssl)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(ssl)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_WRONG_SSL_VERSION);
        return 0;
    }
#endif

    if (sc == NULL)
        return 0;

    if (!SSL_CONNECTION_IS_TLS13(sc)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_WRONG_SSL_VERSION);
        return 0;
    }
    if (!sc->server) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NOT_SERVER);
        return 0;
    }

    if (!SSL_is_init_finished(ssl)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_STILL_IN_INIT);
        return 0;
    }

    switch (sc->post_handshake_auth) {
    case SSL_PHA_NONE:
        ERR_raise(ERR_LIB_SSL, SSL_R_EXTENSION_NOT_RECEIVED);
        return 0;
    default:
    case SSL_PHA_EXT_SENT:
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    case SSL_PHA_EXT_RECEIVED:
        break;
    case SSL_PHA_REQUEST_PENDING:
        ERR_raise(ERR_LIB_SSL, SSL_R_REQUEST_PENDING);
        return 0;
    case SSL_PHA_REQUESTED:
        ERR_raise(ERR_LIB_SSL, SSL_R_REQUEST_SENT);
        return 0;
    }

    sc->post_handshake_auth = SSL_PHA_REQUEST_PENDING;

    /* checks verify_mode and algorithm_auth */
    if (!send_certificate_request(sc)) {
        sc->post_handshake_auth = SSL_PHA_EXT_RECEIVED; /* restore on error */
        ERR_raise(ERR_LIB_SSL, SSL_R_INVALID_CONFIG);
        return 0;
    }

    ossl_statem_set_in_init(sc, 1);
    return 1;
}

int SSL_CTX_set_session_ticket_cb(SSL_CTX *ctx,
                                  SSL_CTX_generate_session_ticket_fn gen_cb,
                                  SSL_CTX_decrypt_session_ticket_fn dec_cb,
                                  void *arg)
{
    ctx->generate_ticket_cb = gen_cb;
    ctx->decrypt_ticket_cb = dec_cb;
    ctx->ticket_cb_data = arg;
    return 1;
}

void SSL_CTX_set_allow_early_data_cb(SSL_CTX *ctx,
                                     SSL_allow_early_data_cb_fn cb,
                                     void *arg)
{
    ctx->allow_early_data_cb = cb;
    ctx->allow_early_data_cb_data = arg;
}

void SSL_set_allow_early_data_cb(SSL *s,
                                 SSL_allow_early_data_cb_fn cb,
                                 void *arg)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return;

    sc->allow_early_data_cb = cb;
    sc->allow_early_data_cb_data = arg;
}

const EVP_CIPHER *ssl_evp_cipher_fetch(OSSL_LIB_CTX *libctx,
                                       int nid,
                                       const char *properties)
{
    const EVP_CIPHER *ciph;

    ciph = tls_get_cipher_from_engine(nid);
    if (ciph != NULL)
        return ciph;

    /*
     * If there is no engine cipher then we do an explicit fetch. This may fail
     * and that could be ok
     */
    ERR_set_mark();
    ciph = EVP_CIPHER_fetch(libctx, OBJ_nid2sn(nid), properties);
    if (ciph != NULL) {
        OSSL_PARAM params[2];
        int decrypt_only = 0;

        params[0] = OSSL_PARAM_construct_int(OSSL_CIPHER_PARAM_DECRYPT_ONLY,
                                             &decrypt_only);
        params[1] = OSSL_PARAM_construct_end();
        if (EVP_CIPHER_get_params((EVP_CIPHER *)ciph, params)
            && decrypt_only) {
            /* If a cipher is decrypt-only, it is unusable */
            EVP_CIPHER_free((EVP_CIPHER *)ciph);
            ciph = NULL;
        }
    }
    ERR_pop_to_mark();
    return ciph;
}


int ssl_evp_cipher_up_ref(const EVP_CIPHER *cipher)
{
    /* Don't up-ref an implicit EVP_CIPHER */
    if (EVP_CIPHER_get0_provider(cipher) == NULL)
        return 1;

    /*
     * The cipher was explicitly fetched and therefore it is safe to cast
     * away the const
     */
    return EVP_CIPHER_up_ref((EVP_CIPHER *)cipher);
}

void ssl_evp_cipher_free(const EVP_CIPHER *cipher)
{
    if (cipher == NULL)
        return;

    if (EVP_CIPHER_get0_provider(cipher) != NULL) {
        /*
         * The cipher was explicitly fetched and therefore it is safe to cast
         * away the const
         */
        EVP_CIPHER_free((EVP_CIPHER *)cipher);
    }
}

const EVP_MD *ssl_evp_md_fetch(OSSL_LIB_CTX *libctx,
                               int nid,
                               const char *properties)
{
    const EVP_MD *md;

    md = tls_get_digest_from_engine(nid);
    if (md != NULL)
        return md;

    /* Otherwise we do an explicit fetch */
    ERR_set_mark();
    md = EVP_MD_fetch(libctx, OBJ_nid2sn(nid), properties);
    ERR_pop_to_mark();
    return md;
}

int ssl_evp_md_up_ref(const EVP_MD *md)
{
    /* Don't up-ref an implicit EVP_MD */
    if (EVP_MD_get0_provider(md) == NULL)
        return 1;

    /*
     * The digest was explicitly fetched and therefore it is safe to cast
     * away the const
     */
    return EVP_MD_up_ref((EVP_MD *)md);
}

void ssl_evp_md_free(const EVP_MD *md)
{
    if (md == NULL)
        return;

    if (EVP_MD_get0_provider(md) != NULL) {
        /*
         * The digest was explicitly fetched and therefore it is safe to cast
         * away the const
         */
        EVP_MD_free((EVP_MD *)md);
    }
}

int SSL_set0_tmp_dh_pkey(SSL *s, EVP_PKEY *dhpkey)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (!ssl_security(sc, SSL_SECOP_TMP_DH,
                      EVP_PKEY_get_security_bits(dhpkey), 0, dhpkey)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DH_KEY_TOO_SMALL);
        return 0;
    }
    EVP_PKEY_free(sc->cert->dh_tmp);
    sc->cert->dh_tmp = dhpkey;
    return 1;
}

int SSL_CTX_set0_tmp_dh_pkey(SSL_CTX *ctx, EVP_PKEY *dhpkey)
{
    if (!ssl_ctx_security(ctx, SSL_SECOP_TMP_DH,
                          EVP_PKEY_get_security_bits(dhpkey), 0, dhpkey)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DH_KEY_TOO_SMALL);
        return 0;
    }
    EVP_PKEY_free(ctx->cert->dh_tmp);
    ctx->cert->dh_tmp = dhpkey;
    return 1;
}

/* QUIC-specific methods which are supported on QUIC connections only. */
int SSL_handle_events(SSL *s)
{
    SSL_CONNECTION *sc;

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_handle_events(s);
#endif

    sc = SSL_CONNECTION_FROM_SSL_ONLY(s);
    if (sc != NULL && SSL_CONNECTION_IS_DTLS(sc))
        /*
         * DTLSv1_handle_timeout returns 0 if the timer wasn't expired yet,
         * which we consider a success case. Theoretically DTLSv1_handle_timeout
         * can also return 0 if s is NULL or not a DTLS object, but we've
         * already ruled out those possibilities above, so this is not possible
         * here. Thus the only failure cases are where DTLSv1_handle_timeout
         * returns -1.
         */
        return DTLSv1_handle_timeout(s) >= 0;

    return 1;
}

int SSL_get_event_timeout(SSL *s, struct timeval *tv, int *is_infinite)
{
    SSL_CONNECTION *sc;

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_get_event_timeout(s, tv, is_infinite);
#endif

    sc = SSL_CONNECTION_FROM_SSL_ONLY(s);
    if (sc != NULL && SSL_CONNECTION_IS_DTLS(sc)
        && DTLSv1_get_timeout(s, tv)) {
        *is_infinite = 0;
        return 1;
    }

    tv->tv_sec  = 1000000;
    tv->tv_usec = 0;
    *is_infinite = 1;
    return 1;
}

int SSL_get_rpoll_descriptor(SSL *s, BIO_POLL_DESCRIPTOR *desc)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_get_rpoll_descriptor(s, desc);
#endif

    if (sc == NULL || sc->rbio == NULL)
        return 0;

    return BIO_get_rpoll_descriptor(sc->rbio, desc);
}

int SSL_get_wpoll_descriptor(SSL *s, BIO_POLL_DESCRIPTOR *desc)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_get_wpoll_descriptor(s, desc);
#endif

    if (sc == NULL || sc->wbio == NULL)
        return 0;

    return BIO_get_wpoll_descriptor(sc->wbio, desc);
}

int SSL_net_read_desired(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return SSL_want_read(s);

    return ossl_quic_get_net_read_desired(s);
#else
    return SSL_want_read(s);
#endif
}

int SSL_net_write_desired(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return SSL_want_write(s);

    return ossl_quic_get_net_write_desired(s);
#else
    return SSL_want_write(s);
#endif
}

int SSL_set_blocking_mode(SSL *s, int blocking)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return 0;

    return ossl_quic_conn_set_blocking_mode(s, blocking);
#else
    return 0;
#endif
}

int SSL_get_blocking_mode(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return -1;

    return ossl_quic_conn_get_blocking_mode(s);
#else
    return -1;
#endif
}

int SSL_set1_initial_peer_addr(SSL *s, const BIO_ADDR *peer_addr)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return 0;

    return ossl_quic_conn_set_initial_peer_addr(s, peer_addr);
#else
    return 0;
#endif
}

int SSL_shutdown_ex(SSL *ssl, uint64_t flags,
                    const SSL_SHUTDOWN_EX_ARGS *args,
                    size_t args_len)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(ssl))
        return SSL_shutdown(ssl);

    return ossl_quic_conn_shutdown(ssl, flags, args, args_len);
#else
    return SSL_shutdown(ssl);
#endif
}

int SSL_stream_conclude(SSL *ssl, uint64_t flags)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(ssl))
        return 0;

    return ossl_quic_conn_stream_conclude(ssl);
#else
    return 0;
#endif
}

SSL *SSL_new_stream(SSL *s, uint64_t flags)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return NULL;

    return ossl_quic_conn_stream_new(s, flags);
#else
    return NULL;
#endif
}

SSL *SSL_get0_connection(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return s;

    return ossl_quic_get0_connection(s);
#else
    return s;
#endif
}

int SSL_is_connection(SSL *s)
{
    return SSL_get0_connection(s) == s;
}

SSL *SSL_get0_listener(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return NULL;

    return ossl_quic_get0_listener(s);
#else
    return NULL;
#endif
}

SSL *SSL_get0_domain(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return NULL;

    return ossl_quic_get0_domain(s);
#else
    return NULL;
#endif
}

int SSL_is_listener(SSL *s)
{
    return SSL_get0_listener(s) == s;
}

int SSL_is_domain(SSL *s)
{
    return SSL_get0_domain(s) == s;
}

int SSL_get_stream_type(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return SSL_STREAM_TYPE_BIDI;

    return ossl_quic_get_stream_type(s);
#else
    return SSL_STREAM_TYPE_BIDI;
#endif
}

uint64_t SSL_get_stream_id(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return UINT64_MAX;

    return ossl_quic_get_stream_id(s);
#else
    return UINT64_MAX;
#endif
}

int SSL_is_stream_local(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return -1;

    return ossl_quic_is_stream_local(s);
#else
    return -1;
#endif
}

int SSL_set_default_stream_mode(SSL *s, uint32_t mode)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return 0;

    return ossl_quic_set_default_stream_mode(s, mode);
#else
    return 0;
#endif
}

int SSL_set_incoming_stream_policy(SSL *s, int policy, uint64_t aec)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return 0;

    return ossl_quic_set_incoming_stream_policy(s, policy, aec);
#else
    return 0;
#endif
}

SSL *SSL_accept_stream(SSL *s, uint64_t flags)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return NULL;

    return ossl_quic_accept_stream(s, flags);
#else
    return NULL;
#endif
}

size_t SSL_get_accept_stream_queue_len(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return 0;

    return ossl_quic_get_accept_stream_queue_len(s);
#else
    return 0;
#endif
}

int SSL_stream_reset(SSL *s,
                     const SSL_STREAM_RESET_ARGS *args,
                     size_t args_len)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return 0;

    return ossl_quic_stream_reset(s, args, args_len);
#else
    return 0;
#endif
}

int SSL_get_stream_read_state(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return SSL_STREAM_STATE_NONE;

    return ossl_quic_get_stream_read_state(s);
#else
    return SSL_STREAM_STATE_NONE;
#endif
}

int SSL_get_stream_write_state(SSL *s)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return SSL_STREAM_STATE_NONE;

    return ossl_quic_get_stream_write_state(s);
#else
    return SSL_STREAM_STATE_NONE;
#endif
}

int SSL_get_stream_read_error_code(SSL *s, uint64_t *app_error_code)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return -1;

    return ossl_quic_get_stream_read_error_code(s, app_error_code);
#else
    return -1;
#endif
}

int SSL_get_stream_write_error_code(SSL *s, uint64_t *app_error_code)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return -1;

    return ossl_quic_get_stream_write_error_code(s, app_error_code);
#else
    return -1;
#endif
}

int SSL_get_conn_close_info(SSL *s, SSL_CONN_CLOSE_INFO *info,
                            size_t info_len)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(s))
        return -1;

    return ossl_quic_get_conn_close_info(s, info, info_len);
#else
    return -1;
#endif
}

int SSL_get_value_uint(SSL *s, uint32_t class_, uint32_t id,
                       uint64_t *value)
{
#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_get_value_uint(s, class_, id, value);
#endif

    ERR_raise(ERR_LIB_SSL, SSL_R_UNSUPPORTED_PROTOCOL);
    return 0;
}

int SSL_set_value_uint(SSL *s, uint32_t class_, uint32_t id,
                       uint64_t value)
{
#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(s))
        return ossl_quic_set_value_uint(s, class_, id, value);
#endif

    ERR_raise(ERR_LIB_SSL, SSL_R_UNSUPPORTED_PROTOCOL);
    return 0;
}

SSL *SSL_new_listener(SSL_CTX *ctx, uint64_t flags)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC_CTX(ctx))
        return NULL;

    return ossl_quic_new_listener(ctx, flags);
#else
    return NULL;
#endif
}

SSL *SSL_new_listener_from(SSL *ssl, uint64_t flags)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(ssl))
        return NULL;

    return ossl_quic_new_listener_from(ssl, flags);
#else
    return NULL;
#endif
}

SSL *SSL_new_from_listener(SSL *ssl, uint64_t flags)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(ssl))
        return NULL;

    return ossl_quic_new_from_listener(ssl, flags);
#else
    return NULL;
#endif
}

SSL *SSL_accept_connection(SSL *ssl, uint64_t flags)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(ssl))
        return NULL;

    return ossl_quic_accept_connection(ssl, flags);
#else
    return NULL;
#endif
}

size_t SSL_get_accept_connection_queue_len(SSL *ssl)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(ssl))
        return 0;

    return ossl_quic_get_accept_connection_queue_len(ssl);
#else
    return 0;
#endif
}

int SSL_listen(SSL *ssl)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC(ssl))
        return 0;

    return ossl_quic_listen(ssl);
#else
    return 0;
#endif
}

SSL *SSL_new_domain(SSL_CTX *ctx, uint64_t flags)
{
#ifndef OPENSSL_NO_QUIC
    if (!IS_QUIC_CTX(ctx))
        return NULL;

    return ossl_quic_new_domain(ctx, flags);
#else
    return NULL;
#endif
}

int ossl_adjust_domain_flags(uint64_t domain_flags, uint64_t *p_domain_flags)
{
    if ((domain_flags & ~OSSL_QUIC_SUPPORTED_DOMAIN_FLAGS) != 0) {
        ERR_raise_data(ERR_LIB_SSL, ERR_R_UNSUPPORTED,
                       "unsupported domain flag requested");
        return 0;
    }

    if ((domain_flags & SSL_DOMAIN_FLAG_THREAD_ASSISTED) != 0)
        domain_flags |= SSL_DOMAIN_FLAG_MULTI_THREAD;

    if ((domain_flags & (SSL_DOMAIN_FLAG_MULTI_THREAD
                         | SSL_DOMAIN_FLAG_SINGLE_THREAD)) == 0)
        domain_flags |= SSL_DOMAIN_FLAG_MULTI_THREAD;

    if ((domain_flags & SSL_DOMAIN_FLAG_SINGLE_THREAD) != 0
        && (domain_flags & SSL_DOMAIN_FLAG_MULTI_THREAD) != 0) {
        ERR_raise_data(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT,
                       "mutually exclusive domain flags specified");
        return 0;
    }

    /*
     * Note: We treat MULTI_THREAD as a no-op in non-threaded builds, but
     * not THREAD_ASSISTED.
     */
# ifndef OPENSSL_THREADS
    if ((domain_flags & SSL_DOMAIN_FLAG_THREAD_ASSISTED) != 0) {
        ERR_raise_data(ERR_LIB_SSL, ERR_R_UNSUPPORTED,
                       "thread assisted mode not available in this build");
        return 0;
    }
# endif

    *p_domain_flags = domain_flags;
    return 1;
}

int SSL_CTX_set_domain_flags(SSL_CTX *ctx, uint64_t domain_flags)
{
#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC_CTX(ctx)) {
        if (!ossl_adjust_domain_flags(domain_flags, &domain_flags))
            return 0;

        ctx->domain_flags = domain_flags;
        return 1;
    }
#endif

    ERR_raise_data(ERR_LIB_SSL, ERR_R_UNSUPPORTED,
                   "domain flags unsupported on this kind of SSL_CTX");
    return 0;
}

int SSL_CTX_get_domain_flags(const SSL_CTX *ctx, uint64_t *domain_flags)
{
#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC_CTX(ctx)) {
        if (domain_flags != NULL)
            *domain_flags = ctx->domain_flags;

        return 1;
    }
#endif

    ERR_raise_data(ERR_LIB_SSL, ERR_R_UNSUPPORTED,
                   "domain flags unsupported on this kind of SSL_CTX");
    return 0;
}

int SSL_get_domain_flags(const SSL *ssl, uint64_t *domain_flags)
{
#ifndef OPENSSL_NO_QUIC
    if (IS_QUIC(ssl))
        return ossl_quic_get_domain_flags(ssl, domain_flags);
#endif

    return 0;
}

int SSL_add_expected_rpk(SSL *s, EVP_PKEY *rpk)
{
    unsigned char *data = NULL;
    SSL_DANE *dane = SSL_get0_dane(s);
    int ret;

    if (dane == NULL || dane->dctx == NULL)
        return 0;
    if ((ret = i2d_PUBKEY(rpk, &data)) <= 0)
        return 0;

    ret = SSL_dane_tlsa_add(s, DANETLS_USAGE_DANE_EE,
                            DANETLS_SELECTOR_SPKI,
                            DANETLS_MATCHING_FULL,
                            data, (size_t)ret) > 0;
    OPENSSL_free(data);
    return ret;
}

EVP_PKEY *SSL_get0_peer_rpk(const SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL || sc->session == NULL)
        return NULL;
    return sc->session->peer_rpk;
}

int SSL_get_negotiated_client_cert_type(const SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->ext.client_cert_type;
}

int SSL_get_negotiated_server_cert_type(const SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    return sc->ext.server_cert_type;
}

static int validate_cert_type(const unsigned char *val, size_t len)
{
    size_t i;
    int saw_rpk = 0;
    int saw_x509 = 0;

    if (val == NULL && len == 0)
        return 1;

    if (val == NULL || len == 0)
        return 0;

    for (i = 0; i < len; i++) {
        switch (val[i]) {
        case TLSEXT_cert_type_rpk:
            if (saw_rpk)
                return 0;
            saw_rpk = 1;
            break;
        case TLSEXT_cert_type_x509:
            if (saw_x509)
                return 0;
            saw_x509 = 1;
            break;
        case TLSEXT_cert_type_pgp:
        case TLSEXT_cert_type_1609dot2:
        default:
            return 0;
        }
    }
    return 1;
}

static int set_cert_type(unsigned char **cert_type,
                         size_t *cert_type_len,
                         const unsigned char *val,
                         size_t len)
{
    unsigned char *tmp = NULL;

    if (!validate_cert_type(val, len))
        return 0;

    if (val != NULL && (tmp = OPENSSL_memdup(val, len)) == NULL)
        return 0;

    OPENSSL_free(*cert_type);
    *cert_type = tmp;
    *cert_type_len = len;
    return 1;
}

int SSL_set1_client_cert_type(SSL *s, const unsigned char *val, size_t len)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    return set_cert_type(&sc->client_cert_type, &sc->client_cert_type_len,
                         val, len);
}

int SSL_set1_server_cert_type(SSL *s, const unsigned char *val, size_t len)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    return set_cert_type(&sc->server_cert_type, &sc->server_cert_type_len,
                         val, len);
}

int SSL_CTX_set1_client_cert_type(SSL_CTX *ctx, const unsigned char *val, size_t len)
{
    return set_cert_type(&ctx->client_cert_type, &ctx->client_cert_type_len,
                         val, len);
}

int SSL_CTX_set1_server_cert_type(SSL_CTX *ctx, const unsigned char *val, size_t len)
{
    return set_cert_type(&ctx->server_cert_type, &ctx->server_cert_type_len,
                         val, len);
}

int SSL_get0_client_cert_type(const SSL *s, unsigned char **t, size_t *len)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (t == NULL || len == NULL || sc == NULL)
        return 0;

    *t = sc->client_cert_type;
    *len = sc->client_cert_type_len;
    return 1;
}

int SSL_get0_server_cert_type(const SSL *s, unsigned char **t, size_t *len)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (t == NULL || len == NULL || sc == NULL)
        return 0;

    *t = sc->server_cert_type;
    *len = sc->server_cert_type_len;
    return 1;
}

int SSL_CTX_get0_client_cert_type(const SSL_CTX *ctx, unsigned char **t, size_t *len)
{
    if (t == NULL || len == NULL)
        return 0;

    *t = ctx->client_cert_type;
    *len = ctx->client_cert_type_len;
    return 1;
}

int SSL_CTX_get0_server_cert_type(const SSL_CTX *ctx, unsigned char **t, size_t *len)
{
    if (t == NULL || len == NULL)
        return 0;

    *t = ctx->server_cert_type;
    *len = ctx->server_cert_type_len;
    return 1;
}
