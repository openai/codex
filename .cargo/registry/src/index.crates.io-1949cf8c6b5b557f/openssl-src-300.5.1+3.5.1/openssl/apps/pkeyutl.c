/*
 * Copyright 2006-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "apps.h"
#include "progs.h"
#include <string.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/evp.h>
#include <sys/stat.h>

#define KEY_NONE        0
#define KEY_PRIVKEY     1
#define KEY_PUBKEY      2
#define KEY_CERT        3

static EVP_PKEY *get_pkey(const char *kdfalg,
                          const char *keyfile, int keyform, int key_type,
                          char *passinarg, int pkey_op, ENGINE *e);
static EVP_PKEY_CTX *init_ctx(const char *kdfalg, int *pkeysize,
                              int pkey_op, ENGINE *e,
                              const int engine_impl, int rawin,
                              EVP_PKEY *pkey /* ownership is passed to ctx */,
                              EVP_MD_CTX *mctx, const char *digestname,
                              const char *kemop, OSSL_LIB_CTX *libctx, const char *propq);

static int setup_peer(EVP_PKEY_CTX *ctx, int peerform, const char *file,
                      ENGINE *e);

static int do_keyop(EVP_PKEY_CTX *ctx, int pkey_op,
                    unsigned char *out, size_t *poutlen,
                    const unsigned char *in, size_t inlen,
                    unsigned char *secret, size_t *psecretlen);

static int do_raw_keyop(int pkey_op, EVP_MD_CTX *mctx,
                        EVP_PKEY *pkey, BIO *in,
                        int filesize, unsigned char *sig, int siglen,
                        unsigned char **out, size_t *poutlen);

static int only_nomd(EVP_PKEY *pkey)
{
#define MADE_UP_MAX_MD_NAME_LEN 100
    char defname[MADE_UP_MAX_MD_NAME_LEN];
    int deftype;

    deftype = EVP_PKEY_get_default_digest_name(pkey, defname, sizeof(defname));
    return deftype == 2 /* Mandatory */
        && strcmp(defname, "UNDEF") == 0;
}

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_ENGINE, OPT_ENGINE_IMPL, OPT_IN, OPT_OUT,
    OPT_PUBIN, OPT_CERTIN, OPT_ASN1PARSE, OPT_HEXDUMP, OPT_SIGN,
    OPT_VERIFY, OPT_VERIFYRECOVER, OPT_REV, OPT_ENCRYPT, OPT_DECRYPT,
    OPT_DERIVE, OPT_SIGFILE, OPT_INKEY, OPT_PEERKEY, OPT_PASSIN,
    OPT_PEERFORM, OPT_KEYFORM, OPT_PKEYOPT, OPT_PKEYOPT_PASSIN, OPT_KDF,
    OPT_KDFLEN, OPT_R_ENUM, OPT_PROV_ENUM,
    OPT_DECAP, OPT_ENCAP, OPT_SECOUT, OPT_KEMOP,
    OPT_CONFIG,
    OPT_RAWIN, OPT_DIGEST
} OPTION_CHOICE;

const OPTIONS pkeyutl_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
    {"engine_impl", OPT_ENGINE_IMPL, '-',
     "Also use engine given by -engine for crypto operations"},
