/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"

#include <stdio.h>
#include <sys/types.h>

#include "internal/nelem.h"
#include "internal/o_dir.h"
#include <openssl/bio.h>
#include <openssl/pem.h>
#include <openssl/store.h>
#include <openssl/x509v3.h>
#include <openssl/dh.h>
#include <openssl/bn.h>
#include <openssl/crypto.h>
#include "internal/refcount.h"
#include "ssl_local.h"
#include "ssl_cert_table.h"
#include "internal/thread_once.h"
#include "internal/ssl_unwrap.h"
#ifndef OPENSSL_NO_POSIX_IO
# include <sys/stat.h>
# ifdef _WIN32
#  define stat _stat
# endif
# ifndef S_ISDIR
#  define S_ISDIR(a) (((a) & S_IFMT) == S_IFDIR)
# endif
#endif


static int ssl_security_default_callback(const SSL *s, const SSL_CTX *ctx,
                                         int op, int bits, int nid, void *other,
                                         void *ex);

static CRYPTO_ONCE ssl_x509_store_ctx_once = CRYPTO_ONCE_STATIC_INIT;
static volatile int ssl_x509_store_ctx_idx = -1;

DEFINE_RUN_ONCE_STATIC(ssl_x509_store_ctx_init)
{
    ssl_x509_store_ctx_idx = X509_STORE_CTX_get_ex_new_index(0,
                                                             "SSL for verify callback",
                                                             NULL, NULL, NULL);
    return ssl_x509_store_ctx_idx >= 0;
}

int SSL_get_ex_data_X509_STORE_CTX_idx(void)
{

    if (!RUN_ONCE(&ssl_x509_store_ctx_once, ssl_x509_store_ctx_init))
        return -1;
    return ssl_x509_store_ctx_idx;
}

CERT *ssl_cert_new(size_t ssl_pkey_num)
{
    CERT *ret = NULL;

    /* Should never happen */
    if (!ossl_assert(ssl_pkey_num >= SSL_PKEY_NUM))
        return NULL;

    ret = OPENSSL_zalloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;

    ret->ssl_pkey_num = ssl_pkey_num;
    ret->pkeys = OPENSSL_zalloc(ret->ssl_pkey_num * sizeof(CERT_PKEY));
    if (ret->pkeys == NULL) {
        OPENSSL_free(ret);
        return NULL;
    }

    ret->key = &(ret->pkeys[SSL_PKEY_RSA]);
    ret->sec_cb = ssl_security_default_callback;
    ret->sec_level = OPENSSL_TLS_SECURITY_LEVEL;
    ret->sec_ex = NULL;
    if (!CRYPTO_NEW_REF(&ret->references, 1)) {
        OPENSSL_free(ret->pkeys);
        OPENSSL_free(ret);
        return NULL;
    }

    return ret;
}

CERT *ssl_cert_dup(CERT *cert)
{
    CERT *ret = OPENSSL_zalloc(sizeof(*ret));
    size_t i;
#ifndef OPENSSL_NO_COMP_ALG
    int j;
#endif

    if (ret == NULL)
        return NULL;

    ret->ssl_pkey_num = cert->ssl_pkey_num;
    ret->pkeys = OPENSSL_zalloc(ret->ssl_pkey_num * sizeof(CERT_PKEY));
    if (ret->pkeys == NULL) {
        OPENSSL_free(ret);
        return NULL;
    }

    ret->key = &ret->pkeys[cert->key - cert->pkeys];
    if (!CRYPTO_NEW_REF(&ret->references, 1)) {
        OPENSSL_free(ret->pkeys);
        OPENSSL_free(ret);
        return NULL;
    }

    if (cert->dh_tmp != NULL) {
        if (!EVP_PKEY_up_ref(cert->dh_tmp))
            goto err;
        ret->dh_tmp = cert->dh_tmp;
    }

    ret->dh_tmp_cb = cert->dh_tmp_cb;
    ret->dh_tmp_auto = cert->dh_tmp_auto;

    for (i = 0; i < ret->ssl_pkey_num; i++) {
        CERT_PKEY *cpk = cert->pkeys + i;
        CERT_PKEY *rpk = ret->pkeys + i;

        if (cpk->x509 != NULL) {
            if (!X509_up_ref(cpk->x509))
                goto err;
            rpk->x509 = cpk->x509;
        }

        if (cpk->privatekey != NULL) {
            if (!EVP_PKEY_up_ref(cpk->privatekey))
                goto err;
            rpk->privatekey = cpk->privatekey;
        }

        if (cpk->chain) {
            rpk->chain = X509_chain_up_ref(cpk->chain);
            if (!rpk->chain) {
                ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
                goto err;
            }
        }
        if (cpk->serverinfo != NULL) {
            /* Just copy everything. */
            rpk->serverinfo = OPENSSL_memdup(cpk->serverinfo, cpk->serverinfo_length);
            if (rpk->serverinfo == NULL)
                goto err;
            rpk->serverinfo_length = cpk->serverinfo_length;
        }
#ifndef OPENSSL_NO_COMP_ALG
        for (j = TLSEXT_comp_cert_none; j < TLSEXT_comp_cert_limit; j++) {
            if (cpk->comp_cert[j] != NULL) {
                if (!OSSL_COMP_CERT_up_ref(cpk->comp_cert[j]))
                    goto err;
                rpk->comp_cert[j] = cpk->comp_cert[j];
            }
        }
#endif
    }

    /* Configured sigalgs copied across */
    if (cert->conf_sigalgs) {
        ret->conf_sigalgs = OPENSSL_malloc(cert->conf_sigalgslen
                                           * sizeof(*cert->conf_sigalgs));
        if (ret->conf_sigalgs == NULL)
            goto err;
        memcpy(ret->conf_sigalgs, cert->conf_sigalgs,
               cert->conf_sigalgslen * sizeof(*cert->conf_sigalgs));
        ret->conf_sigalgslen = cert->conf_sigalgslen;
    } else
        ret->conf_sigalgs = NULL;

    if (cert->client_sigalgs) {
        ret->client_sigalgs = OPENSSL_malloc(cert->client_sigalgslen
                                             * sizeof(*cert->client_sigalgs));
        if (ret->client_sigalgs == NULL)
            goto err;
        memcpy(ret->client_sigalgs, cert->client_sigalgs,
               cert->client_sigalgslen * sizeof(*cert->client_sigalgs));
        ret->client_sigalgslen = cert->client_sigalgslen;
    } else
        ret->client_sigalgs = NULL;
    /* Copy any custom client certificate types */
    if (cert->ctype) {
        ret->ctype = OPENSSL_memdup(cert->ctype, cert->ctype_len);
        if (ret->ctype == NULL)
            goto err;
        ret->ctype_len = cert->ctype_len;
    }

    ret->cert_flags = cert->cert_flags;

    ret->cert_cb = cert->cert_cb;
    ret->cert_cb_arg = cert->cert_cb_arg;

    if (cert->verify_store) {
        if (!X509_STORE_up_ref(cert->verify_store))
            goto err;
        ret->verify_store = cert->verify_store;
    }

    if (cert->chain_store) {
        if (!X509_STORE_up_ref(cert->chain_store))
            goto err;
        ret->chain_store = cert->chain_store;
    }

    ret->sec_cb = cert->sec_cb;
    ret->sec_level = cert->sec_level;
    ret->sec_ex = cert->sec_ex;

    if (!custom_exts_copy(&ret->custext, &cert->custext))
        goto err;
#ifndef OPENSSL_NO_PSK
    if (cert->psk_identity_hint) {
        ret->psk_identity_hint = OPENSSL_strdup(cert->psk_identity_hint);
        if (ret->psk_identity_hint == NULL)
            goto err;
    }
#endif
    return ret;

 err:
    ssl_cert_free(ret);

    return NULL;
}

