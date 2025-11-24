/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
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
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/x509.h>
#include <openssl/pem.h>
#include <openssl/hmac.h>
#include <ctype.h>

#undef BUFSIZE
#define BUFSIZE 1024*8

static int do_fp_oneshot_sign(BIO *out, EVP_MD_CTX *ctx, BIO *in, int sep, int binout,
                              EVP_PKEY *key, unsigned char *sigin, int siglen,
                              const char *sig_name, const char *file);
int do_fp(BIO *out, unsigned char *buf, BIO *bp, int sep, int binout, int xoflen,
          EVP_PKEY *key, unsigned char *sigin, int siglen,
          const char *sig_name, const char *md_name,
          const char *file);
static void show_digests(const OBJ_NAME *name, void *bio_);

struct doall_dgst_digests {
    BIO *bio;
    int n;
};

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_LIST,
    OPT_C, OPT_R, OPT_OUT, OPT_SIGN, OPT_PASSIN, OPT_VERIFY,
    OPT_PRVERIFY, OPT_SIGNATURE, OPT_KEYFORM, OPT_ENGINE, OPT_ENGINE_IMPL,
    OPT_HEX, OPT_BINARY, OPT_DEBUG, OPT_FIPS_FINGERPRINT,
    OPT_HMAC, OPT_MAC, OPT_SIGOPT, OPT_MACOPT, OPT_XOFLEN,
    OPT_DIGEST,
    OPT_R_ENUM, OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS dgst_options[] = {
    {OPT_HELP_STR, 1, '-', "Usage: %s [options] [file...]\n"},

    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"list", OPT_LIST, '-', "List digests"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine e, possibly a hardware device"},
    {"engine_impl", OPT_ENGINE_IMPL, '-',
     "Also use engine given by -engine for digest operations"},
#endif
    {"passin", OPT_PASSIN, 's', "Input file pass phrase source"},

    OPT_SECTION("Output"),
    {"c", OPT_C, '-', "Print the digest with separating colons"},
    {"r", OPT_R, '-', "Print the digest in coreutils format"},
    {"out", OPT_OUT, '>', "Output to filename rather than stdout"},
    {"keyform", OPT_KEYFORM, 'f', "Key file format (ENGINE, other values ignored)"},
    {"hex", OPT_HEX, '-', "Print as hex dump"},
    {"binary", OPT_BINARY, '-', "Print in binary form"},
    {"xoflen", OPT_XOFLEN, 'p', "Output length for XOF algorithms. To obtain the maximum security strength set this to 32 (or greater) for SHAKE128, and 64 (or greater) for SHAKE256"},
    {"d", OPT_DEBUG, '-', "Print debug info"},
    {"debug", OPT_DEBUG, '-', "Print debug info"},

    OPT_SECTION("Signing"),
    {"sign", OPT_SIGN, 's', "Sign digest using private key"},
    {"verify", OPT_VERIFY, 's', "Verify a signature using public key"},
    {"prverify", OPT_PRVERIFY, 's', "Verify a signature using private key"},
    {"sigopt", OPT_SIGOPT, 's', "Signature parameter in n:v form"},
    {"signature", OPT_SIGNATURE, '<', "File with signature to verify"},
    {"hmac", OPT_HMAC, 's', "Create hashed MAC with key"},
    {"mac", OPT_MAC, 's', "Create MAC (not necessarily HMAC)"},
    {"macopt", OPT_MACOPT, 's', "MAC algorithm parameters in n:v form or key"},
    {"", OPT_DIGEST, '-', "Any supported digest"},
    {"fips-fingerprint", OPT_FIPS_FINGERPRINT, '-',
     "Compute HMAC with the key used in OpenSSL-FIPS fingerprint"},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,

    OPT_PARAMETERS(),
    {"file", 0, 0, "Files to digest (optional; default is stdin)"},
    {NULL}
};

