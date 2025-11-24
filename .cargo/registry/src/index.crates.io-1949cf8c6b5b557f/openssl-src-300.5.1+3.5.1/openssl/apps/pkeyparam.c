/*
 * Copyright 2006-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include "apps.h"
#include "progs.h"
#include <openssl/pem.h>
#include <openssl/err.h>
#include <openssl/evp.h>

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_IN, OPT_OUT, OPT_TEXT, OPT_NOOUT,
    OPT_ENGINE, OPT_CHECK,
    OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS pkeyparam_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif
    {"check", OPT_CHECK, '-', "Check key param consistency"},

    OPT_SECTION("Input"),
    {"in", OPT_IN, '<', "Input file"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file"},
    {"text", OPT_TEXT, '-', "Print parameters as text"},
    {"noout", OPT_NOOUT, '-', "Don't output encoded parameters"},

    OPT_PROV_OPTIONS,
    {NULL}
};

int pkeyparam_main(int argc, char **argv)
{
    ENGINE *e = NULL;
    BIO *in = NULL, *out = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    int text = 0, noout = 0, ret = EXIT_FAILURE, check = 0, r;
    OPTION_CHOICE o;
    char *infile = NULL, *outfile = NULL, *prog;

    prog = opt_init(argc, argv, pkeyparam_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(pkeyparam_options);
            ret = 0;
            goto end;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_TEXT:
            text = 1;
            break;
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_CHECK:
            check = 1;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* No extra arguments. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    in = bio_open_default(infile, 'r', FORMAT_PEM);
    if (in == NULL)
        goto end;
    pkey = PEM_read_bio_Parameters_ex(in, NULL, app_get0_libctx(),
                                      app_get0_propq());
    if (pkey == NULL) {
        BIO_printf(bio_err, "Error reading parameters\n");
        ERR_print_errors(bio_err);
        goto end;
    }
    out = bio_open_default(outfile, 'w', FORMAT_PEM);
    if (out == NULL)
        goto end;

    if (check) {
        if (e == NULL)
            ctx = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(), pkey,
                                             app_get0_propq());
        else
            ctx = EVP_PKEY_CTX_new(pkey, e);
        if (ctx == NULL) {
            ERR_print_errors(bio_err);
            goto end;
        }

        r = EVP_PKEY_param_check(ctx);

        if (r == 1) {
            BIO_printf(out, "Parameters are valid\n");
        } else {
            /*
             * Note: at least for RSA keys if this function returns
             * -1, there will be no error reasons.
             */
            BIO_printf(bio_err, "Parameters are invalid\n");
            ERR_print_errors(bio_err);
            goto end;
        }
    }

    if (!noout)
        PEM_write_bio_Parameters(out, pkey);

    if (text)
        EVP_PKEY_print_params(out, pkey, 0, NULL);

    ret = EXIT_SUCCESS;

 end:
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    release_engine(e);
    BIO_free_all(out);
    BIO_free(in);

    return ret;
}