/* Free up and clear all certificates and chains */

void ssl_cert_clear_certs(CERT *c)
{
    size_t i;
#ifndef OPENSSL_NO_COMP_ALG
    int j;
#endif

    if (c == NULL)
        return;
    for (i = 0; i < c->ssl_pkey_num; i++) {
        CERT_PKEY *cpk = c->pkeys + i;
        X509_free(cpk->x509);
        cpk->x509 = NULL;
        EVP_PKEY_free(cpk->privatekey);
        cpk->privatekey = NULL;
        OSSL_STACK_OF_X509_free(cpk->chain);
        cpk->chain = NULL;
        OPENSSL_free(cpk->serverinfo);
        cpk->serverinfo = NULL;
        cpk->serverinfo_length = 0;
#ifndef OPENSSL_NO_COMP_ALG
        for (j = 0; j < TLSEXT_comp_cert_limit; j++) {
            OSSL_COMP_CERT_free(cpk->comp_cert[j]);
            cpk->comp_cert[j] = NULL;
            cpk->cert_comp_used = 0;
        }
#endif
    }
}

void ssl_cert_free(CERT *c)
{
    int i;

    if (c == NULL)
        return;
    CRYPTO_DOWN_REF(&c->references, &i);
    REF_PRINT_COUNT("CERT", i, c);
    if (i > 0)
        return;
    REF_ASSERT_ISNT(i < 0);

    EVP_PKEY_free(c->dh_tmp);

    ssl_cert_clear_certs(c);
    OPENSSL_free(c->conf_sigalgs);
    OPENSSL_free(c->client_sigalgs);
    OPENSSL_free(c->ctype);
    X509_STORE_free(c->verify_store);
    X509_STORE_free(c->chain_store);
    custom_exts_free(&c->custext);
#ifndef OPENSSL_NO_PSK
    OPENSSL_free(c->psk_identity_hint);
#endif
    OPENSSL_free(c->pkeys);
    CRYPTO_FREE_REF(&c->references);
    OPENSSL_free(c);
}

int ssl_cert_set0_chain(SSL_CONNECTION *s, SSL_CTX *ctx, STACK_OF(X509) *chain)
{
    int i, r;
    CERT_PKEY *cpk = s != NULL ? s->cert->key : ctx->cert->key;

    if (!cpk)
        return 0;
    for (i = 0; i < sk_X509_num(chain); i++) {
        X509 *x = sk_X509_value(chain, i);

        r = ssl_security_cert(s, ctx, x, 0, 0);
        if (r != 1) {
            ERR_raise(ERR_LIB_SSL, r);
            return 0;
        }
    }
    OSSL_STACK_OF_X509_free(cpk->chain);
    cpk->chain = chain;
    return 1;
}