int dgst_main(int argc, char **argv)
{
    BIO *in = NULL, *inp = NULL, *bmd = NULL, *out = NULL;
    ENGINE *e = NULL, *impl = NULL;
    EVP_PKEY *sigkey = NULL;
    STACK_OF(OPENSSL_STRING) *sigopts = NULL, *macopts = NULL;
    char *hmac_key = NULL;
    char *mac_name = NULL, *digestname = NULL;
    char *passinarg = NULL, *passin = NULL;
    EVP_MD *md = NULL;
    const char *outfile = NULL, *keyfile = NULL, *prog = NULL;
    const char *sigfile = NULL;
    const char *md_name = NULL;
    OPTION_CHOICE o;
    int separator = 0, debug = 0, keyform = FORMAT_UNDEF, siglen = 0;
    int i, ret = EXIT_FAILURE, out_bin = -1, want_pub = 0, do_verify = 0;
    int xoflen = 0;
    unsigned char *buf = NULL, *sigbuf = NULL;
    int engine_impl = 0;
    struct doall_dgst_digests dec;
    EVP_MD_CTX *signctx = NULL;
    int oneshot_sign = 0;

    buf = app_malloc(BUFSIZE, "I/O buffer");
    md = (EVP_MD *)EVP_get_digestbyname(argv[0]);
    if (md != NULL)
        digestname = argv[0];

    opt_set_unknown_name("digest");
    prog = opt_init(argc, argv, dgst_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(dgst_options);
            ret = EXIT_SUCCESS;
            goto end;
        case OPT_LIST:
            BIO_printf(bio_out, "Supported digests:\n");
            dec.bio = bio_out;
            dec.n = 0;
            OBJ_NAME_do_all_sorted(OBJ_NAME_TYPE_MD_METH,
                                   show_digests, &dec);
            BIO_printf(bio_out, "\n");
            ret = EXIT_SUCCESS;
            goto end;
        case OPT_C:
            separator = 1;
            break;
        case OPT_R:
            separator = 2;
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_SIGN:
            keyfile = opt_arg();
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_VERIFY:
            keyfile = opt_arg();
            want_pub = do_verify = 1;
            break;
        case OPT_PRVERIFY:
            keyfile = opt_arg();
            do_verify = 1;
            break;
        case OPT_SIGNATURE:
            sigfile = opt_arg();
            break;
        case OPT_KEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &keyform))
                goto opthelp;
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_ENGINE_IMPL:
            engine_impl = 1;
            break;
        case OPT_HEX:
            out_bin = 0;
            break;
        case OPT_BINARY:
            out_bin = 1;
            break;
        case OPT_XOFLEN:
            xoflen = atoi(opt_arg());
            break;
        case OPT_DEBUG:
            debug = 1;
            break;
        case OPT_FIPS_FINGERPRINT:
            hmac_key = "etaonrishdlcupfm";
            break;
        case OPT_HMAC:
            hmac_key = opt_arg();
            break;
        case OPT_MAC:
            mac_name = opt_arg();
            break;
        case OPT_SIGOPT:
            if (!sigopts)
                sigopts = sk_OPENSSL_STRING_new_null();
            if (!sigopts || !sk_OPENSSL_STRING_push(sigopts, opt_arg()))
                goto opthelp;
            break;
        case OPT_MACOPT:
            if (!macopts)
                macopts = sk_OPENSSL_STRING_new_null();
            if (!macopts || !sk_OPENSSL_STRING_push(macopts, opt_arg()))
                goto opthelp;
            break;
        case OPT_DIGEST:
            digestname = opt_unknown();
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* Remaining args are files to digest. */
    argc = opt_num_rest();
    argv = opt_rest();
    if (keyfile != NULL && argc > 1) {
        BIO_printf(bio_err, "%s: Can only sign or verify one file.\n", prog);
        goto end;
    }
    if (!app_RAND_load())
        goto end;

    if (digestname != NULL) {
        if (!opt_md(digestname, &md))
            goto opthelp;
    }

    if (do_verify && sigfile == NULL) {
        BIO_printf(bio_err,
                   "No signature to verify: use the -signature option\n");
        goto end;
    }
    if (engine_impl)
        impl = e;

    in = BIO_new(BIO_s_file());
    bmd = BIO_new(BIO_f_md());
    if (in == NULL || bmd == NULL)
        goto end;

    if (debug) {
        BIO_set_callback_ex(in, BIO_debug_callback_ex);
        /* needed for windows 3.1 */
        BIO_set_callback_arg(in, (char *)bio_err);
    }

    if (!app_passwd(passinarg, NULL, &passin, NULL)) {
        BIO_printf(bio_err, "Error getting password\n");
        goto end;
    }

    if (out_bin == -1) {
        if (keyfile != NULL)
            out_bin = 1;
        else
            out_bin = 0;
    }

    out = bio_open_default(outfile, 'w', out_bin ? FORMAT_BINARY : FORMAT_TEXT);
    if (out == NULL)
        goto end;

    if ((!(mac_name == NULL) + !(keyfile == NULL) + !(hmac_key == NULL)) > 1) {
        BIO_printf(bio_err, "MAC and signing key cannot both be specified\n");
        goto end;
    }

    if (keyfile != NULL) {
        if (want_pub)
            sigkey = load_pubkey(keyfile, keyform, 0, NULL, e, "public key");
        else
            sigkey = load_key(keyfile, keyform, 0, passin, e, "private key");
        if (sigkey == NULL) {
            /*
             * load_[pub]key() has already printed an appropriate message
             */
            goto end;
        }
        {
            char def_md[80];

            if (EVP_PKEY_get_default_digest_name(sigkey, def_md,
                                                 sizeof(def_md)) == 2
                    && strcmp(def_md, "UNDEF") == 0)
                oneshot_sign = 1;
            signctx = EVP_MD_CTX_new();
            if (signctx == NULL)
                goto end;
        }
    }

    if (mac_name != NULL) {
        EVP_PKEY_CTX *mac_ctx = NULL;

        if (!init_gen_str(&mac_ctx, mac_name, impl, 0, NULL, NULL))
            goto end;
        if (macopts != NULL) {
            for (i = 0; i < sk_OPENSSL_STRING_num(macopts); i++) {
                char *macopt = sk_OPENSSL_STRING_value(macopts, i);

                if (pkey_ctrl_string(mac_ctx, macopt) <= 0) {
                    EVP_PKEY_CTX_free(mac_ctx);
                    BIO_printf(bio_err, "MAC parameter error \"%s\"\n", macopt);
                    goto end;
                }
            }
        }

        sigkey = app_keygen(mac_ctx, mac_name, 0, 0 /* not verbose */);
        /* Verbose output would make external-tests gost-engine fail */
        EVP_PKEY_CTX_free(mac_ctx);
        if (sigkey == NULL)
            goto end;
    }

    if (hmac_key != NULL) {
        if (md == NULL) {
            md = (EVP_MD *)EVP_sha256();
            digestname = SN_sha256;
        }
        sigkey = EVP_PKEY_new_raw_private_key(EVP_PKEY_HMAC, impl,
                                              (unsigned char *)hmac_key,
                                              strlen(hmac_key));
        if (sigkey == NULL)
            goto end;
    }

    if (sigkey != NULL) {
        EVP_MD_CTX *mctx = NULL;
        EVP_PKEY_CTX *pctx = NULL;
        int res;

        if (oneshot_sign) {
            mctx = signctx;
        } else if (BIO_get_md_ctx(bmd, &mctx) <= 0) {
            BIO_printf(bio_err, "Error getting context\n");
            goto end;
        }
        if (do_verify)
            if (impl == NULL)
                res = EVP_DigestVerifyInit_ex(mctx, &pctx, digestname,
                                              app_get0_libctx(),
                                              app_get0_propq(), sigkey, NULL);
            else
                res = EVP_DigestVerifyInit(mctx, &pctx, md, impl, sigkey);
        else
            if (impl == NULL)
                res = EVP_DigestSignInit_ex(mctx, &pctx, digestname,
                                            app_get0_libctx(),
                                            app_get0_propq(), sigkey, NULL);
            else
                res = EVP_DigestSignInit(mctx, &pctx, md, impl, sigkey);
        if (res == 0) {
            BIO_printf(bio_err, "Error setting context\n");
            goto end;
        }
        if (sigopts != NULL) {
            for (i = 0; i < sk_OPENSSL_STRING_num(sigopts); i++) {
                char *sigopt = sk_OPENSSL_STRING_value(sigopts, i);

                if (pkey_ctrl_string(pctx, sigopt) <= 0) {
                    BIO_printf(bio_err, "Signature parameter error \"%s\"\n",
                               sigopt);
                    goto end;
                }
            }
        }
    }
    /* we use md as a filter, reading from 'in' */
    else {
        EVP_MD_CTX *mctx = NULL;

        if (oneshot_sign) {
            BIO_printf(bio_err, "Oneshot algorithms don't use a digest\n");
            goto end;
        }
        if (BIO_get_md_ctx(bmd, &mctx) <= 0) {
            BIO_printf(bio_err, "Error getting context\n");
            goto end;
        }
        if (md == NULL)
            md = (EVP_MD *)EVP_sha256();
        if (!EVP_DigestInit_ex(mctx, md, impl)) {
            BIO_printf(bio_err, "Error setting digest\n");
            goto end;
        }
    }

    if (sigfile != NULL && sigkey != NULL) {
        BIO *sigbio = BIO_new_file(sigfile, "rb");

        if (sigbio == NULL) {
            BIO_printf(bio_err, "Error opening signature file %s\n", sigfile);
            goto end;
        }
        siglen = EVP_PKEY_get_size(sigkey);
        sigbuf = app_malloc(siglen, "signature buffer");
        siglen = BIO_read(sigbio, sigbuf, siglen);
        BIO_free(sigbio);
        if (siglen <= 0) {
            BIO_printf(bio_err, "Error reading signature file %s\n", sigfile);
            goto end;
        }
    }
    if (!oneshot_sign) {
        inp = BIO_push(bmd, in);

        if (md == NULL) {
            EVP_MD_CTX *tctx;

            BIO_get_md_ctx(bmd, &tctx);
            md = EVP_MD_CTX_get1_md(tctx);
        }
        if (md != NULL)
            md_name = EVP_MD_get0_name(md);
    }
    if (xoflen > 0) {
        if (!EVP_MD_xof(md)) {
            BIO_printf(bio_err, "Length can only be specified for XOF\n");
            goto end;
        }
        /*
         * Signing using XOF is not supported by any algorithms currently since
         * each algorithm only calls EVP_DigestFinal_ex() in their sign_final
         * and verify_final methods.
         */
        if (sigkey != NULL) {
            BIO_printf(bio_err, "Signing key cannot be specified for XOF\n");
            goto end;
        }
    }

    if (argc == 0) {
        BIO_set_fp(in, stdin, BIO_NOCLOSE);
        if (oneshot_sign)
            ret = do_fp_oneshot_sign(out, signctx, in, separator, out_bin,
                                     sigkey, sigbuf, siglen, NULL, "stdin");
        else
            ret = do_fp(out, buf, inp, separator, out_bin, xoflen,
                        sigkey, sigbuf, siglen, NULL, md_name, "stdin");
    } else {
        const char *sig_name = NULL;

        if (out_bin == 0) {
            if (sigkey != NULL)
                sig_name = EVP_PKEY_get0_type_name(sigkey);
        }
        ret = EXIT_SUCCESS;
        for (i = 0; i < argc; i++) {
            if (BIO_read_filename(in, argv[i]) <= 0) {
                perror(argv[i]);
                ret = EXIT_FAILURE;
                continue;
            } else {
                if (oneshot_sign) {
                    if (do_fp_oneshot_sign(out, signctx, in, separator, out_bin,
                                           sigkey, sigbuf, siglen, sig_name,
                                           argv[i]))
                        ret = EXIT_FAILURE;
                } else {
                    if (do_fp(out, buf, inp, separator, out_bin, xoflen,
                              sigkey, sigbuf, siglen, sig_name, md_name, argv[i]))
                        ret = EXIT_FAILURE;
                }
            }
            (void)BIO_reset(bmd);
        }
    }
 end:
    if (ret != EXIT_SUCCESS)
        ERR_print_errors(bio_err);
    OPENSSL_clear_free(buf, BUFSIZE);
    BIO_free(in);
    OPENSSL_free(passin);
    BIO_free_all(out);
    EVP_MD_free(md);
    EVP_PKEY_free(sigkey);
    EVP_MD_CTX_free(signctx);
    sk_OPENSSL_STRING_free(sigopts);
    sk_OPENSSL_STRING_free(macopts);
    OPENSSL_free(sigbuf);
    BIO_free(bmd);
    release_engine(e);
    return ret;
}

