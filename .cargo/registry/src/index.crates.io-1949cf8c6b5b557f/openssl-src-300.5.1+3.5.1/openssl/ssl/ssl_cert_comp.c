/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "ssl_local.h"
#include "internal/e_os.h"
#include "internal/refcount.h"
#include "internal/ssl_unwrap.h"

size_t ossl_calculate_comp_expansion(int alg, size_t length)
{
    size_t ret;
    /*
     * Uncompressibility expansion:
     * ZLIB: N + 11 + 5 * (N >> 14)
     * Brotli: per RFC7932: N + 5 + 3 * (N >> 16)
     * ZSTD: N + 4 + 14 + 3 * (N >> 17) + 4
     */

    switch (alg) {
    case TLSEXT_comp_cert_zlib:
        ret = length + 11 + 5 * (length >> 14);
        break;
    case TLSEXT_comp_cert_brotli:
        ret = length + 5 + 3 * (length >> 16);
        break;
    case TLSEXT_comp_cert_zstd:
        ret = length + 22 + 3 * (length >> 17);
        break;
    default:
        return 0;
    }
    /* Check for overflow */
    if (ret < length)
        return 0;
    return ret;
}

int ossl_comp_has_alg(int a)
{
#ifndef OPENSSL_NO_COMP_ALG
    /* 0 means "any" algorithm */
    if ((a == 0 || a == TLSEXT_comp_cert_brotli) && BIO_f_brotli() != NULL)
        return 1;
    if ((a == 0 || a == TLSEXT_comp_cert_zstd) && BIO_f_zstd() != NULL)
        return 1;
    if ((a == 0 || a == TLSEXT_comp_cert_zlib) && BIO_f_zlib() != NULL)
        return 1;
#endif
    return 0;
}

/* New operation Helper routine */
#ifndef OPENSSL_NO_COMP_ALG
static OSSL_COMP_CERT *OSSL_COMP_CERT_new(unsigned char *data, size_t len, size_t orig_len, int alg)
{
    OSSL_COMP_CERT *ret = NULL;

    if (!ossl_comp_has_alg(alg)
            || data == NULL
            || (ret = OPENSSL_zalloc(sizeof(*ret))) == NULL
            || !CRYPTO_NEW_REF(&ret->references, 1))
        goto err;

    ret->data = data;
    ret->len = len;
    ret->orig_len = orig_len;
    ret->alg = alg;
    return ret;
 err:
    ERR_raise(ERR_LIB_SSL, ERR_R_MALLOC_FAILURE);
    OPENSSL_free(data);
    OPENSSL_free(ret);
    return NULL;
}

__owur static OSSL_COMP_CERT *OSSL_COMP_CERT_from_compressed_data(unsigned char *data, size_t len,
                                                                  size_t orig_len, int alg)
{
    return OSSL_COMP_CERT_new(OPENSSL_memdup(data, len), len, orig_len, alg);
}

__owur static OSSL_COMP_CERT *OSSL_COMP_CERT_from_uncompressed_data(unsigned char *data, size_t len,
                                                                    int alg)
{
    OSSL_COMP_CERT *ret = NULL;
    size_t max_length;
    int comp_length;
    COMP_METHOD *method;
    unsigned char *comp_data = NULL;
    COMP_CTX *comp_ctx = NULL;

    switch (alg) {
    case TLSEXT_comp_cert_brotli:
        method = COMP_brotli_oneshot();
        break;
    case TLSEXT_comp_cert_zlib:
        method = COMP_zlib_oneshot();
        break;
    case TLSEXT_comp_cert_zstd:
        method = COMP_zstd_oneshot();
        break;
    default:
        goto err;
    }

    if ((max_length = ossl_calculate_comp_expansion(alg, len)) == 0
          || method == NULL
          || (comp_ctx = COMP_CTX_new(method)) == NULL
          || (comp_data = OPENSSL_zalloc(max_length)) == NULL)
        goto err;

    comp_length = COMP_compress_block(comp_ctx, comp_data, max_length, data, len);
    if (comp_length <= 0)
        goto err;

    ret = OSSL_COMP_CERT_new(comp_data, comp_length, len, alg);
    comp_data = NULL;

 err:
    OPENSSL_free(comp_data);
    COMP_CTX_free(comp_ctx);
    return ret;
}

void OSSL_COMP_CERT_free(OSSL_COMP_CERT *cc)
{
    int i;

    if (cc == NULL)
        return;

    CRYPTO_DOWN_REF(&cc->references, &i);
    REF_PRINT_COUNT("OSSL_COMP_CERT", i, cc);
    if (i > 0)
        return;
    REF_ASSERT_ISNT(i < 0);

    OPENSSL_free(cc->data);
    CRYPTO_FREE_REF(&cc->references);
    OPENSSL_free(cc);
}
int OSSL_COMP_CERT_up_ref(OSSL_COMP_CERT *cc)
{
    int i;

    if (CRYPTO_UP_REF(&cc->references, &i) <= 0)
        return 0;

    REF_PRINT_COUNT("OSSL_COMP_CERT", i, cc);
    REF_ASSERT_ISNT(i < 2);
    return ((i > 1) ? 1 : 0);
}

