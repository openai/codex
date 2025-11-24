/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include "apps.h"
#include "progs.h"
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/x509.h>
#include <openssl/rand.h>
#include <openssl/pem.h>
#ifndef OPENSSL_NO_COMP
# include <openssl/comp.h>
#endif
#include <ctype.h>

#undef SIZE
#undef BSIZE
#define SIZE    (512)
#define BSIZE   (8*1024)

#define PBKDF2_ITER_DEFAULT     10000
#define STR(a) XSTR(a)
#define XSTR(a) #a

static int set_hex(const char *in, unsigned char *out, int size);
static void show_ciphers(const OBJ_NAME *name, void *bio_);

struct doall_enc_ciphers {
    BIO *bio;
    int n;
};

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_LIST,
    OPT_E, OPT_IN, OPT_OUT, OPT_PASS, OPT_ENGINE, OPT_D, OPT_P, OPT_V,
    OPT_NOPAD, OPT_SALT, OPT_NOSALT, OPT_DEBUG, OPT_UPPER_P, OPT_UPPER_A,
    OPT_A, OPT_Z, OPT_BUFSIZE, OPT_K, OPT_KFILE, OPT_UPPER_K, OPT_NONE,
    OPT_UPPER_S, OPT_IV, OPT_MD, OPT_ITER, OPT_PBKDF2, OPT_CIPHER,
    OPT_SALTLEN, OPT_R_ENUM, OPT_PROV_ENUM,
    OPT_SKEYOPT, OPT_SKEYMGMT
} OPTION_CHOICE;

const OPTIONS enc_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"list", OPT_LIST, '-', "List ciphers"},
#ifndef OPENSSL_NO_DEPRECATED_3_0
    {"ciphers", OPT_LIST, '-', "Alias for -list"},
