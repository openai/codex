/*
 * Copyright 1999-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* S/MIME utility function */

#include <stdio.h>
#include <string.h>
#include "apps.h"
#include "progs.h"
#include <openssl/crypto.h>
#include <openssl/pem.h>
#include <openssl/err.h>
#include <openssl/x509_vfy.h>
#include <openssl/x509v3.h>

static int save_certs(char *signerfile, STACK_OF(X509) *signers);
static int smime_cb(int ok, X509_STORE_CTX *ctx);

#define SMIME_OP        0x10
#define SMIME_IP        0x20
#define SMIME_SIGNERS   0x40
#define SMIME_ENCRYPT   (1 | SMIME_OP)
#define SMIME_DECRYPT   (2 | SMIME_IP)
#define SMIME_SIGN      (3 | SMIME_OP | SMIME_SIGNERS)
#define SMIME_RESIGN    (6 | SMIME_IP | SMIME_OP | SMIME_SIGNERS)
#define SMIME_VERIFY    (4 | SMIME_IP)
#define SMIME_PK7OUT    (5 | SMIME_IP | SMIME_OP)

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_ENCRYPT, OPT_DECRYPT, OPT_SIGN, OPT_RESIGN, OPT_VERIFY,
    OPT_PK7OUT, OPT_TEXT, OPT_NOINTERN, OPT_NOVERIFY, OPT_NOCHAIN,
    OPT_NOCERTS, OPT_NOATTR, OPT_NODETACH, OPT_NOSMIMECAP,
    OPT_BINARY, OPT_NOSIGS, OPT_STREAM, OPT_INDEF, OPT_NOINDEF,
    OPT_CRLFEOL, OPT_ENGINE, OPT_PASSIN,
    OPT_TO, OPT_FROM, OPT_SUBJECT, OPT_SIGNER, OPT_RECIP, OPT_MD,
    OPT_CIPHER, OPT_INKEY, OPT_KEYFORM, OPT_CERTFILE, OPT_CAFILE,
    OPT_CAPATH, OPT_CASTORE, OPT_NOCAFILE, OPT_NOCAPATH, OPT_NOCASTORE,
    OPT_R_ENUM, OPT_PROV_ENUM, OPT_CONFIG,
    OPT_V_ENUM,
    OPT_IN, OPT_INFORM, OPT_OUT,
    OPT_OUTFORM, OPT_CONTENT
} OPTION_CHOICE;