#endif
    {"sign", OPT_SIGN, '-', "Sign input data with private key"},
    {"verify", OPT_VERIFY, '-', "Verify with public key"},
    {"encrypt", OPT_ENCRYPT, '-', "Encrypt input data with public key"},
    {"decrypt", OPT_DECRYPT, '-', "Decrypt input data with private key"},
    {"derive", OPT_DERIVE, '-', "Derive shared secret from own and peer (EC)DH keys"},
    {"decap", OPT_DECAP, '-', "Decapsulate shared secret"},
    {"encap", OPT_ENCAP, '-', "Encapsulate shared secret"},
    OPT_CONFIG_OPTION,

    OPT_SECTION("Input"),
    {"in", OPT_IN, '<', "Input file - default stdin"},
    {"inkey", OPT_INKEY, 's', "Input key, by default private key"},
    {"pubin", OPT_PUBIN, '-', "Input key is a public key"},
    {"passin", OPT_PASSIN, 's', "Input file pass phrase source"},
    {"peerkey", OPT_PEERKEY, 's', "Peer key file used in key derivation"},
    {"peerform", OPT_PEERFORM, 'E', "Peer key format (DER/PEM/P12/ENGINE)"},
    {"certin", OPT_CERTIN, '-', "Input is a cert with a public key"},
    {"rev", OPT_REV, '-', "Reverse the order of the input buffer"},
    {"sigfile", OPT_SIGFILE, '<', "Signature file (verify operation only)"},
    {"keyform", OPT_KEYFORM, 'E', "Private key format (ENGINE, other values ignored)"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file - default stdout"},
    {"secret", OPT_SECOUT, '>', "File to store secret on encapsulation"},
    {"asn1parse", OPT_ASN1PARSE, '-',
     "parse the output as ASN.1 data to check its DER encoding and print errors"},
    {"hexdump", OPT_HEXDUMP, '-', "Hex dump output"},
    {"verifyrecover", OPT_VERIFYRECOVER, '-',
     "Verify RSA signature, recovering original signature input data"},

    OPT_SECTION("Signing/Derivation/Encapsulation"),
    {"rawin", OPT_RAWIN, '-',
     "Indicate that the signature/verification input data is not yet hashed"},
    {"digest", OPT_DIGEST, 's',
     "The digest algorithm to use for signing/verifying raw input data. Implies -rawin"},
    {"pkeyopt", OPT_PKEYOPT, 's', "Public key options as opt:value"},
    {"pkeyopt_passin", OPT_PKEYOPT_PASSIN, 's',
     "Public key option that is read as a passphrase argument opt:passphrase"},
    {"kdf", OPT_KDF, 's', "Use KDF algorithm"},
    {"kdflen", OPT_KDFLEN, 'p', "KDF algorithm output length"},
    {"kemop", OPT_KEMOP, 's', "KEM operation specific to the key algorithm"},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,
    {NULL}
};

