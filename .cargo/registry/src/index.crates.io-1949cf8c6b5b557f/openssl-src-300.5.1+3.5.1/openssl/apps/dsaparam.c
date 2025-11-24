/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/opensslconf.h>

#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <string.h>
#include "apps.h"
#include "progs.h"
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/bn.h>
#include <openssl/dsa.h>
#include <openssl/x509.h>
#include <openssl/pem.h>

static int verbose = 0;

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_INFORM, OPT_OUTFORM, OPT_IN, OPT_OUT, OPT_TEXT,
    OPT_NOOUT, OPT_GENKEY, OPT_ENGINE, OPT_VERBOSE, OPT_QUIET,
    OPT_R_ENUM, OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS dsaparam_options[] = {
    {OPT_HELP_STR, 1, '-', "Usage: %s [options] [numbits] [numqbits]\n"},

    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine e, possibly a hardware device"},
#endif

    OPT_SECTION("Input"),
    {"in", OPT_IN, '<', "Input file"},
    {"inform", OPT_INFORM, 'F', "Input format - DER or PEM"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file"},
    {"outform", OPT_OUTFORM, 'F', "Output format - DER or PEM"},
    {"text", OPT_TEXT, '-', "Print as text"},
    {"noout", OPT_NOOUT, '-', "No output"},
    {"verbose", OPT_VERBOSE, '-', "Verbose output"},
    {"quiet", OPT_QUIET, '-', "Terse output"},
    {"genkey", OPT_GENKEY, '-', "Generate a DSA key"},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,

    OPT_PARAMETERS(),
    {"numbits", 0, 0, "Number of bits if generating parameters or key (optional)"},
    {"numqbits", 0, 0, "Number of bits in the subprime parameter q if generating parameters or key (optional)"},
    {NULL}
};

int dsaparam_main(int argc, char **argv)
{
    ENGINE *e = NULL;
    BIO *out = NULL;
    EVP_PKEY *params = NULL, *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    int numbits = -1, numqbits = -1, num = 0, genkey = 0;
    int informat = FORMAT_UNDEF, outformat = FORMAT_PEM, noout = 0;
    int ret = 1, i, text = 0, private = 0;
    char *infile = NULL, *outfile = NULL, *prog;
    OPTION_CHOICE o;

    prog = opt_init(argc, argv, dsaparam_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(dsaparam_options);
            ret = 0;
            goto end;
        case OPT_INFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PEMDER, &informat))
                goto opthelp;
            break;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUTFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PEMDER, &outformat))
                goto opthelp;
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
        case OPT_GENKEY:
            genkey = 1;
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_VERBOSE:
            verbose = 1;
            break;
        case OPT_QUIET:
            verbose = 0;
            break;
        }
    }

    /* Optional args are bitsize and q bitsize. */
    argc = opt_num_rest();
    argv = opt_rest();
    if (argc == 2) {
        if (!opt_int(argv[0], &num) || num < 0)
            goto opthelp;
        if (!opt_int(argv[1], &numqbits) || numqbits < 0)
            goto opthelp;
    } else if (argc == 1) {
        if (!opt_int(argv[0], &num) || num < 0)
            goto opthelp;
    } else if (!opt_check_rest_arg(NULL)) {
        goto opthelp;
    }
    if (!app_RAND_load())
        goto end;

    /* generate a key */
    numbits = num;
    private = genkey ? 1 : 0;

    ctx = EVP_PKEY_CTX_new_from_name(app_get0_libctx(), "DSA", app_get0_propq());
    if (ctx == NULL) {
        BIO_printf(bio_err,
                   "Error, DSA parameter generation context allocation failed\n");
        goto end;
    }
    if (numbits > 0) {
        if (numbits > OPENSSL_DSA_MAX_MODULUS_BITS)
            BIO_printf(bio_err,
                       "Warning: It is not recommended to use more than %d bit for DSA keys.\n"
                       "         Your key size is %d! Larger key size may behave not as expected.\n",
                       OPENSSL_DSA_MAX_MODULUS_BITS, numbits);

        EVP_PKEY_CTX_set_app_data(ctx, bio_err);
        if (verbose) {
            EVP_PKEY_CTX_set_cb(ctx, progress_cb);
            BIO_printf(bio_err, "Generating DSA parameters, %d bit long prime\n",
                       num);
            BIO_printf(bio_err, "This could take some time\n");
        }
        if (EVP_PKEY_paramgen_init(ctx) <= 0) {
            BIO_printf(bio_err,
                       "Error, DSA key generation paramgen init failed\n");
            goto end;
        }
        if (EVP_PKEY_CTX_set_dsa_paramgen_bits(ctx, num) <= 0) {
            BIO_printf(bio_err,
                       "Error, DSA key generation setting bit length failed\n");
            goto end;
        }
        if (numqbits > 0) {
            if (EVP_PKEY_CTX_set_dsa_paramgen_q_bits(ctx, numqbits) <= 0) {
                BIO_printf(bio_err,
                        "Error, DSA key generation setting subprime bit length failed\n");
                goto end;
            }
        }
        params = app_paramgen(ctx, "DSA");
    } else {
        params = load_keyparams(infile, informat, 1, "DSA", "DSA parameters");
    }
    if (params == NULL) {
        /* Error message should already have been displayed */
        goto end;
    }

    out = bio_open_owner(outfile, outformat, private);
    if (out == NULL)
        goto end;

    if (text) {
        EVP_PKEY_print_params(out, params, 0, NULL);
    }

    if (outformat == FORMAT_ASN1 && genkey)
        noout = 1;

    if (!noout) {
        if (outformat == FORMAT_ASN1)
            i = i2d_KeyParams_bio(out, params);
        else
            i = PEM_write_bio_Parameters(out, params);
        if (!i) {
            BIO_printf(bio_err, "Error, unable to write DSA parameters\n");
            goto end;
        }
    }
    if (genkey) {
        EVP_PKEY_CTX_free(ctx);
        ctx = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(), params,
                app_get0_propq());
        if (ctx == NULL) {
            BIO_printf(bio_err,
                       "Error, DSA key generation context allocation failed\n");
            goto end;
        }
        if (EVP_PKEY_keygen_init(ctx) <= 0) {
            BIO_printf(bio_err,
                       "Error, unable to initialise for key generation\n");
            goto end;
        }
        pkey = app_keygen(ctx, "DSA", numbits, verbose);
        if (pkey == NULL)
            goto end;
        assert(private);
        if (outformat == FORMAT_ASN1)
            i = i2d_PrivateKey_bio(out, pkey);
        else
            i = PEM_write_bio_PrivateKey(out, pkey, NULL, NULL, 0, NULL, NULL);
    }
    ret = 0;
 end:
    if (ret != 0)
        ERR_print_errors(bio_err);
    BIO_free_all(out);
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    EVP_PKEY_free(params);
    release_engine(e);
    return ret;
}