static void show_digests(const OBJ_NAME *name, void *arg)
{
    struct doall_dgst_digests *dec = (struct doall_dgst_digests *)arg;
    EVP_MD *md = NULL;

    /* Filter out signed digests (a.k.a signature algorithms) */
    if (strstr(name->name, "rsa") != NULL || strstr(name->name, "RSA") != NULL)
        return;

    if (!islower((unsigned char)*name->name))
        return;

    /* Filter out message digests that we cannot use */
    md = EVP_MD_fetch(app_get0_libctx(), name->name, app_get0_propq());
    if (md == NULL) {
        if (EVP_get_digestbyname(name->name) == NULL)
            return;
    }

    BIO_printf(dec->bio, "-%-25s", name->name);
    if (++dec->n == 3) {
        BIO_printf(dec->bio, "\n");
        dec->n = 0;
    } else {
        BIO_printf(dec->bio, " ");
    }

    EVP_MD_free(md);
}

/*
 * The newline_escape_filename function performs newline escaping for any
 * filename that contains a newline.  This function also takes a pointer
 * to backslash. The backslash pointer is a flag to indicating whether a newline
 * is present in the filename.  If a newline is present, the backslash flag is
 * set and the output format will contain a backslash at the beginning of the
 * digest output. This output format is to replicate the output format found
 * in the '*sum' checksum programs. This aims to preserve backward
 * compatibility.
 */