int pkeyutl_main(int argc, char **argv)
{
    CONF *conf = NULL;
    BIO *in = NULL, *out = NULL, *secout = NULL;
    ENGINE *e = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *pkey = NULL;
    char *infile = NULL, *outfile = NULL, *secoutfile = NULL, *sigfile = NULL, *passinarg = NULL;
    char hexdump = 0, asn1parse = 0, rev = 0, *prog;
    unsigned char *buf_in = NULL, *buf_out = NULL, *sig = NULL, *secret = NULL;
    OPTION_CHOICE o;
    int buf_inlen = 0, siglen = -1;
    int keyform = FORMAT_UNDEF, peerform = FORMAT_UNDEF;
    int keysize = -1, pkey_op = EVP_PKEY_OP_SIGN, key_type = KEY_PRIVKEY;
    int engine_impl = 0;
    int ret = 1, rv = -1;
    size_t buf_outlen = 0, secretlen = 0;
    const char *inkey = NULL;
    const char *peerkey = NULL;
    const char *kdfalg = NULL, *digestname = NULL, *kemop = NULL;
    int kdflen = 0;
    STACK_OF(OPENSSL_STRING) *pkeyopts = NULL;
    STACK_OF(OPENSSL_STRING) *pkeyopts_passin = NULL;
    int rawin = 0;
    EVP_MD_CTX *mctx = NULL;
    EVP_MD *md = NULL;
    int filesize = -1;
    OSSL_LIB_CTX *libctx = app_get0_libctx();

    prog = opt_init(argc, argv, pkeyutl_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(pkeyutl_options);
            ret = 0;
            goto end;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_SECOUT:
            secoutfile = opt_arg();
            break;
        case OPT_SIGFILE:
            sigfile = opt_arg();
            break;
        case OPT_ENGINE_IMPL:
            engine_impl = 1;
            break;
        case OPT_INKEY:
            inkey = opt_arg();
            break;
        case OPT_PEERKEY:
            peerkey = opt_arg();
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_PEERFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &peerform))
                goto opthelp;
            break;
        case OPT_KEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &keyform))
                goto opthelp;
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_CONFIG:
            conf = app_load_config_modules(opt_arg());
            if (conf == NULL)
                goto end;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_PUBIN:
            key_type = KEY_PUBKEY;
            break;
        case OPT_CERTIN:
            key_type = KEY_CERT;
            break;
        case OPT_ASN1PARSE:
            asn1parse = 1;
            break;
        case OPT_HEXDUMP:
            hexdump = 1;
            break;
        case OPT_SIGN:
            pkey_op = EVP_PKEY_OP_SIGN;
            break;
        case OPT_VERIFY:
            pkey_op = EVP_PKEY_OP_VERIFY;
            break;
        case OPT_VERIFYRECOVER:
            pkey_op = EVP_PKEY_OP_VERIFYRECOVER;
            break;
        case OPT_ENCRYPT:
            pkey_op = EVP_PKEY_OP_ENCRYPT;
            break;
        case OPT_DECRYPT:
            pkey_op = EVP_PKEY_OP_DECRYPT;
            break;
        case OPT_DERIVE:
            pkey_op = EVP_PKEY_OP_DERIVE;
            break;
        case OPT_DECAP:
            pkey_op = EVP_PKEY_OP_DECAPSULATE;
            break;
        case OPT_ENCAP:
            key_type = KEY_PUBKEY;
            pkey_op = EVP_PKEY_OP_ENCAPSULATE;
            break;
        case OPT_KEMOP:
            kemop = opt_arg();
            break;
        case OPT_KDF:
            pkey_op = EVP_PKEY_OP_DERIVE;
            key_type = KEY_NONE;
            kdfalg = opt_arg();
            break;
        case OPT_KDFLEN:
            kdflen = atoi(opt_arg());
            break;
        case OPT_REV:
            rev = 1;
            break;
        case OPT_PKEYOPT:
            if ((pkeyopts == NULL &&
                 (pkeyopts = sk_OPENSSL_STRING_new_null()) == NULL) ||
                sk_OPENSSL_STRING_push(pkeyopts, opt_arg()) == 0) {
                BIO_puts(bio_err, "out of memory\n");
                goto end;
            }
            break;
        case OPT_PKEYOPT_PASSIN:
            if ((pkeyopts_passin == NULL &&
                 (pkeyopts_passin = sk_OPENSSL_STRING_new_null()) == NULL) ||
                sk_OPENSSL_STRING_push(pkeyopts_passin, opt_arg()) == 0) {
                BIO_puts(bio_err, "out of memory\n");
                goto end;
            }
            break;
        case OPT_RAWIN:
            rawin = 1;
            break;
        case OPT_DIGEST:
            digestname = opt_arg();
            break;
        }
    }

    /* No extra arguments. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    if (!app_RAND_load())
        goto end;

    if (digestname != NULL)
        rawin = 1;

    if (kdfalg != NULL) {
        if (kdflen == 0) {
            BIO_printf(bio_err,
                       "%s: no KDF length given (-kdflen parameter).\n", prog);
            goto opthelp;
        }
    } else if (inkey == NULL) {
        BIO_printf(bio_err,
                   "%s: no private key given (-inkey parameter).\n", prog);
        goto opthelp;
    } else if (peerkey != NULL && pkey_op != EVP_PKEY_OP_DERIVE) {
        BIO_printf(bio_err,
                   "%s: -peerkey option not allowed without -derive.\n", prog);
        goto opthelp;
    } else if (peerkey == NULL && pkey_op == EVP_PKEY_OP_DERIVE) {
        BIO_printf(bio_err,
                   "%s: missing -peerkey option for -derive operation.\n", prog);
        goto opthelp;
    }

    pkey = get_pkey(kdfalg, inkey, keyform, key_type, passinarg, pkey_op, e);
    if (key_type != KEY_NONE && pkey == NULL) {
        BIO_printf(bio_err, "%s: Error loading key\n", prog);
        goto end;
    }

    if (pkey_op == EVP_PKEY_OP_VERIFYRECOVER && !EVP_PKEY_is_a(pkey, "RSA")) {
        BIO_printf(bio_err, "%s: -verifyrecover can be used only with RSA\n", prog);
        goto end;
    }

    if (pkey_op == EVP_PKEY_OP_SIGN || pkey_op == EVP_PKEY_OP_VERIFY) {
        if (only_nomd(pkey)) {
            if (digestname != NULL) {
                const char *alg = EVP_PKEY_get0_type_name(pkey);

                BIO_printf(bio_err,
                           "%s: -digest (prehash) is not supported with %s\n",
                           prog, alg != NULL ? alg : "(unknown key type)");
                goto end;
            }
            rawin = 1;
        }
    } else if (digestname != NULL || rawin) {
        BIO_printf(bio_err,
                   "%s: -digest and -rawin can only be used with -sign or -verify\n", prog);
        goto opthelp;
    }

    if (rawin && rev) {
        BIO_printf(bio_err, "%s: -rev cannot be used with raw input\n", prog);
        goto opthelp;
    }

    if (rawin) {
        if ((mctx = EVP_MD_CTX_new()) == NULL) {
            BIO_printf(bio_err, "Error: out of memory\n");
            goto end;
        }
    }
    ctx = init_ctx(kdfalg, &keysize, pkey_op, e, engine_impl, rawin, pkey,
                   mctx, digestname, kemop, libctx, app_get0_propq());
    if (ctx == NULL) {
        BIO_printf(bio_err, "%s: Error initializing context\n", prog);
        goto end;
    }
    if (peerkey != NULL && !setup_peer(ctx, peerform, peerkey, e)) {
        BIO_printf(bio_err, "%s: Error setting up peer key\n", prog);
        goto end;
    }
    if (pkeyopts != NULL) {
        int num = sk_OPENSSL_STRING_num(pkeyopts);
        int i;

        for (i = 0; i < num; ++i) {
            const char *opt = sk_OPENSSL_STRING_value(pkeyopts, i);

            if (pkey_ctrl_string(ctx, opt) <= 0) {
                BIO_printf(bio_err, "%s: Can't set parameter \"%s\":\n",
                           prog, opt);
                goto end;
            }
        }
    }
    if (pkeyopts_passin != NULL) {
        int num = sk_OPENSSL_STRING_num(pkeyopts_passin);
        int i;

        for (i = 0; i < num; i++) {
            char *opt = sk_OPENSSL_STRING_value(pkeyopts_passin, i);
            char *passin = strchr(opt, ':');
            char *passwd;

            if (passin == NULL) {
                /* Get password interactively */
                char passwd_buf[4096];
                int r;

                BIO_snprintf(passwd_buf, sizeof(passwd_buf), "Enter %s: ", opt);
                r = EVP_read_pw_string(passwd_buf, sizeof(passwd_buf) - 1,
                                       passwd_buf, 0);
                if (r < 0) {
                    if (r == -2)
                        BIO_puts(bio_err, "user abort\n");
                    else
                        BIO_puts(bio_err, "entry failed\n");
                    goto end;
                }
                passwd = OPENSSL_strdup(passwd_buf);
                if (passwd == NULL) {
                    BIO_puts(bio_err, "out of memory\n");
                    goto end;
                }
            } else {
                /*
                 * Get password as a passin argument: First split option name
                 * and passphrase argument into two strings
                 */
                *passin = 0;
                passin++;
                if (app_passwd(passin, NULL, &passwd, NULL) == 0) {
                    BIO_printf(bio_err, "failed to get '%s'\n", opt);
                    goto end;
                }
            }

            if (EVP_PKEY_CTX_ctrl_str(ctx, opt, passwd) <= 0) {
                BIO_printf(bio_err, "%s: Can't set parameter \"%s\":\n",
                           prog, opt);
                OPENSSL_free(passwd);
                goto end;
            }
            OPENSSL_free(passwd);
        }
    }

    if (sigfile != NULL && (pkey_op != EVP_PKEY_OP_VERIFY)) {
        BIO_printf(bio_err,
                   "%s: Signature file specified for non verify\n", prog);
        goto end;
    }

    if (sigfile == NULL && (pkey_op == EVP_PKEY_OP_VERIFY)) {
        BIO_printf(bio_err,
                   "%s: No signature file specified for verify\n", prog);
        goto end;
    }

    if (pkey_op != EVP_PKEY_OP_DERIVE && pkey_op != EVP_PKEY_OP_ENCAPSULATE) {
        in = bio_open_default(infile, 'r', FORMAT_BINARY);
        if (infile != NULL) {
            struct stat st;

            if (stat(infile, &st) == 0 && st.st_size <= INT_MAX)
                filesize = (int)st.st_size;
        }
        if (in == NULL)
            goto end;
    }
    if (pkey_op == EVP_PKEY_OP_DECAPSULATE && outfile != NULL) {
        if (secoutfile != NULL) {
            BIO_printf(bio_err, "%s: Decapsulation produces only a shared "
                                "secret and no output. The '-out' option "
                                "is not applicable.\n", prog);
            goto end;
        }
        if ((out = bio_open_owner(outfile, 'w', FORMAT_BINARY)) == NULL)
            goto end;
    } else {
        out = bio_open_default(outfile, 'w', FORMAT_BINARY);
        if (out == NULL)
            goto end;
    }

    if (pkey_op == EVP_PKEY_OP_ENCAPSULATE
        || pkey_op == EVP_PKEY_OP_DECAPSULATE) {
        if (secoutfile == NULL && pkey_op == EVP_PKEY_OP_ENCAPSULATE) {
            BIO_printf(bio_err, "KEM-based shared-secret derivation requires "
                                "the '-secret <file>' option\n");
            goto end;
        }
        /* For backwards compatibility, default decap secrets to the output */
        if (secoutfile != NULL
            && (secout = bio_open_owner(secoutfile, 'w', FORMAT_BINARY)) == NULL)
            goto end;
    }

    if (sigfile != NULL) {
        BIO *sigbio = BIO_new_file(sigfile, "rb");

        if (sigbio == NULL) {
            BIO_printf(bio_err, "Can't open signature file %s\n", sigfile);
            goto end;
        }
        siglen = bio_to_mem(&sig, keysize * 10, sigbio);
        BIO_free(sigbio);
        if (siglen < 0) {
            BIO_printf(bio_err, "Error reading signature data\n");
            goto end;
        }
    }

    /* Raw input data is handled elsewhere */
    if (in != NULL && !rawin) {
        /* Read the input data */
        buf_inlen = bio_to_mem(&buf_in, -1, in);
        if (buf_inlen < 0) {
            BIO_printf(bio_err, "Error reading input Data\n");
            goto end;
        }
        if (rev) {
            size_t i;
            unsigned char ctmp;
            size_t l = (size_t)buf_inlen;

            for (i = 0; i < l / 2; i++) {
                ctmp = buf_in[i];
                buf_in[i] = buf_in[l - 1 - i];
                buf_in[l - 1 - i] = ctmp;
            }
        }
    }

    /* Sanity check the input if the input is not raw */
    if (!rawin
        && (pkey_op == EVP_PKEY_OP_SIGN || pkey_op == EVP_PKEY_OP_VERIFY)) {
        if (buf_inlen > EVP_MAX_MD_SIZE) {
            BIO_printf(bio_err,
                       "Error: The non-raw input data length %d is too long - max supported hashed size is %d\n",
                       buf_inlen, EVP_MAX_MD_SIZE);
            goto end;
        }
    }

    if (pkey_op == EVP_PKEY_OP_VERIFY) {
        if (rawin) {
            rv = do_raw_keyop(pkey_op, mctx, pkey, in, filesize, sig, siglen,
                              NULL, 0);
        } else {
            rv = EVP_PKEY_verify(ctx, sig, (size_t)siglen,
                                 buf_in, (size_t)buf_inlen);
        }
        if (rv == 1) {
            BIO_puts(out, "Signature Verified Successfully\n");
            ret = 0;
        } else {
            BIO_puts(out, "Signature Verification Failure\n");
        }
        goto end;
    }
    if (rawin) {
        /* rawin allocates the buffer in do_raw_keyop() */
        rv = do_raw_keyop(pkey_op, mctx, pkey, in, filesize, NULL, 0,
                          &buf_out, (size_t *)&buf_outlen);
    } else {
        if (kdflen != 0) {
            buf_outlen = kdflen;
            rv = 1;
        } else {
            rv = do_keyop(ctx, pkey_op, NULL, (size_t *)&buf_outlen,
                          buf_in, (size_t)buf_inlen, NULL, (size_t *)&secretlen);
        }
        if (rv > 0
            && (secretlen > 0 || (pkey_op != EVP_PKEY_OP_ENCAPSULATE
                                  && pkey_op != EVP_PKEY_OP_DECAPSULATE))
            && (buf_outlen > 0 || pkey_op == EVP_PKEY_OP_DECAPSULATE)) {
            if (buf_outlen > 0)
                buf_out = app_malloc(buf_outlen, "buffer output");
            if (secretlen > 0)
                secret = app_malloc(secretlen, "secret output");
            rv = do_keyop(ctx, pkey_op,
                          buf_out, (size_t *)&buf_outlen,
                          buf_in, (size_t)buf_inlen, secret, (size_t *)&secretlen);
        }
    }
    if (rv <= 0) {
        if (pkey_op != EVP_PKEY_OP_DERIVE) {
            BIO_puts(bio_err, "Public Key operation error\n");
        } else {
            BIO_puts(bio_err, "Key derivation failed\n");
        }
        goto end;
    }
    ret = 0;

    if (asn1parse) {
        if (!ASN1_parse_dump(out, buf_out, buf_outlen, 1, -1))
            ERR_print_errors(bio_err); /* but still return success */
    } else if (hexdump) {
        BIO_dump(out, (char *)buf_out, buf_outlen);
    } else {
        BIO_write(out, buf_out, buf_outlen);
    }
    /* Backwards compatible decap output fallback */
    if (secretlen > 0)
        BIO_write(secout ? secout : out, secret, secretlen);

 end:
    if (ret != 0)
        ERR_print_errors(bio_err);
    EVP_MD_CTX_free(mctx);
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    EVP_MD_free(md);
    release_engine(e);
    BIO_free(in);
    BIO_free_all(out);
    BIO_free_all(secout);
    OPENSSL_free(buf_in);
    OPENSSL_free(buf_out);
    OPENSSL_free(sig);
    OPENSSL_free(secret);
    sk_OPENSSL_STRING_free(pkeyopts);
    sk_OPENSSL_STRING_free(pkeyopts_passin);
    NCONF_free(conf);
    return ret;
}

