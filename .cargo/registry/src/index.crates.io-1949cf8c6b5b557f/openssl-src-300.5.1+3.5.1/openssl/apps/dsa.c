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
#include <string.h>
#include <time.h>
#include "apps.h"
#include "progs.h"
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/dsa.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/pem.h>
#include <openssl/bn.h>
#include <openssl/encoder.h>
#include <openssl/core_names.h>
#include <openssl/core_dispatch.h>

#ifndef OPENSSL_NO_RC4
# define DEFAULT_PVK_ENCR_STRENGTH      2
#else
# define DEFAULT_PVK_ENCR_STRENGTH      0
#endif

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_INFORM, OPT_OUTFORM, OPT_IN, OPT_OUT, OPT_ENGINE,
    /* Do not change the order here; see case statements below */
    OPT_PVK_NONE, OPT_PVK_WEAK, OPT_PVK_STRONG,
    OPT_NOOUT, OPT_TEXT, OPT_MODULUS, OPT_PUBIN,
    OPT_PUBOUT, OPT_CIPHER, OPT_PASSIN, OPT_PASSOUT,
    OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS dsa_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"", OPT_CIPHER, '-', "Any supported cipher"},
#ifndef OPENSSL_NO_RC4
    {"pvk-strong", OPT_PVK_STRONG, '-', "Enable 'Strong' PVK encoding level (default)"},
    {"pvk-weak", OPT_PVK_WEAK, '-', "Enable 'Weak' PVK encoding level"},
    {"pvk-none", OPT_PVK_NONE, '-', "Don't enforce PVK encoding"},
#endif
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine e, possibly a hardware device"},
#endif

    OPT_SECTION("Input"),
    {"in", OPT_IN, 's', "Input key"},
    {"inform", OPT_INFORM, 'f', "Input format (DER/PEM/PVK); has no effect"},
    {"pubin", OPT_PUBIN, '-', "Expect a public key in input file"},
    {"passin", OPT_PASSIN, 's', "Input file pass phrase source"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file"},
    {"outform", OPT_OUTFORM, 'f', "Output format, DER PEM PVK"},
    {"noout", OPT_NOOUT, '-', "Don't print key out"},
    {"text", OPT_TEXT, '-', "Print the key in text"},
    {"modulus", OPT_MODULUS, '-', "Print the DSA public value"},
    {"pubout", OPT_PUBOUT, '-', "Output public key, not private"},
    {"passout", OPT_PASSOUT, 's', "Output file pass phrase source"},

    OPT_PROV_OPTIONS,
    {NULL}
};

int dsa_main(int argc, char **argv)
{
    BIO *out = NULL;
    ENGINE *e = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_CIPHER *enc = NULL;
    char *infile = NULL, *outfile = NULL, *prog;
    char *passin = NULL, *passout = NULL, *passinarg = NULL, *passoutarg = NULL;
    OPTION_CHOICE o;
    int informat = FORMAT_UNDEF, outformat = FORMAT_PEM, text = 0, noout = 0;
    int modulus = 0, pubin = 0, pubout = 0, ret = 1;
    int pvk_encr = DEFAULT_PVK_ENCR_STRENGTH;
    int private = 0;
    const char *output_type = NULL, *ciphername = NULL;
    const char *output_structure = NULL;
    int selection = 0;
    OSSL_ENCODER_CTX *ectx = NULL;

    opt_set_unknown_name("cipher");
    prog = opt_init(argc, argv, dsa_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            ret = 0;
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(dsa_options);
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
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &outformat))
                goto opthelp;
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_PASSOUT:
            passoutarg = opt_arg();
            break;
        case OPT_PVK_STRONG:    /* pvk_encr:= 2 */
        case OPT_PVK_WEAK:      /* pvk_encr:= 1 */
        case OPT_PVK_NONE:      /* pvk_encr:= 0 */
#ifndef OPENSSL_NO_RC4
            pvk_encr = (o - OPT_PVK_NONE);