static int ssl_set_cert_comp_pref(int *prefs, int *algs, size_t len)
{
    size_t j = 0;
    size_t i;
    int found = 0;
    int already_set[TLSEXT_comp_cert_limit];
    int tmp_prefs[TLSEXT_comp_cert_limit];

    /* Note that |len| is the number of |algs| elements */
    /* clear all algorithms */
    if (len == 0 || algs == NULL) {
        memset(prefs, 0, sizeof(tmp_prefs));
        return 1;
    }

    /* This will 0-terminate the array */
    memset(tmp_prefs, 0, sizeof(tmp_prefs));
    memset(already_set, 0, sizeof(already_set));
    /* Include only those algorithms we support, ignoring duplicates and unknowns */
    for (i = 0; i < len; i++) {
        if (algs[i] != 0 && ossl_comp_has_alg(algs[i])) {
            /* Check for duplicate */
            if (already_set[algs[i]])
                return 0;
            tmp_prefs[j++] = algs[i];
            already_set[algs[i]] = 1;
            found = 1;
        }
    }
    if (found)
        memcpy(prefs, tmp_prefs, sizeof(tmp_prefs));
    return found;
}

static size_t ssl_get_cert_to_compress(SSL *ssl, CERT_PKEY *cpk, unsigned char **data)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);
    WPACKET tmppkt;
    BUF_MEM buf = { 0 };
    size_t ret = 0;

    if (sc == NULL
            || cpk == NULL
            || !sc->server
            || !SSL_in_before(ssl))
        return 0;

    /* Use the |tmppkt| for the to-be-compressed data */
    if (!WPACKET_init(&tmppkt, &buf))
        goto out;

    /* no context present, add 0-length context */
    if (!WPACKET_put_bytes_u8(&tmppkt, 0))
        goto out;

    /*
     * ssl3_output_cert_chain() may generate an SSLfatal() error,
     * for this case, we want to ignore it, argument for_comp = 1
     */
    if (!ssl3_output_cert_chain(sc, &tmppkt, cpk, 1))
        goto out;
    WPACKET_get_total_written(&tmppkt, &ret);

 out:
    WPACKET_cleanup(&tmppkt);
    if (ret != 0 && data != NULL)
        *data = (unsigned char *)buf.data;
    else
        OPENSSL_free(buf.data);
    return ret;
}

static int ssl_compress_one_cert(SSL *ssl, CERT_PKEY *cpk, int alg)
{
    unsigned char *cert_data = NULL;
    OSSL_COMP_CERT *comp_cert = NULL;
    size_t length;

    if (cpk == NULL
            || alg == TLSEXT_comp_cert_none
            || !ossl_comp_has_alg(alg))
        return 0;

    if ((length = ssl_get_cert_to_compress(ssl, cpk, &cert_data)) == 0)
        return 0;
    comp_cert = OSSL_COMP_CERT_from_uncompressed_data(cert_data, length, alg);
    OPENSSL_free(cert_data);
    if (comp_cert == NULL)
        return 0;

    OSSL_COMP_CERT_free(cpk->comp_cert[alg]);
    cpk->comp_cert[alg] = comp_cert;
    return 1;
}

/* alg_in can be 0, meaning any/all algorithms */
static int ssl_compress_certs(SSL *ssl, CERT_PKEY *cpks, int alg_in)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);
    int i;
    int j;
    int alg;
    int count = 0;

    if (sc == NULL
            || cpks == NULL
            || !ossl_comp_has_alg(alg_in))
        return 0;

    /* Look through the preferences to see what we have */
    for (i = 0; i < TLSEXT_comp_cert_limit; i++) {
        /*
         * alg = 0 means compress for everything, but only for algorithms enabled
         * alg != 0 means compress for that algorithm if enabled
         */
        alg = sc->cert_comp_prefs[i];
        if ((alg_in == 0 && alg != TLSEXT_comp_cert_none)
                || (alg_in != 0 && alg == alg_in)) {

            for (j = 0; j < SSL_PKEY_NUM; j++) {
                /* No cert, move on */
                if (cpks[j].x509 == NULL)
                    continue;

                if (!ssl_compress_one_cert(ssl, &cpks[j], alg))
                    return 0;

                /* if the cert expanded, set the value in the CERT_PKEY to NULL */
                if (cpks[j].comp_cert[alg]->len >= cpks[j].comp_cert[alg]->orig_len) {
                    OSSL_COMP_CERT_free(cpks[j].comp_cert[alg]);
                    cpks[j].comp_cert[alg] = NULL;
                } else {
                    count++;
                }
            }
        }
    }
    return (count > 0);
}