static EVP_PKEY *get_pkey(const char *kdfalg,
                          const char *keyfile, int keyform, int key_type,
                          char *passinarg, int pkey_op, ENGINE *e)
{
    EVP_PKEY *pkey = NULL;
    char *passin = NULL;
    X509 *x;

    if (((pkey_op == EVP_PKEY_OP_SIGN) || (pkey_op == EVP_PKEY_OP_DECRYPT)
         || (pkey_op == EVP_PKEY_OP_DERIVE))
        && (key_type != KEY_PRIVKEY && kdfalg == NULL)) {
        BIO_printf(bio_err, "A private key is needed for this operation\n");
        return NULL;
    }
    if (!app_passwd(passinarg, NULL, &passin, NULL)) {
        BIO_printf(bio_err, "Error getting password\n");
        return NULL;
    }
    switch (key_type) {
    case KEY_PRIVKEY:
        pkey = load_key(keyfile, keyform, 0, passin, e, "private key");
        break;

    case KEY_PUBKEY:
        pkey = load_pubkey(keyfile, keyform, 0, NULL, e, "public key");
        break;

    case KEY_CERT:
        x = load_cert(keyfile, keyform, "Certificate");
        if (x) {
            pkey = X509_get_pubkey(x);
            X509_free(x);
        }
        break;

    case KEY_NONE:
        break;

    }
    OPENSSL_free(passin);
    return pkey;
}