static const char *newline_escape_filename(const char *file, int *backslash)
{
    size_t i, e = 0, length = strlen(file), newline_count = 0, mem_len = 0;
    char *file_cpy = NULL;

    for (i = 0; i < length; i++)
        if (file[i] == '\n')
            newline_count++;

    mem_len = length + newline_count + 1;
    file_cpy = app_malloc(mem_len, file);
    i = 0;

    while (e < length) {
        const char c = file[e];
        if (c == '\n') {
            file_cpy[i++] = '\\';
            file_cpy[i++] = 'n';
            *backslash = 1;
        } else {
            file_cpy[i++] = c;
        }
        e++;
    }
    file_cpy[i] = '\0';
    return (const char*)file_cpy;
}

static void print_out(BIO *out, unsigned char *buf, size_t len,
                      int sep, int binout,
                      const char *sig_name, const char *md_name, const char *file)
{
    int i, backslash = 0;

    if (binout) {
        BIO_write(out, buf, len);
    } else if (sep == 2) {
        file = newline_escape_filename(file, &backslash);

        if (backslash == 1)
            BIO_puts(out, "\\");

        for (i = 0; i < (int)len; i++)
            BIO_printf(out, "%02x", buf[i]);

        BIO_printf(out, " *%s\n", file);
        OPENSSL_free((char *)file);
    } else {
        if (sig_name != NULL) {
            BIO_puts(out, sig_name);
            if (md_name != NULL)
                BIO_printf(out, "-%s", md_name);
            BIO_printf(out, "(%s)= ", file);
        } else if (md_name != NULL) {
            BIO_printf(out, "%s(%s)= ", md_name, file);
        } else {
            BIO_printf(out, "(%s)= ", file);
        }
        for (i = 0; i < (int)len; i++) {
            if (sep && (i != 0))
                BIO_printf(out, ":");
            BIO_printf(out, "%02x", buf[i]);
        }
        BIO_printf(out, "\n");
    }
}