int ssl_cert_set1_chain(SSL_CONNECTION *s, SSL_CTX *ctx, STACK_OF(X509) *chain)
{
    STACK_OF(X509) *dchain;

    if (!chain)
        return ssl_cert_set0_chain(s, ctx, NULL);
    dchain = X509_chain_up_ref(chain);
    if (!dchain)
        return 0;
    if (!ssl_cert_set0_chain(s, ctx, dchain)) {
        OSSL_STACK_OF_X509_free(dchain);
        return 0;
    }
    return 1;
}

int ssl_cert_add0_chain_cert(SSL_CONNECTION *s, SSL_CTX *ctx, X509 *x)
{
    int r;
    CERT_PKEY *cpk = s ? s->cert->key : ctx->cert->key;

    if (!cpk)
        return 0;
    r = ssl_security_cert(s, ctx, x, 0, 0);
    if (r != 1) {
        ERR_raise(ERR_LIB_SSL, r);
        return 0;
    }
    if (!cpk->chain)
        cpk->chain = sk_X509_new_null();
    if (!cpk->chain || !sk_X509_push(cpk->chain, x))
        return 0;
    return 1;
}

int ssl_cert_add1_chain_cert(SSL_CONNECTION *s, SSL_CTX *ctx, X509 *x)
{
    if (!X509_up_ref(x))
        return 0;
    if (!ssl_cert_add0_chain_cert(s, ctx, x)) {
        X509_free(x);
        return 0;
    }
    return 1;
}

int ssl_cert_select_current(CERT *c, X509 *x)
{
    size_t i;

    if (x == NULL)
        return 0;
    for (i = 0; i < c->ssl_pkey_num; i++) {
        CERT_PKEY *cpk = c->pkeys + i;
        if (cpk->x509 == x && cpk->privatekey) {
            c->key = cpk;
            return 1;
        }
    }

    for (i = 0; i < c->ssl_pkey_num; i++) {
        CERT_PKEY *cpk = c->pkeys + i;
        if (cpk->privatekey && cpk->x509 && !X509_cmp(cpk->x509, x)) {
            c->key = cpk;
            return 1;
        }
    }
    return 0;
}

int ssl_cert_set_current(CERT *c, long op)
{
    size_t i, idx;

    if (!c)
        return 0;
    if (op == SSL_CERT_SET_FIRST)
        idx = 0;
    else if (op == SSL_CERT_SET_NEXT) {
        idx = (size_t)(c->key - c->pkeys + 1);
        if (idx >= c->ssl_pkey_num)
            return 0;
    } else
        return 0;
    for (i = idx; i < c->ssl_pkey_num; i++) {
        CERT_PKEY *cpk = c->pkeys + i;
        if (cpk->x509 && cpk->privatekey) {
            c->key = cpk;
            return 1;
        }
    }
    return 0;
}

void ssl_cert_set_cert_cb(CERT *c, int (*cb) (SSL *ssl, void *arg), void *arg)
{
    c->cert_cb = cb;
    c->cert_cb_arg = arg;
}

/*
 * Verify a certificate chain/raw public key
 * Return codes:
 *  1: Verify success
 *  0: Verify failure or error
 * -1: Retry required
 */
static int ssl_verify_internal(SSL_CONNECTION *s, STACK_OF(X509) *sk, EVP_PKEY *rpk)
{
    X509 *x;
    int i = 0;
    X509_STORE *verify_store;
    X509_STORE_CTX *ctx = NULL;
    X509_VERIFY_PARAM *param;
    SSL_CTX *sctx;

    /* Something must be passed in */
    if ((sk == NULL || sk_X509_num(sk) == 0) && rpk == NULL)
        return 0;

    /* Only one can be set */
    if (sk != NULL && rpk != NULL)
        return 0;

    sctx = SSL_CONNECTION_GET_CTX(s);
    if (s->cert->verify_store)
        verify_store = s->cert->verify_store;
    else
        verify_store = sctx->cert_store;

    ctx = X509_STORE_CTX_new_ex(sctx->libctx, sctx->propq);
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
        return 0;
    }

    if (sk != NULL) {
        x = sk_X509_value(sk, 0);
        if (!X509_STORE_CTX_init(ctx, verify_store, x, sk)) {
            ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
            goto end;
        }
    } else {
        if (!X509_STORE_CTX_init_rpk(ctx, verify_store, rpk)) {
            ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
            goto end;
        }
    }
    param = X509_STORE_CTX_get0_param(ctx);
    /*
     * XXX: Separate @AUTHSECLEVEL and @TLSSECLEVEL would be useful at some
     * point, for now a single @SECLEVEL sets the same policy for TLS crypto
     * and PKI authentication.
     */
    X509_VERIFY_PARAM_set_auth_level(param,
        SSL_get_security_level(SSL_CONNECTION_GET_SSL(s)));

    /* Set suite B flags if needed */
    X509_STORE_CTX_set_flags(ctx, tls1_suiteb(s));
    if (!X509_STORE_CTX_set_ex_data(ctx,
            SSL_get_ex_data_X509_STORE_CTX_idx(),
            SSL_CONNECTION_GET_USER_SSL(s)))
        goto end;

    /* Verify via DANE if enabled */
    if (DANETLS_ENABLED(&s->dane))
        X509_STORE_CTX_set0_dane(ctx, &s->dane);

    /*
     * We need to inherit the verify parameters. These can be determined by
     * the context: if its a server it will verify SSL client certificates or
     * vice versa.
     */

    X509_STORE_CTX_set_default(ctx, s->server ? "ssl_client" : "ssl_server");
    /*
     * Anything non-default in "s->param" should overwrite anything in the ctx.
     */
    X509_VERIFY_PARAM_set1(param, s->param);

    if (s->verify_callback)
        X509_STORE_CTX_set_verify_cb(ctx, s->verify_callback);

    if (sctx->app_verify_callback != NULL) {
        i = sctx->app_verify_callback(ctx, sctx->app_verify_arg);
    } else {
        i = X509_verify_cert(ctx);
        /* We treat an error in the same way as a failure to verify */
        if (i < 0)
            i = 0;
    }

    s->verify_result = X509_STORE_CTX_get_error(ctx);
    OSSL_STACK_OF_X509_free(s->verified_chain);
    s->verified_chain = NULL;

    if (sk != NULL && X509_STORE_CTX_get0_chain(ctx) != NULL) {
        s->verified_chain = X509_STORE_CTX_get1_chain(ctx);
        if (s->verified_chain == NULL) {
            ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
            i = 0;
        }
    }

    /* Move peername from the store context params to the SSL handle's */
    X509_VERIFY_PARAM_move_peername(s->param, param);

 end:
    X509_STORE_CTX_free(ctx);
    return i;
}

