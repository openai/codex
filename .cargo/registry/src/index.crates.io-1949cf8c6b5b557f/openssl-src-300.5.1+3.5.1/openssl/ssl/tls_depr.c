/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine and HMAC deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/engine.h>
#include "ssl_local.h"
#include "internal/ssl_unwrap.h"

/*
 * Engine APIs are only used to support applications that still use ENGINEs.
 * Once ENGINE is removed completely, all of this code can also be removed.
 */

#ifndef OPENSSL_NO_ENGINE
void tls_engine_finish(ENGINE *e)
{
    ENGINE_finish(e);
}
#endif

const EVP_CIPHER *tls_get_cipher_from_engine(int nid)
{
    const EVP_CIPHER *ret = NULL;
#ifndef OPENSSL_NO_ENGINE
    ENGINE *eng;

    /*
     * If there is an Engine available for this cipher we use the "implicit"
     * form to ensure we use that engine later.
     */
    eng = ENGINE_get_cipher_engine(nid);
    if (eng != NULL) {
        ret = ENGINE_get_cipher(eng, nid);
        ENGINE_finish(eng);
    }
#endif
    return ret;
}

const EVP_MD *tls_get_digest_from_engine(int nid)
{
    const EVP_MD *ret = NULL;
#ifndef OPENSSL_NO_ENGINE
    ENGINE *eng;

    /*
     * If there is an Engine available for this digest we use the "implicit"
     * form to ensure we use that engine later.
     */
    eng = ENGINE_get_digest_engine(nid);
    if (eng != NULL) {
        ret = ENGINE_get_digest(eng, nid);
        ENGINE_finish(eng);
    }
#endif
    return ret;
}

#ifndef OPENSSL_NO_ENGINE
int tls_engine_load_ssl_client_cert(SSL_CONNECTION *s, X509 **px509,
                                    EVP_PKEY **ppkey)
{
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    return ENGINE_load_ssl_client_cert(SSL_CONNECTION_GET_CTX(s)->client_cert_engine,
                                       ssl,
                                       SSL_get_client_CA_list(ssl),
                                       px509, ppkey, NULL, NULL, NULL);
}
#endif

#ifndef OPENSSL_NO_ENGINE
int SSL_CTX_set_client_cert_engine(SSL_CTX *ctx, ENGINE *e)
{
    if (!ENGINE_init(e)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_ENGINE_LIB);
        return 0;
    }
    if (!ENGINE_get_ssl_client_cert_function(e)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_CLIENT_CERT_METHOD);
        ENGINE_finish(e);
        return 0;
    }
    ctx->client_cert_engine = e;
    return 1;
}
#endif

/*
 * The HMAC APIs below are only used to support the deprecated public API
 * macro SSL_CTX_set_tlsext_ticket_key_cb(). The application supplied callback
 * takes an HMAC_CTX in its argument list. The preferred alternative is
 * SSL_CTX_set_tlsext_ticket_key_evp_cb(). Once
 * SSL_CTX_set_tlsext_ticket_key_cb() is removed, then all of this code can also
 * be removed.
 */
#ifndef OPENSSL_NO_DEPRECATED_3_0
int ssl_hmac_old_new(SSL_HMAC *ret)
{
    ret->old_ctx = HMAC_CTX_new();
    if (ret->old_ctx == NULL)
        return 0;

    return 1;
}

void ssl_hmac_old_free(SSL_HMAC *ctx)
{
    HMAC_CTX_free(ctx->old_ctx);
}

int ssl_hmac_old_init(SSL_HMAC *ctx, void *key, size_t len, char *md)
{
    return HMAC_Init_ex(ctx->old_ctx, key, len, EVP_get_digestbyname(md), NULL);
}

int ssl_hmac_old_update(SSL_HMAC *ctx, const unsigned char *data, size_t len)
{
    return HMAC_Update(ctx->old_ctx, data, len);
}

int ssl_hmac_old_final(SSL_HMAC *ctx, unsigned char *md, size_t *len)
{
    unsigned int l;

    if (HMAC_Final(ctx->old_ctx, md, &l) > 0) {
        if (len != NULL)
            *len = l;
        return 1;
    }

    return 0;
}

size_t ssl_hmac_old_size(const SSL_HMAC *ctx)
{
    return HMAC_size(ctx->old_ctx);
}

HMAC_CTX *ssl_hmac_get0_HMAC_CTX(SSL_HMAC *ctx)
{
    return ctx->old_ctx;
}

/* Some deprecated public APIs pass DH objects */
EVP_PKEY *ssl_dh_to_pkey(DH *dh)
{
# ifndef OPENSSL_NO_DH
    EVP_PKEY *ret;

    if (dh == NULL)
        return NULL;
    ret = EVP_PKEY_new();
    if (EVP_PKEY_set1_DH(ret, dh) <= 0) {
        EVP_PKEY_free(ret);
        return NULL;
    }
    return ret;
# else
    return NULL;
# endif
}

/* Some deprecated public APIs pass EC_KEY objects */
int ssl_set_tmp_ecdh_groups(uint16_t **pext, size_t *pextlen,
                            uint16_t **ksext, size_t *ksextlen,
                            size_t **tplext, size_t *tplextlen,
                            void *key)
{
# ifndef OPENSSL_NO_EC
    const EC_GROUP *group = EC_KEY_get0_group((const EC_KEY *)key);
    int nid;

    if (group == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_MISSING_PARAMETERS);
        return 0;
    }
    nid = EC_GROUP_get_curve_name(group);
    if (nid == NID_undef)
        return 0;
    return tls1_set_groups(pext, pextlen,
                           ksext, ksextlen,
                           tplext, tplextlen,
                           &nid, 1);
# else
    return 0;
# endif
}

/*
 * Set the callback for generating temporary DH keys.
 * ctx: the SSL context.
 * dh: the callback
 */
# if !defined(OPENSSL_NO_DH)
void SSL_CTX_set_tmp_dh_callback(SSL_CTX *ctx,
                                 DH *(*dh) (SSL *ssl, int is_export,
                                            int keylength))
{
    SSL_CTX_callback_ctrl(ctx, SSL_CTRL_SET_TMP_DH_CB, (void (*)(void))dh);
}

void SSL_set_tmp_dh_callback(SSL *ssl, DH *(*dh) (SSL *ssl, int is_export,
                                                  int keylength))
{
    SSL_callback_ctrl(ssl, SSL_CTRL_SET_TMP_DH_CB, (void (*)(void))dh);
}
# endif
#endif /* OPENSSL_NO_DEPRECATED */