static void print_verify_result(BIO *out, int i)
{
    if (i > 0)
        BIO_printf(out, "Verified OK\n");
    else if (i == 0)
        BIO_printf(out, "Verification failure\n");
    else
        BIO_printf(bio_err, "Error verifying data\n");
}

int do_fp(BIO *out, unsigned char *buf, BIO *bp, int sep, int binout, int xoflen,
          EVP_PKEY *key, unsigned char *sigin, int siglen,
          const char *sig_name, const char *md_name,
          const char *file)
{
    size_t len = BUFSIZE;
    int i, ret = EXIT_FAILURE;
    unsigned char *allocated_buf = NULL;

    while (BIO_pending(bp) || !BIO_eof(bp)) {
        i = BIO_read(bp, (char *)buf, BUFSIZE);
        if (i < 0) {
            BIO_printf(bio_err, "Read error in %s\n", file);
            goto end;
        }
        if (i == 0)
            break;
    }
    if (sigin != NULL) {
        EVP_MD_CTX *ctx;
        BIO_get_md_ctx(bp, &ctx);
        i = EVP_DigestVerifyFinal(ctx, sigin, (unsigned int)siglen);
        print_verify_result(out, i);
        if (i > 0)
            ret = EXIT_SUCCESS;
        goto end;
    }
    if (key != NULL) {
        EVP_MD_CTX *ctx;
        size_t tmplen;

        BIO_get_md_ctx(bp, &ctx);
        if (!EVP_DigestSignFinal(ctx, NULL, &tmplen)) {
            BIO_printf(bio_err, "Error getting maximum length of signed data\n");
            goto end;
        }
        if (tmplen > BUFSIZE) {
            len = tmplen;
            allocated_buf = app_malloc(len, "Signature buffer");
            buf = allocated_buf;
        }
        if (!EVP_DigestSignFinal(ctx, buf, &len)) {
            BIO_printf(bio_err, "Error signing data\n");
            goto end;
        }
    } else if (xoflen > 0) {
        EVP_MD_CTX *ctx;

        len = xoflen;
        if (len > BUFSIZE) {
            allocated_buf = app_malloc(len, "Digest buffer");
            buf = allocated_buf;
        }

        BIO_get_md_ctx(bp, &ctx);

        if (!EVP_DigestFinalXOF(ctx, buf, len)) {
            BIO_printf(bio_err, "Error Digesting Data\n");
            goto end;
        }
    } else {
        len = BIO_gets(bp, (char *)buf, BUFSIZE);
        if ((int)len < 0)
            goto end;
    }
    print_out(out, buf, len, sep, binout, sig_name, md_name, file);
    ret = EXIT_SUCCESS;
 end:
    if (allocated_buf != NULL)
        OPENSSL_clear_free(allocated_buf, len);

    return ret;
}