static EVP_PKEY_CTX *init_ctx(const char *kdfalg, int *pkeysize,
                              int pkey_op, ENGINE *e,
                              const int engine_impl, int rawin,
                              EVP_PKEY *pkey /* ownership is passed to ctx */,
                              EVP_MD_CTX *mctx, const char *digestname,
                              const char *kemop, OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_PKEY_CTX *ctx = NULL;
    ENGINE *impl = NULL;
    int rv = -1;

#ifndef OPENSSL_NO_ENGINE
    if (engine_impl)
        impl = e;
#endif

    if (kdfalg != NULL) {
        int kdfnid = OBJ_sn2nid(kdfalg);

        if (kdfnid == NID_undef) {
            kdfnid = OBJ_ln2nid(kdfalg);
            if (kdfnid == NID_undef) {
                BIO_printf(bio_err, "The given KDF \"%s\" is unknown.\n",
                           kdfalg);
                return NULL;
            }
        }
        if (impl != NULL)
            ctx = EVP_PKEY_CTX_new_id(kdfnid, impl);
        else
            ctx = EVP_PKEY_CTX_new_from_name(libctx, kdfalg, propq);
    } else {
        if (pkey == NULL)
            return NULL;

        *pkeysize = EVP_PKEY_get_size(pkey);
        if (impl != NULL)
            ctx = EVP_PKEY_CTX_new(pkey, impl);
        else
            ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, propq);
    }

    if (ctx == NULL)
        return NULL;

    if (rawin) {
        EVP_MD_CTX_set_pkey_ctx(mctx, ctx);

        switch (pkey_op) {
        case EVP_PKEY_OP_SIGN:
            rv = EVP_DigestSignInit_ex(mctx, NULL, digestname, libctx, propq,
                                       pkey, NULL);
            break;

        case EVP_PKEY_OP_VERIFY:
            rv = EVP_DigestVerifyInit_ex(mctx, NULL, digestname, libctx, propq,
                                         pkey, NULL);
            break;
        }

    } else {
        switch (pkey_op) {
        case EVP_PKEY_OP_SIGN:
            rv = EVP_PKEY_sign_init(ctx);
            break;

        case EVP_PKEY_OP_VERIFY:
            rv = EVP_PKEY_verify_init(ctx);
            break;

        case EVP_PKEY_OP_VERIFYRECOVER:
            rv = EVP_PKEY_verify_recover_init(ctx);
            break;

        case EVP_PKEY_OP_ENCRYPT:
            rv = EVP_PKEY_encrypt_init(ctx);
            break;

        case EVP_PKEY_OP_DECRYPT:
            rv = EVP_PKEY_decrypt_init(ctx);
            break;

        case EVP_PKEY_OP_DERIVE:
            rv = EVP_PKEY_derive_init(ctx);
            break;

        case EVP_PKEY_OP_ENCAPSULATE:
            rv = EVP_PKEY_encapsulate_init(ctx, NULL);
            if (rv > 0 && kemop != NULL)
                rv = EVP_PKEY_CTX_set_kem_op(ctx, kemop);
            break;

        case EVP_PKEY_OP_DECAPSULATE:
            rv = EVP_PKEY_decapsulate_init(ctx, NULL);
            if (rv > 0 && kemop != NULL)
                rv = EVP_PKEY_CTX_set_kem_op(ctx, kemop);
            break;
        }
    }

    if (rv <= 0) {
        EVP_PKEY_CTX_free(ctx);
        ctx = NULL;
    }

    return ctx;
}