const OPTIONS smime_options[] = {
    {OPT_HELP_STR, 1, '-', "Usage: %s [options] [cert...]\n"},

    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"in", OPT_IN, '<', "Input file"},
    {"inform", OPT_INFORM, 'c', "Input format SMIME (default), PEM or DER"},
    {"out", OPT_OUT, '>', "Output file"},
    {"outform", OPT_OUTFORM, 'c',
     "Output format SMIME (default), PEM or DER"},
    {"inkey", OPT_INKEY, 's',
     "Input private key (if not signer or recipient)"},
    {"keyform", OPT_KEYFORM, 'f', "Input private key format (ENGINE, other values ignored)"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif
    {"stream", OPT_STREAM, '-', "Enable CMS streaming" },
    {"indef", OPT_INDEF, '-', "Same as -stream" },
    {"noindef", OPT_NOINDEF, '-', "Disable CMS streaming"},
    OPT_CONFIG_OPTION,

    OPT_SECTION("Action"),
    {"encrypt", OPT_ENCRYPT, '-', "Encrypt message"},
    {"decrypt", OPT_DECRYPT, '-', "Decrypt encrypted message"},
    {"sign", OPT_SIGN, '-', "Sign message"},
    {"resign", OPT_RESIGN, '-', "Resign a signed message"},
    {"verify", OPT_VERIFY, '-', "Verify signed message"},
    {"pk7out", OPT_PK7OUT, '-', "Output PKCS#7 structure"},

    OPT_SECTION("Signing/Encryption"),
    {"passin", OPT_PASSIN, 's', "Input file pass phrase source"},
    {"md", OPT_MD, 's', "Digest algorithm to use when signing or resigning"},
    {"", OPT_CIPHER, '-', "Any supported cipher"},
    {"nointern", OPT_NOINTERN, '-',
     "Don't search certificates in message for signer"},
    {"nodetach", OPT_NODETACH, '-', "Use opaque signing"},
    {"noattr", OPT_NOATTR, '-', "Don't include any signed attributes"},
    {"binary", OPT_BINARY, '-', "Don't translate message to text"},
    {"signer", OPT_SIGNER, 's', "Signer certificate file"},
    {"content", OPT_CONTENT, '<',
     "Supply or override content for detached signature"},
    {"nocerts", OPT_NOCERTS, '-',
     "Don't include signers certificate when signing"},

    OPT_SECTION("Verification/Decryption"),
    {"nosigs", OPT_NOSIGS, '-', "Don't verify message signature"},
    {"noverify", OPT_NOVERIFY, '-', "Don't verify signers certificate"},

    {"certfile", OPT_CERTFILE, '<',
     "Extra signer and intermediate CA certificates to include when signing"},
    {OPT_MORE_STR, 0, 0,
     "or to use as preferred signer certs and for chain building when verifying"},
    {"recip", OPT_RECIP, '<', "Recipient certificate file for decryption"},

    OPT_SECTION("Email"),
    {"to", OPT_TO, 's', "To address"},
    {"from", OPT_FROM, 's', "From address"},
    {"subject", OPT_SUBJECT, 's', "Subject"},
    {"text", OPT_TEXT, '-', "Include or delete text MIME headers"},
    {"nosmimecap", OPT_NOSMIMECAP, '-', "Omit the SMIMECapabilities attribute"},

    OPT_SECTION("Certificate chain"),
    {"CApath", OPT_CAPATH, '/', "Trusted certificates directory"},
    {"CAfile", OPT_CAFILE, '<', "Trusted certificates file"},
    {"CAstore", OPT_CASTORE, ':', "Trusted certificates store URI"},
    {"no-CAfile", OPT_NOCAFILE, '-',
     "Do not load the default certificates file"},
    {"no-CApath", OPT_NOCAPATH, '-',
     "Do not load certificates from the default certificates directory"},
    {"no-CAstore", OPT_NOCASTORE, '-',
     "Do not load certificates from the default certificates store"},
    {"nochain", OPT_NOCHAIN, '-',
     "set PKCS7_NOCHAIN so certificates contained in the message are not used as untrusted CAs" },
    {"crlfeol", OPT_CRLFEOL, '-', "Use CRLF as EOL termination instead of LF only"},

    OPT_R_OPTIONS,
    OPT_V_OPTIONS,
    OPT_PROV_OPTIONS,

    OPT_PARAMETERS(),
    {"cert", 0, 0, "Recipient certs, used when encrypting"},
    {NULL}
};

static const char *operation_name(int operation)
{
    switch (operation) {
    case SMIME_ENCRYPT:
        return "encrypt";
    case SMIME_DECRYPT:
        return "decrypt";
    case SMIME_SIGN:
        return "sign";
    case SMIME_RESIGN:
        return "resign";
    case SMIME_VERIFY:
        return "verify";
    case SMIME_PK7OUT:
        return "pk7out";
    default:
        return "(invalid operation)";
    }
}

#define SET_OPERATION(op) \
    ((operation != 0 && (operation != (op))) \
     ? 0 * BIO_printf(bio_err, "%s: Cannot use -%s together with -%s\n", \
                      prog, operation_name(op), operation_name(operation)) \
     : (operation = (op)))

