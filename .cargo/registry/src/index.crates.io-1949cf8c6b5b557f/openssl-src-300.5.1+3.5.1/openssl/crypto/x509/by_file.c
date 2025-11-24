/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <time.h>
#include <errno.h>

#include "internal/cryptlib.h"
#include <openssl/buffer.h>
#include <openssl/x509.h>
#include <openssl/pem.h>
#include "x509_local.h"

static int by_file_ctrl(X509_LOOKUP *ctx, int cmd, const char *argc,
                        long argl, char **ret);
static int by_file_ctrl_ex(X509_LOOKUP *ctx, int cmd, const char *argc,
                           long argl, char **ret, OSSL_LIB_CTX *libctx,
                           const char *propq);


static X509_LOOKUP_METHOD x509_file_lookup = {
    "Load file into cache",
    NULL,                       /* new_item */
    NULL,                       /* free */
    NULL,                       /* init */
    NULL,                       /* shutdown */
    by_file_ctrl,               /* ctrl */
    NULL,                       /* get_by_subject */
    NULL,                       /* get_by_issuer_serial */
    NULL,                       /* get_by_fingerprint */
    NULL,                       /* get_by_alias */
    NULL,                       /* get_by_subject_ex */
    by_file_ctrl_ex,            /* ctrl_ex */
};

X509_LOOKUP_METHOD *X509_LOOKUP_file(void)
{
    return &x509_file_lookup;
}

static int by_file_ctrl_ex(X509_LOOKUP *ctx, int cmd, const char *argp,
                           long argl, char **ret, OSSL_LIB_CTX *libctx,
                           const char *propq)
{
    int ok = 0;
    const char *file;

    switch (cmd) {
    case X509_L_FILE_LOAD:
        if (argl == X509_FILETYPE_DEFAULT) {
            file = ossl_safe_getenv(X509_get_default_cert_file_env());
            if (file)
                ok = (X509_load_cert_crl_file_ex(ctx, file, X509_FILETYPE_PEM,
                                                 libctx, propq) != 0);
            else
                ok = (X509_load_cert_crl_file_ex(
                         ctx, X509_get_default_cert_file(),
                         X509_FILETYPE_PEM, libctx, propq) != 0);

            if (!ok)
                ERR_raise(ERR_LIB_X509, X509_R_LOADING_DEFAULTS);
        } else {
            if (argl == X509_FILETYPE_PEM)
                ok = (X509_load_cert_crl_file_ex(ctx, argp, X509_FILETYPE_PEM,
                                                 libctx, propq) != 0);
            else
                ok = (X509_load_cert_file_ex(ctx, argp, (int)argl, libctx,
                                             propq) != 0);
        }
        break;
    }
    return ok;
}

static int by_file_ctrl(X509_LOOKUP *ctx, int cmd,
                        const char *argp, long argl, char **ret)
{
    return by_file_ctrl_ex(ctx, cmd, argp, argl, ret, NULL, NULL);
}

int X509_load_cert_file_ex(X509_LOOKUP *ctx, const char *file, int type,
                           OSSL_LIB_CTX *libctx, const char *propq)
{
    BIO *in = NULL;
    int count = 0;
    X509 *x = NULL;

    if (file == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        goto err;
    }

    in = BIO_new(BIO_s_file());

    if ((in == NULL) || (BIO_read_filename(in, file) <= 0)) {
        ERR_raise(ERR_LIB_X509, ERR_R_BIO_LIB);
        goto err;
    }

    x = X509_new_ex(libctx, propq);
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        goto err;
    }

    if (type == X509_FILETYPE_PEM) {
        for (;;) {
            ERR_set_mark();
            if (PEM_read_bio_X509_AUX(in, &x, NULL, "") == NULL) {
                if ((ERR_GET_REASON(ERR_peek_last_error()) ==
                     PEM_R_NO_START_LINE) && (count > 0)) {
                    ERR_pop_to_mark();
                    break;
                } else {
                    ERR_clear_last_mark();
                    if (count == 0) {
                        ERR_raise(ERR_LIB_X509, X509_R_NO_CERTIFICATE_FOUND);
                    } else {
                        ERR_raise(ERR_LIB_X509, ERR_R_PEM_LIB);
                        count = 0;
                    }
                    goto err;
                }
            }
            ERR_clear_last_mark();
            if (!X509_STORE_add_cert(ctx->store_ctx, x)) {
                count = 0;
                goto err;
            }
            /*
             * X509_STORE_add_cert() added a reference rather than a copy,
             * so we need a fresh X509 object.
             */
            X509_free(x);
            x = X509_new_ex(libctx, propq);
            if (x == NULL) {
                ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
                count = 0;
                goto err;
            }
            count++;
        }
    } else if (type == X509_FILETYPE_ASN1) {
        if (d2i_X509_bio(in, &x) == NULL) {
            ERR_raise(ERR_LIB_X509, X509_R_NO_CERTIFICATE_FOUND);
            goto err;
        }
        count = X509_STORE_add_cert(ctx->store_ctx, x);
    } else {
        ERR_raise(ERR_LIB_X509, X509_R_BAD_X509_FILETYPE);
        goto err;
    }
 err:
    X509_free(x);
    BIO_free(in);
    return count;
}