static int setup_peer(EVP_PKEY_CTX *ctx, int peerform, const char *file,
                      ENGINE *e)
{
    EVP_PKEY *pkey = EVP_PKEY_CTX_get0_pkey(ctx);
    EVP_PKEY *peer = NULL;
    ENGINE *engine = NULL;
    int ret = 1;

    if (peerform == FORMAT_ENGINE)
        engine = e;
    peer = load_pubkey(file, peerform, 0, NULL, engine, "peer key");
    if (peer == NULL) {
        BIO_printf(bio_err, "Error reading peer key %s\n", file);
        return 0;
    }
    if (strcmp(EVP_PKEY_get0_type_name(peer), EVP_PKEY_get0_type_name(pkey)) != 0) {
        BIO_printf(bio_err,
                   "Type of peer public key: %s does not match type of private key: %s\n",
                   EVP_PKEY_get0_type_name(peer), EVP_PKEY_get0_type_name(pkey));
        ret = 0;
    } else {
        ret = EVP_PKEY_derive_set_peer(ctx, peer) > 0;
    }

    EVP_PKEY_free(peer);
    return ret;
}

static int do_keyop(EVP_PKEY_CTX *ctx, int pkey_op,
                    unsigned char *out, size_t *poutlen,
                    const unsigned char *in, size_t inlen,
                    unsigned char *secret, size_t *pseclen)
{
    int rv = 0;

    switch (pkey_op) {
    case EVP_PKEY_OP_VERIFYRECOVER:
        rv = EVP_PKEY_verify_recover(ctx, out, poutlen, in, inlen);
        break;

    case EVP_PKEY_OP_SIGN:
        rv = EVP_PKEY_sign(ctx, out, poutlen, in, inlen);
        break;

    case EVP_PKEY_OP_ENCRYPT:
        rv = EVP_PKEY_encrypt(ctx, out, poutlen, in, inlen);
        break;

    case EVP_PKEY_OP_DECRYPT:
        rv = EVP_PKEY_decrypt(ctx, out, poutlen, in, inlen);
        break;

    case EVP_PKEY_OP_DERIVE:
        rv = EVP_PKEY_derive(ctx, out, poutlen);
        break;

    case EVP_PKEY_OP_ENCAPSULATE:
        rv = EVP_PKEY_encapsulate(ctx, out, poutlen, secret, pseclen);
        break;

    case EVP_PKEY_OP_DECAPSULATE:
        rv = EVP_PKEY_decapsulate(ctx, secret, pseclen, in, inlen);
        break;

    }
    return rv;
}