static size_t ssl_get_compressed_cert(SSL *ssl, CERT_PKEY *cpk, int alg, unsigned char **data,
                                      size_t *orig_len)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);
    size_t cert_len = 0;
    size_t comp_len = 0;
    unsigned char *cert_data = NULL;
    OSSL_COMP_CERT *comp_cert = NULL;

    if (sc == NULL
            || cpk == NULL
            || data == NULL
            || orig_len == NULL
            || !sc->server
            || !SSL_in_before(ssl)
            || !ossl_comp_has_alg(alg))
        return 0;

    if ((cert_len = ssl_get_cert_to_compress(ssl, cpk, &cert_data)) == 0)
        goto err;

    comp_cert = OSSL_COMP_CERT_from_uncompressed_data(cert_data, cert_len, alg);
    OPENSSL_free(cert_data);
    if (comp_cert == NULL)
        goto err;

    comp_len = comp_cert->len;
    *orig_len = comp_cert->orig_len;
    *data = comp_cert->data;
    comp_cert->data = NULL;
 err:
    OSSL_COMP_CERT_free(comp_cert);
    return comp_len;
}

static int ossl_set1_compressed_cert(CERT *cert, int algorithm,
                                     unsigned char *comp_data, size_t comp_length,
                                     size_t orig_length)
{
    OSSL_COMP_CERT *comp_cert;

    /* No explicit cert set */
    if (cert == NULL || cert->key == NULL)
        return 0;

    comp_cert = OSSL_COMP_CERT_from_compressed_data(comp_data, comp_length,
                                                    orig_length, algorithm);
    if (comp_cert == NULL)
        return 0;

    OSSL_COMP_CERT_free(cert->key->comp_cert[algorithm]);
    cert->key->comp_cert[algorithm] = comp_cert;

    return 1;
}
#endif

/*-
 * Public API
 */
int SSL_CTX_set1_cert_comp_preference(SSL_CTX *ctx, int *algs, size_t len)
{
#ifndef OPENSSL_NO_COMP_ALG
    return ssl_set_cert_comp_pref(ctx->cert_comp_prefs, algs, len);
#else
    return 0;
#endif
}

int SSL_set1_cert_comp_preference(SSL *ssl, int *algs, size_t len)
{
#ifndef OPENSSL_NO_COMP_ALG
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL)
        return 0;
    return ssl_set_cert_comp_pref(sc->cert_comp_prefs, algs, len);
#else
    return 0;
#endif
}

int SSL_compress_certs(SSL *ssl, int alg)
{
#ifndef OPENSSL_NO_COMP_ALG
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    if (sc == NULL || sc->cert == NULL)
        return 0;

    return ssl_compress_certs(ssl, sc->cert->pkeys, alg);
#endif
    return 0;
}

int SSL_CTX_compress_certs(SSL_CTX *ctx, int alg)
{
    int ret = 0;
#ifndef OPENSSL_NO_COMP_ALG
    SSL *new = SSL_new(ctx);

    if (new == NULL)
        return 0;

    ret = ssl_compress_certs(new, ctx->cert->pkeys, alg);
    SSL_free(new);
#endif
    return ret;
}

size_t SSL_get1_compressed_cert(SSL *ssl, int alg, unsigned char **data, size_t *orig_len)
{
#ifndef OPENSSL_NO_COMP_ALG
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);
    CERT_PKEY *cpk = NULL;

    if (sc == NULL)
        return 0;

    if (sc->cert != NULL)
        cpk = sc->cert->key;
    else
        cpk = ssl->ctx->cert->key;

    return ssl_get_compressed_cert(ssl, cpk, alg, data, orig_len);
#else
    return 0;
#endif
}

size_t SSL_CTX_get1_compressed_cert(SSL_CTX *ctx, int alg, unsigned char **data, size_t *orig_len)
{
#ifndef OPENSSL_NO_COMP_ALG
    size_t ret;
    SSL *new = SSL_new(ctx);

    ret = ssl_get_compressed_cert(new, ctx->cert->key, alg, data, orig_len);
    SSL_free(new);
    return ret;
#else
        return 0;
#endif
}

int SSL_CTX_set1_compressed_cert(SSL_CTX *ctx, int algorithm, unsigned char *comp_data,
                                 size_t comp_length, size_t orig_length)
{
#ifndef OPENSSL_NO_COMP_ALG
    return ossl_set1_compressed_cert(ctx->cert, algorithm, comp_data, comp_length, orig_length);
#else
    return 0;
#endif
}

int SSL_set1_compressed_cert(SSL *ssl, int algorithm, unsigned char *comp_data,
                             size_t comp_length, size_t orig_length)
{
#ifndef OPENSSL_NO_COMP_ALG
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(ssl);

    /* Cannot set a pre-compressed certificate on a client */
    if (sc == NULL || !sc->server)
        return 0;

    return ossl_set1_compressed_cert(sc->cert, algorithm, comp_data, comp_length, orig_length);
#else
    return 0;
#endif
}