/*
 * Verify a raw public key
 * Return codes:
 *  1: Verify success
 *  0: Verify failure or error
 * -1: Retry required
 */
int ssl_verify_rpk(SSL_CONNECTION *s, EVP_PKEY *rpk)
{
    return ssl_verify_internal(s, NULL, rpk);
}

/*
 * Verify a certificate chain
 * Return codes:
 *  1: Verify success
 *  0: Verify failure or error
 * -1: Retry required
 */
int ssl_verify_cert_chain(SSL_CONNECTION *s, STACK_OF(X509) *sk)
{
    return ssl_verify_internal(s, sk, NULL);
}

static void set0_CA_list(STACK_OF(X509_NAME) **ca_list,
                        STACK_OF(X509_NAME) *name_list)
{
    sk_X509_NAME_pop_free(*ca_list, X509_NAME_free);
    *ca_list = name_list;
}

STACK_OF(X509_NAME) *SSL_dup_CA_list(const STACK_OF(X509_NAME) *sk)
{
    int i;
    const int num = sk_X509_NAME_num(sk);
    STACK_OF(X509_NAME) *ret;
    X509_NAME *name;

    ret = sk_X509_NAME_new_reserve(NULL, num);
    if (ret == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        return NULL;
    }
    for (i = 0; i < num; i++) {
        name = X509_NAME_dup(sk_X509_NAME_value(sk, i));
        if (name == NULL) {
            ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
            sk_X509_NAME_pop_free(ret, X509_NAME_free);
            return NULL;
        }
        sk_X509_NAME_push(ret, name);   /* Cannot fail after reserve call */
    }
    return ret;
}

void SSL_set0_CA_list(SSL *s, STACK_OF(X509_NAME) *name_list)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    set0_CA_list(&sc->ca_names, name_list);
}

void SSL_CTX_set0_CA_list(SSL_CTX *ctx, STACK_OF(X509_NAME) *name_list)
{
    set0_CA_list(&ctx->ca_names, name_list);
}

const STACK_OF(X509_NAME) *SSL_CTX_get0_CA_list(const SSL_CTX *ctx)
{
    return ctx->ca_names;
}

const STACK_OF(X509_NAME) *SSL_get0_CA_list(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    return sc->ca_names != NULL ? sc->ca_names : s->ctx->ca_names;
}

void SSL_CTX_set_client_CA_list(SSL_CTX *ctx, STACK_OF(X509_NAME) *name_list)
{
    set0_CA_list(&ctx->client_ca_names, name_list);
}

STACK_OF(X509_NAME) *SSL_CTX_get_client_CA_list(const SSL_CTX *ctx)
{
    return ctx->client_ca_names;
}

void SSL_set_client_CA_list(SSL *s, STACK_OF(X509_NAME) *name_list)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return;

    set0_CA_list(&sc->client_ca_names, name_list);
}

const STACK_OF(X509_NAME) *SSL_get0_peer_CA_list(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    return sc->s3.tmp.peer_ca_names;
}

STACK_OF(X509_NAME) *SSL_get_client_CA_list(const SSL *s)
{
    const SSL_CONNECTION *sc = SSL_CONNECTION_FROM_CONST_SSL(s);

    if (sc == NULL)
        return NULL;

    if (!sc->server)
        return sc->s3.tmp.peer_ca_names;
    return sc->client_ca_names != NULL ? sc->client_ca_names
                                       : s->ctx->client_ca_names;
}

static int add_ca_name(STACK_OF(X509_NAME) **sk, const X509 *x)
{
    X509_NAME *name;

    if (x == NULL)
        return 0;
    if (*sk == NULL && ((*sk = sk_X509_NAME_new_null()) == NULL))
        return 0;

    if ((name = X509_NAME_dup(X509_get_subject_name(x))) == NULL)
        return 0;

    if (!sk_X509_NAME_push(*sk, name)) {
        X509_NAME_free(name);
        return 0;
    }
    return 1;
}

int SSL_add1_to_CA_list(SSL *ssl, const X509 *x)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return 0;

    return add_ca_name(&sc->ca_names, x);
}

int SSL_CTX_add1_to_CA_list(SSL_CTX *ctx, const X509 *x)
{
    return add_ca_name(&ctx->ca_names, x);
}

/*
 * The following two are older names are to be replaced with
 * SSL(_CTX)_add1_to_CA_list
 */
