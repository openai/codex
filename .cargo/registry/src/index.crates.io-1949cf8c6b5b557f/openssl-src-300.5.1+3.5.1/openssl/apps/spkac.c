/*
 * Copyright 1999-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "apps.h"
#include "progs.h"
#include <openssl/bio.h>
#include <openssl/conf.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/pem.h>

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_NOOUT, OPT_PUBKEY, OPT_VERIFY, OPT_IN, OPT_OUT,
    OPT_ENGINE, OPT_KEY, OPT_CHALLENGE, OPT_PASSIN, OPT_SPKAC,
    OPT_SPKSECT, OPT_KEYFORM, OPT_DIGEST,
    OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS spkac_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"spksect", OPT_SPKSECT, 's',
     "Specify the name of an SPKAC-dedicated section of configuration"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif

    OPT_SECTION("Input"),
    {"in", OPT_IN, '<', "Input file"},
    {"key", OPT_KEY, '<', "Create SPKAC using private key"},
    {"keyform", OPT_KEYFORM, 'f', "Private key file format (ENGINE, other values ignored)"},
    {"passin", OPT_PASSIN, 's', "Input file pass phrase source"},
    {"challenge", OPT_CHALLENGE, 's', "Challenge string"},
    {"spkac", OPT_SPKAC, 's', "Alternative SPKAC name"},

    OPT_SECTION("Output"),
    {"digest", OPT_DIGEST, 's', "Sign new SPKAC with the specified digest (default: MD5)" },
    {"out", OPT_OUT, '>', "Output file"},
    {"noout", OPT_NOOUT, '-', "Don't print SPKAC"},
    {"pubkey", OPT_PUBKEY, '-', "Output public key"},
    {"verify", OPT_VERIFY, '-', "Verify SPKAC signature"},

    OPT_PROV_OPTIONS,
    {NULL}
};

int spkac_main(int argc, char **argv)
{
    BIO *out = NULL;
    CONF *conf = NULL;
    ENGINE *e = NULL;
    EVP_PKEY *pkey = NULL;
    NETSCAPE_SPKI *spki = NULL;
    char *challenge = NULL, *keyfile = NULL;
    char *infile = NULL, *outfile = NULL, *passinarg = NULL, *passin = NULL;
    char *spkstr = NULL, *prog;
    const char *spkac = "SPKAC", *spksect = "default";
    const char *digest = "MD5";
    EVP_MD *md = NULL;
    int i, ret = 1, verify = 0, noout = 0, pubkey = 0;
    int keyformat = FORMAT_UNDEF;
    OPTION_CHOICE o;

    prog = opt_init(argc, argv, spkac_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(spkac_options);
            ret = 0;
            goto end;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_PUBKEY:
            pubkey = 1;
            break;
        case OPT_VERIFY:
            verify = 1;
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_KEY:
            keyfile = opt_arg();
            break;
        case OPT_KEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &keyformat))
                goto opthelp;
            break;
        case OPT_CHALLENGE:
            challenge = opt_arg();
            break;
        case OPT_SPKAC:
            spkac = opt_arg();
            break;
        case OPT_SPKSECT:
            spksect = opt_arg();
            break;
        case OPT_DIGEST:
            digest = opt_arg();
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
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

    if (!app_passwd(passinarg, NULL, &passin, NULL)) {
        BIO_printf(bio_err, "Error getting password\n");
        goto end;
    }

    if (keyfile != NULL) {
        if (!opt_md(digest, &md))
            goto end;

        pkey = load_key(strcmp(keyfile, "-") ? keyfile : NULL,
                        keyformat, 1, passin, e, "private key");
        if (pkey == NULL)
            goto end;
        spki = NETSCAPE_SPKI_new();
        if (spki == NULL)
            goto end;
        if (challenge != NULL
            && !ASN1_STRING_set(spki->spkac->challenge,
                                challenge, (int)strlen(challenge)))
            goto end;
        if (!NETSCAPE_SPKI_set_pubkey(spki, pkey)) {
            BIO_printf(bio_err, "Error setting public key\n");
            goto end;
        }
        i = NETSCAPE_SPKI_sign(spki, pkey, md);
        if (i <= 0) {
            BIO_printf(bio_err, "Error signing SPKAC\n");
            goto end;
        }
        spkstr = NETSCAPE_SPKI_b64_encode(spki);
        if (spkstr == NULL)
            goto end;

        out = bio_open_default(outfile, 'w', FORMAT_TEXT);
        if (out == NULL) {
            OPENSSL_free(spkstr);
            goto end;
        }
        BIO_printf(out, "SPKAC=%s\n", spkstr);
        OPENSSL_free(spkstr);
        ret = 0;
        goto end;
    }

    if ((conf = app_load_config(infile)) == NULL)
        goto end;

    spkstr = NCONF_get_string(conf, spksect, spkac);

    if (spkstr == NULL) {
        BIO_printf(bio_err, "Can't find SPKAC called \"%s\"\n", spkac);
        ERR_print_errors(bio_err);
        goto end;
    }

    spki = NETSCAPE_SPKI_b64_decode(spkstr, -1);

    if (spki == NULL) {
        BIO_printf(bio_err, "Error loading SPKAC\n");
        ERR_print_errors(bio_err);
        goto end;
    }

    out = bio_open_default(outfile, 'w', FORMAT_TEXT);
    if (out == NULL)
        goto end;

    if (!noout)
        NETSCAPE_SPKI_print(out, spki);
    pkey = NETSCAPE_SPKI_get_pubkey(spki);
    if (verify) {
        i = NETSCAPE_SPKI_verify(spki, pkey);
        if (i > 0) {
            BIO_printf(bio_err, "Signature OK\n");
        } else {
            BIO_printf(bio_err, "Signature Failure\n");
            ERR_print_errors(bio_err);
            goto end;
        }
    }
    if (pubkey)
        PEM_write_bio_PUBKEY(out, pkey);

    ret = 0;

 end:
    EVP_MD_free(md);
    NCONF_free(conf);
    NETSCAPE_SPKI_free(spki);
    BIO_free_all(out);
    EVP_PKEY_free(pkey);
    release_engine(e);
    OPENSSL_free(passin);
    return ret;
}