int X509_load_cert_file(X509_LOOKUP *ctx, const char *file, int type)
{
    return X509_load_cert_file_ex(ctx, file, type, NULL, NULL);
}

int X509_load_crl_file(X509_LOOKUP *ctx, const char *file, int type)
{
    BIO *in = NULL;
    int count = 0;
    X509_CRL *x = NULL;

    if (file == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        goto err;
    }

    in = BIO_new(BIO_s_file());

    if ((in == NULL) || (BIO_read_filename(in, file) <= 0)) {
        ERR_raise(ERR_LIB_X509, ERR_R_BIO_LIB);
        goto err;
    }

    if (type == X509_FILETYPE_PEM) {
        for (;;) {
            x = PEM_read_bio_X509_CRL(in, NULL, NULL, "");
            if (x == NULL) {
                if ((ERR_GET_REASON(ERR_peek_last_error()) ==
                     PEM_R_NO_START_LINE) && (count > 0)) {
                    ERR_clear_error();
                    break;
                } else {
                    if (count == 0) {
                        ERR_raise(ERR_LIB_X509, X509_R_NO_CRL_FOUND);
                    } else {
                        ERR_raise(ERR_LIB_X509, ERR_R_PEM_LIB);
                        count = 0;
                    }
                    goto err;
                }
            }
            if (!X509_STORE_add_crl(ctx->store_ctx, x)) {
                count = 0;
                goto err;
            }
            count++;
            X509_CRL_free(x);
            x = NULL;
        }
    } else if (type == X509_FILETYPE_ASN1) {
        x = d2i_X509_CRL_bio(in, NULL);
        if (x == NULL) {
            ERR_raise(ERR_LIB_X509, X509_R_NO_CRL_FOUND);
            goto err;
        }
        count = X509_STORE_add_crl(ctx->store_ctx, x);
    } else {
        ERR_raise(ERR_LIB_X509, X509_R_BAD_X509_FILETYPE);
        goto err;
    }
 err:
    X509_CRL_free(x);
    BIO_free(in);
    return count;
}

int X509_load_cert_crl_file_ex(X509_LOOKUP *ctx, const char *file, int type,
                               OSSL_LIB_CTX *libctx, const char *propq)
{
    STACK_OF(X509_INFO) *inf = NULL;
    X509_INFO *itmp = NULL;
    BIO *in = NULL;
    int i, count = 0;

    if (type != X509_FILETYPE_PEM)
        return X509_load_cert_file_ex(ctx, file, type, libctx, propq);
#if defined(OPENSSL_SYS_WINDOWS)
    in = BIO_new_file(file, "rb");
#else
    in = BIO_new_file(file, "r");
#endif
    if (in == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_BIO_LIB);
        return 0;
    }
    inf = PEM_X509_INFO_read_bio_ex(in, NULL, NULL, "", libctx, propq);
    BIO_free(in);
    if (inf == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PEM_LIB);
        return 0;
    }
    for (i = 0; i < sk_X509_INFO_num(inf); i++) {
        itmp = sk_X509_INFO_value(inf, i);
        if (itmp->x509) {
            if (!X509_STORE_add_cert(ctx->store_ctx, itmp->x509)) {
                count = 0;
                goto err;
            }
            count++;
        }
        if (itmp->crl) {
            if (!X509_STORE_add_crl(ctx->store_ctx, itmp->crl)) {
                count = 0;
                goto err;
            }
            count++;
        }
    }
    if (count == 0)
        ERR_raise(ERR_LIB_X509, X509_R_NO_CERTIFICATE_OR_CRL_FOUND);
 err:
    sk_X509_INFO_pop_free(inf, X509_INFO_free);
    return count;
}

int X509_load_cert_crl_file(X509_LOOKUP *ctx, const char *file, int type)
{
    return X509_load_cert_crl_file_ex(ctx, file, type, NULL, NULL);
}