#define TBUF_MAXSIZE 2048

static int do_raw_keyop(int pkey_op, EVP_MD_CTX *mctx,
                        EVP_PKEY *pkey, BIO *in,
                        int filesize, unsigned char *sig, int siglen,
                        unsigned char **out, size_t *poutlen)
{
    int rv = 0;
    unsigned char tbuf[TBUF_MAXSIZE];
    unsigned char *mbuf = NULL;
    int buf_len = 0;

    /* Some algorithms only support oneshot digests */
    if (only_nomd(pkey)) {
        if (filesize < 0) {
            BIO_printf(bio_err,
                       "Error: unable to determine file size for oneshot operation\n");
            goto end;
        }
        mbuf = app_malloc(filesize, "oneshot sign/verify buffer");
        switch (pkey_op) {
        case EVP_PKEY_OP_VERIFY:
            buf_len = BIO_read(in, mbuf, filesize);
            if (buf_len != filesize) {
                BIO_printf(bio_err, "Error reading raw input data\n");
                goto end;
            }
            rv = EVP_DigestVerify(mctx, sig, (size_t)siglen, mbuf, buf_len);
            break;
        case EVP_PKEY_OP_SIGN:
            buf_len = BIO_read(in, mbuf, filesize);
            if (buf_len != filesize) {
                BIO_printf(bio_err, "Error reading raw input data\n");
                goto end;
            }
            rv = EVP_DigestSign(mctx, NULL, poutlen, mbuf, buf_len);
            if (rv == 1 && out != NULL) {
                *out = app_malloc(*poutlen, "buffer output");
                rv = EVP_DigestSign(mctx, *out, poutlen, mbuf, buf_len);
            }
            break;
        }
        goto end;
    }

    switch (pkey_op) {
    case EVP_PKEY_OP_VERIFY:
        for (;;) {
            buf_len = BIO_read(in, tbuf, TBUF_MAXSIZE);
            if (buf_len == 0)
                break;
            if (buf_len < 0) {
                BIO_printf(bio_err, "Error reading raw input data\n");
                goto end;
            }
            rv = EVP_DigestVerifyUpdate(mctx, tbuf, (size_t)buf_len);
            if (rv != 1) {
                BIO_printf(bio_err, "Error verifying raw input data\n");
                goto end;
            }
        }
        rv = EVP_DigestVerifyFinal(mctx, sig, (size_t)siglen);
        break;
    case EVP_PKEY_OP_SIGN:
        for (;;) {
            buf_len = BIO_read(in, tbuf, TBUF_MAXSIZE);
            if (buf_len == 0)
                break;
            if (buf_len < 0) {
                BIO_printf(bio_err, "Error reading raw input data\n");
                goto end;
            }
            rv = EVP_DigestSignUpdate(mctx, tbuf, (size_t)buf_len);
            if (rv != 1) {
                BIO_printf(bio_err, "Error signing raw input data\n");
                goto end;
            }
        }
        rv = EVP_DigestSignFinal(mctx, NULL, poutlen);
        if (rv == 1 && out != NULL) {
            *out = app_malloc(*poutlen, "buffer output");
            rv = EVP_DigestSignFinal(mctx, *out, poutlen);
        }
        break;
    }

 end:
    OPENSSL_free(mbuf);
    return rv;
}