int SSL_add_client_CA(SSL *ssl, X509 *x)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return 0;

    return add_ca_name(&sc->client_ca_names, x);
}

int SSL_CTX_add_client_CA(SSL_CTX *ctx, X509 *x)
{
    return add_ca_name(&ctx->client_ca_names, x);
}

static int xname_cmp(const X509_NAME *a, const X509_NAME *b)
{
    unsigned char *abuf = NULL, *bbuf = NULL;
    int alen, blen, ret;

    /* X509_NAME_cmp() itself casts away constness in this way, so
     * assume it's safe:
     */
    alen = i2d_X509_NAME((X509_NAME *)a, &abuf);
    blen = i2d_X509_NAME((X509_NAME *)b, &bbuf);

    if (alen < 0 || blen < 0)
        ret = -2;
    else if (alen != blen)
        ret = alen - blen;
    else /* alen == blen */
        ret = memcmp(abuf, bbuf, alen);

    OPENSSL_free(abuf);
    OPENSSL_free(bbuf);

    return ret;
}

static int xname_sk_cmp(const X509_NAME *const *a, const X509_NAME *const *b)
{
    return xname_cmp(*a, *b);
}

static unsigned long xname_hash(const X509_NAME *a)
{
    /* This returns 0 also if SHA1 is not available */
    return X509_NAME_hash_ex((X509_NAME *)a, NULL, NULL, NULL);
}

STACK_OF(X509_NAME) *SSL_load_client_CA_file_ex(const char *file,
                                                OSSL_LIB_CTX *libctx,
                                                const char *propq)
{
    BIO *in = BIO_new(BIO_s_file());
    X509 *x = NULL;
    X509_NAME *xn = NULL;
    STACK_OF(X509_NAME) *ret = NULL;
    LHASH_OF(X509_NAME) *name_hash = lh_X509_NAME_new(xname_hash, xname_cmp);
    OSSL_LIB_CTX *prev_libctx = NULL;

    if (file == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        goto err;
    }
    if (name_hash == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }
    if (in == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_BIO_LIB);
        goto err;
    }

    x = X509_new_ex(libctx, propq);
    if (x == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
        goto err;
    }
    if (BIO_read_filename(in, file) <= 0)
        goto err;

    /* Internally lh_X509_NAME_retrieve() needs the libctx to retrieve SHA1 */
    prev_libctx = OSSL_LIB_CTX_set0_default(libctx);
    for (;;) {
        if (PEM_read_bio_X509(in, &x, NULL, NULL) == NULL)
            break;
        if (ret == NULL) {
            ret = sk_X509_NAME_new_null();
            if (ret == NULL) {
                ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
                goto err;
            }
        }
        if ((xn = X509_get_subject_name(x)) == NULL)
            goto err;
        /* check for duplicates */
        xn = X509_NAME_dup(xn);
        if (xn == NULL)
            goto err;
        if (lh_X509_NAME_retrieve(name_hash, xn) != NULL) {
            /* Duplicate. */
            X509_NAME_free(xn);
            xn = NULL;
        } else {
            lh_X509_NAME_insert(name_hash, xn);
            if (!sk_X509_NAME_push(ret, xn))
                goto err;
        }
    }
    goto done;

 err:
    X509_NAME_free(xn);
    sk_X509_NAME_pop_free(ret, X509_NAME_free);
    ret = NULL;
 done:
    /* restore the old libctx */
    OSSL_LIB_CTX_set0_default(prev_libctx);
    BIO_free(in);
    X509_free(x);
    lh_X509_NAME_free(name_hash);
    if (ret != NULL)
        ERR_clear_error();
    return ret;
}

STACK_OF(X509_NAME) *SSL_load_client_CA_file(const char *file)
{
    return SSL_load_client_CA_file_ex(file, NULL, NULL);
}

static int add_file_cert_subjects_to_stack(STACK_OF(X509_NAME) *stack,
                                           const char *file,
                                           LHASH_OF(X509_NAME) *name_hash)
{
    BIO *in;
    X509 *x = NULL;
    X509_NAME *xn = NULL;
    int ret = 1;

    in = BIO_new(BIO_s_file());

    if (in == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_BIO_LIB);
        goto err;
    }

    if (BIO_read_filename(in, file) <= 0)
        goto err;

    for (;;) {
        if (PEM_read_bio_X509(in, &x, NULL, NULL) == NULL)
            break;
        if ((xn = X509_get_subject_name(x)) == NULL)
            goto err;
        xn = X509_NAME_dup(xn);
        if (xn == NULL)
            goto err;
        if (lh_X509_NAME_retrieve(name_hash, xn) != NULL) {
            /* Duplicate. */
            X509_NAME_free(xn);
        } else if (!sk_X509_NAME_push(stack, xn)) {
            X509_NAME_free(xn);
            goto err;
        } else {
            /* Successful insert, add to hash table */
            lh_X509_NAME_insert(name_hash, xn);
        }
    }

    ERR_clear_error();
    goto done;

 err:
    ret = 0;
 done:
    BIO_free(in);
    X509_free(x);
    return ret;
}