#endif
    {"e", OPT_E, '-', "Encrypt"},
    {"d", OPT_D, '-', "Decrypt"},
    {"p", OPT_P, '-', "Print the iv/key"},
    {"P", OPT_UPPER_P, '-', "Print the iv/key and exit"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif

    OPT_SECTION("Input"),
    {"in", OPT_IN, '<', "Input file"},
    {"k", OPT_K, 's', "Passphrase"},
    {"kfile", OPT_KFILE, '<', "Read passphrase from file"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file"},
    {"pass", OPT_PASS, 's', "Passphrase source"},
    {"v", OPT_V, '-', "Verbose output"},
    {"a", OPT_A, '-', "Base64 encode/decode, depending on encryption flag"},
    {"base64", OPT_A, '-', "Same as option -a"},
    {"A", OPT_UPPER_A, '-',
     "Used with -[base64|a] to specify base64 buffer as a single line"},

    OPT_SECTION("Encryption"),
    {"nopad", OPT_NOPAD, '-', "Disable standard block padding"},
    {"salt", OPT_SALT, '-', "Use salt in the KDF (default)"},
    {"nosalt", OPT_NOSALT, '-', "Do not use salt in the KDF"},
    {"debug", OPT_DEBUG, '-', "Print debug info"},

    {"bufsize", OPT_BUFSIZE, 's', "Buffer size"},
    {"K", OPT_UPPER_K, 's', "Raw key, in hex"},
    {"S", OPT_UPPER_S, 's', "Salt, in hex"},
    {"iv", OPT_IV, 's', "IV in hex"},
    {"md", OPT_MD, 's', "Use specified digest to create a key from the passphrase"},
    {"iter", OPT_ITER, 'p',
     "Specify the iteration count and force the use of PBKDF2"},
    {OPT_MORE_STR, 0, 0, "Default: " STR(PBKDF2_ITER_DEFAULT)},
    {"pbkdf2", OPT_PBKDF2, '-',
     "Use password-based key derivation function 2 (PBKDF2)"},
    {OPT_MORE_STR, 0, 0,
     "Use -iter to change the iteration count from " STR(PBKDF2_ITER_DEFAULT)},
    {"none", OPT_NONE, '-', "Don't encrypt"},
    {"saltlen", OPT_SALTLEN, 'p', "Specify the PBKDF2 salt length (in bytes)"},
    {OPT_MORE_STR, 0, 0, "Default: 16"},
#ifndef OPENSSL_NO_ZLIB
    {"z", OPT_Z, '-', "Compress or decompress encrypted data using zlib"},
#endif
    {"skeyopt", OPT_SKEYOPT, 's', "Key options as opt:value for opaque symmetric key handling"},
    {"skeymgmt", OPT_SKEYMGMT, 's', "Symmetric key management name for opaque symmetric key handling"},
    {"", OPT_CIPHER, '-', "Any supported cipher"},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,
    {NULL}
};

int enc_main(int argc, char **argv)
{
    static char buf[128];
    static const char magic[] = "Salted__";
    ENGINE *e = NULL;
    BIO *in = NULL, *out = NULL, *b64 = NULL, *benc = NULL, *rbio =
        NULL, *wbio = NULL;
    EVP_CIPHER_CTX *ctx = NULL;
    EVP_CIPHER *cipher = NULL;
    EVP_MD *dgst = NULL;
    const char *digestname = NULL;
    char *hkey = NULL, *hiv = NULL, *hsalt = NULL, *p;
    char *infile = NULL, *outfile = NULL, *prog;
    char *str = NULL, *passarg = NULL, *pass = NULL, *strbuf = NULL;
    const char *ciphername = NULL;
    char mbuf[sizeof(magic) - 1];
    OPTION_CHOICE o;
    int bsize = BSIZE, verbose = 0, debug = 0, olb64 = 0, nosalt = 0;
    int enc = 1, printkey = 0, i, k;
    int base64 = 0, informat = FORMAT_BINARY, outformat = FORMAT_BINARY;
    int ret = 1, inl, nopad = 0;
    unsigned char key[EVP_MAX_KEY_LENGTH], iv[EVP_MAX_IV_LENGTH];
    int rawkey_set = 0;
    unsigned char *buff = NULL, salt[EVP_MAX_IV_LENGTH];
    int saltlen = 0;
    int pbkdf2 = 0;
    int iter = 0;
    long n;
    int streamable = 1;
    int wrap = 0;
    struct doall_enc_ciphers dec;
#ifndef OPENSSL_NO_ZLIB
    int do_zlib = 0;
    BIO *bzl = NULL;
#endif
    int do_brotli = 0;
    BIO *bbrot = NULL;
    int do_zstd = 0;
    BIO *bzstd = NULL;
    STACK_OF(OPENSSL_STRING) *skeyopts = NULL;
    const char *skeymgmt = NULL;
    EVP_SKEY *skey = NULL;
    EVP_SKEYMGMT *mgmt = NULL;

    /* first check the command name */
    if (strcmp(argv[0], "base64") == 0)
        base64 = 1;
#ifndef OPENSSL_NO_ZLIB
    else if (strcmp(argv[0], "zlib") == 0)
        do_zlib = 1;
#endif
#ifndef OPENSSL_NO_BROTLI
    else if (strcmp(argv[0], "brotli") == 0)
        do_brotli = 1;
#endif
#ifndef OPENSSL_NO_ZSTD
    else if (strcmp(argv[0], "zstd") == 0)
        do_zstd = 1;
#endif
    else if (strcmp(argv[0], "enc") != 0)
        ciphername = argv[0];

    opt_set_unknown_name("cipher");
    prog = opt_init(argc, argv, enc_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(enc_options);
            ret = 0;
            goto end;
        case OPT_LIST:
            BIO_printf(bio_out, "Supported ciphers:\n");
            dec.bio = bio_out;
            dec.n = 0;
            OBJ_NAME_do_all_sorted(OBJ_NAME_TYPE_CIPHER_METH,
                                   show_ciphers, &dec);
            BIO_printf(bio_out, "\n");
            ret = 0;
            goto end;
        case OPT_E:
            enc = 1;
            break;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_PASS:
            passarg = opt_arg();
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_D:
            enc = 0;
            break;
        case OPT_P:
            printkey = 1;
            break;
        case OPT_V:
            verbose = 1;
            break;
        case OPT_NOPAD:
            nopad = 1;
            break;
        case OPT_SALT:
            nosalt = 0;
            break;
        case OPT_NOSALT:
            nosalt = 1;
            break;
        case OPT_DEBUG:
            debug = 1;
            break;
        case OPT_UPPER_P:
            printkey = 2;
            break;
        case OPT_UPPER_A:
            olb64 = 1;
            break;
        case OPT_A:
            base64 = 1;
            break;
        case OPT_Z:
#ifndef OPENSSL_NO_ZLIB
            do_zlib = 1;
#endif
            break;
        case OPT_BUFSIZE:
            p = opt_arg();
            i = (int)strlen(p) - 1;
            k = i >= 1 && p[i] == 'k';
            if (k)
                p[i] = '\0';
            if (!opt_long(opt_arg(), &n)
                    || n < 0 || (k && n >= LONG_MAX / 1024))
                goto opthelp;
            if (k)
                n *= 1024;
            bsize = (int)n;
            break;
        case OPT_K:
            str = opt_arg();
            break;
        case OPT_KFILE:
            in = bio_open_default(opt_arg(), 'r', FORMAT_TEXT);
            if (in == NULL)
                goto opthelp;
            i = BIO_gets(in, buf, sizeof(buf));
            BIO_free(in);
            in = NULL;
            if (i <= 0) {
                BIO_printf(bio_err,
                           "%s Can't read key from %s\n", prog, opt_arg());
                goto opthelp;
            }
            while (--i > 0 && (buf[i] == '\r' || buf[i] == '\n'))
                buf[i] = '\0';
            if (i <= 0) {
                BIO_printf(bio_err, "%s: zero length password\n", prog);
                goto opthelp;
            }
            str = buf;
            break;
        case OPT_UPPER_K:
            hkey = opt_arg();
            break;
        case OPT_UPPER_S:
            hsalt = opt_arg();
            break;
        case OPT_IV:
            hiv = opt_arg();
            break;
        case OPT_MD:
            digestname = opt_arg();
            break;
        case OPT_CIPHER:
            ciphername = opt_unknown();
            break;
        case OPT_ITER:
            iter = opt_int_arg();
            pbkdf2 = 1;
            break;
        case OPT_SALTLEN:
            if (!opt_int(opt_arg(), &saltlen))
                goto opthelp;
            if (saltlen > (int)sizeof(salt))
                saltlen = (int)sizeof(salt);
            break;
        case OPT_PBKDF2:
            pbkdf2 = 1;
            if (iter == 0)    /* do not overwrite a chosen value */
                iter = PBKDF2_ITER_DEFAULT;
            break;
        case OPT_NONE:
            cipher = NULL;
            break;
        case OPT_SKEYOPT:
            if ((skeyopts == NULL &&
                 (skeyopts = sk_OPENSSL_STRING_new_null()) == NULL) ||
                sk_OPENSSL_STRING_push(skeyopts, opt_arg()) == 0) {
                BIO_printf(bio_err, "%s: out of memory\n", prog);
                goto end;
            }
            break;
        case OPT_SKEYMGMT:
            skeymgmt = opt_arg();
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
    if (saltlen == 0 || pbkdf2 == 0)
        saltlen = PKCS5_SALT_LEN;

    /* Get the cipher name, either from progname (if set) or flag. */
    if (!opt_cipher(ciphername, &cipher))
        goto opthelp;
    if (cipher && (EVP_CIPHER_mode(cipher) == EVP_CIPH_WRAP_MODE)) {
        wrap = 1;
        streamable = 0;
    }
    if (digestname != NULL) {
        if (!opt_md(digestname, &dgst))
            goto opthelp;
    }
    if (dgst == NULL)
        dgst = (EVP_MD *)EVP_sha256();

    if (iter == 0)
        iter = 1;

    /* It must be large enough for a base64 encoded line */
    if (base64 && bsize < 80)
        bsize = 80;
    if (verbose)
        BIO_printf(bio_err, "bufsize=%d\n", bsize);

#ifndef OPENSSL_NO_ZLIB
    if (do_zlib)
        base64 = 0;
#endif
    if (do_brotli)
        base64 = 0;
    if (do_zstd)
        base64 = 0;

    if (base64) {
        if (enc)
            outformat = FORMAT_BASE64;
        else
            informat = FORMAT_BASE64;
    }

    strbuf = app_malloc(SIZE, "strbuf");
    buff = app_malloc(EVP_ENCODE_LENGTH(bsize), "evp buffer");

    if (infile == NULL) {
        if (!streamable && printkey != 2) {  /* if just print key and exit, it's ok */
            BIO_printf(bio_err, "Unstreamable cipher mode\n");
            goto end;
        }
        in = dup_bio_in(informat);
    } else {
        in = bio_open_default(infile, 'r', informat);
    }
    if (in == NULL)
        goto end;

    if (str == NULL && passarg != NULL) {
        if (!app_passwd(passarg, NULL, &pass, NULL)) {
            BIO_printf(bio_err, "Error getting password\n");
            goto end;
        }
        str = pass;
    }

    if ((str == NULL) && (cipher != NULL) && (hkey == NULL) && (skeyopts == NULL)) {
        if (1) {
#ifndef OPENSSL_NO_UI_CONSOLE
            for (;;) {
                char prompt[200];

                BIO_snprintf(prompt, sizeof(prompt), "enter %s %s password:",
                        EVP_CIPHER_get0_name(cipher),
                        (enc) ? "encryption" : "decryption");
                strbuf[0] = '\0';
                i = EVP_read_pw_string((char *)strbuf, SIZE, prompt, enc);
                if (i == 0) {
                    if (strbuf[0] == '\0') {
                        ret = 1;
                        goto end;
                    }
                    str = strbuf;
                    break;
                }
                if (i < 0) {
                    BIO_printf(bio_err, "bad password read\n");
                    goto end;
                }
            }
        } else {
#endif
            BIO_printf(bio_err, "password required\n");
            goto end;
        }
    }

    out = bio_open_default(outfile, 'w', outformat);
    if (out == NULL)
        goto end;

    if (debug) {
        BIO_set_callback_ex(in, BIO_debug_callback_ex);
        BIO_set_callback_ex(out, BIO_debug_callback_ex);
        BIO_set_callback_arg(in, (char *)bio_err);
        BIO_set_callback_arg(out, (char *)bio_err);
    }

    rbio = in;
    wbio = out;

#ifndef OPENSSL_NO_COMP
# ifndef OPENSSL_NO_ZLIB
    if (do_zlib) {
        if ((bzl = BIO_new(BIO_f_zlib())) == NULL)
            goto end;
        if (debug) {
            BIO_set_callback_ex(bzl, BIO_debug_callback_ex);
            BIO_set_callback_arg(bzl, (char *)bio_err);
        }
        if (enc)
            wbio = BIO_push(bzl, wbio);
        else
            rbio = BIO_push(bzl, rbio);
    }
# endif

    if (do_brotli) {
        if ((bbrot = BIO_new(BIO_f_brotli())) == NULL)
            goto end;
        if (debug) {
            BIO_set_callback_ex(bbrot, BIO_debug_callback_ex);
            BIO_set_callback_arg(bbrot, (char *)bio_err);
        }
        if (enc)
            wbio = BIO_push(bbrot, wbio);
        else
            rbio = BIO_push(bbrot, rbio);
    }

    if (do_zstd) {
        if ((bzstd = BIO_new(BIO_f_zstd())) == NULL)
            goto end;
        if (debug) {
            BIO_set_callback_ex(bzstd, BIO_debug_callback_ex);
            BIO_set_callback_arg(bzstd, (char *)bio_err);
        }
        if (enc)
            wbio = BIO_push(bzstd, wbio);
        else
            rbio = BIO_push(bzstd, rbio);
    }
#endif

    if (base64) {
        if ((b64 = BIO_new(BIO_f_base64())) == NULL)
            goto end;
        if (debug) {
            BIO_set_callback_ex(b64, BIO_debug_callback_ex);
            BIO_set_callback_arg(b64, (char *)bio_err);
        }
        if (olb64)
            BIO_set_flags(b64, BIO_FLAGS_BASE64_NO_NL);
        if (enc)
            wbio = BIO_push(b64, wbio);
        else
            rbio = BIO_push(b64, rbio);
    }

    if (cipher != NULL) {
        if (str != NULL) { /* a passphrase is available */
            /*
             * Salt handling: if encrypting generate a salt if not supplied,
             * and write to output BIO. If decrypting use salt from input BIO
             * if not given with args
             */
            unsigned char *sptr;
            size_t str_len = strlen(str);

            if (nosalt) {
                sptr = NULL;
            } else {
                if (hsalt != NULL && !set_hex(hsalt, salt, saltlen)) {
                    BIO_printf(bio_err, "invalid hex salt value\n");
                    goto end;
                }
                if (enc) {  /* encryption */
                    if (hsalt == NULL) {
                        if (RAND_bytes(salt, saltlen) <= 0) {
                            BIO_printf(bio_err, "RAND_bytes failed\n");
                            goto end;
                        }
                        /*
                         * If -P option then don't bother writing.
                         * If salt is given, shouldn't either ?
                         */
                        if ((printkey != 2)
                            && (BIO_write(wbio, magic,
                                          sizeof(magic) - 1) != sizeof(magic) - 1
                                || BIO_write(wbio,
                                             (char *)salt,
                                             saltlen) != saltlen)) {
                            BIO_printf(bio_err, "error writing output file\n");
                            goto end;
                        }
                    }
                } else {    /* decryption */
                    if (hsalt == NULL) {
                        if (BIO_read(rbio, mbuf, sizeof(mbuf)) != sizeof(mbuf)) {
                            BIO_printf(bio_err, "error reading input file\n");
                            goto end;
                        }
                        if (memcmp(mbuf, magic, sizeof(mbuf)) == 0) { /* file IS salted */
                            if (BIO_read(rbio, salt,
                                         saltlen) != saltlen) {
                                BIO_printf(bio_err, "error reading input file\n");
                                goto end;
                            }
                        } else { /* file is NOT salted, NO salt available */
                            BIO_printf(bio_err, "bad magic number\n");
                            goto end;
                        }
                    }
                }
                sptr = salt;
            }

            if (pbkdf2 == 1) {
                /*
                * derive key and default iv
                * concatenated into a temporary buffer
                */
                unsigned char tmpkeyiv[EVP_MAX_KEY_LENGTH + EVP_MAX_IV_LENGTH];
                int iklen = EVP_CIPHER_get_key_length(cipher);
                int ivlen = EVP_CIPHER_get_iv_length(cipher);
                /* not needed if HASH_UPDATE() is fixed : */
                int islen = (sptr != NULL ? saltlen : 0);

                if (!PKCS5_PBKDF2_HMAC(str, str_len, sptr, islen,
                                       iter, dgst, iklen+ivlen, tmpkeyiv)) {
                    BIO_printf(bio_err, "PKCS5_PBKDF2_HMAC failed\n");
                    goto end;
                }
                /* split and move data back to global buffer */
                memcpy(key, tmpkeyiv, iklen);
                memcpy(iv, tmpkeyiv+iklen, ivlen);
                rawkey_set = 1;
            } else {
                BIO_printf(bio_err, "*** WARNING : "
                                    "deprecated key derivation used.\n"
                                    "Using -iter or -pbkdf2 would be better.\n");
                if (!EVP_BytesToKey(cipher, dgst, sptr,
                                    (unsigned char *)str, str_len,
                                    1, key, iv)) {
                    BIO_printf(bio_err, "EVP_BytesToKey failed\n");
                    goto end;
                }
                rawkey_set = 1;
            }
            /*
             * zero the complete buffer or the string passed from the command
             * line.
             */
            if (str == strbuf)
                OPENSSL_cleanse(str, SIZE);
            else
                OPENSSL_cleanse(str, str_len);
        }
        if (hiv != NULL) {
            int siz = EVP_CIPHER_get_iv_length(cipher);

            if (siz == 0) {
                BIO_printf(bio_err, "warning: iv not used by this cipher\n");
            } else if (!set_hex(hiv, iv, siz)) {
                BIO_printf(bio_err, "invalid hex iv value\n");
                goto end;
            }
        }
        if ((hiv == NULL) && (str == NULL)
            && EVP_CIPHER_get_iv_length(cipher) != 0
            && wrap == 0) {
            /*
             * No IV was explicitly set and no IV was generated.
             * Hence the IV is undefined, making correct decryption impossible.
             */
            BIO_printf(bio_err, "iv undefined\n");
            goto end;
        }
        if (hkey != NULL) {
            if (!set_hex(hkey, key, EVP_CIPHER_get_key_length(cipher))) {
                BIO_printf(bio_err, "invalid hex key value\n");
                goto end;
            }
            /* wiping secret data as we no longer need it */
            cleanse(hkey);
            rawkey_set = 1;
        }

        /*
         * At this moment we know whether we trying to use raw bytes as the key
         * or an opaque symmetric key. We do not allow both options simultaneously.
         */
        if (rawkey_set > 0 && skeyopts != NULL) {
            BIO_printf(bio_err, "Either a raw key or the 'skeyopt' args must be used.\n");
            goto end;
        }

        if ((benc = BIO_new(BIO_f_cipher())) == NULL)
            goto end;

        /*
         * Since we may be changing parameters work on the encryption context
         * rather than calling BIO_set_cipher().
         */

        BIO_get_cipher_ctx(benc, &ctx);

        if (wrap == 1)
            EVP_CIPHER_CTX_set_flags(ctx, EVP_CIPHER_CTX_FLAG_WRAP_ALLOW);

        if (rawkey_set) {
            if (!EVP_CipherInit_ex(ctx, cipher, e, key,
                                   (hiv == NULL && wrap == 1 ? NULL : iv), enc)) {
                BIO_printf(bio_err, "Error setting cipher %s\n",
                           EVP_CIPHER_get0_name(cipher));
                ERR_print_errors(bio_err);
                goto end;
            }
        } else {
            OSSL_PARAM *params = NULL;

            mgmt = EVP_SKEYMGMT_fetch(app_get0_libctx(),
                                      skeymgmt != NULL ? skeymgmt : EVP_CIPHER_name(cipher),
                                      app_get0_propq());
            if (mgmt == NULL)
                goto end;

            params = app_params_new_from_opts(skeyopts,
                                              EVP_SKEYMGMT_get0_imp_settable_params(mgmt));
            if (params == NULL)
                goto end;

            skey = EVP_SKEY_import(app_get0_libctx(), EVP_SKEYMGMT_get0_name(mgmt),
                                   app_get0_propq(), OSSL_SKEYMGMT_SELECT_ALL, params);
            OSSL_PARAM_free(params);
            if (skey == NULL) {
                BIO_printf(bio_err, "Error creating opaque key object for skeymgmt %s\n",
                           skeymgmt ? skeymgmt : EVP_CIPHER_name(cipher));
                ERR_print_errors(bio_err);
                goto end;
            }

            if (!EVP_CipherInit_SKEY(ctx, cipher, skey,
                                     (hiv == NULL && wrap == 1 ? NULL : iv),
                                     EVP_CIPHER_get_iv_length(cipher), enc, NULL)) {
                BIO_printf(bio_err, "Error setting an opaque key for cipher %s\n",
                           EVP_CIPHER_get0_name(cipher));
                ERR_print_errors(bio_err);
                goto end;
            }
        }

        if (nopad)
            EVP_CIPHER_CTX_set_padding(ctx, 0);

        if (debug) {
            BIO_set_callback_ex(benc, BIO_debug_callback_ex);
            BIO_set_callback_arg(benc, (char *)bio_err);
        }

        if (printkey) {
            if (!nosalt) {
                printf("salt=");
                for (i = 0; i < (int)saltlen; i++)
                    printf("%02X", salt[i]);
                printf("\n");
            }
            if (EVP_CIPHER_get_key_length(cipher) > 0) {
                printf("key=");
                for (i = 0; i < EVP_CIPHER_get_key_length(cipher); i++)
                    printf("%02X", key[i]);
                printf("\n");
            }
            if (EVP_CIPHER_get_iv_length(cipher) > 0) {
                printf("iv =");
                for (i = 0; i < EVP_CIPHER_get_iv_length(cipher); i++)
                    printf("%02X", iv[i]);
                printf("\n");
            }
            if (printkey == 2) {
                ret = 0;
                goto end;
            }
        }
    }

    /* Only encrypt/decrypt as we write the file */
    if (benc != NULL)
        wbio = BIO_push(benc, wbio);

    while (BIO_pending(rbio) || !BIO_eof(rbio)) {
        inl = BIO_read(rbio, (char *)buff, bsize);
        if (inl <= 0)
            break;
        if (!streamable && !BIO_eof(rbio)) {    /* do not output data */
            BIO_printf(bio_err, "Unstreamable cipher mode\n");
            goto end;
        }
        if (BIO_write(wbio, (char *)buff, inl) != inl) {
            BIO_printf(bio_err, "error writing output file\n");
            goto end;
        }
        if (!streamable)
            break;
    }
    if (!BIO_flush(wbio)) {
        if (enc)
            BIO_printf(bio_err, "bad encrypt\n");
        else
            BIO_printf(bio_err, "bad decrypt\n");
        goto end;
    }

    ret = 0;
    if (verbose) {
        BIO_printf(bio_err, "bytes read   : %8ju\n", BIO_number_read(in));
        BIO_printf(bio_err, "bytes written: %8ju\n", BIO_number_written(out));
    }
 end:
    ERR_print_errors(bio_err);
    sk_OPENSSL_STRING_free(skeyopts);
    EVP_SKEYMGMT_free(mgmt);
    EVP_SKEY_free(skey);
    OPENSSL_free(strbuf);
    OPENSSL_free(buff);
    BIO_free(in);
    BIO_free_all(out);
    BIO_free(benc);
    BIO_free(b64);
    EVP_MD_free(dgst);
    EVP_CIPHER_free(cipher);
#ifndef OPENSSL_NO_ZLIB
    BIO_free(bzl);
#endif
    BIO_free(bbrot);
    BIO_free(bzstd);
    release_engine(e);
    OPENSSL_free(pass);
    return ret;
}

static void show_ciphers(const OBJ_NAME *name, void *arg)
{
    struct doall_enc_ciphers *dec = (struct doall_enc_ciphers *)arg;
    const EVP_CIPHER *cipher;

    if (!islower((unsigned char)*name->name))
        return;

    /* Filter out ciphers that we cannot use */
    cipher = EVP_get_cipherbyname(name->name);
    if (cipher == NULL
            || (EVP_CIPHER_get_flags(cipher) & EVP_CIPH_FLAG_AEAD_CIPHER) != 0
            || EVP_CIPHER_get_mode(cipher) == EVP_CIPH_XTS_MODE)
        return;

    BIO_printf(dec->bio, "-%-25s", name->name);
    if (++dec->n == 3) {
        BIO_printf(dec->bio, "\n");
        dec->n = 0;
    } else
        BIO_printf(dec->bio, " ");
}

static int set_hex(const char *in, unsigned char *out, int size)
{
    int i, n;
    unsigned char j;

    i = size * 2;
    n = strlen(in);
    if (n > i) {
        BIO_printf(bio_err, "hex string is too long, ignoring excess\n");
        n = i; /* ignore exceeding part */
    } else if (n < i) {
        BIO_printf(bio_err, "hex string is too short, padding with zero bytes to length\n");
    }

    memset(out, 0, size);
    for (i = 0; i < n; i++) {
        j = (unsigned char)*in++;
        if (!isxdigit(j)) {
            BIO_printf(bio_err, "non-hex digit\n");
            return 0;
        }
        j = (unsigned char)OPENSSL_hexchar2int(j);
        if (i & 1)
            out[i / 2] |= j;
        else
            out[i / 2] = (j << 4);
    }
    return 1;
}