/*
 * Some new algorithms only support one shot operations.
 * For these we need to buffer all input and then do the sign on the
 * total buffered input. These algorithms set a NULL digest name which is
 * then used inside EVP_DigestVerify() and EVP_DigestSign().
 */
static int do_fp_oneshot_sign(BIO *out, EVP_MD_CTX *ctx, BIO *in, int sep, int binout,
                              EVP_PKEY *key, unsigned char *sigin, int siglen,
                              const char *sig_name, const char *file)
{
    int res, ret = EXIT_FAILURE;
    size_t len = 0;
    int buflen = 0;
    int maxlen = 16 * 1024 * 1024;
    uint8_t *buf = NULL, *sig = NULL;

    buflen = bio_to_mem(&buf, maxlen, in);
    if (buflen <= 0) {
        BIO_printf(bio_err, "Read error in %s\n", file);
        return ret;
    }
    if (sigin != NULL) {
        res = EVP_DigestVerify(ctx, sigin, siglen, buf, buflen);
        print_verify_result(out, res);
        if (res > 0)
            ret = EXIT_SUCCESS;
        goto end;
    }
    if (key != NULL) {
        if (EVP_DigestSign(ctx, NULL, &len, buf, buflen) != 1) {
            BIO_printf(bio_err, "Error getting maximum length of signed data\n");
            goto end;
        }
        sig = app_malloc(len, "Signature buffer");
        if (EVP_DigestSign(ctx, sig, &len, buf, buflen) != 1) {
            BIO_printf(bio_err, "Error signing data\n");
            goto end;
        }
        print_out(out, sig, len, sep, binout, sig_name, NULL, file);
        ret = EXIT_SUCCESS;
    } else {
        BIO_printf(bio_err, "key must be set for one-shot algorithms\n");
        goto end;
    }

 end:
    OPENSSL_free(sig);
    OPENSSL_clear_free(buf, buflen);

    return ret;
}