int SSL_add_file_cert_subjects_to_stack(STACK_OF(X509_NAME) *stack,
                                        const char *file)
{
    X509_NAME *xn = NULL;
    int ret = 1;
    int idx = 0;
    int num = 0;
    LHASH_OF(X509_NAME) *name_hash = lh_X509_NAME_new(xname_hash, xname_cmp);

    if (file == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        goto err;
    }

    if (name_hash == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

    /*
     * Pre-populate the lhash with the existing entries of the stack, since
     * using the LHASH_OF is much faster for duplicate checking. That's because
     * xname_cmp converts the X509_NAMEs to DER involving a memory allocation
     * for every single invocation of the comparison function.
     */
    num = sk_X509_NAME_num(stack);
    for (idx = 0; idx < num; idx++) {
        xn = sk_X509_NAME_value(stack, idx);
        lh_X509_NAME_insert(name_hash, xn);
    }

    ret = add_file_cert_subjects_to_stack(stack, file, name_hash);
    goto done;

 err:
    ret = 0;
 done:
    lh_X509_NAME_free(name_hash);
    return ret;
}

int SSL_add_dir_cert_subjects_to_stack(STACK_OF(X509_NAME) *stack,
                                       const char *dir)
{
    OPENSSL_DIR_CTX *d = NULL;
    const char *filename;
    int ret = 0;
    X509_NAME *xn = NULL;
    int idx = 0;
    int num = 0;
    LHASH_OF(X509_NAME) *name_hash = lh_X509_NAME_new(xname_hash, xname_cmp);

    if (name_hash == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        goto err;
    }

    /*
     * Pre-populate the lhash with the existing entries of the stack, since
     * using the LHASH_OF is much faster for duplicate checking. That's because
     * xname_cmp converts the X509_NAMEs to DER involving a memory allocation
     * for every single invocation of the comparison function.
     */
    num = sk_X509_NAME_num(stack);
    for (idx = 0; idx < num; idx++) {
        xn = sk_X509_NAME_value(stack, idx);
        lh_X509_NAME_insert(name_hash, xn);
    }

    while ((filename = OPENSSL_DIR_read(&d, dir))) {
        char buf[1024];
        int r;
#ifndef OPENSSL_NO_POSIX_IO
        struct stat st;

#else
        /* Cannot use stat so just skip current and parent directories */
        if (strcmp(filename, ".") == 0 || strcmp(filename, "..") == 0)
            continue;
#endif
        if (strlen(dir) + strlen(filename) + 2 > sizeof(buf)) {
            ERR_raise(ERR_LIB_SSL, SSL_R_PATH_TOO_LONG);
            goto err;
        }
#ifdef OPENSSL_SYS_VMS
        r = BIO_snprintf(buf, sizeof(buf), "%s%s", dir, filename);
#else
        r = BIO_snprintf(buf, sizeof(buf), "%s/%s", dir, filename);
#endif
#ifndef OPENSSL_NO_POSIX_IO
        /* Skip subdirectories */
        if (!stat(buf, &st) && S_ISDIR(st.st_mode))
            continue;
#endif
        if (r <= 0 || r >= (int)sizeof(buf))
            goto err;
        if (!add_file_cert_subjects_to_stack(stack, buf, name_hash))
            goto err;
    }

    if (errno) {
        ERR_raise_data(ERR_LIB_SYS, get_last_sys_error(),
                       "calling OPENSSL_dir_read(%s)", dir);
        ERR_raise(ERR_LIB_SSL, ERR_R_SYS_LIB);
        goto err;
    }

    ret = 1;

 err:
    if (d)
        OPENSSL_DIR_end(&d);
    lh_X509_NAME_free(name_hash);

    return ret;
}

static int add_uris_recursive(STACK_OF(X509_NAME) *stack,
                              const char *uri, int depth)
{
    int ok = 1;
    OSSL_STORE_CTX *ctx = NULL;
    X509 *x = NULL;
    X509_NAME *xn = NULL;
    OSSL_STORE_INFO *info = NULL;

    if ((ctx = OSSL_STORE_open(uri, NULL, NULL, NULL, NULL)) == NULL)
        goto err;

    while (!OSSL_STORE_eof(ctx) && !OSSL_STORE_error(ctx)) {
        int infotype;

        if ((info = OSSL_STORE_load(ctx)) == NULL)
            continue;
        infotype = OSSL_STORE_INFO_get_type(info);

        if (infotype == OSSL_STORE_INFO_NAME) {
            /*
             * This is an entry in the "directory" represented by the current
             * uri.  if |depth| allows, dive into it.
             */
            if (depth > 0)
                ok = add_uris_recursive(stack, OSSL_STORE_INFO_get0_NAME(info),
                                        depth - 1);
        } else if (infotype == OSSL_STORE_INFO_CERT) {
            if ((x = OSSL_STORE_INFO_get0_CERT(info)) == NULL
                || (xn = X509_get_subject_name(x)) == NULL
                || (xn = X509_NAME_dup(xn)) == NULL)
                goto err;
            if (sk_X509_NAME_find(stack, xn) >= 0) {
                /* Duplicate. */
                X509_NAME_free(xn);
            } else if (!sk_X509_NAME_push(stack, xn)) {
                X509_NAME_free(xn);
                goto err;
            }
        }

        OSSL_STORE_INFO_free(info);
        info = NULL;
    }

    ERR_clear_error();
    goto done;

 err:
    ok = 0;
    OSSL_STORE_INFO_free(info);
 done:
    OSSL_STORE_close(ctx);

    return ok;
}

int SSL_add_store_cert_subjects_to_stack(STACK_OF(X509_NAME) *stack,
                                         const char *store)
{
    int (*oldcmp) (const X509_NAME *const *a, const X509_NAME *const *b)
        = sk_X509_NAME_set_cmp_func(stack, xname_sk_cmp);
    int ret = add_uris_recursive(stack, store, 1);

    (void)sk_X509_NAME_set_cmp_func(stack, oldcmp);
    return ret;
}

/* Build a certificate chain for current certificate */
int ssl_build_cert_chain(SSL_CONNECTION *s, SSL_CTX *ctx, int flags)
{
    CERT *c = s != NULL ? s->cert : ctx->cert;
    CERT_PKEY *cpk = c->key;
    X509_STORE *chain_store = NULL;
    X509_STORE_CTX *xs_ctx = NULL;
    STACK_OF(X509) *chain = NULL, *untrusted = NULL;
    X509 *x;
    SSL_CTX *real_ctx = (s == NULL) ? ctx : SSL_CONNECTION_GET_CTX(s);
    int i, rv = 0;

    if (cpk->x509 == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_CERTIFICATE_SET);
        goto err;
    }
    /* Rearranging and check the chain: add everything to a store */
    if (flags & SSL_BUILD_CHAIN_FLAG_CHECK) {
        chain_store = X509_STORE_new();
        if (chain_store == NULL)
            goto err;
        for (i = 0; i < sk_X509_num(cpk->chain); i++) {
            x = sk_X509_value(cpk->chain, i);
            if (!X509_STORE_add_cert(chain_store, x))
                goto err;
        }
        /* Add EE cert too: it might be self signed */
        if (!X509_STORE_add_cert(chain_store, cpk->x509))
            goto err;
    } else {
        if (c->chain_store != NULL)
            chain_store = c->chain_store;
        else
            chain_store = real_ctx->cert_store;

        if (flags & SSL_BUILD_CHAIN_FLAG_UNTRUSTED)
            untrusted = cpk->chain;
    }

    xs_ctx = X509_STORE_CTX_new_ex(real_ctx->libctx, real_ctx->propq);
    if (xs_ctx == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
        goto err;
    }
    if (!X509_STORE_CTX_init(xs_ctx, chain_store, cpk->x509, untrusted)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_X509_LIB);
        goto err;
    }
    /* Set suite B flags if needed */
    X509_STORE_CTX_set_flags(xs_ctx,
                             c->cert_flags & SSL_CERT_FLAG_SUITEB_128_LOS);

    i = X509_verify_cert(xs_ctx);
    if (i <= 0 && flags & SSL_BUILD_CHAIN_FLAG_IGNORE_ERROR) {
        if (flags & SSL_BUILD_CHAIN_FLAG_CLEAR_ERROR)
            ERR_clear_error();
        i = 1;
        rv = 2;
    }
    if (i > 0)
        chain = X509_STORE_CTX_get1_chain(xs_ctx);
    if (i <= 0) {
        i = X509_STORE_CTX_get_error(xs_ctx);
        ERR_raise_data(ERR_LIB_SSL, SSL_R_CERTIFICATE_VERIFY_FAILED,
                       "Verify error:%s", X509_verify_cert_error_string(i));

        goto err;
    }
    /* Remove EE certificate from chain */
    x = sk_X509_shift(chain);
    X509_free(x);
    if (flags & SSL_BUILD_CHAIN_FLAG_NO_ROOT) {
        if (sk_X509_num(chain) > 0) {
            /* See if last cert is self signed */
            x = sk_X509_value(chain, sk_X509_num(chain) - 1);
            if (X509_get_extension_flags(x) & EXFLAG_SS) {
                x = sk_X509_pop(chain);
                X509_free(x);
            }
        }
    }
    /*
     * Check security level of all CA certificates: EE will have been checked
     * already.
     */
    for (i = 0; i < sk_X509_num(chain); i++) {
        x = sk_X509_value(chain, i);
        rv = ssl_security_cert(s, ctx, x, 0, 0);
        if (rv != 1) {
            ERR_raise(ERR_LIB_SSL, rv);
            OSSL_STACK_OF_X509_free(chain);
            rv = 0;
            goto err;
        }
    }
    OSSL_STACK_OF_X509_free(cpk->chain);
    cpk->chain = chain;
    if (rv == 0)
        rv = 1;
 err:
    if (flags & SSL_BUILD_CHAIN_FLAG_CHECK)
        X509_STORE_free(chain_store);
    X509_STORE_CTX_free(xs_ctx);

    return rv;
}

