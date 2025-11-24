/*
 * Copyright 2002-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/opensslconf.h>
#include <openssl/evp.h>
#include <openssl/encoder.h>
#include <openssl/decoder.h>
#include <openssl/core_names.h>
#include <openssl/core_dispatch.h>
#include <openssl/params.h>
#include <openssl/err.h>

#include "apps.h"
#include "progs.h"
#include "ec_common.h"

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_INFORM, OPT_OUTFORM, OPT_ENGINE, OPT_IN, OPT_OUT,
    OPT_NOOUT, OPT_TEXT, OPT_PARAM_OUT, OPT_PUBIN, OPT_PUBOUT,
    OPT_PASSIN, OPT_PASSOUT, OPT_PARAM_ENC, OPT_CONV_FORM, OPT_CIPHER,
    OPT_NO_PUBLIC, OPT_CHECK, OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS ec_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif

    OPT_SECTION("Input"),
    {"in", OPT_IN, 's', "Input file"},
    {"inform", OPT_INFORM, 'f', "Input format (DER/PEM/P12/ENGINE)"},
    {"pubin", OPT_PUBIN, '-', "Expect a public key in input file"},
    {"passin", OPT_PASSIN, 's', "Input file pass phrase source"},
    {"check", OPT_CHECK, '-', "check key consistency"},
    {"", OPT_CIPHER, '-', "Any supported cipher"},
    {"param_enc", OPT_PARAM_ENC, 's',
     "Specifies the way the ec parameters are encoded"},
    {"conv_form", OPT_CONV_FORM, 's', "Specifies the point conversion form "},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file"},
    {"outform", OPT_OUTFORM, 'F', "Output format - DER or PEM"},
    {"noout", OPT_NOOUT, '-', "Don't print key out"},
    {"text", OPT_TEXT, '-', "Print the key"},
    {"param_out", OPT_PARAM_OUT, '-', "Print the elliptic curve parameters"},
    {"pubout", OPT_PUBOUT, '-', "Output public key, not private"},
    {"no_public", OPT_NO_PUBLIC, '-', "exclude public key from private key"},
    {"passout", OPT_PASSOUT, 's', "Output file pass phrase source"},

    OPT_PROV_OPTIONS,
    {NULL}
};

int ec_main(int argc, char **argv)
{
    OSSL_ENCODER_CTX *ectx = NULL;
    OSSL_DECODER_CTX *dctx = NULL;
    EVP_PKEY_CTX *pctx = NULL;
    EVP_PKEY *eckey = NULL;
    BIO *out = NULL;
    ENGINE *e = NULL;
    EVP_CIPHER *enc = NULL;
    char *infile = NULL, *outfile = NULL, *ciphername = NULL, *prog;
    char *passin = NULL, *passout = NULL, *passinarg = NULL, *passoutarg = NULL;
    OPTION_CHOICE o;
    int informat = FORMAT_UNDEF, outformat = FORMAT_PEM, text = 0, noout = 0;
    int pubin = 0, pubout = 0, param_out = 0, ret = 1, private = 0;
    int check = 0;
    char *asn1_encoding = NULL;
    char *point_format = NULL;
    int no_public = 0;

    opt_set_unknown_name("cipher");
    prog = opt_init(argc, argv, ec_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(ec_options);
            ret = 0;
            goto end;
        case OPT_INFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &informat))
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
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_TEXT:
            text = 1;
            break;
        case OPT_PARAM_OUT:
            param_out = 1;
            break;
        case OPT_PUBIN:
            pubin = 1;
            break;
        case OPT_PUBOUT:
            pubout = 1;
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_PASSOUT:
            passoutarg = opt_arg();
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_CIPHER:
            ciphername = opt_unknown();
            break;
        case OPT_CONV_FORM:
            point_format = opt_arg();
            if (!opt_string(point_format, point_format_options))
                goto opthelp;
            break;
        case OPT_PARAM_ENC:
            asn1_encoding = opt_arg();
            if (!opt_string(asn1_encoding, asn1_encoding_options))
                goto opthelp;
            break;
        case OPT_NO_PUBLIC:
            no_public = 1;
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

    if (!opt_cipher(ciphername, &enc))
        goto opthelp;
    private = !pubin && (text || (!param_out && !pubout));

    if (!app_passwd(passinarg, passoutarg, &passin, &passout)) {
        BIO_printf(bio_err, "Error getting passwords\n");
        goto end;
    }

    BIO_printf(bio_err, "read EC key\n");

    if (pubin)
        eckey = load_pubkey(infile, informat, 1, passin, e, "public key");
    else
        eckey = load_key(infile, informat, 1, passin, e, "private key");

    if (eckey == NULL) {
        BIO_printf(bio_err, "unable to load Key\n");
        goto end;
    }

    out = bio_open_owner(outfile, outformat, private);
    if (out == NULL)
        goto end;

    if (point_format
        && !EVP_PKEY_set_utf8_string_param(
                eckey, OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT,
                point_format)) {
        BIO_printf(bio_err, "unable to set point conversion format\n");
        goto end;
    }

    if (asn1_encoding != NULL
        && !EVP_PKEY_set_utf8_string_param(
                eckey, OSSL_PKEY_PARAM_EC_ENCODING, asn1_encoding)) {
        BIO_printf(bio_err, "unable to set asn1 encoding format\n");
        goto end;
    }

    if (no_public) {
        if (!EVP_PKEY_set_int_param(eckey, OSSL_PKEY_PARAM_EC_INCLUDE_PUBLIC, 0)) {
            BIO_printf(bio_err, "unable to disable public key encoding\n");
            goto end;
        }
    } else {
        if (!EVP_PKEY_set_int_param(eckey, OSSL_PKEY_PARAM_EC_INCLUDE_PUBLIC, 1)) {
            BIO_printf(bio_err, "unable to enable public key encoding\n");
            goto end;
        }
    }

    if (text) {
        assert(pubin || private);
        if ((pubin && EVP_PKEY_print_public(out, eckey, 0, NULL) <= 0)
            || (!pubin && EVP_PKEY_print_private(out, eckey, 0, NULL) <= 0)) {
            BIO_printf(bio_err, "unable to print EC key\n");
            goto end;
        }
    }

    if (check) {
        pctx = EVP_PKEY_CTX_new_from_pkey(NULL, eckey, NULL);
        if (pctx == NULL) {
            BIO_printf(bio_err, "unable to check EC key\n");
            goto end;
        }
        if (EVP_PKEY_check(pctx) <= 0)
            BIO_printf(bio_err, "EC Key Invalid!\n");
        else
            BIO_printf(bio_err, "EC Key valid.\n");
        ERR_print_errors(bio_err);
    }

    if (!noout) {
        int selection;
        const char *output_type = outformat == FORMAT_ASN1 ? "DER" : "PEM";
        const char *output_structure = "type-specific";

        BIO_printf(bio_err, "writing EC key\n");
        if (param_out) {
            selection = OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS;
        } else if (pubin || pubout) {
            selection = OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS
                | OSSL_KEYMGMT_SELECT_PUBLIC_KEY;
            output_structure = "SubjectPublicKeyInfo";
        } else {
            selection = OSSL_KEYMGMT_SELECT_ALL;
            assert(private);
        }

        ectx = OSSL_ENCODER_CTX_new_for_pkey(eckey, selection,
                                             output_type, output_structure,
                                             NULL);
        if (enc != NULL) {
            OSSL_ENCODER_CTX_set_cipher(ectx, EVP_CIPHER_get0_name(enc), NULL);
            /* Default passphrase prompter */
            OSSL_ENCODER_CTX_set_passphrase_ui(ectx, get_ui_method(), NULL);
            if (passout != NULL)
                /* When passout given, override the passphrase prompter */
                OSSL_ENCODER_CTX_set_passphrase(ectx,
                                                (const unsigned char *)passout,
                                                strlen(passout));
        }
        if (!OSSL_ENCODER_to_bio(ectx, out)) {
            BIO_printf(bio_err, "unable to write EC key\n");
            goto end;
        }
    }

    ret = 0;
end:
    if (ret != 0)
        ERR_print_errors(bio_err);
    BIO_free_all(out);
    EVP_PKEY_free(eckey);
    EVP_CIPHER_free(enc);
    OSSL_ENCODER_CTX_free(ectx);
    OSSL_DECODER_CTX_free(dctx);
    EVP_PKEY_CTX_free(pctx);
    release_engine(e);
    if (passin != NULL)
        OPENSSL_clear_free(passin, strlen(passin));
    if (passout != NULL)
        OPENSSL_clear_free(passout, strlen(passout));
    return ret;
}
