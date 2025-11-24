/*
 * Copyright 2015-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include <limits.h>
#include <errno.h>

#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/conf.h>
#ifndef OPENSSL_NO_ENGINE
# include <openssl/engine.h>
#endif
#include "testutil.h"

#include "internal/nelem.h"

#define _UC(c) ((unsigned char)(c))

static const char *basedomain;
static const char *CAfile;
static const char *tlsafile;

/*
 * Forward declaration, of function that uses internal interfaces, from headers
 * included at the end of this module.
 */
static void store_ctx_dane_init(X509_STORE_CTX *, SSL *);

static int saved_errno;

static void save_errno(void)
{
    saved_errno = errno;
}

static int restore_errno(void)
{
    int ret = errno;
    errno = saved_errno;
    return ret;
}

static int verify_chain(SSL *ssl, STACK_OF(X509) *chain)
{
    X509_STORE_CTX *store_ctx = NULL;
    SSL_CTX *ssl_ctx = NULL;
    X509_STORE *store = NULL;
    int ret = 0;
    int store_ctx_idx = SSL_get_ex_data_X509_STORE_CTX_idx();

    if (!TEST_ptr(store_ctx = X509_STORE_CTX_new())
            || !TEST_ptr(ssl_ctx = SSL_get_SSL_CTX(ssl))
            || !TEST_ptr(store = SSL_CTX_get_cert_store(ssl_ctx))
            || !TEST_true(X509_STORE_CTX_init(store_ctx, store, NULL, chain))
            || !TEST_true(X509_STORE_CTX_set_ex_data(store_ctx, store_ctx_idx,
                                                     ssl)))
        goto end;

    X509_STORE_CTX_set_default(store_ctx, SSL_is_server(ssl)
                               ? "ssl_client" : "ssl_server");
    X509_VERIFY_PARAM_set1(X509_STORE_CTX_get0_param(store_ctx),
                           SSL_get0_param(ssl));
    store_ctx_dane_init(store_ctx, ssl);

    if (SSL_get_verify_callback(ssl) != NULL)
        X509_STORE_CTX_set_verify_cb(store_ctx, SSL_get_verify_callback(ssl));

    /* Mask "internal failures" (-1) from our return value. */
    if (!TEST_int_ge(ret = X509_STORE_CTX_verify(store_ctx), 0))
        ret = 0;

    SSL_set_verify_result(ssl, X509_STORE_CTX_get_error(store_ctx));

end:
    X509_STORE_CTX_free(store_ctx);
    return ret;
}

static STACK_OF(X509) *load_chain(BIO *fp, int nelem)
{
    int count;
    char *name = 0;
    char *header = 0;
    unsigned char *data = 0;
    long len;
    char *errtype = 0; /* if error: cert or pkey? */
    STACK_OF(X509) *chain;
    typedef X509 *(*d2i_X509_t)(X509 **, const unsigned char **, long);

    if (!TEST_ptr(chain = sk_X509_new_null()))
        goto err;

    for (count = 0;
         count < nelem && errtype == 0
         && PEM_read_bio(fp, &name, &header, &data, &len) == 1;
         ++count) {
        if (strcmp(name, PEM_STRING_X509) == 0
                || strcmp(name, PEM_STRING_X509_TRUSTED) == 0
                || strcmp(name, PEM_STRING_X509_OLD) == 0) {
            d2i_X509_t d = strcmp(name, PEM_STRING_X509_TRUSTED) != 0
                ? d2i_X509_AUX : d2i_X509;
            X509 *cert;
            const unsigned char *p = data;

            if (!TEST_ptr(cert = d(0, &p, len))
                    || !TEST_long_eq(p - data, len)) {
                TEST_info("Certificate parsing error");
                goto err;
            }

            if (!TEST_true(sk_X509_push(chain, cert)))
                goto err;
        } else {
            TEST_info("Unknown chain file object %s", name);
            goto err;
        }

        OPENSSL_free(name);
        OPENSSL_free(header);
        OPENSSL_free(data);
        name = header = NULL;
        data = NULL;
    }

    if (count == nelem) {
        ERR_clear_error();
        return chain;
    }

err:
    OPENSSL_free(name);
    OPENSSL_free(header);
    OPENSSL_free(data);
    OSSL_STACK_OF_X509_free(chain);
    return NULL;
}