int ssl_cert_set_cert_store(CERT *c, X509_STORE *store, int chain, int ref)
{
    X509_STORE **pstore;

    if (ref && store && !X509_STORE_up_ref(store))
        return 0;

    if (chain)
        pstore = &c->chain_store;
    else
        pstore = &c->verify_store;
    X509_STORE_free(*pstore);
    *pstore = store;

    return 1;
}

int ssl_cert_get_cert_store(CERT *c, X509_STORE **pstore, int chain)
{
    *pstore = (chain ? c->chain_store : c->verify_store);
    return 1;
}

int ssl_get_security_level_bits(const SSL *s, const SSL_CTX *ctx, int *levelp)
{
    int level;
    /*
     * note that there's a corresponding minbits_table
     * in crypto/x509/x509_vfy.c that's used for checking the security level
     * of RSA and DSA keys
     */
    static const int minbits_table[5 + 1] = { 0, 80, 112, 128, 192, 256 };

    if (ctx != NULL)
        level = SSL_CTX_get_security_level(ctx);
    else
        level = SSL_get_security_level(s);

    if (level > 5)
        level = 5;
    else if (level < 0)
        level = 0;

    if (levelp != NULL)
        *levelp = level;

    return minbits_table[level];
}

static int ssl_security_default_callback(const SSL *s, const SSL_CTX *ctx,
                                         int op, int bits, int nid, void *other,
                                         void *ex)
{
    int level, minbits, pfs_mask;
    const SSL_CONNECTION *sc;

    minbits = ssl_get_security_level_bits(s, ctx, &level);

    if (level == 0) {
        /*
         * No EDH keys weaker than 1024-bits even at level 0, otherwise,
         * anything goes.
         */
        if (op == SSL_SECOP_TMP_DH && bits < 80)
            return 0;
        return 1;
    }
    switch (op) {
    case SSL_SECOP_CIPHER_SUPPORTED:
    case SSL_SECOP_CIPHER_SHARED:
    case SSL_SECOP_CIPHER_CHECK:
        {
            const SSL_CIPHER *c = other;
            /* No ciphers below security level */
            if (bits < minbits)
                return 0;
            /* No unauthenticated ciphersuites */
            if (c->algorithm_auth & SSL_aNULL)
                return 0;
            /* No MD5 mac ciphersuites */
            if (c->algorithm_mac & SSL_MD5)
                return 0;
            /* SHA1 HMAC is 160 bits of security */
            if (minbits > 160 && c->algorithm_mac & SSL_SHA1)
                return 0;
            /* Level 3: forward secure ciphersuites only */
            pfs_mask = SSL_kDHE | SSL_kECDHE | SSL_kDHEPSK | SSL_kECDHEPSK;
            if (level >= 3 && c->min_tls != TLS1_3_VERSION &&
                               !(c->algorithm_mkey & pfs_mask))
                return 0;
            break;
        }
    case SSL_SECOP_VERSION:
        if ((sc = SSL_CONNECTION_FROM_CONST_SSL(s)) == NULL)
            return 0;
        if (!SSL_CONNECTION_IS_DTLS(sc)) {
            /* SSLv3, TLS v1.0 and TLS v1.1 only allowed at level 0 */
            if (nid <= TLS1_1_VERSION && level > 0)
                return 0;
        } else {
            /* DTLS v1.0 only allowed at level 0 */
            if (DTLS_VERSION_LT(nid, DTLS1_2_VERSION) && level > 0)
                return 0;
        }
        break;

    case SSL_SECOP_COMPRESSION:
        if (level >= 2)
            return 0;
        break;
    case SSL_SECOP_TICKET:
        if (level >= 3)
            return 0;
        break;
    default:
        if (bits < minbits)
            return 0;
    }
    return 1;
}

