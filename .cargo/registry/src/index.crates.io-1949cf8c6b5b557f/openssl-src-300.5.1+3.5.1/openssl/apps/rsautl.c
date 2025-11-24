/*
 * Copyright 2000-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/opensslconf.h>

#include "apps.h"
#include "progs.h"
#include <string.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/rsa.h>

#define RSA_SIGN        1
#define RSA_VERIFY      2
#define RSA_ENCRYPT     3
#define RSA_DECRYPT     4

#define KEY_PRIVKEY     1
#define KEY_PUBKEY      2
#define KEY_CERT        3

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_ENGINE, OPT_IN, OPT_OUT, OPT_ASN1PARSE, OPT_HEXDUMP,
    OPT_RSA_RAW, OPT_OAEP, OPT_PKCS, OPT_X931,
    OPT_SIGN, OPT_VERIFY, OPT_REV, OPT_ENCRYPT, OPT_DECRYPT,
    OPT_PUBIN, OPT_CERTIN, OPT_INKEY, OPT_PASSIN, OPT_KEYFORM,
    OPT_R_ENUM, OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS rsautl_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"sign", OPT_SIGN, '-', "Sign with private key"},
    {"verify", OPT_VERIFY, '-', "Verify with public key"},
    {"encrypt", OPT_ENCRYPT, '-', "Encrypt with public key"},
    {"decrypt", OPT_DECRYPT, '-', "Decrypt with private key"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif

    OPT_SECTION("Input"),
    {"in", OPT_IN, '<', "Input file"},
    {"inkey", OPT_INKEY, 's', "Input key, by default an RSA private key"},
    {"keyform", OPT_KEYFORM, 'E', "Private key format (ENGINE, other values ignored)"},
    {"pubin", OPT_PUBIN, '-', "Input key is an RSA public pkey"},
    {"certin", OPT_CERTIN, '-', "Input is a cert carrying an RSA public key"},
    {"rev", OPT_REV, '-', "Reverse the order of the input buffer"},
    {"passin", OPT_PASSIN, 's', "Input file pass phrase source"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file"},
    {"raw", OPT_RSA_RAW, '-', "Use no padding"},
    {"pkcs", OPT_PKCS, '-', "Use PKCS#1 v1.5 padding (default)"},
    {"x931", OPT_X931, '-', "Use ANSI X9.31 padding"},
    {"oaep", OPT_OAEP, '-', "Use PKCS#1 OAEP"},
    {"asn1parse", OPT_ASN1PARSE, '-',
     "Run output through asn1parse; useful with -verify"},
    {"hexdump", OPT_HEXDUMP, '-', "Hex dump output"},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,
    {NULL}
};

int rsautl_main(int argc, char **argv)
{
    BIO *in = NULL, *out = NULL;
    ENGINE *e = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    X509 *x;
    char *infile = NULL, *outfile = NULL, *keyfile = NULL;
    char *passinarg = NULL, *passin = NULL, *prog;
    char rsa_mode = RSA_VERIFY, key_type = KEY_PRIVKEY;
    unsigned char *rsa_in = NULL, *rsa_out = NULL, pad = RSA_PKCS1_PADDING;
    size_t rsa_inlen, rsa_outlen = 0;
    int keyformat = FORMAT_UNDEF, keysize, ret = 1, rv;
    int hexdump = 0, asn1parse = 0, need_priv = 0, rev = 0;
    OPTION_CHOICE o;

    prog = opt_init(argc, argv, rsautl_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(rsautl_options);
            ret = 0;
            goto end;
        case OPT_KEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &keyformat))
                goto opthelp;
            break;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_ASN1PARSE:
            asn1parse = 1;
            break;
        case OPT_HEXDUMP:
            hexdump = 1;
            break;
        case OPT_RSA_RAW:
            pad = RSA_NO_PADDING;
            break;
        case OPT_OAEP:
            pad = RSA_PKCS1_OAEP_PADDING;
            break;
        case OPT_PKCS:
            pad = RSA_PKCS1_PADDING;
            break;
        case OPT_X931:
            pad = RSA_X931_PADDING;
            break;
        case OPT_SIGN:
            rsa_mode = RSA_SIGN;
            need_priv = 1;
            break;
        case OPT_VERIFY:
            rsa_mode = RSA_VERIFY;
            break;
        case OPT_REV:
            rev = 1;
            break;
        case OPT_ENCRYPT:
            rsa_mode = RSA_ENCRYPT;
            break;
        case OPT_DECRYPT:
            rsa_mode = RSA_DECRYPT;
            need_priv = 1;
            break;
        case OPT_PUBIN:
            key_type = KEY_PUBKEY;
            break;
        case OPT_CERTIN:
            key_type = KEY_CERT;
            break;
        case OPT_INKEY:
            keyfile = opt_arg();
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
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

    if (!app_RAND_load())
        goto end;

    if (need_priv && (key_type != KEY_PRIVKEY)) {
        BIO_printf(bio_err, "A private key is needed for this operation\n");
        goto end;
    }

    if (!app_passwd(passinarg, NULL, &passin, NULL)) {
        BIO_printf(bio_err, "Error getting password\n");
        goto end;
    }

    switch (key_type) {
    case KEY_PRIVKEY:
        pkey = load_key(keyfile, keyformat, 0, passin, e, "private key");
        break;

    case KEY_PUBKEY:
        pkey = load_pubkey(keyfile, keyformat, 0, NULL, e, "public key");
        break;

    case KEY_CERT:
        x = load_cert(keyfile, FORMAT_UNDEF, "Certificate");
        if (x) {
            pkey = X509_get_pubkey(x);
            X509_free(x);
        }
        break;
    }

    if (pkey == NULL)
        return 1;

    in = bio_open_default(infile, 'r', FORMAT_BINARY);
    if (in == NULL)
        goto end;
    out = bio_open_default(outfile, 'w', FORMAT_BINARY);
    if (out == NULL)
        goto end;

    keysize = EVP_PKEY_get_size(pkey);

    rsa_in = app_malloc(keysize * 2, "hold rsa key");
    rsa_out = app_malloc(keysize, "output rsa key");
    rsa_outlen = keysize;

    /* Read the input data */
    rv = BIO_read(in, rsa_in, keysize * 2);
    if (rv < 0) {
        BIO_printf(bio_err, "Error reading input Data\n");
        goto end;
    }
    rsa_inlen = rv;
    if (rev) {
        size_t i;
        unsigned char ctmp;

        for (i = 0; i < rsa_inlen / 2; i++) {
            ctmp = rsa_in[i];
            rsa_in[i] = rsa_in[rsa_inlen - 1 - i];
            rsa_in[rsa_inlen - 1 - i] = ctmp;
        }
    }

    if ((ctx = EVP_PKEY_CTX_new_from_pkey(NULL, pkey, NULL)) == NULL)
        goto end;

    switch (rsa_mode) {
    case RSA_VERIFY:
        rv = EVP_PKEY_verify_recover_init(ctx) > 0
            && EVP_PKEY_CTX_set_rsa_padding(ctx, pad) > 0
            && EVP_PKEY_verify_recover(ctx, rsa_out, &rsa_outlen,
                                       rsa_in, rsa_inlen) > 0;
        break;
    case RSA_SIGN:
        rv = EVP_PKEY_sign_init(ctx) > 0
            && EVP_PKEY_CTX_set_rsa_padding(ctx, pad) > 0
            && EVP_PKEY_sign(ctx, rsa_out, &rsa_outlen, rsa_in, rsa_inlen) > 0;
        break;
    case RSA_ENCRYPT:
        rv = EVP_PKEY_encrypt_init(ctx) > 0
            && EVP_PKEY_CTX_set_rsa_padding(ctx, pad) > 0
            && EVP_PKEY_encrypt(ctx, rsa_out, &rsa_outlen, rsa_in, rsa_inlen) > 0;
        break;
    case RSA_DECRYPT:
        rv = EVP_PKEY_decrypt_init(ctx) > 0
            && EVP_PKEY_CTX_set_rsa_padding(ctx, pad) > 0
            && EVP_PKEY_decrypt(ctx, rsa_out, &rsa_outlen, rsa_in, rsa_inlen) > 0;
        break;
    }

    if (!rv) {
        BIO_printf(bio_err, "RSA operation error\n");
        ERR_print_errors(bio_err);
        goto end;
    }
    ret = 0;
    if (asn1parse) {
        if (!ASN1_parse_dump(out, rsa_out, rsa_outlen, 1, -1)) {
            ERR_print_errors(bio_err);
        }
    } else if (hexdump) {
        BIO_dump(out, (char *)rsa_out, rsa_outlen);
    } else {
        BIO_write(out, rsa_out, rsa_outlen);
    }
 end:
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    release_engine(e);
    BIO_free(in);
    BIO_free_all(out);
    OPENSSL_free(rsa_in);
    OPENSSL_free(rsa_out);
    OPENSSL_free(passin);
    return ret;
}