static char *read_to_eol(BIO *f)
{
    static char buf[4096];
    int n;

    if (BIO_gets(f, buf, sizeof(buf)) <= 0)
        return NULL;

    n = strlen(buf);
    if (buf[n - 1] != '\n') {
        if (n + 1 == sizeof(buf))
            TEST_error("input too long");
        else
            TEST_error("EOF before newline");
        return NULL;
    }

    /* Trim trailing whitespace */
    while (n > 0 && isspace(_UC(buf[n - 1])))
        buf[--n] = '\0';

    return buf;
}

/*
 * Hex decoder that tolerates optional whitespace
 */
static ossl_ssize_t hexdecode(const char *in, void *result)
{
    unsigned char **out = (unsigned char **)result;
    unsigned char *ret;
    unsigned char *cp;
    uint8_t byte;
    int nibble = 0;

    if (!TEST_ptr(ret = OPENSSL_malloc(strlen(in) / 2)))
        return -1;
    cp = ret;

    for (byte = 0; *in; ++in) {
        int x;

        if (isspace(_UC(*in)))
            continue;
        x = OPENSSL_hexchar2int(*in);
        if (x < 0) {
            OPENSSL_free(ret);
            return 0;
        }
        byte |= (char)x;
        if ((nibble ^= 1) == 0) {
            *cp++ = byte;
            byte = 0;
        } else {
            byte <<= 4;
        }
    }
    if (nibble != 0) {
        OPENSSL_free(ret);
        return 0;
    }

    return cp - (*out = ret);
}

static ossl_ssize_t checked_uint8(const char *in, void *out)
{
    uint8_t *result = (uint8_t *)out;
    const char *cp = in;
    char *endp;
    long v;
    int e;

    save_errno();
    v = strtol(cp, &endp, 10);
    e = restore_errno();

    if (((v == LONG_MIN || v == LONG_MAX) && e == ERANGE) ||
        endp == cp || !isspace(_UC(*endp)) ||
        v != (*(uint8_t *)result = (uint8_t) v)) {
        return -1;
    }
    for (cp = endp; isspace(_UC(*cp)); ++cp)
        continue;
    return cp - in;
}

struct tlsa_field {
    void *var;
    const char *name;
    ossl_ssize_t (*parser)(const char *, void *);
};

static int tlsa_import_rr(SSL *ssl, const char *rrdata)
{
    static uint8_t usage;
    static uint8_t selector;
    static uint8_t mtype;
    static unsigned char *data = NULL;
    static struct tlsa_field tlsa_fields[] = {
        { &usage, "usage", checked_uint8 },
        { &selector, "selector", checked_uint8 },
        { &mtype, "mtype", checked_uint8 },
        { &data, "data", hexdecode },
        { NULL, }
    };
    int ret;
    struct tlsa_field *f;
    const char *cp = rrdata;
    ossl_ssize_t len = 0;

    for (f = tlsa_fields; f->var; ++f) {
        if ((len = f->parser(cp += len, f->var)) <= 0) {
            TEST_info("bad TLSA %s field in: %s", f->name, rrdata);
            return 0;
        }
    }

    ret = SSL_dane_tlsa_add(ssl, usage, selector, mtype, data, len);
    OPENSSL_free(data);
    if (ret == 0) {
        TEST_info("unusable TLSA rrdata: %s", rrdata);
        return 0;
    }
    if (ret < 0) {
        TEST_info("error loading TLSA rrdata: %s", rrdata);
        return 0;
    }

    return ret;
}

static int allws(const char *cp)
{
    while (*cp)
        if (!isspace(_UC(*cp++)))
            return 0;
    return 1;
}