int ssl_security(const SSL_CONNECTION *s, int op, int bits, int nid, void *other)
{
    return s->cert->sec_cb(SSL_CONNECTION_GET_USER_SSL(s), NULL, op, bits, nid,
                           other, s->cert->sec_ex);
}

int ssl_ctx_security(const SSL_CTX *ctx, int op, int bits, int nid, void *other)
{
    return ctx->cert->sec_cb(NULL, ctx, op, bits, nid, other,
                             ctx->cert->sec_ex);
}

int ssl_cert_lookup_by_nid(int nid, size_t *pidx, SSL_CTX *ctx)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(ssl_cert_info); i++) {
        if (ssl_cert_info[i].nid == nid) {
            *pidx = i;
            return 1;
        }
    }
    for (i = 0; i < ctx->sigalg_list_len; i++) {
        if (ctx->ssl_cert_info[i].nid == nid) {
            *pidx = SSL_PKEY_NUM + i;
            return 1;
        }
    }
    return 0;
}

const SSL_CERT_LOOKUP *ssl_cert_lookup_by_pkey(const EVP_PKEY *pk, size_t *pidx, SSL_CTX *ctx)
{
    size_t i;

    /* check classic pk types */
    for (i = 0; i < OSSL_NELEM(ssl_cert_info); i++) {
        const SSL_CERT_LOOKUP *tmp_lu = &ssl_cert_info[i];

        if (EVP_PKEY_is_a(pk, OBJ_nid2sn(tmp_lu->nid))
            || EVP_PKEY_is_a(pk, OBJ_nid2ln(tmp_lu->nid))) {
            if (pidx != NULL)
                *pidx = i;
            return tmp_lu;
        }
    }
    /* check provider-loaded pk types */
    for (i = 0; i < ctx->sigalg_list_len; i++) {
        SSL_CERT_LOOKUP *tmp_lu = &(ctx->ssl_cert_info[i]);

        if (EVP_PKEY_is_a(pk, OBJ_nid2sn(tmp_lu->nid))
            || EVP_PKEY_is_a(pk, OBJ_nid2ln(tmp_lu->nid))) {
            if (pidx != NULL)
                *pidx = SSL_PKEY_NUM + i;
            return &ctx->ssl_cert_info[i];
        }
    }

    return NULL;
}

const SSL_CERT_LOOKUP *ssl_cert_lookup_by_idx(size_t idx, SSL_CTX *ctx)
{
    if (idx >= (OSSL_NELEM(ssl_cert_info) + ctx->sigalg_list_len))
        return NULL;
    else if (idx >= (OSSL_NELEM(ssl_cert_info)))
        return &(ctx->ssl_cert_info[idx - SSL_PKEY_NUM]);
    return &ssl_cert_info[idx];
}