int smime_main(int argc, char **argv)
{
    CONF *conf = NULL;
    BIO *in = NULL, *out = NULL, *indata = NULL;
    EVP_PKEY *key = NULL;
    PKCS7 *p7 = NULL;
    STACK_OF(OPENSSL_STRING) *sksigners = NULL, *skkeys = NULL;
    STACK_OF(X509) *encerts = NULL, *other = NULL;
    X509 *cert = NULL, *recip = NULL, *signer = NULL;
    X509_STORE *store = NULL;
    X509_VERIFY_PARAM *vpm = NULL;
    EVP_CIPHER *cipher = NULL;
    EVP_MD *sign_md = NULL;
    const char *CAfile = NULL, *CApath = NULL, *CAstore = NULL, *prog = NULL;
    char *certfile = NULL, *keyfile = NULL, *contfile = NULL;
    char *infile = NULL, *outfile = NULL, *signerfile = NULL, *recipfile = NULL;
    char *passinarg = NULL, *passin = NULL, *to = NULL, *from = NULL;
    char *subject = NULL, *digestname = NULL, *ciphername = NULL;
    OPTION_CHOICE o;
    int noCApath = 0, noCAfile = 0, noCAstore = 0;
    int flags = PKCS7_DETACHED, operation = 0, ret = 0, indef = 0;
    int informat = FORMAT_SMIME, outformat = FORMAT_SMIME, keyform =
        FORMAT_UNDEF;
    int vpmtouched = 0, rv = 0;
    ENGINE *e = NULL;
    const char *mime_eol = "\n";
    OSSL_LIB_CTX *libctx = app_get0_libctx();

    if ((vpm = X509_VERIFY_PARAM_new()) == NULL)
        return 1;

    opt_set_unknown_name("cipher");
    prog = opt_init(argc, argv, smime_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(smime_options);
            ret = 0;
            goto end;
        case OPT_INFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PDS, &informat))
                goto opthelp;
            break;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUTFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PDS, &outformat))
                goto opthelp;
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_ENCRYPT:
            if (!SET_OPERATION(SMIME_ENCRYPT))
                goto end;
            break;
        case OPT_DECRYPT:
            if (!SET_OPERATION(SMIME_DECRYPT))
                goto end;
            break;
        case OPT_SIGN:
            if (!SET_OPERATION(SMIME_SIGN))
                goto end;
            break;
        case OPT_RESIGN:
            if (!SET_OPERATION(SMIME_RESIGN))
                goto end;
            break;
        case OPT_VERIFY:
            if (!SET_OPERATION(SMIME_VERIFY))
                goto end;
            break;
        case OPT_PK7OUT:
            if (!SET_OPERATION(SMIME_PK7OUT))
                goto end;
            break;
        case OPT_TEXT:
            flags |= PKCS7_TEXT;
            break;
        case OPT_NOINTERN:
            flags |= PKCS7_NOINTERN;
            break;
        case OPT_NOVERIFY:
            flags |= PKCS7_NOVERIFY;
            break;
        case OPT_NOCHAIN:
            flags |= PKCS7_NOCHAIN;
            break;
        case OPT_NOCERTS:
            flags |= PKCS7_NOCERTS;
            break;
        case OPT_NOATTR:
            flags |= PKCS7_NOATTR;
            break;
        case OPT_NODETACH:
            flags &= ~PKCS7_DETACHED;
            break;
        case OPT_NOSMIMECAP:
            flags |= PKCS7_NOSMIMECAP;
            break;
        case OPT_BINARY:
            flags |= PKCS7_BINARY;
            break;
        case OPT_NOSIGS:
            flags |= PKCS7_NOSIGS;
            break;
        case OPT_STREAM:
        case OPT_INDEF:
            indef = 1;
            break;
        case OPT_NOINDEF:
            indef = 0;
            break;
        case OPT_CRLFEOL:
            flags |= PKCS7_CRLFEOL;
            mime_eol = "\r\n";
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        case OPT_CONFIG:
            conf = app_load_config_modules(opt_arg());
            if (conf == NULL)
                goto end;
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_TO:
            to = opt_arg();
            break;
        case OPT_FROM:
            from = opt_arg();
            break;
        case OPT_SUBJECT:
            subject = opt_arg();
            break;
        case OPT_SIGNER:
            /* If previous -signer argument add signer to list */
            if (signerfile != NULL) {
                if (sksigners == NULL
                    && (sksigners = sk_OPENSSL_STRING_new_null()) == NULL)
                    goto end;
                if (sk_OPENSSL_STRING_push(sksigners, signerfile) <= 0)
                    goto end;
                if (keyfile == NULL)
                    keyfile = signerfile;
                if (skkeys == NULL
                    && (skkeys = sk_OPENSSL_STRING_new_null()) == NULL)
                    goto end;
                if (sk_OPENSSL_STRING_push(skkeys, keyfile) <= 0)
                    goto end;
                keyfile = NULL;
            }
            signerfile = opt_arg();
            break;
        case OPT_RECIP:
            recipfile = opt_arg();
            break;
        case OPT_MD:
            digestname = opt_arg();
            break;
        case OPT_CIPHER:
            ciphername = opt_unknown();
            break;
        case OPT_INKEY:
            /* If previous -inkey argument add signer to list */
            if (keyfile != NULL) {
                if (signerfile == NULL) {
                    BIO_printf(bio_err,
                               "%s: Must have -signer before -inkey\n", prog);
                    goto opthelp;
                }
                if (sksigners == NULL
                    && (sksigners = sk_OPENSSL_STRING_new_null()) == NULL)
                    goto end;
                if (sk_OPENSSL_STRING_push(sksigners, signerfile) <= 0)
                    goto end;
                signerfile = NULL;
                if (skkeys == NULL
                    && (skkeys = sk_OPENSSL_STRING_new_null()) == NULL)
                    goto end;
                if (sk_OPENSSL_STRING_push(skkeys, keyfile) <= 0)
                    goto end;
            }
            keyfile = opt_arg();
            break;
        case OPT_KEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &keyform))
                goto opthelp;
            break;
        case OPT_CERTFILE:
            certfile = opt_arg();
            break;
        case OPT_CAFILE:
            CAfile = opt_arg();
            break;
        case OPT_CAPATH:
            CApath = opt_arg();
            break;
        case OPT_CASTORE:
            CAstore = opt_arg();
            break;
        case OPT_NOCAFILE:
            noCAfile = 1;
            break;
        case OPT_NOCAPATH:
            noCApath = 1;
            break;
        case OPT_NOCASTORE:
            noCAstore = 1;
            break;
        case OPT_CONTENT:
            contfile = opt_arg();
            break;
        case OPT_V_CASES:
            if (!opt_verify(o, vpm))
                goto opthelp;
            vpmtouched++;
            break;
        }
    }

    /* Extra arguments are files with recipient keys. */
    argc = opt_num_rest();
    argv = opt_rest();

    if (!app_RAND_load())
        goto end;

    if (digestname != NULL) {
        if (!opt_md(digestname, &sign_md))
            goto opthelp;
    }
    if (!opt_cipher_any(ciphername, &cipher))
            goto opthelp;
    if (!(operation & SMIME_SIGNERS) && (skkeys != NULL || sksigners != NULL)) {
        BIO_puts(bio_err, "Multiple signers or keys not allowed\n");
        goto opthelp;
    }
    if (!operation) {
        BIO_puts(bio_err,
                "No operation (-encrypt|-sign|...) specified\n");
        goto opthelp;
    }

    if (operation & SMIME_SIGNERS) {
        /* Check to see if any final signer needs to be appended */
        if (keyfile && !signerfile) {
            BIO_puts(bio_err, "Illegal -inkey without -signer\n");
            goto opthelp;
        }
        if (signerfile != NULL) {
            if (sksigners == NULL
                && (sksigners = sk_OPENSSL_STRING_new_null()) == NULL)
                goto end;
            if (sk_OPENSSL_STRING_push(sksigners, signerfile) <= 0)
                goto end;
            if (!skkeys && (skkeys = sk_OPENSSL_STRING_new_null()) == NULL)
                goto end;
            if (!keyfile)
                keyfile = signerfile;
            if (sk_OPENSSL_STRING_push(skkeys, keyfile) <= 0)
                goto end;
        }
        if (sksigners == NULL) {
            BIO_printf(bio_err, "No signer certificate specified\n");
            goto opthelp;
        }
        signerfile = NULL;
        keyfile = NULL;
    } else if (operation == SMIME_DECRYPT) {
        if (recipfile == NULL && keyfile == NULL) {
            BIO_printf(bio_err,
                       "No recipient certificate or key specified\n");
            goto opthelp;
        }
    } else if (operation == SMIME_ENCRYPT) {
        if (argc == 0) {
            BIO_printf(bio_err, "No recipient(s) certificate(s) specified\n");
            goto opthelp;
        }
    }

    if (!app_passwd(passinarg, NULL, &passin, NULL)) {
        BIO_printf(bio_err, "Error getting password\n");
        goto end;
    }

    ret = 2;

    if (!(operation & SMIME_SIGNERS))
        flags &= ~PKCS7_DETACHED;

    if (!(operation & SMIME_OP)) {
        if (flags & PKCS7_BINARY)
            outformat = FORMAT_BINARY;
    }

    if (!(operation & SMIME_IP)) {
        if (flags & PKCS7_BINARY)
            informat = FORMAT_BINARY;
    }

    if (operation == SMIME_ENCRYPT) {
        if (cipher == NULL)
            cipher = (EVP_CIPHER *)EVP_aes_256_cbc();
        encerts = sk_X509_new_null();
        if (encerts == NULL)
            goto end;
        while (*argv != NULL) {
            cert = load_cert(*argv, FORMAT_UNDEF,
                             "recipient certificate file");
            if (cert == NULL)
                goto end;
            if (!sk_X509_push(encerts, cert))
                goto end;
            cert = NULL;
            argv++;
        }
    }

    if (certfile != NULL) {
        if (!load_certs(certfile, 0, &other, NULL, "certificates")) {
            ERR_print_errors(bio_err);
            goto end;
        }
    }

    if (recipfile != NULL && (operation == SMIME_DECRYPT)) {
        if ((recip = load_cert(recipfile, FORMAT_UNDEF,
                               "recipient certificate file")) == NULL) {
            ERR_print_errors(bio_err);
            goto end;
        }
    }

    if (operation == SMIME_DECRYPT) {
        if (keyfile == NULL)
            keyfile = recipfile;
    } else if (operation == SMIME_SIGN) {
        if (keyfile == NULL)
            keyfile = signerfile;
    } else {
        keyfile = NULL;
    }

    if (keyfile != NULL) {
        key = load_key(keyfile, keyform, 0, passin, e, "signing key");
        if (key == NULL)
            goto end;
    }

    in = bio_open_default(infile, 'r', informat);
    if (in == NULL)
        goto end;

    if (operation & SMIME_IP) {
        PKCS7 *p7_in = NULL;

        p7 = PKCS7_new_ex(libctx, app_get0_propq());
        if (p7 == NULL) {
            BIO_printf(bio_err, "Error allocating PKCS7 object\n");
            goto end;
        }
        if (informat == FORMAT_SMIME) {
            p7_in = SMIME_read_PKCS7_ex(in, &indata, &p7);
        } else if (informat == FORMAT_PEM) {
            p7_in = PEM_read_bio_PKCS7(in, &p7, NULL, NULL);
        } else if (informat == FORMAT_ASN1) {
            p7_in = d2i_PKCS7_bio(in, &p7);
        } else {
            BIO_printf(bio_err, "Bad input format for PKCS#7 file\n");
            goto end;
        }

        if (p7_in == NULL) {
            BIO_printf(bio_err, "Error reading S/MIME message\n");
            goto end;
        }
        if (contfile != NULL) {
            BIO_free(indata);
            if ((indata = BIO_new_file(contfile, "rb")) == NULL) {
                BIO_printf(bio_err, "Can't read content file %s\n", contfile);
                goto end;
            }
        }
    }

    out = bio_open_default(outfile, 'w', outformat);
    if (out == NULL)
        goto end;

    if (operation == SMIME_VERIFY) {
        if ((store = setup_verify(CAfile, noCAfile, CApath, noCApath,
                                  CAstore, noCAstore)) == NULL)
            goto end;
        X509_STORE_set_verify_cb(store, smime_cb);
        if (vpmtouched)
            X509_STORE_set1_param(store, vpm);
    }

    ret = 3;

    if (operation == SMIME_ENCRYPT) {
        if (indef)
            flags |= PKCS7_STREAM;
        p7 = PKCS7_encrypt_ex(encerts, in, cipher, flags, libctx, app_get0_propq());
    } else if (operation & SMIME_SIGNERS) {
        int i;
        /*
         * If detached data content we only enable streaming if S/MIME output
         * format.
         */
        if (operation == SMIME_SIGN) {
            if (flags & PKCS7_DETACHED) {
                if (outformat == FORMAT_SMIME)
                    flags |= PKCS7_STREAM;
            } else if (indef) {
                flags |= PKCS7_STREAM;
            }
            flags |= PKCS7_PARTIAL;
            p7 = PKCS7_sign_ex(NULL, NULL, other, in, flags, libctx, app_get0_propq());
            if (p7 == NULL)
                goto end;
            if (flags & PKCS7_NOCERTS) {
                for (i = 0; i < sk_X509_num(other); i++) {
                    X509 *x = sk_X509_value(other, i);
                    PKCS7_add_certificate(p7, x);
                }
            }
        } else {
            flags |= PKCS7_REUSE_DIGEST;
        }
        for (i = 0; i < sk_OPENSSL_STRING_num(sksigners); i++) {
            signerfile = sk_OPENSSL_STRING_value(sksigners, i);
            keyfile = sk_OPENSSL_STRING_value(skkeys, i);
            signer = load_cert(signerfile, FORMAT_UNDEF, "signer certificate");
            if (signer == NULL)
                goto end;
            key = load_key(keyfile, keyform, 0, passin, e, "signing key");
            if (key == NULL)
                goto end;

            if (!PKCS7_sign_add_signer(p7, signer, key, sign_md, flags))
                goto end;
            X509_free(signer);
            signer = NULL;
            EVP_PKEY_free(key);
            key = NULL;
        }
        /* If not streaming or resigning finalize structure */
        if ((operation == SMIME_SIGN) && !(flags & PKCS7_STREAM)) {
            if (!PKCS7_final(p7, in, flags))
                goto end;
        }
    }

    if (p7 == NULL) {
        BIO_printf(bio_err, "Error creating PKCS#7 structure\n");
        goto end;
    }

    ret = 4;
    if (operation == SMIME_DECRYPT) {
        if (!PKCS7_decrypt(p7, key, recip, out, flags)) {
            BIO_printf(bio_err, "Error decrypting PKCS#7 structure\n");
            goto end;
        }
    } else if (operation == SMIME_VERIFY) {
        STACK_OF(X509) *signers;
        if (PKCS7_verify(p7, other, store, indata, out, flags))
            BIO_printf(bio_err, "Verification successful\n");
        else {
            BIO_printf(bio_err, "Verification failure\n");
            goto end;
        }
        signers = PKCS7_get0_signers(p7, other, flags);
        if (!save_certs(signerfile, signers)) {
            BIO_printf(bio_err, "Error writing signers to %s\n", signerfile);
            ret = 5;
            goto end;
        }
        sk_X509_free(signers);
    } else if (operation == SMIME_PK7OUT) {
        PEM_write_bio_PKCS7(out, p7);
    } else {
        if (to)
            BIO_printf(out, "To: %s%s", to, mime_eol);
        if (from)
            BIO_printf(out, "From: %s%s", from, mime_eol);
        if (subject)
            BIO_printf(out, "Subject: %s%s", subject, mime_eol);
        if (outformat == FORMAT_SMIME) {
            if (operation == SMIME_RESIGN)
                rv = SMIME_write_PKCS7(out, p7, indata, flags);
            else
                rv = SMIME_write_PKCS7(out, p7, in, flags);
        } else if (outformat == FORMAT_PEM) {
            rv = PEM_write_bio_PKCS7_stream(out, p7, in, flags);
        } else if (outformat == FORMAT_ASN1) {
            rv = i2d_PKCS7_bio_stream(out, p7, in, flags);
        } else {
            BIO_printf(bio_err, "Bad output format for PKCS#7 file\n");
            goto end;
        }
        if (rv == 0) {
            BIO_printf(bio_err, "Error writing output\n");
            ret = 3;
            goto end;
        }
    }
    ret = 0;
 end:
    if (ret)
        ERR_print_errors(bio_err);
    OSSL_STACK_OF_X509_free(encerts);
    OSSL_STACK_OF_X509_free(other);
    X509_VERIFY_PARAM_free(vpm);
    sk_OPENSSL_STRING_free(sksigners);
    sk_OPENSSL_STRING_free(skkeys);
    X509_STORE_free(store);
    X509_free(cert);
    X509_free(recip);
    X509_free(signer);
    EVP_PKEY_free(key);
    EVP_MD_free(sign_md);
    EVP_CIPHER_free(cipher);
    PKCS7_free(p7);
    release_engine(e);
    BIO_free(in);
    BIO_free(indata);
    BIO_free_all(out);
    OPENSSL_free(passin);
    NCONF_free(conf);
    return ret;
}

static int save_certs(char *signerfile, STACK_OF(X509) *signers)
{
    int i;
    BIO *tmp;

    if (signerfile == NULL)
        return 1;
    tmp = BIO_new_file(signerfile, "w");
    if (tmp == NULL)
        return 0;
    for (i = 0; i < sk_X509_num(signers); i++)
        PEM_write_bio_X509(tmp, sk_X509_value(signers, i));
    BIO_free(tmp);
    return 1;
}

/* Minimal callback just to output policy info (if any) */

static int smime_cb(int ok, X509_STORE_CTX *ctx)
{
    int error;

    error = X509_STORE_CTX_get_error(ctx);

    if ((error != X509_V_ERR_NO_EXPLICIT_POLICY)
        && ((error != X509_V_OK) || (ok != 2)))
        return ok;

    policies_print(ctx);

    return ok;
}