static int test_tlsafile(SSL_CTX *ctx, const char *base_name,
                         BIO *f, const char *path)
{
    char *line;
    int testno = 0;
    int ret = 1;
    SSL *ssl;

    while (ret > 0 && (line = read_to_eol(f)) != NULL) {
        STACK_OF(X509) *chain;
        int ntlsa;
        int ncert;
        int noncheck;
        int want;
        int want_depth;
        int off;
        int i;
        int ok;
        int err;
        int mdpth;

        if (*line == '\0' || *line == '#')
            continue;

        ++testno;
        if (sscanf(line, "%d %d %d %d %d%n",
                   &ntlsa, &ncert, &noncheck, &want, &want_depth, &off) != 5
            || !allws(line + off)) {
            TEST_error("Malformed line for test %d", testno);
            return 0;
        }

        if (!TEST_ptr(ssl = SSL_new(ctx)))
            return 0;
        SSL_set_connect_state(ssl);
        if (SSL_dane_enable(ssl, base_name) <= 0) {
            SSL_free(ssl);
            return 0;
        }
        if (noncheck)
            SSL_dane_set_flags(ssl, DANE_FLAG_NO_DANE_EE_NAMECHECKS);

        for (i = 0; i < ntlsa; ++i) {
            if ((line = read_to_eol(f)) == NULL || !tlsa_import_rr(ssl, line)) {
                SSL_free(ssl);
                return 0;
            }
        }

        /* Don't report old news */
        ERR_clear_error();
        if (!TEST_ptr(chain = load_chain(f, ncert))) {
            SSL_free(ssl);
            return 0;
        }

        ok = verify_chain(ssl, chain);
        OSSL_STACK_OF_X509_free(chain);
        err = SSL_get_verify_result(ssl);
        /*
         * Peek under the hood, normally TLSA match data is hidden when
         * verification fails, we can obtain any suppressed data by setting the
         * verification result to X509_V_OK before looking.
         */
        SSL_set_verify_result(ssl, X509_V_OK);
        mdpth = SSL_get0_dane_authority(ssl, NULL, NULL);
        /* Not needed any more, but lead by example and put the error back. */
        SSL_set_verify_result(ssl, err);
        SSL_free(ssl);

        if (!TEST_int_eq(err, want)) {
            if (want == X509_V_OK)
                TEST_info("Verification failure in test %d: %d=%s",
                          testno, err, X509_verify_cert_error_string(err));
            else
                TEST_info("Unexpected error in test %d", testno);
            ret = 0;
            continue;
        }
        if (!TEST_false(want == 0 && ok == 0)) {
            TEST_info("Verification failure in test %d: ok=0", testno);
            ret = 0;
            continue;
        }
        if (!TEST_int_eq(mdpth, want_depth)) {
            TEST_info("In test test %d", testno);
            ret = 0;
        }
    }
    ERR_clear_error();

    return ret;
}

static int run_tlsatest(void)
{
    SSL_CTX *ctx = NULL;
    BIO *f = NULL;
    int ret = 0;

    if (!TEST_ptr(f = BIO_new_file(tlsafile, "r"))
            || !TEST_ptr(ctx = SSL_CTX_new(TLS_client_method()))
            || !TEST_int_gt(SSL_CTX_dane_enable(ctx), 0)
            || !TEST_true(SSL_CTX_load_verify_file(ctx, CAfile))
            || !TEST_int_gt(SSL_CTX_dane_mtype_set(ctx, EVP_sha512(), 2, 1), 0)
            || !TEST_int_gt(SSL_CTX_dane_mtype_set(ctx, EVP_sha256(), 1, 2), 0)
            || !TEST_int_gt(test_tlsafile(ctx, basedomain, f, tlsafile), 0))
        goto end;
    ret = 1;

end:
    BIO_free(f);
    SSL_CTX_free(ctx);

    return ret;
}

OPT_TEST_DECLARE_USAGE("basedomain CAfile tlsafile\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(basedomain = test_get_argument(0))
            || !TEST_ptr(CAfile = test_get_argument(1))
            || !TEST_ptr(tlsafile = test_get_argument(2)))
        return 0;

    ADD_TEST(run_tlsatest);
    return 1;
}

#include "internal/dane.h"

static void store_ctx_dane_init(X509_STORE_CTX *store_ctx, SSL *ssl)
{
    X509_STORE_CTX_set0_dane(store_ctx, SSL_get0_dane(ssl));
}