#endif
            break;
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_TEXT:
            text = 1;
            break;
        case OPT_MODULUS:
            modulus = 1;
            break;
        case OPT_PUBIN:
            pubin = 1;
            break;
        case OPT_PUBOUT:
            pubout = 1;
            break;
        case OPT_CIPHER:
            ciphername = opt_unknown();
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* No extra args. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    if (!opt_cipher(ciphername, &enc))
        goto end;
    private = !pubin && (!pubout || text);

    if (!app_passwd(passinarg, passoutarg, &passin, &passout)) {
        BIO_printf(bio_err, "Error getting passwords\n");
        goto end;
    }

    BIO_printf(bio_err, "read DSA key\n");
    if (pubin)
        pkey = load_pubkey(infile, informat, 1, passin, e, "public key");
    else
        pkey = load_key(infile, informat, 1, passin, e, "private key");

    if (pkey == NULL) {
        BIO_printf(bio_err, "unable to load Key\n");
        ERR_print_errors(bio_err);
        goto end;
    }
    if (!EVP_PKEY_is_a(pkey, "DSA")) {
        BIO_printf(bio_err, "Not a DSA key\n");
        goto end;
    }

    out = bio_open_owner(outfile, outformat, private);
    if (out == NULL)
        goto end;

    if (text) {
        assert(pubin || private);
        if ((pubin && EVP_PKEY_print_public(out, pkey, 0, NULL) <= 0)
            || (!pubin && EVP_PKEY_print_private(out, pkey, 0, NULL) <= 0)) {
            perror(outfile);
            ERR_print_errors(bio_err);
            goto end;
        }
    }

    if (modulus) {
        BIGNUM *pub_key = NULL;

        if (!EVP_PKEY_get_bn_param(pkey, "pub", &pub_key)) {
            ERR_print_errors(bio_err);
            goto end;
        }
        BIO_printf(out, "Public Key=");
        BN_print(out, pub_key);
        BIO_printf(out, "\n");
        BN_free(pub_key);
    }

    if (noout) {
        ret = 0;
        goto end;
    }
    BIO_printf(bio_err, "writing DSA key\n");
    if (outformat == FORMAT_ASN1) {
        output_type = "DER";
    } else if (outformat == FORMAT_PEM) {
        output_type = "PEM";
    } else if (outformat == FORMAT_MSBLOB) {
        output_type = "MSBLOB";
    } else if (outformat == FORMAT_PVK) {
        if (pubin) {
            BIO_printf(bio_err, "PVK form impossible with public key input\n");
            goto end;
        }
        output_type = "PVK";
    } else {
        BIO_printf(bio_err, "bad output format specified for outfile\n");
        goto end;
    }

    if (outformat == FORMAT_ASN1 || outformat == FORMAT_PEM) {
        if (pubout || pubin)
            output_structure = "SubjectPublicKeyInfo";
        else
            output_structure = "type-specific";
    }

    /* Select what you want in the output */
    if (pubout || pubin) {
        selection = OSSL_KEYMGMT_SELECT_PUBLIC_KEY;
    } else {
        assert(private);
        selection = (OSSL_KEYMGMT_SELECT_KEYPAIR
                     | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS);
    }

    /* Perform the encoding */
    ectx = OSSL_ENCODER_CTX_new_for_pkey(pkey, selection, output_type,
                                         output_structure, NULL);
    if (OSSL_ENCODER_CTX_get_num_encoders(ectx) == 0) {
        BIO_printf(bio_err, "%s format not supported\n", output_type);
        goto end;
    }

    /* Passphrase setup */
    if (enc != NULL)
        OSSL_ENCODER_CTX_set_cipher(ectx, EVP_CIPHER_get0_name(enc), NULL);

    /* Default passphrase prompter */
    if (enc != NULL || outformat == FORMAT_PVK) {
        OSSL_ENCODER_CTX_set_passphrase_ui(ectx, get_ui_method(), NULL);
        if (passout != NULL)
            /* When passout given, override the passphrase prompter */
            OSSL_ENCODER_CTX_set_passphrase(ectx,
                                            (const unsigned char *)passout,
                                            strlen(passout));
    }

    /* PVK requires a bit more */
    if (outformat == FORMAT_PVK) {
        OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };

        params[0] = OSSL_PARAM_construct_int("encrypt-level", &pvk_encr);
        if (!OSSL_ENCODER_CTX_set_params(ectx, params)) {
            BIO_printf(bio_err, "invalid PVK encryption level\n");
            goto end;
        }
    }

    if (!OSSL_ENCODER_to_bio(ectx, out)) {
        BIO_printf(bio_err, "unable to write key\n");
        goto end;
    }
    ret = 0;
 end:
    if (ret != 0)
        ERR_print_errors(bio_err);
    OSSL_ENCODER_CTX_free(ectx);
    BIO_free_all(out);
    EVP_PKEY_free(pkey);
    EVP_CIPHER_free(enc);
    release_engine(e);
    OPENSSL_free(passin);
    OPENSSL_free(passout);
    return ret;
}
