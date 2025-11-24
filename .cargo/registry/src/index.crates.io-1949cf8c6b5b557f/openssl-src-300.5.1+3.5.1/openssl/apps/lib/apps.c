/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#if !defined(_POSIX_C_SOURCE) && defined(OPENSSL_SYS_VMS)
/*
 * On VMS, you need to define this to get the declaration of fileno().  The
 * value 2 is to make sure no function defined in POSIX-2 is left undefined.
 */
# define _POSIX_C_SOURCE 2
#endif

#ifndef OPENSSL_NO_ENGINE
/* We need to use some deprecated APIs */
# define OPENSSL_SUPPRESS_DEPRECATED
# include <openssl/engine.h>
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#ifndef OPENSSL_NO_POSIX_IO
# include <sys/stat.h>
# include <fcntl.h>
#endif
#include <ctype.h>
#include <errno.h>
#include <openssl/err.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/http.h>
#include <openssl/pem.h>
#include <openssl/store.h>
#include <openssl/pkcs12.h>
#include <openssl/ui.h>
#include <openssl/safestack.h>
#include <openssl/rsa.h>
#include <openssl/rand.h>
#include <openssl/bn.h>
#include <openssl/ssl.h>
#include <openssl/core_names.h>
#include "s_apps.h"
#include "apps.h"

#include "internal/sockets.h" /* for openssl_fdset() */
#include "internal/e_os.h"

#ifdef _WIN32
static int WIN32_rename(const char *from, const char *to);
# define rename(from, to) WIN32_rename((from), (to))
#endif

#if defined(OPENSSL_SYS_WINDOWS) || defined(OPENSSL_SYS_MSDOS)
# include <conio.h>
#endif

#if defined(OPENSSL_SYS_MSDOS) && !defined(_WIN32) || defined(__BORLANDC__)
# define _kbhit kbhit
#endif

static BIO *bio_open_default_(const char *filename, char mode, int format,
                              int quiet);

#define PASS_SOURCE_SIZE_MAX 4

DEFINE_STACK_OF(CONF)

typedef struct {
    const char *name;
    unsigned long flag;
    unsigned long mask;
} NAME_EX_TBL;

static int set_table_opts(unsigned long *flags, const char *arg,
                          const NAME_EX_TBL *in_tbl);
static int set_multi_opts(unsigned long *flags, const char *arg,
                          const NAME_EX_TBL *in_tbl);
int app_init(long mesgwin);

int chopup_args(ARGS *arg, char *buf)
{
    int quoted;
    char c = '\0', *p = NULL;

    arg->argc = 0;
    if (arg->size == 0) {
        arg->size = 20;
        arg->argv = app_malloc(sizeof(*arg->argv) * arg->size, "argv space");
    }

    for (p = buf;;) {
        /* Skip whitespace. */
        while (*p && isspace(_UC(*p)))
            p++;
        if (*p == '\0')
            break;

        /* The start of something good :-) */
        if (arg->argc >= arg->size) {
            char **tmp;

            arg->size += 20;
            tmp = OPENSSL_realloc(arg->argv, sizeof(*arg->argv) * arg->size);
            if (tmp == NULL)
                return 0;
            arg->argv = tmp;
        }
        quoted = *p == '\'' || *p == '"';
        if (quoted)
            c = *p++;
        arg->argv[arg->argc++] = p;

        /* now look for the end of this */
        if (quoted) {
            while (*p && *p != c)
                p++;
            *p++ = '\0';
        } else {
            while (*p && !isspace(_UC(*p)))
                p++;
            if (*p)
                *p++ = '\0';
        }
    }
    arg->argv[arg->argc] = NULL;
    return 1;
}

#ifndef APP_INIT
int app_init(long mesgwin)
{
    return 1;
}
#endif

int ctx_set_verify_locations(SSL_CTX *ctx,
                             const char *CAfile, int noCAfile,
                             const char *CApath, int noCApath,
                             const char *CAstore, int noCAstore)
{
    if (CAfile == NULL && CApath == NULL && CAstore == NULL) {
        if (!noCAfile && SSL_CTX_set_default_verify_file(ctx) <= 0)
            return 0;
        if (!noCApath && SSL_CTX_set_default_verify_dir(ctx) <= 0)
            return 0;
        if (!noCAstore && SSL_CTX_set_default_verify_store(ctx) <= 0)
            return 0;

        return 1;
    }

    if (CAfile != NULL && !SSL_CTX_load_verify_file(ctx, CAfile))
        return 0;
    if (CApath != NULL && !SSL_CTX_load_verify_dir(ctx, CApath))
        return 0;
    if (CAstore != NULL && !SSL_CTX_load_verify_store(ctx, CAstore))
        return 0;
    return 1;
}

#ifndef OPENSSL_NO_CT

int ctx_set_ctlog_list_file(SSL_CTX *ctx, const char *path)
{
    if (path == NULL)
        return SSL_CTX_set_default_ctlog_list_file(ctx);

    return SSL_CTX_set_ctlog_list_file(ctx, path);
}

#endif

static unsigned long nmflag = 0;
static char nmflag_set = 0;

int set_nameopt(const char *arg)
{
    int ret = set_name_ex(&nmflag, arg);

    if (ret)
        nmflag_set = 1;

    return ret;
}

unsigned long get_nameopt(void)
{
    return
        nmflag_set ? nmflag : XN_FLAG_SEP_CPLUS_SPC | XN_FLAG_FN_SN
                              | ASN1_STRFLGS_ESC_CTRL
                              | ASN1_STRFLGS_UTF8_CONVERT
                              | ASN1_STRFLGS_DUMP_UNKNOWN
                              | ASN1_STRFLGS_DUMP_DER;
}

void dump_cert_text(BIO *out, X509 *x)
{
    print_name(out, "subject=", X509_get_subject_name(x));
    print_name(out, "issuer=", X509_get_issuer_name(x));
}

int wrap_password_callback(char *buf, int bufsiz, int verify, void *userdata)
{
    return password_callback(buf, bufsiz, verify, (PW_CB_DATA *)userdata);
}

static char *app_get_pass(const char *arg, int keepbio);

char *get_passwd(const char *pass, const char *desc)
{
    char *result = NULL;

    if (desc == NULL)
        desc = "<unknown>";
    if (!app_passwd(pass, NULL, &result, NULL))
        BIO_printf(bio_err, "Error getting password for %s\n", desc);
    if (pass != NULL && result == NULL) {
        BIO_printf(bio_err,
                   "Trying plain input string (better precede with 'pass:')\n");
        result = OPENSSL_strdup(pass);
        if (result == NULL)
            BIO_printf(bio_err,
                       "Out of memory getting password for %s\n", desc);
    }
    return result;
}

int app_passwd(const char *arg1, const char *arg2, char **pass1, char **pass2)
{
    int same = arg1 != NULL && arg2 != NULL && strcmp(arg1, arg2) == 0;

    if (arg1 != NULL) {
        *pass1 = app_get_pass(arg1, same);
        if (*pass1 == NULL)
            return 0;
    } else if (pass1 != NULL) {
        *pass1 = NULL;
    }
    if (arg2 != NULL) {
        *pass2 = app_get_pass(arg2, same ? 2 : 0);
        if (*pass2 == NULL)
            return 0;
    } else if (pass2 != NULL) {
        *pass2 = NULL;
    }
    return 1;
}

static char *app_get_pass(const char *arg, int keepbio)
{
    static BIO *pwdbio = NULL;
    char *tmp, tpass[APP_PASS_LEN];
    int i;

    /* PASS_SOURCE_SIZE_MAX = max number of chars before ':' in below strings */
    if (CHECK_AND_SKIP_PREFIX(arg, "pass:"))
        return OPENSSL_strdup(arg);
    if (CHECK_AND_SKIP_PREFIX(arg, "env:")) {
        tmp = getenv(arg);
        if (tmp == NULL) {
            BIO_printf(bio_err, "No environment variable %s\n", arg);
            return NULL;
        }
        return OPENSSL_strdup(tmp);
    }
    if (!keepbio || pwdbio == NULL) {
        if (CHECK_AND_SKIP_PREFIX(arg, "file:")) {
            pwdbio = BIO_new_file(arg, "r");
            if (pwdbio == NULL) {
                BIO_printf(bio_err, "Can't open file %s\n", arg);
                return NULL;
            }
#if !defined(_WIN32)
            /*
             * Under _WIN32, which covers even Win64 and CE, file
             * descriptors referenced by BIO_s_fd are not inherited
             * by child process and therefore below is not an option.
             * It could have been an option if bss_fd.c was operating
             * on real Windows descriptors, such as those obtained
             * with CreateFile.
             */
        } else if (CHECK_AND_SKIP_PREFIX(arg, "fd:")) {
            BIO *btmp;

            i = atoi(arg);
            if (i >= 0)
                pwdbio = BIO_new_fd(i, BIO_NOCLOSE);
            if ((i < 0) || pwdbio == NULL) {
                BIO_printf(bio_err, "Can't access file descriptor %s\n", arg);
                return NULL;
            }
            /*
             * Can't do BIO_gets on an fd BIO so add a buffering BIO
             */
            btmp = BIO_new(BIO_f_buffer());
            if (btmp == NULL) {
                BIO_free_all(pwdbio);
                pwdbio = NULL;
                BIO_printf(bio_err, "Out of memory\n");
                return NULL;
            }
            pwdbio = BIO_push(btmp, pwdbio);
#endif
        } else if (strcmp(arg, "stdin") == 0) {
            unbuffer(stdin);
            pwdbio = dup_bio_in(FORMAT_TEXT);
            if (pwdbio == NULL) {
                BIO_printf(bio_err, "Can't open BIO for stdin\n");
                return NULL;
            }
        } else {
            /* argument syntax error; do not reveal too much about arg */
            tmp = strchr(arg, ':');
            if (tmp == NULL || tmp - arg > PASS_SOURCE_SIZE_MAX)
                BIO_printf(bio_err,
                           "Invalid password argument, missing ':' within the first %d chars\n",
                           PASS_SOURCE_SIZE_MAX + 1);
            else
                BIO_printf(bio_err,
                           "Invalid password argument, starting with \"%.*s\"\n",
                           (int)(tmp - arg + 1), arg);
            return NULL;
        }
    }
    i = BIO_gets(pwdbio, tpass, APP_PASS_LEN);
    if (keepbio != 1) {
        BIO_free_all(pwdbio);
        pwdbio = NULL;
    }
    if (i <= 0) {
        BIO_printf(bio_err, "Error reading password from BIO\n");
        return NULL;
    }
    tmp = strchr(tpass, '\n');
    if (tmp != NULL)
        *tmp = 0;
    return OPENSSL_strdup(tpass);
}

char *app_conf_try_string(const CONF *conf, const char *group, const char *name)
{
    char *res;

    ERR_set_mark();
    res = NCONF_get_string(conf, group, name);
    if (res == NULL)
        ERR_pop_to_mark();
    else
        ERR_clear_last_mark();
    return res;
}

int app_conf_try_number(const CONF *conf, const char *group, const char *name,
                        long *result)
{
    int ok;

    ERR_set_mark();
    ok = NCONF_get_number(conf, group, name, result);
    if (!ok)
        ERR_pop_to_mark();
    else
        ERR_clear_last_mark();
    return ok;
}

CONF *app_load_config_bio(BIO *in, const char *filename)
{
    long errorline = -1;
    CONF *conf;
    int i;

    conf = NCONF_new_ex(app_get0_libctx(), NULL);
    i = NCONF_load_bio(conf, in, &errorline);
    if (i > 0)
        return conf;

    if (errorline <= 0) {
        BIO_printf(bio_err, "%s: Can't load ", opt_getprog());
    } else {
        BIO_printf(bio_err, "%s: Error on line %ld of ", opt_getprog(),
                   errorline);
    }
    if (filename != NULL)
        BIO_printf(bio_err, "config file \"%s\"\n", filename);
    else
        BIO_printf(bio_err, "config input");

    NCONF_free(conf);
    return NULL;
}

CONF *app_load_config_verbose(const char *filename, int verbose)
{
    if (verbose) {
        if (*filename == '\0')
            BIO_printf(bio_err, "No configuration used\n");
        else
            BIO_printf(bio_err, "Using configuration from %s\n", filename);
    }
    return app_load_config_internal(filename, 0);
}

CONF *app_load_config_internal(const char *filename, int quiet)
{
    BIO *in;
    CONF *conf;

    if (filename == NULL || *filename != '\0') {
        if ((in = bio_open_default_(filename, 'r', FORMAT_TEXT, quiet)) == NULL)
            return NULL;
        conf = app_load_config_bio(in, filename);
        BIO_free(in);
    } else {
        /* Return empty config if filename is empty string. */
        conf = NCONF_new_ex(app_get0_libctx(), NULL);
    }
    return conf;
}

int app_load_modules(const CONF *config)
{
    CONF *to_free = NULL;

    if (config == NULL)
        config = to_free = app_load_config_quiet(default_config_file);
    if (config == NULL)
        return 1;

    if (CONF_modules_load(config, NULL, 0) <= 0) {
        BIO_printf(bio_err, "Error configuring OpenSSL modules\n");
        ERR_print_errors(bio_err);
        NCONF_free(to_free);
        return 0;
    }
    NCONF_free(to_free);
    return 1;
}

int add_oid_section(CONF *conf)
{
    char *p;
    STACK_OF(CONF_VALUE) *sktmp;
    CONF_VALUE *cnf;
    int i;

    if ((p = app_conf_try_string(conf, NULL, "oid_section")) == NULL)
        return 1;
    if ((sktmp = NCONF_get_section(conf, p)) == NULL) {
        BIO_printf(bio_err, "problem loading oid section %s\n", p);
        return 0;
    }
    for (i = 0; i < sk_CONF_VALUE_num(sktmp); i++) {
        cnf = sk_CONF_VALUE_value(sktmp, i);
        if (OBJ_create(cnf->value, cnf->name, cnf->name) == NID_undef) {
            BIO_printf(bio_err, "problem creating object %s=%s\n",
                       cnf->name, cnf->value);
            return 0;
        }
    }
    return 1;
}

CONF *app_load_config_modules(const char *configfile)
{
    CONF *conf = NULL;

    if (configfile != NULL) {
        if ((conf = app_load_config_verbose(configfile, 1)) == NULL)
            return NULL;
        if (configfile != default_config_file && !app_load_modules(conf)) {
            NCONF_free(conf);
            conf = NULL;
        }
    }
    return conf;
}

#define IS_HTTP(uri) ((uri) != NULL  && HAS_PREFIX(uri, OSSL_HTTP_PREFIX))
#define IS_HTTPS(uri) ((uri) != NULL && HAS_PREFIX(uri, OSSL_HTTPS_PREFIX))

X509 *load_cert_pass(const char *uri, int format, int maybe_stdin,
                     const char *pass, const char *desc)
{
    X509 *cert = NULL;

    if (desc == NULL)
        desc = "certificate";
    if (IS_HTTPS(uri)) {
        BIO_printf(bio_err, "Loading %s over HTTPS is unsupported\n", desc);
    } else if (IS_HTTP(uri)) {
        cert = X509_load_http(uri, NULL, NULL, 0 /* timeout */);
        if (cert == NULL) {
            ERR_print_errors(bio_err);
            BIO_printf(bio_err, "Unable to load %s from %s\n", desc, uri);
        }
    } else {
        (void)load_key_certs_crls(uri, format, maybe_stdin, pass, desc, 0,
                                  NULL, NULL, NULL, &cert, NULL, NULL, NULL);
    }
    return cert;
}

X509_CRL *load_crl(const char *uri, int format, int maybe_stdin,
                   const char *desc)
{
    X509_CRL *crl = NULL;

    if (desc == NULL)
        desc = "CRL";
    if (IS_HTTPS(uri)) {
        BIO_printf(bio_err, "Loading %s over HTTPS is unsupported\n", desc);
    } else if (IS_HTTP(uri)) {
        crl = X509_CRL_load_http(uri, NULL, NULL, 0 /* timeout */);
        if (crl == NULL) {
            ERR_print_errors(bio_err);
            BIO_printf(bio_err, "Unable to load %s from %s\n", desc, uri);
        }
    } else {
        (void)load_key_certs_crls(uri, format, maybe_stdin, NULL, desc, 0,
                                  NULL, NULL,  NULL, NULL, NULL, &crl, NULL);
    }
    return crl;
}

/* Could be simplified if OSSL_STORE supported CSRs, see FR #15725 */
X509_REQ *load_csr(const char *file, int format, const char *desc)
{
    X509_REQ *req = NULL;
    BIO *in;

    if (format == FORMAT_UNDEF)
        format = FORMAT_PEM;
    in = bio_open_default(file, 'r', format);
    if (in == NULL)
        goto end;

    if (format == FORMAT_ASN1)
        req = d2i_X509_REQ_bio(in, NULL);
    else if (format == FORMAT_PEM)
        req = PEM_read_bio_X509_REQ(in, NULL, NULL, NULL);
    else
        print_format_error(format, OPT_FMT_PEMDER);

 end:
    if (req == NULL) {
        ERR_print_errors(bio_err);
        if (desc != NULL)
            BIO_printf(bio_err, "Unable to load %s\n", desc);
    }
    BIO_free(in);
    return req;
}

/* Better extend OSSL_STORE to support CSRs, see FR #15725 */
X509_REQ *load_csr_autofmt(const char *infile, int format,
                           STACK_OF(OPENSSL_STRING) *vfyopts, const char *desc)
{
    X509_REQ *csr;

    if (format != FORMAT_UNDEF) {
        csr = load_csr(infile, format, desc);
    } else { /* try PEM, then DER */
        BIO *bio_bak = bio_err;

        bio_err = NULL; /* do not show errors on more than one try */
        csr = load_csr(infile, FORMAT_PEM, NULL /* desc */);
        bio_err = bio_bak;
        if (csr == NULL) {
            ERR_clear_error();
            csr = load_csr(infile, FORMAT_ASN1, NULL /* desc */);
        }
        if (csr == NULL) {
            BIO_printf(bio_err, "error: unable to load %s from file '%s'\n",
                       desc, infile);
        }
    }
    if (csr != NULL) {
        EVP_PKEY *pkey = X509_REQ_get0_pubkey(csr);
        int ret = do_X509_REQ_verify(csr, pkey, vfyopts);

        if (pkey == NULL || ret < 0)
            BIO_puts(bio_err, "Warning: error while verifying CSR self-signature\n");
        else if (ret == 0)
            BIO_puts(bio_err, "Warning: CSR self-signature does not match the contents\n");
        return csr;
    }
    return csr;
}

void cleanse(char *str)
{
    if (str != NULL)
        OPENSSL_cleanse(str, strlen(str));
}

void clear_free(char *str)
{
    if (str != NULL)
        OPENSSL_clear_free(str, strlen(str));
}

EVP_PKEY *load_key(const char *uri, int format, int may_stdin,
                   const char *pass, ENGINE *e, const char *desc)
{
    EVP_PKEY *pkey = NULL;
    char *allocated_uri = NULL;

    if (desc == NULL)
        desc = "private key";

    if (format == FORMAT_ENGINE)
        uri = allocated_uri = make_engine_uri(e, uri, desc);
    (void)load_key_certs_crls(uri, format, may_stdin, pass, desc, 0,
                              &pkey, NULL, NULL, NULL, NULL, NULL, NULL);

    OPENSSL_free(allocated_uri);
    return pkey;
}

/* first try reading public key, on failure resort to loading private key */
EVP_PKEY *load_pubkey(const char *uri, int format, int maybe_stdin,
                      const char *pass, ENGINE *e, const char *desc)
{
    EVP_PKEY *pkey = NULL;
    char *allocated_uri = NULL;

    if (desc == NULL)
        desc = "public key";

    if (format == FORMAT_ENGINE)
        uri = allocated_uri = make_engine_uri(e, uri, desc);
    (void)load_key_certs_crls(uri, format, maybe_stdin, pass, desc, 1,
                              NULL, &pkey, NULL, NULL, NULL, NULL, NULL);
    if (pkey == NULL)
        (void)load_key_certs_crls(uri, format, maybe_stdin, pass, desc, 0,
                                  &pkey, NULL, NULL, NULL, NULL, NULL, NULL);
    OPENSSL_free(allocated_uri);
    return pkey;
}

EVP_PKEY *load_keyparams_suppress(const char *uri, int format, int maybe_stdin,
                                  const char *keytype, const char *desc,
                                  int suppress_decode_errors)
{
    EVP_PKEY *params = NULL;

    if (desc == NULL)
        desc = "key parameters";
    (void)load_key_certs_crls(uri, format, maybe_stdin, NULL, desc,
                              suppress_decode_errors,
                              NULL, NULL, &params, NULL, NULL, NULL, NULL);
    if (params != NULL && keytype != NULL && !EVP_PKEY_is_a(params, keytype)) {
        ERR_print_errors(bio_err);
        BIO_printf(bio_err,
                   "Unable to load %s from %s (unexpected parameters type)\n",
                   desc, uri);
        EVP_PKEY_free(params);
        params = NULL;
    }
    return params;
}

EVP_PKEY *load_keyparams(const char *uri, int format, int maybe_stdin,
                         const char *keytype, const char *desc)
{
    return load_keyparams_suppress(uri, format, maybe_stdin, keytype, desc, 0);
}

void app_bail_out(char *fmt, ...)
{
    va_list args;

    va_start(args, fmt);
    BIO_vprintf(bio_err, fmt, args);
    va_end(args);
    ERR_print_errors(bio_err);
    exit(EXIT_FAILURE);
}

void *app_malloc(size_t sz, const char *what)
{
    void *vp = OPENSSL_malloc(sz);

    if (vp == NULL)
        app_bail_out("%s: Could not allocate %zu bytes for %s\n",
                     opt_getprog(), sz, what);
    return vp;
}

char *next_item(char *opt) /* in list separated by comma and/or space */
{
    /* advance to separator (comma or whitespace), if any */
    while (*opt != ',' && !isspace(_UC(*opt)) && *opt != '\0')
        opt++;
    if (*opt != '\0') {
        /* terminate current item */
        *opt++ = '\0';
        /* skip over any whitespace after separator */
        while (isspace(_UC(*opt)))
            opt++;
    }
    return *opt == '\0' ? NULL : opt; /* NULL indicates end of input */
}

static void warn_cert_msg(const char *uri, X509 *cert, const char *msg)
{
    char *subj = X509_NAME_oneline(X509_get_subject_name(cert), NULL, 0);

    BIO_printf(bio_err, "Warning: certificate from '%s' with subject '%s' %s\n",
               uri, subj, msg);
    OPENSSL_free(subj);
}

static void warn_cert(const char *uri, X509 *cert, int warn_EE,
                      X509_VERIFY_PARAM *vpm)
{
    uint32_t ex_flags = X509_get_extension_flags(cert);
    int res = X509_cmp_timeframe(vpm, X509_get0_notBefore(cert),
                                 X509_get0_notAfter(cert));

    if (res != 0)
        warn_cert_msg(uri, cert, res > 0 ? "has expired" : "not yet valid");
    if (warn_EE && (ex_flags & EXFLAG_V1) == 0 && (ex_flags & EXFLAG_CA) == 0)
        warn_cert_msg(uri, cert, "is not a CA cert");
}

static void warn_certs(const char *uri, STACK_OF(X509) *certs, int warn_EE,
                       X509_VERIFY_PARAM *vpm)
{
    int i;

    for (i = 0; i < sk_X509_num(certs); i++)
        warn_cert(uri, sk_X509_value(certs, i), warn_EE, vpm);
}

int load_cert_certs(const char *uri,
                    X509 **pcert, STACK_OF(X509) **pcerts,
                    int exclude_http, const char *pass, const char *desc,
                    X509_VERIFY_PARAM *vpm)
{
    int ret = 0;
    char *pass_string;

    if (desc == NULL)
        desc = pcerts == NULL ? "certificate" : "certificates";
    if (exclude_http && (HAS_CASE_PREFIX(uri, "http://")
                         || HAS_CASE_PREFIX(uri, "https://"))) {
        BIO_printf(bio_err, "error: HTTP retrieval not allowed for %s\n", desc);
        return ret;
    }
    pass_string = get_passwd(pass, desc);
    ret = load_key_certs_crls(uri, FORMAT_UNDEF, 0, pass_string, desc, 0,
                              NULL, NULL, NULL, pcert, pcerts, NULL, NULL);
    clear_free(pass_string);

    if (ret) {
        if (pcert != NULL)
            warn_cert(uri, *pcert, 0, vpm);
        if (pcerts != NULL)
            warn_certs(uri, *pcerts, 1, vpm);
    } else {
        if (pcerts != NULL) {
            OSSL_STACK_OF_X509_free(*pcerts);
            *pcerts = NULL;
        }
    }
    return ret;
}

STACK_OF(X509) *load_certs_multifile(char *files, const char *pass,
                                     const char *desc, X509_VERIFY_PARAM *vpm)
{
    STACK_OF(X509) *certs = NULL;
    STACK_OF(X509) *result = sk_X509_new_null();

    if (files == NULL)
        goto err;
    if (result == NULL)
        goto oom;

    while (files != NULL) {
        char *next = next_item(files);

        if (!load_cert_certs(files, NULL, &certs, 0, pass, desc, vpm))
            goto err;
        if (!X509_add_certs(result, certs,
                            X509_ADD_FLAG_UP_REF | X509_ADD_FLAG_NO_DUP))
            goto oom;
        OSSL_STACK_OF_X509_free(certs);
        certs = NULL;
        files = next;
    }
    return result;

 oom:
    BIO_printf(bio_err, "out of memory\n");
 err:
    OSSL_STACK_OF_X509_free(certs);
    OSSL_STACK_OF_X509_free(result);
    return NULL;
}

static X509_STORE *sk_X509_to_store(X509_STORE *store /* may be NULL */,
                                    const STACK_OF(X509) *certs /* may NULL */)
{
    int i;

    if (store == NULL)
        store = X509_STORE_new();
    if (store == NULL)
        return NULL;
    for (i = 0; i < sk_X509_num(certs); i++) {
        if (!X509_STORE_add_cert(store, sk_X509_value(certs, i))) {
            X509_STORE_free(store);
            return NULL;
        }
    }
    return store;
}

/*
 * Create cert store structure with certificates read from given file(s).
 * Returns pointer to created X509_STORE on success, NULL on error.
 */
X509_STORE *load_certstore(char *input, const char *pass, const char *desc,
                           X509_VERIFY_PARAM *vpm)
{
    X509_STORE *store = NULL;
    STACK_OF(X509) *certs = NULL;

    while (input != NULL) {
        char *next = next_item(input);
        int ok;

        if (!load_cert_certs(input, NULL, &certs, 1, pass, desc, vpm)) {
            X509_STORE_free(store);
            return NULL;
        }
        ok = (store = sk_X509_to_store(store, certs)) != NULL;
        OSSL_STACK_OF_X509_free(certs);
        certs = NULL;
        if (!ok)
            return NULL;
        input = next;
    }
    return store;
}

/*
 * Initialize or extend, if *certs != NULL, a certificate stack.
 * The caller is responsible for freeing *certs if its value is left not NULL.
 */
int load_certs(const char *uri, int maybe_stdin, STACK_OF(X509) **certs,
               const char *pass, const char *desc)
{
    int ret, was_NULL = *certs == NULL;

    if (desc == NULL)
        desc = "certificates";
    ret = load_key_certs_crls(uri, FORMAT_UNDEF, maybe_stdin, pass, desc, 0,
                              NULL, NULL, NULL, NULL, certs, NULL, NULL);

    if (!ret && was_NULL) {
        OSSL_STACK_OF_X509_free(*certs);
        *certs = NULL;
    }
    return ret;
}

/*
 * Initialize or extend, if *crls != NULL, a certificate stack.
 * The caller is responsible for freeing *crls if its value is left not NULL.
 */
int load_crls(const char *uri, STACK_OF(X509_CRL) **crls,
              const char *pass, const char *desc)
{
    int ret, was_NULL = *crls == NULL;

    if (desc == NULL)
        desc = "CRLs";
    ret = load_key_certs_crls(uri, FORMAT_UNDEF, 0, pass, desc, 0,
                              NULL, NULL, NULL, NULL, NULL, NULL, crls);

    if (!ret && was_NULL) {
        sk_X509_CRL_pop_free(*crls, X509_CRL_free);
        *crls = NULL;
    }
    return ret;
}

static const char *format2string(int format)
{
    switch (format) {
    case FORMAT_PEM:
        return "PEM";
    case FORMAT_ASN1:
        return "DER";
    case FORMAT_PVK:
        return "PVK";
    case FORMAT_MSBLOB:
        return "MSBLOB";
    }
    return NULL;
}

/* Set type expectation, but set to 0 if objects of multiple types expected. */
#define SET_EXPECT(val) \
    (expect = expect < 0 ? (val) : (expect == (val) ? (val) : 0))
#define SET_EXPECT1(pvar, val) \
    if ((pvar) != NULL) { \
        *(pvar) = NULL; \
        SET_EXPECT(val); \
    }
/* Provide (error msg) text for some of the credential types to be loaded. */
#define FAIL_NAME \
    (ppkey != NULL ? "private key" : ppubkey != NULL ? "public key" :  \
     pparams != NULL ? "key parameters" :                              \
     pcert != NULL ? "certificate" : pcerts != NULL ? "certificates" : \
     pcrl != NULL ? "CRL" : pcrls != NULL ? "CRLs" : NULL)
/*
 * Load those types of credentials for which the result pointer is not NULL.
 * Reads from stdin if 'uri' is NULL and 'maybe_stdin' is nonzero.
 * 'format' parameter may be FORMAT_PEM, FORMAT_ASN1, or 0 for no hint.
 * desc may contain more detail on the credential(s) to be loaded for error msg
 * For non-NULL ppkey, pcert, and pcrl the first suitable value found is loaded.
 * If pcerts is non-NULL and *pcerts == NULL then a new cert list is allocated.
 * If pcerts is non-NULL then all available certificates are appended to *pcerts
 * except any certificate assigned to *pcert.
 * If pcrls is non-NULL and *pcrls == NULL then a new list of CRLs is allocated.
 * If pcrls is non-NULL then all available CRLs are appended to *pcerts
 * except any CRL assigned to *pcrl.
 * In any case (also on error) the caller is responsible for freeing all members
 * of *pcerts and *pcrls (as far as they are not NULL).
 */
int load_key_certs_crls(const char *uri, int format, int maybe_stdin,
                        const char *pass, const char *desc, int quiet,
                        EVP_PKEY **ppkey, EVP_PKEY **ppubkey,
                        EVP_PKEY **pparams,
                        X509 **pcert, STACK_OF(X509) **pcerts,
                        X509_CRL **pcrl, STACK_OF(X509_CRL) **pcrls)
{
    PW_CB_DATA uidata;
    OSSL_STORE_CTX *ctx = NULL;
    OSSL_LIB_CTX *libctx = app_get0_libctx();
    const char *propq = app_get0_propq();
    int ncerts = 0, ncrls = 0, expect = -1;
    const char *failed = FAIL_NAME;
    const char *input_type;
    OSSL_PARAM itp[2];
    const OSSL_PARAM *params = NULL;

    /* 'failed' describes type of credential to load for potential error msg */
    if (failed == NULL) {
        if (!quiet)
            BIO_printf(bio_err, "Internal error: nothing was requested to load from %s\n",
                       uri != NULL ? uri : "<stdin>");
        return 0;
    }
    /* suppress any extraneous errors left over from failed parse attempts */
    ERR_set_mark();

    SET_EXPECT1(ppkey, OSSL_STORE_INFO_PKEY);
    SET_EXPECT1(ppubkey, OSSL_STORE_INFO_PUBKEY);
    SET_EXPECT1(pparams, OSSL_STORE_INFO_PARAMS);
    SET_EXPECT1(pcert, OSSL_STORE_INFO_CERT);
    /*
     * Up to here, the follwing holds.
     * If just one of the ppkey, ppubkey, pparams, and pcert function parameters
     * is nonzero, expect > 0 indicates which type of credential is expected.
     * If expect == 0, more than one of them is nonzero (multiple types expected).
     */

    if (pcerts != NULL) {
        if (*pcerts == NULL && (*pcerts = sk_X509_new_null()) == NULL) {
            if (!quiet)
                BIO_printf(bio_err, "Out of memory loading");
            goto end;
        }
        /*
         * Adapt the 'expect' variable:
         * set to OSSL_STORE_INFO_CERT if no other type is expected so far,
         * otherwise set to 0 (indicating that multiple types are expected).
         */
        SET_EXPECT(OSSL_STORE_INFO_CERT);
    }
    SET_EXPECT1(pcrl, OSSL_STORE_INFO_CRL);
    if (pcrls != NULL) {
        if (*pcrls == NULL && (*pcrls = sk_X509_CRL_new_null()) == NULL) {
            if (!quiet)
                BIO_printf(bio_err, "Out of memory loading");
            goto end;
        }
        /*
         * Adapt the 'expect' variable:
         * set to OSSL_STORE_INFO_CRL if no other type is expected so far,
         * otherwise set to 0 (indicating that multiple types are expected).
         */
        SET_EXPECT(OSSL_STORE_INFO_CRL);
    }

    uidata.password = pass;
    uidata.prompt_info = uri;

    if ((input_type = format2string(format)) != NULL) {
        itp[0] = OSSL_PARAM_construct_utf8_string(OSSL_STORE_PARAM_INPUT_TYPE,
                                                  (char *)input_type, 0);
        itp[1] = OSSL_PARAM_construct_end();
        params = itp;
    }

    if (uri == NULL) {
        BIO *bio;

        if (!maybe_stdin) {
            if (!quiet)
                BIO_printf(bio_err, "No filename or uri specified for loading\n");
            goto end;
        }
        uri = "<stdin>";
        unbuffer(stdin);
        bio = BIO_new_fp(stdin, 0);
        if (bio != NULL) {
            ctx = OSSL_STORE_attach(bio, "file", libctx, propq,
                                    get_ui_method(), &uidata, params,
                                    NULL, NULL);
            BIO_free(bio);
        }
    } else {
        ctx = OSSL_STORE_open_ex(uri, libctx, propq, get_ui_method(), &uidata,
                                 params, NULL, NULL);
    }
    if (ctx == NULL) {
        if (!quiet)
            BIO_printf(bio_err, "Could not open file or uri for loading");
        goto end;
    }
    /* expect == 0 means here multiple types of credentials are to be loaded */
    if (expect > 0 && !OSSL_STORE_expect(ctx, expect)) {
        if (!quiet)
            BIO_printf(bio_err, "Internal error trying to load");
        goto end;
    }

    failed = NULL;
    /* from here, failed != NULL only if actually an error has been detected */

    while ((ppkey != NULL || ppubkey != NULL || pparams != NULL
            || pcert != NULL || pcerts != NULL || pcrl != NULL || pcrls != NULL)
           && !OSSL_STORE_eof(ctx)) {
        OSSL_STORE_INFO *info = OSSL_STORE_load(ctx);
        int type, ok = 1;

        /*
         * This can happen (for example) if we attempt to load a file with
         * multiple different types of things in it - but the thing we just
         * tried to load wasn't one of the ones we wanted, e.g. if we're trying
         * to load a certificate but the file has both the private key and the
         * certificate in it. We just retry until eof.
         */
        if (info == NULL) {
            continue;
        }

        type = OSSL_STORE_INFO_get_type(info);
        switch (type) {
        case OSSL_STORE_INFO_PKEY:
            if (ppkey != NULL) {
                ok = (*ppkey = OSSL_STORE_INFO_get1_PKEY(info)) != NULL;
                if (ok)
                    ppkey = NULL;
                break;
            }
            /*
             * An EVP_PKEY with private parts also holds the public parts,
             * so if the caller asked for a public key, and we got a private
             * key, we can still pass it back.
             */
            /* fall through */
        case OSSL_STORE_INFO_PUBKEY:
            if (ppubkey != NULL) {
                ok = (*ppubkey = OSSL_STORE_INFO_get1_PUBKEY(info)) != NULL;
                if (ok)
                    ppubkey = NULL;
            }
            break;
        case OSSL_STORE_INFO_PARAMS:
            if (pparams != NULL) {
                ok = (*pparams = OSSL_STORE_INFO_get1_PARAMS(info)) != NULL;
                if (ok)
                    pparams = NULL;
            }
            break;
        case OSSL_STORE_INFO_CERT:
            if (pcert != NULL) {
                ok = (*pcert = OSSL_STORE_INFO_get1_CERT(info)) != NULL;
                if (ok)
                    pcert = NULL;
            } else if (pcerts != NULL) {
                ok = X509_add_cert(*pcerts,
                                   OSSL_STORE_INFO_get1_CERT(info),
                                   X509_ADD_FLAG_DEFAULT);
            }
            ncerts += ok;
            break;
        case OSSL_STORE_INFO_CRL:
            if (pcrl != NULL) {
                ok = (*pcrl = OSSL_STORE_INFO_get1_CRL(info)) != NULL;
                if (ok)
                    pcrl = NULL;
            } else if (pcrls != NULL) {
                ok = sk_X509_CRL_push(*pcrls, OSSL_STORE_INFO_get1_CRL(info));
            }
            ncrls += ok;
            break;
        default:
            /* skip any other type; ok stays == 1 */
            break;
        }
        OSSL_STORE_INFO_free(info);
        if (!ok) {
            failed = OSSL_STORE_INFO_type_string(type);
            if (!quiet)
                BIO_printf(bio_err, "Error reading");
            break;
        }
    }

 end:
    OSSL_STORE_close(ctx);

    /* see if any of the requested types of credentials was not found */
    if (failed == NULL) {
        if (ncerts > 0)
            pcerts = NULL;
        if (ncrls > 0)
            pcrls = NULL;
        failed = FAIL_NAME;
        if (failed != NULL && !quiet)
            BIO_printf(bio_err, "Could not find");
    }

    if (failed != NULL && !quiet) {
        unsigned long err = ERR_peek_last_error();

        /* continue the error message with the type of credential affected */
        if (desc != NULL && strstr(desc, failed) != NULL) {
            BIO_printf(bio_err, " %s", desc);
        } else {
            BIO_printf(bio_err, " %s", failed);
            if (desc != NULL)
                BIO_printf(bio_err, " of %s", desc);
        }
        if (uri != NULL)
            BIO_printf(bio_err, " from %s", uri);
        if (ERR_SYSTEM_ERROR(err)) {
            /* provide more readable diagnostic output */
            BIO_printf(bio_err, ": %s", strerror(ERR_GET_REASON(err)));
            ERR_pop_to_mark();
            ERR_set_mark();
        }
        BIO_printf(bio_err, "\n");
        ERR_print_errors(bio_err);
    }
    if (quiet || failed == NULL)
        /* clear any suppressed or spurious errors */
        ERR_pop_to_mark();
    else
        ERR_clear_last_mark();
    return failed == NULL;
}

#define X509V3_EXT_UNKNOWN_MASK  (0xfL << 16)
#define X509V3_EXT_DEFAULT       0          /* Return error for unknown exts */
#define X509V3_EXT_ERROR_UNKNOWN (1L << 16) /* Print error for unknown exts */
#define X509V3_EXT_PARSE_UNKNOWN (2L << 16) /* ASN1 parse unknown extensions */
#define X509V3_EXT_DUMP_UNKNOWN  (3L << 16) /* BIO_dump unknown extensions */

#define X509_FLAG_CA (X509_FLAG_NO_ISSUER | X509_FLAG_NO_PUBKEY | \
                      X509_FLAG_NO_HEADER | X509_FLAG_NO_VERSION)

int set_cert_ex(unsigned long *flags, const char *arg)
{
    static const NAME_EX_TBL cert_tbl[] = {
        {"compatible", X509_FLAG_COMPAT, 0xffffffffl},
        {"ca_default", X509_FLAG_CA, 0xffffffffl},
        {"no_header", X509_FLAG_NO_HEADER, 0},
        {"no_version", X509_FLAG_NO_VERSION, 0},
        {"no_serial", X509_FLAG_NO_SERIAL, 0},
        {"no_signame", X509_FLAG_NO_SIGNAME, 0},
        {"no_validity", X509_FLAG_NO_VALIDITY, 0},
        {"no_subject", X509_FLAG_NO_SUBJECT, 0},
        {"no_issuer", X509_FLAG_NO_ISSUER, 0},
        {"no_pubkey", X509_FLAG_NO_PUBKEY, 0},
        {"no_extensions", X509_FLAG_NO_EXTENSIONS, 0},
        {"no_sigdump", X509_FLAG_NO_SIGDUMP, 0},
        {"no_aux", X509_FLAG_NO_AUX, 0},
        {"no_attributes", X509_FLAG_NO_ATTRIBUTES, 0},
        {"ext_default", X509V3_EXT_DEFAULT, X509V3_EXT_UNKNOWN_MASK},
        {"ext_error", X509V3_EXT_ERROR_UNKNOWN, X509V3_EXT_UNKNOWN_MASK},
        {"ext_parse", X509V3_EXT_PARSE_UNKNOWN, X509V3_EXT_UNKNOWN_MASK},
        {"ext_dump", X509V3_EXT_DUMP_UNKNOWN, X509V3_EXT_UNKNOWN_MASK},
        {NULL, 0, 0}
    };
    return set_multi_opts(flags, arg, cert_tbl);
}

int set_name_ex(unsigned long *flags, const char *arg)
{
    static const NAME_EX_TBL ex_tbl[] = {
        {"esc_2253", ASN1_STRFLGS_ESC_2253, 0},
        {"esc_2254", ASN1_STRFLGS_ESC_2254, 0},
        {"esc_ctrl", ASN1_STRFLGS_ESC_CTRL, 0},
        {"esc_msb", ASN1_STRFLGS_ESC_MSB, 0},
        {"use_quote", ASN1_STRFLGS_ESC_QUOTE, 0},
        {"utf8", ASN1_STRFLGS_UTF8_CONVERT, 0},
        {"ignore_type", ASN1_STRFLGS_IGNORE_TYPE, 0},
        {"show_type", ASN1_STRFLGS_SHOW_TYPE, 0},
        {"dump_all", ASN1_STRFLGS_DUMP_ALL, 0},
        {"dump_nostr", ASN1_STRFLGS_DUMP_UNKNOWN, 0},
        {"dump_der", ASN1_STRFLGS_DUMP_DER, 0},
        {"compat", XN_FLAG_COMPAT, 0xffffffffL},
        {"sep_comma_plus", XN_FLAG_SEP_COMMA_PLUS, XN_FLAG_SEP_MASK},
        {"sep_comma_plus_space", XN_FLAG_SEP_CPLUS_SPC, XN_FLAG_SEP_MASK},
        {"sep_semi_plus_space", XN_FLAG_SEP_SPLUS_SPC, XN_FLAG_SEP_MASK},
        {"sep_multiline", XN_FLAG_SEP_MULTILINE, XN_FLAG_SEP_MASK},
        {"dn_rev", XN_FLAG_DN_REV, 0},
        {"nofname", XN_FLAG_FN_NONE, XN_FLAG_FN_MASK},
        {"sname", XN_FLAG_FN_SN, XN_FLAG_FN_MASK},
        {"lname", XN_FLAG_FN_LN, XN_FLAG_FN_MASK},
        {"align", XN_FLAG_FN_ALIGN, 0},
        {"oid", XN_FLAG_FN_OID, XN_FLAG_FN_MASK},
        {"space_eq", XN_FLAG_SPC_EQ, 0},
        {"dump_unknown", XN_FLAG_DUMP_UNKNOWN_FIELDS, 0},
        {"RFC2253", XN_FLAG_RFC2253, 0xffffffffL},
        {"oneline", XN_FLAG_ONELINE, 0xffffffffL},
        {"multiline", XN_FLAG_MULTILINE, 0xffffffffL},
        {"ca_default", XN_FLAG_MULTILINE, 0xffffffffL},
        {NULL, 0, 0}
    };
    if (set_multi_opts(flags, arg, ex_tbl) == 0)
        return 0;
    if (*flags != XN_FLAG_COMPAT
        && (*flags & XN_FLAG_SEP_MASK) == 0)
        *flags |= XN_FLAG_SEP_CPLUS_SPC;
    return 1;
}

int set_dateopt(unsigned long *dateopt, const char *arg)
{
    if (OPENSSL_strcasecmp(arg, "rfc_822") == 0)
        *dateopt = ASN1_DTFLGS_RFC822;
    else if (OPENSSL_strcasecmp(arg, "iso_8601") == 0)
        *dateopt = ASN1_DTFLGS_ISO8601;
    else
        return 0;
    return 1;
}

int set_ext_copy(int *copy_type, const char *arg)
{
    if (OPENSSL_strcasecmp(arg, "none") == 0)
        *copy_type = EXT_COPY_NONE;
    else if (OPENSSL_strcasecmp(arg, "copy") == 0)
        *copy_type = EXT_COPY_ADD;
    else if (OPENSSL_strcasecmp(arg, "copyall") == 0)
        *copy_type = EXT_COPY_ALL;
    else
        return 0;
    return 1;
}

int copy_extensions(X509 *x, X509_REQ *req, int copy_type)
{
    STACK_OF(X509_EXTENSION) *exts;
    int i, ret = 0;

    if (x == NULL || req == NULL)
        return 0;
    if (copy_type == EXT_COPY_NONE)
        return 1;
    exts = X509_REQ_get_extensions(req);

    for (i = 0; i < sk_X509_EXTENSION_num(exts); i++) {
        X509_EXTENSION *ext = sk_X509_EXTENSION_value(exts, i);
        ASN1_OBJECT *obj = X509_EXTENSION_get_object(ext);
        int idx = X509_get_ext_by_OBJ(x, obj, -1);

        /* Does extension exist in target? */
        if (idx != -1) {
            /* If normal copy don't override existing extension */
            if (copy_type == EXT_COPY_ADD)
                continue;
            /* Delete all extensions of same type */
            do {
                X509_EXTENSION_free(X509_delete_ext(x, idx));
                idx = X509_get_ext_by_OBJ(x, obj, -1);
            } while (idx != -1);
        }
        if (!X509_add_ext(x, ext, -1))
            goto end;
    }
    ret = 1;

 end:
    sk_X509_EXTENSION_pop_free(exts, X509_EXTENSION_free);
    return ret;
}

static int set_multi_opts(unsigned long *flags, const char *arg,
                          const NAME_EX_TBL *in_tbl)
{
    STACK_OF(CONF_VALUE) *vals;
    CONF_VALUE *val;
    int i, ret = 1;

    if (!arg)
        return 0;
    vals = X509V3_parse_list(arg);
    for (i = 0; i < sk_CONF_VALUE_num(vals); i++) {
        val = sk_CONF_VALUE_value(vals, i);
        if (!set_table_opts(flags, val->name, in_tbl))
            ret = 0;
    }
    sk_CONF_VALUE_pop_free(vals, X509V3_conf_free);
    return ret;
}

static int set_table_opts(unsigned long *flags, const char *arg,
                          const NAME_EX_TBL *in_tbl)
{
    char c;
    const NAME_EX_TBL *ptbl;

    c = arg[0];
    if (c == '-') {
        c = 0;
        arg++;
    } else if (c == '+') {
        c = 1;
        arg++;
    } else {
        c = 1;
    }

    for (ptbl = in_tbl; ptbl->name; ptbl++) {
        if (OPENSSL_strcasecmp(arg, ptbl->name) == 0) {
            *flags &= ~ptbl->mask;
            if (c)
                *flags |= ptbl->flag;
            else
                *flags &= ~ptbl->flag;
            return 1;
        }
    }
    return 0;
}

void print_name(BIO *out, const char *title, const X509_NAME *nm)
{
    char *buf;
    char mline = 0;
    int indent = 0;
    unsigned long lflags = get_nameopt();

    if (out == NULL)
        return;
    if (title != NULL)
        BIO_puts(out, title);
    if ((lflags & XN_FLAG_SEP_MASK) == XN_FLAG_SEP_MULTILINE) {
        mline = 1;
        indent = 4;
    }
    if (lflags == XN_FLAG_COMPAT) {
        buf = X509_NAME_oneline(nm, 0, 0);
        BIO_puts(out, buf);
        BIO_puts(out, "\n");
        OPENSSL_free(buf);
    } else {
        if (mline)
            BIO_puts(out, "\n");
        X509_NAME_print_ex(out, nm, indent, lflags);
        BIO_puts(out, "\n");
    }
}

void print_bignum_var(BIO *out, const BIGNUM *in, const char *var,
                      int len, unsigned char *buffer)
{
    BIO_printf(out, "    static unsigned char %s_%d[] = {", var, len);
    if (BN_is_zero(in)) {
        BIO_printf(out, "\n        0x00");
    } else {
        int i, l;

        l = BN_bn2bin(in, buffer);
        for (i = 0; i < l; i++) {
            BIO_printf(out, (i % 10) == 0 ? "\n        " : " ");
            if (i < l - 1)
                BIO_printf(out, "0x%02X,", buffer[i]);
            else
                BIO_printf(out, "0x%02X", buffer[i]);
        }
    }
    BIO_printf(out, "\n    };\n");
}

void print_array(BIO *out, const char *title, int len, const unsigned char *d)
{
    int i;

    BIO_printf(out, "unsigned char %s[%d] = {", title, len);
    for (i = 0; i < len; i++) {
        if ((i % 10) == 0)
            BIO_printf(out, "\n    ");
        if (i < len - 1)
            BIO_printf(out, "0x%02X, ", d[i]);
        else
            BIO_printf(out, "0x%02X", d[i]);
    }
    BIO_printf(out, "\n};\n");
}

X509_STORE *setup_verify(const char *CAfile, int noCAfile,
                         const char *CApath, int noCApath,
                         const char *CAstore, int noCAstore)
{
    X509_STORE *store = X509_STORE_new();
    X509_LOOKUP *lookup;
    OSSL_LIB_CTX *libctx = app_get0_libctx();
    const char *propq = app_get0_propq();

    if (store == NULL)
        goto end;

    if (CAfile != NULL || !noCAfile) {
        lookup = X509_STORE_add_lookup(store, X509_LOOKUP_file());
        if (lookup == NULL)
            goto end;
        if (CAfile != NULL) {
            if (X509_LOOKUP_load_file_ex(lookup, CAfile, X509_FILETYPE_PEM,
                                         libctx, propq) <= 0) {
                ERR_clear_error();
                if (X509_LOOKUP_load_file_ex(lookup, CAfile, X509_FILETYPE_ASN1,
                                             libctx, propq) <= 0) {
                    BIO_printf(bio_err, "Error loading file %s\n", CAfile);
                    goto end;
                }
            }
        } else {
            X509_LOOKUP_load_file_ex(lookup, NULL, X509_FILETYPE_DEFAULT,
                                     libctx, propq);
        }
    }

    if (CApath != NULL || !noCApath) {
        lookup = X509_STORE_add_lookup(store, X509_LOOKUP_hash_dir());
        if (lookup == NULL)
            goto end;
        if (CApath != NULL) {
            if (X509_LOOKUP_add_dir(lookup, CApath, X509_FILETYPE_PEM) <= 0) {
                BIO_printf(bio_err, "Error loading directory %s\n", CApath);
                goto end;
            }
        } else {
            X509_LOOKUP_add_dir(lookup, NULL, X509_FILETYPE_DEFAULT);
        }
    }

    if (CAstore != NULL || !noCAstore) {
        lookup = X509_STORE_add_lookup(store, X509_LOOKUP_store());
        if (lookup == NULL)
            goto end;
        if (!X509_LOOKUP_add_store_ex(lookup, CAstore, libctx, propq)) {
            if (CAstore != NULL)
                BIO_printf(bio_err, "Error loading store URI %s\n", CAstore);
            goto end;
        }
    }

    ERR_clear_error();
    return store;
 end:
    ERR_print_errors(bio_err);
    X509_STORE_free(store);
    return NULL;
}

static unsigned long index_serial_hash(const OPENSSL_CSTRING *a)
{
    const char *n;

    n = a[DB_serial];
    while (*n == '0')
        n++;
    return OPENSSL_LH_strhash(n);
}

static int index_serial_cmp(const OPENSSL_CSTRING *a,
                            const OPENSSL_CSTRING *b)
{
    const char *aa, *bb;

    for (aa = a[DB_serial]; *aa == '0'; aa++) ;
    for (bb = b[DB_serial]; *bb == '0'; bb++) ;
    return strcmp(aa, bb);
}

static int index_name_qual(char **a)
{
    return (a[0][0] == 'V');
}

static unsigned long index_name_hash(const OPENSSL_CSTRING *a)
{
    return OPENSSL_LH_strhash(a[DB_name]);
}

int index_name_cmp(const OPENSSL_CSTRING *a, const OPENSSL_CSTRING *b)
{
    return strcmp(a[DB_name], b[DB_name]);
}

static IMPLEMENT_LHASH_HASH_FN(index_serial, OPENSSL_CSTRING)
static IMPLEMENT_LHASH_COMP_FN(index_serial, OPENSSL_CSTRING)
static IMPLEMENT_LHASH_HASH_FN(index_name, OPENSSL_CSTRING)
static IMPLEMENT_LHASH_COMP_FN(index_name, OPENSSL_CSTRING)
#undef BSIZE
#define BSIZE 256
BIGNUM *load_serial(const char *serialfile, int *exists, int create,
                    ASN1_INTEGER **retai)
{
    BIO *in = NULL;
    BIGNUM *ret = NULL;
    char buf[1024];
    ASN1_INTEGER *ai = NULL;

    ai = ASN1_INTEGER_new();
    if (ai == NULL)
        goto err;

    in = BIO_new_file(serialfile, "r");
    if (exists != NULL)
        *exists = in != NULL;
    if (in == NULL) {
        if (!create) {
            perror(serialfile);
            goto err;
        }
        ERR_clear_error();
        ret = BN_new();
        if (ret == NULL) {
            BIO_printf(bio_err, "Out of memory\n");
        } else if (!rand_serial(ret, ai)) {
            BIO_printf(bio_err, "Error creating random number to store in %s\n",
                       serialfile);
            BN_free(ret);
            ret = NULL;
        }
    } else {
        if (!a2i_ASN1_INTEGER(in, ai, buf, 1024)) {
            BIO_printf(bio_err, "Unable to load number from %s\n",
                       serialfile);
            goto err;
        }
        ret = ASN1_INTEGER_to_BN(ai, NULL);
        if (ret == NULL) {
            BIO_printf(bio_err, "Error converting number from bin to BIGNUM\n");
            goto err;
        }
    }

    if (ret != NULL && retai != NULL) {
        *retai = ai;
        ai = NULL;
    }
 err:
    if (ret == NULL)
        ERR_print_errors(bio_err);
    BIO_free(in);
    ASN1_INTEGER_free(ai);
    return ret;
}

int save_serial(const char *serialfile, const char *suffix,
                const BIGNUM *serial, ASN1_INTEGER **retai)
{
    char buf[1][BSIZE];
    BIO *out = NULL;
    int ret = 0;
    ASN1_INTEGER *ai = NULL;
    int j;

    if (suffix == NULL)
        j = strlen(serialfile);
    else
        j = strlen(serialfile) + strlen(suffix) + 1;
    if (j >= BSIZE) {
        BIO_printf(bio_err, "File name too long\n");
        goto err;
    }

    if (suffix == NULL) {
        OPENSSL_strlcpy(buf[0], serialfile, BSIZE);
    } else {
#ifndef OPENSSL_SYS_VMS
        BIO_snprintf(buf[0], sizeof(buf[0]), "%s.%s", serialfile, suffix);
#else
        BIO_snprintf(buf[0], sizeof(buf[0]), "%s-%s", serialfile, suffix);
#endif
    }
    out = BIO_new_file(buf[0], "w");
    if (out == NULL) {
        goto err;
    }

    if ((ai = BN_to_ASN1_INTEGER(serial, NULL)) == NULL) {
        BIO_printf(bio_err, "error converting serial to ASN.1 format\n");
        goto err;
    }
    i2a_ASN1_INTEGER(out, ai);
    BIO_puts(out, "\n");
    ret = 1;
    if (retai) {
        *retai = ai;
        ai = NULL;
    }
 err:
    if (!ret)
        ERR_print_errors(bio_err);
    BIO_free_all(out);
    ASN1_INTEGER_free(ai);
    return ret;
}

int rotate_serial(const char *serialfile, const char *new_suffix,
                  const char *old_suffix)
{
    char buf[2][BSIZE];
    int i, j;

    i = strlen(serialfile) + strlen(old_suffix);
    j = strlen(serialfile) + strlen(new_suffix);
    if (i > j)
        j = i;
    if (j + 1 >= BSIZE) {
        BIO_printf(bio_err, "File name too long\n");
        goto err;
    }
#ifndef OPENSSL_SYS_VMS
    BIO_snprintf(buf[0], sizeof(buf[0]), "%s.%s", serialfile, new_suffix);
    BIO_snprintf(buf[1], sizeof(buf[1]), "%s.%s", serialfile, old_suffix);
#else
    BIO_snprintf(buf[0], sizeof(buf[0]), "%s-%s", serialfile, new_suffix);
    BIO_snprintf(buf[1], sizeof(buf[1]), "%s-%s", serialfile, old_suffix);
#endif
    if (rename(serialfile, buf[1]) < 0 && errno != ENOENT
#ifdef ENOTDIR
        && errno != ENOTDIR
#endif
        ) {
        BIO_printf(bio_err,
                   "Unable to rename %s to %s\n", serialfile, buf[1]);
        perror("reason");
        goto err;
    }
    if (rename(buf[0], serialfile) < 0) {
        BIO_printf(bio_err,
                   "Unable to rename %s to %s\n", buf[0], serialfile);
        perror("reason");
        rename(buf[1], serialfile);
        goto err;
    }
    return 1;
 err:
    ERR_print_errors(bio_err);
    return 0;
}

int rand_serial(BIGNUM *b, ASN1_INTEGER *ai)
{
    BIGNUM *btmp;
    int ret = 0;

    btmp = b == NULL ? BN_new() : b;
    if (btmp == NULL)
        return 0;

    if (!BN_rand(btmp, SERIAL_RAND_BITS, BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY))
        goto error;
    if (ai && !BN_to_ASN1_INTEGER(btmp, ai))
        goto error;

    ret = 1;

 error:

    if (btmp != b)
        BN_free(btmp);

    return ret;
}

CA_DB *load_index(const char *dbfile, DB_ATTR *db_attr)
{
    CA_DB *retdb = NULL;
    TXT_DB *tmpdb = NULL;
    BIO *in;
    CONF *dbattr_conf = NULL;
    char buf[BSIZE];
#ifndef OPENSSL_NO_POSIX_IO
    FILE *dbfp;
    struct stat dbst;
#endif

    in = BIO_new_file(dbfile, "r");
    if (in == NULL)
        goto err;

#ifndef OPENSSL_NO_POSIX_IO
    BIO_get_fp(in, &dbfp);
    if (fstat(fileno(dbfp), &dbst) == -1) {
        ERR_raise_data(ERR_LIB_SYS, errno,
                       "calling fstat(%s)", dbfile);
        goto err;
    }
#endif

    if ((tmpdb = TXT_DB_read(in, DB_NUMBER)) == NULL)
        goto err;

#ifndef OPENSSL_SYS_VMS
    BIO_snprintf(buf, sizeof(buf), "%s.attr", dbfile);
#else
    BIO_snprintf(buf, sizeof(buf), "%s-attr", dbfile);
#endif
    dbattr_conf = app_load_config_quiet(buf);

    retdb = app_malloc(sizeof(*retdb), "new DB");
    retdb->db = tmpdb;
    tmpdb = NULL;
    if (db_attr)
        retdb->attributes = *db_attr;
    else
        retdb->attributes.unique_subject = 1;

    if (dbattr_conf != NULL) {
        char *p = app_conf_try_string(dbattr_conf, NULL, "unique_subject");

        if (p != NULL)
            retdb->attributes.unique_subject = parse_yesno(p, 1);
    }

    retdb->dbfname = OPENSSL_strdup(dbfile);
    if (retdb->dbfname == NULL)
        goto err;

#ifndef OPENSSL_NO_POSIX_IO
    retdb->dbst = dbst;
#endif

 err:
    ERR_print_errors(bio_err);
    NCONF_free(dbattr_conf);
    TXT_DB_free(tmpdb);
    BIO_free_all(in);
    return retdb;
}

/*
 * Returns > 0 on success, <= 0 on error
 */
int index_index(CA_DB *db)
{
    if (!TXT_DB_create_index(db->db, DB_serial, NULL,
                             LHASH_HASH_FN(index_serial),
                             LHASH_COMP_FN(index_serial))) {
        BIO_printf(bio_err,
                   "Error creating serial number index:(%ld,%ld,%ld)\n",
                   db->db->error, db->db->arg1, db->db->arg2);
        goto err;
    }

    if (db->attributes.unique_subject
        && !TXT_DB_create_index(db->db, DB_name, index_name_qual,
                                LHASH_HASH_FN(index_name),
                                LHASH_COMP_FN(index_name))) {
        BIO_printf(bio_err, "Error creating name index:(%ld,%ld,%ld)\n",
                   db->db->error, db->db->arg1, db->db->arg2);
        goto err;
    }
    return 1;
 err:
    ERR_print_errors(bio_err);
    return 0;
}

int save_index(const char *dbfile, const char *suffix, CA_DB *db)
{
    char buf[3][BSIZE];
    BIO *out;
    int j;

    j = strlen(dbfile) + strlen(suffix);
    if (j + 6 >= BSIZE) {
        BIO_printf(bio_err, "File name too long\n");
        goto err;
    }
#ifndef OPENSSL_SYS_VMS
    BIO_snprintf(buf[2], sizeof(buf[2]), "%s.attr", dbfile);
    BIO_snprintf(buf[1], sizeof(buf[1]), "%s.attr.%s", dbfile, suffix);
    BIO_snprintf(buf[0], sizeof(buf[0]), "%s.%s", dbfile, suffix);
#else
    BIO_snprintf(buf[2], sizeof(buf[2]), "%s-attr", dbfile);
    BIO_snprintf(buf[1], sizeof(buf[1]), "%s-attr-%s", dbfile, suffix);
    BIO_snprintf(buf[0], sizeof(buf[0]), "%s-%s", dbfile, suffix);
#endif
    out = BIO_new_file(buf[0], "w");
    if (out == NULL) {
        perror(dbfile);
        BIO_printf(bio_err, "Unable to open '%s'\n", dbfile);
        goto err;
    }
    j = TXT_DB_write(out, db->db);
    BIO_free(out);
    if (j <= 0)
        goto err;

    out = BIO_new_file(buf[1], "w");
    if (out == NULL) {
        perror(buf[2]);
        BIO_printf(bio_err, "Unable to open '%s'\n", buf[2]);
        goto err;
    }
    BIO_printf(out, "unique_subject = %s\n",
               db->attributes.unique_subject ? "yes" : "no");
    BIO_free(out);

    return 1;
 err:
    ERR_print_errors(bio_err);
    return 0;
}

int rotate_index(const char *dbfile, const char *new_suffix,
                 const char *old_suffix)
{
    char buf[5][BSIZE];
    int i, j;

    i = strlen(dbfile) + strlen(old_suffix);
    j = strlen(dbfile) + strlen(new_suffix);
    if (i > j)
        j = i;
    if (j + 6 >= BSIZE) {
        BIO_printf(bio_err, "File name too long\n");
        goto err;
    }
#ifndef OPENSSL_SYS_VMS
    BIO_snprintf(buf[4], sizeof(buf[4]), "%s.attr", dbfile);
    BIO_snprintf(buf[3], sizeof(buf[3]), "%s.attr.%s", dbfile, old_suffix);
    BIO_snprintf(buf[2], sizeof(buf[2]), "%s.attr.%s", dbfile, new_suffix);
    BIO_snprintf(buf[1], sizeof(buf[1]), "%s.%s", dbfile, old_suffix);
    BIO_snprintf(buf[0], sizeof(buf[0]), "%s.%s", dbfile, new_suffix);
#else
    BIO_snprintf(buf[4], sizeof(buf[4]), "%s-attr", dbfile);
    BIO_snprintf(buf[3], sizeof(buf[3]), "%s-attr-%s", dbfile, old_suffix);
    BIO_snprintf(buf[2], sizeof(buf[2]), "%s-attr-%s", dbfile, new_suffix);
    BIO_snprintf(buf[1], sizeof(buf[1]), "%s-%s", dbfile, old_suffix);
    BIO_snprintf(buf[0], sizeof(buf[0]), "%s-%s", dbfile, new_suffix);
#endif
    if (rename(dbfile, buf[1]) < 0 && errno != ENOENT
#ifdef ENOTDIR
        && errno != ENOTDIR
#endif
        ) {
        BIO_printf(bio_err, "Unable to rename %s to %s\n", dbfile, buf[1]);
        perror("reason");
        goto err;
    }
    if (rename(buf[0], dbfile) < 0) {
        BIO_printf(bio_err, "Unable to rename %s to %s\n", buf[0], dbfile);
        perror("reason");
        rename(buf[1], dbfile);
        goto err;
    }
    if (rename(buf[4], buf[3]) < 0 && errno != ENOENT
#ifdef ENOTDIR
        && errno != ENOTDIR
#endif
        ) {
        BIO_printf(bio_err, "Unable to rename %s to %s\n", buf[4], buf[3]);
        perror("reason");
        rename(dbfile, buf[0]);
        rename(buf[1], dbfile);
        goto err;
    }
    if (rename(buf[2], buf[4]) < 0) {
        BIO_printf(bio_err, "Unable to rename %s to %s\n", buf[2], buf[4]);
        perror("reason");
        rename(buf[3], buf[4]);
        rename(dbfile, buf[0]);
        rename(buf[1], dbfile);
        goto err;
    }
    return 1;
 err:
    ERR_print_errors(bio_err);
    return 0;
}

void free_index(CA_DB *db)
{
    if (db) {
        TXT_DB_free(db->db);
        OPENSSL_free(db->dbfname);
        OPENSSL_free(db);
    }
}

int parse_yesno(const char *str, int def)
{
    if (str) {
        switch (*str) {
        case 'f':              /* false */
        case 'F':              /* FALSE */
        case 'n':              /* no */
        case 'N':              /* NO */
        case '0':              /* 0 */
            return 0;
        case 't':              /* true */
        case 'T':              /* TRUE */
        case 'y':              /* yes */
        case 'Y':              /* YES */
        case '1':              /* 1 */
            return 1;
        }
    }
    return def;
}

/*
 * name is expected to be in the format /type0=value0/type1=value1/type2=...
 * where + can be used instead of / to form multi-valued RDNs if canmulti
 * and characters may be escaped by \
 */
X509_NAME *parse_name(const char *cp, int chtype, int canmulti,
                      const char *desc)
{
    int nextismulti = 0;
    char *work;
    X509_NAME *n;

    if (*cp++ != '/') {
        BIO_printf(bio_err,
                   "%s: %s name is expected to be in the format "
                   "/type0=value0/type1=value1/type2=... where characters may "
                   "be escaped by \\. This name is not in that format: '%s'\n",
                   opt_getprog(), desc, --cp);
        return NULL;
    }

    n = X509_NAME_new();
    if (n == NULL) {
        BIO_printf(bio_err, "%s: Out of memory\n", opt_getprog());
        return NULL;
    }
    work = OPENSSL_strdup(cp);
    if (work == NULL) {
        BIO_printf(bio_err, "%s: Error copying %s name input\n",
                   opt_getprog(), desc);
        goto err;
    }

    while (*cp != '\0') {
        char *bp = work;
        char *typestr = bp;
        unsigned char *valstr;
        int nid;
        int ismulti = nextismulti;

        nextismulti = 0;

        /* Collect the type */
        while (*cp != '\0' && *cp != '=')
            *bp++ = *cp++;
        *bp++ = '\0';
        if (*cp == '\0') {
            BIO_printf(bio_err,
                       "%s: Missing '=' after RDN type string '%s' in %s name string\n",
                       opt_getprog(), typestr, desc);
            goto err;
        }
        ++cp;

        /* Collect the value. */
        valstr = (unsigned char *)bp;
        for (; *cp != '\0' && *cp != '/'; *bp++ = *cp++) {
            /* unescaped '+' symbol string signals further member of multiRDN */
            if (canmulti && *cp == '+') {
                nextismulti = 1;
                break;
            }
            if (*cp == '\\' && *++cp == '\0') {
                BIO_printf(bio_err,
                           "%s: Escape character at end of %s name string\n",
                           opt_getprog(), desc);
                goto err;
            }
        }
        *bp++ = '\0';

        /* If not at EOS (must be + or /), move forward. */
        if (*cp != '\0')
            ++cp;

        /* Parse */
        nid = OBJ_txt2nid(typestr);
        if (nid == NID_undef) {
            BIO_printf(bio_err,
                       "%s warning: Skipping unknown %s name attribute \"%s\"\n",
                       opt_getprog(), desc, typestr);
            if (ismulti)
                BIO_printf(bio_err,
                           "%s hint: a '+' in a value string needs be escaped using '\\' else a new member of a multi-valued RDN is expected\n",
                           opt_getprog());
            continue;
        }
        if (*valstr == '\0') {
            BIO_printf(bio_err,
                       "%s warning: No value provided for %s name attribute \"%s\", skipped\n",
                       opt_getprog(), desc, typestr);
            continue;
        }
        if (!X509_NAME_add_entry_by_NID(n, nid, chtype,
                                        valstr, strlen((char *)valstr),
                                        -1, ismulti ? -1 : 0)) {
            ERR_print_errors(bio_err);
            BIO_printf(bio_err,
                       "%s: Error adding %s name attribute \"/%s=%s\"\n",
                       opt_getprog(), desc, typestr, valstr);
            goto err;
        }
    }

    OPENSSL_free(work);
    return n;

 err:
    X509_NAME_free(n);
    OPENSSL_free(work);
    return NULL;
}

/*
 * Read whole contents of a BIO into an allocated memory buffer and return
 * it.
 */

int bio_to_mem(unsigned char **out, int maxlen, BIO *in)
{
    BIO *mem;
    int len, ret;
    unsigned char tbuf[1024];

    mem = BIO_new(BIO_s_mem());
    if (mem == NULL)
        return -1;
    for (;;) {
        if ((maxlen != -1) && maxlen < 1024)
            len = maxlen;
        else
            len = 1024;
        len = BIO_read(in, tbuf, len);
        if (len < 0) {
            BIO_free(mem);
            return -1;
        }
        if (len == 0)
            break;
        if (BIO_write(mem, tbuf, len) != len) {
            BIO_free(mem);
            return -1;
        }
        if (maxlen != -1)
            maxlen -= len;

        if (maxlen == 0)
            break;
    }
    ret = BIO_get_mem_data(mem, (char **)out);
    BIO_set_flags(mem, BIO_FLAGS_MEM_RDONLY);
    BIO_free(mem);
    return ret;
}

int pkey_ctrl_string(EVP_PKEY_CTX *ctx, const char *value)
{
    int rv = 0;
    char *stmp, *vtmp = NULL;

    stmp = OPENSSL_strdup(value);
    if (stmp == NULL)
        return -1;
    vtmp = strchr(stmp, ':');
    if (vtmp == NULL)
        goto err;

    *vtmp = 0;
    vtmp++;
    rv = EVP_PKEY_CTX_ctrl_str(ctx, stmp, vtmp);

 err:
    OPENSSL_free(stmp);
    return rv;
}

static void nodes_print(const char *name, STACK_OF(X509_POLICY_NODE) *nodes)
{
    X509_POLICY_NODE *node;
    int i;

    BIO_printf(bio_err, "%s Policies:", name);
    if (nodes) {
        BIO_puts(bio_err, "\n");
        for (i = 0; i < sk_X509_POLICY_NODE_num(nodes); i++) {
            node = sk_X509_POLICY_NODE_value(nodes, i);
            X509_POLICY_NODE_print(bio_err, node, 2);
        }
    } else {
        BIO_puts(bio_err, " <empty>\n");
    }
}

void policies_print(X509_STORE_CTX *ctx)
{
    X509_POLICY_TREE *tree;
    int explicit_policy;

    tree = X509_STORE_CTX_get0_policy_tree(ctx);
    explicit_policy = X509_STORE_CTX_get_explicit_policy(ctx);

    BIO_printf(bio_err, "Require explicit Policy: %s\n",
               explicit_policy ? "True" : "False");

    nodes_print("Authority", X509_policy_tree_get0_policies(tree));
    nodes_print("User", X509_policy_tree_get0_user_policies(tree));
}

/*-
 * next_protos_parse parses a comma separated list of strings into a string
 * in a format suitable for passing to SSL_CTX_set_next_protos_advertised.
 *   outlen: (output) set to the length of the resulting buffer on success.
 *   err: (maybe NULL) on failure, an error message line is written to this BIO.
 *   in: a NUL terminated string like "abc,def,ghi"
 *
 *   returns: a malloc'd buffer or NULL on failure.
 */
unsigned char *next_protos_parse(size_t *outlen, const char *in)
{
    size_t len;
    unsigned char *out;
    size_t i, start = 0;
    size_t skipped = 0;

    len = strlen(in);
    if (len == 0 || len >= 65535)
        return NULL;

    out = app_malloc(len + 1, "NPN buffer");
    for (i = 0; i <= len; ++i) {
        if (i == len || in[i] == ',') {
            /*
             * Zero-length ALPN elements are invalid on the wire, we could be
             * strict and reject the entire string, but just ignoring extra
             * commas seems harmless and more friendly.
             *
             * Every comma we skip in this way puts the input buffer another
             * byte ahead of the output buffer, so all stores into the output
             * buffer need to be decremented by the number commas skipped.
             */
            if (i == start) {
                ++start;
                ++skipped;
                continue;
            }
            if (i - start > 255) {
                OPENSSL_free(out);
                return NULL;
            }
            out[start - skipped] = (unsigned char)(i - start);
            start = i + 1;
        } else {
            out[i + 1 - skipped] = in[i];
        }
    }

    if (len <= skipped) {
        OPENSSL_free(out);
        return NULL;
    }

    *outlen = len + 1 - skipped;
    return out;
}

int check_cert_attributes(BIO *bio, X509 *x, const char *checkhost,
                          const char *checkemail, const char *checkip,
                          int print)
{
    int valid_host = 0;
    int valid_mail = 0;
    int valid_ip = 0;
    int ret = 1;

    if (x == NULL)
        return 0;

    if (checkhost != NULL) {
        valid_host = X509_check_host(x, checkhost, 0, 0, NULL);
        if (print)
            BIO_printf(bio, "Hostname %s does%s match certificate\n",
                       checkhost, valid_host == 1 ? "" : " NOT");
        ret = ret && valid_host > 0;
    }

    if (checkemail != NULL) {
        valid_mail = X509_check_email(x, checkemail, 0, 0);
        if (print)
            BIO_printf(bio, "Email %s does%s match certificate\n",
                       checkemail, valid_mail ? "" : " NOT");
        ret = ret && valid_mail > 0;
    }

    if (checkip != NULL) {
        valid_ip = X509_check_ip_asc(x, checkip, 0);
        if (print)
            BIO_printf(bio, "IP %s does%s match certificate\n",
                       checkip, valid_ip ? "" : " NOT");
        ret = ret && valid_ip > 0;
    }

    return ret;
}

static int do_pkey_ctx_init(EVP_PKEY_CTX *pkctx, STACK_OF(OPENSSL_STRING) *opts)
{
    int i;

    if (opts == NULL)
        return 1;

    for (i = 0; i < sk_OPENSSL_STRING_num(opts); i++) {
        char *opt = sk_OPENSSL_STRING_value(opts, i);

        if (pkey_ctrl_string(pkctx, opt) <= 0) {
            BIO_printf(bio_err, "parameter error \"%s\"\n", opt);
            ERR_print_errors(bio_err);
            return 0;
        }
    }

    return 1;
}

static int do_x509_init(X509 *x, STACK_OF(OPENSSL_STRING) *opts)
{
    int i;

    if (opts == NULL)
        return 1;

    for (i = 0; i < sk_OPENSSL_STRING_num(opts); i++) {
        char *opt = sk_OPENSSL_STRING_value(opts, i);

        if (x509_ctrl_string(x, opt) <= 0) {
            BIO_printf(bio_err, "parameter error \"%s\"\n", opt);
            ERR_print_errors(bio_err);
            return 0;
        }
    }

    return 1;
}

static int do_x509_req_init(X509_REQ *x, STACK_OF(OPENSSL_STRING) *opts)
{
    int i;

    if (opts == NULL)
        return 1;

    for (i = 0; i < sk_OPENSSL_STRING_num(opts); i++) {
        char *opt = sk_OPENSSL_STRING_value(opts, i);

        if (x509_req_ctrl_string(x, opt) <= 0) {
            BIO_printf(bio_err, "parameter error \"%s\"\n", opt);
            ERR_print_errors(bio_err);
            return 0;
        }
    }

    return 1;
}

static int do_sign_init(EVP_MD_CTX *ctx, EVP_PKEY *pkey,
                        const char *md, STACK_OF(OPENSSL_STRING) *sigopts)
{
    EVP_PKEY_CTX *pkctx = NULL;
    char def_md[80];

    if (ctx == NULL)
        return 0;
    /*
     * EVP_PKEY_get_default_digest_name() returns 2 if the digest is mandatory
     * for this algorithm.
     */
    if (EVP_PKEY_get_default_digest_name(pkey, def_md, sizeof(def_md)) == 2
            && strcmp(def_md, "UNDEF") == 0) {
        /* The signing algorithm requires there to be no digest */
        md = NULL;
    }

    return EVP_DigestSignInit_ex(ctx, &pkctx, md, app_get0_libctx(),
                                 app_get0_propq(), pkey, NULL)
        && do_pkey_ctx_init(pkctx, sigopts);
}

static int adapt_keyid_ext(X509 *cert, X509V3_CTX *ext_ctx,
                           const char *name, const char *value, int add_default)
{
    const STACK_OF(X509_EXTENSION) *exts = X509_get0_extensions(cert);
    X509_EXTENSION *new_ext = X509V3_EXT_nconf(NULL, ext_ctx, name, value);
    int idx, rv = 0;

    if (new_ext == NULL)
        return rv;

    idx = X509v3_get_ext_by_OBJ(exts, X509_EXTENSION_get_object(new_ext), -1);
    if (idx >= 0) {
        X509_EXTENSION *found_ext = X509v3_get_ext(exts, idx);
        ASN1_OCTET_STRING *encoded = X509_EXTENSION_get_data(found_ext);
        int disabled = ASN1_STRING_length(encoded) <= 2; /* indicating "none" */

        if (disabled) {
            X509_delete_ext(cert, idx);
            X509_EXTENSION_free(found_ext);
        } /* else keep existing key identifier, which might be outdated */
        rv = 1;
    } else {
        rv = !add_default || X509_add_ext(cert, new_ext, -1);
    }
    X509_EXTENSION_free(new_ext);
    return rv;
}

int cert_matches_key(const X509 *cert, const EVP_PKEY *pkey)
{
    int match;

    ERR_set_mark();
    match = X509_check_private_key(cert, pkey);
    ERR_pop_to_mark();
    return match;
}

/* Ensure RFC 5280 compliance, adapt keyIDs as needed, and sign the cert info */
int do_X509_sign(X509 *cert, int force_v1, EVP_PKEY *pkey, const char *md,
                 STACK_OF(OPENSSL_STRING) *sigopts, X509V3_CTX *ext_ctx)
{
    EVP_MD_CTX *mctx = EVP_MD_CTX_new();
    int self_sign;
    int rv = 0;

    if (!force_v1) {
        if (!X509_set_version(cert, X509_VERSION_3))
            goto end;

        /*
         * Add default SKID before AKID such that AKID can make use of it
         * in case the certificate is self-signed
         */
        /* Prevent X509_V_ERR_MISSING_SUBJECT_KEY_IDENTIFIER */
        if (!adapt_keyid_ext(cert, ext_ctx, "subjectKeyIdentifier", "hash", 1))
            goto end;
        /* Prevent X509_V_ERR_MISSING_AUTHORITY_KEY_IDENTIFIER */
        self_sign = cert_matches_key(cert, pkey);
        if (!adapt_keyid_ext(cert, ext_ctx, "authorityKeyIdentifier",
                             "keyid, issuer", !self_sign))
            goto end;
    }
    /* May add further measures for ensuring RFC 5280 compliance, see #19805 */

    if (mctx != NULL && do_sign_init(mctx, pkey, md, sigopts) > 0)
        rv = (X509_sign_ctx(cert, mctx) > 0);
 end:
    EVP_MD_CTX_free(mctx);
    return rv;
}

/* Sign the certificate request info */
int do_X509_REQ_sign(X509_REQ *x, EVP_PKEY *pkey, const char *md,
                     STACK_OF(OPENSSL_STRING) *sigopts)
{
    int rv = 0;
    EVP_MD_CTX *mctx = EVP_MD_CTX_new();

    if (do_sign_init(mctx, pkey, md, sigopts) > 0)
        rv = (X509_REQ_sign_ctx(x, mctx) > 0);
    EVP_MD_CTX_free(mctx);
    return rv;
}

/* Sign the CRL info */
int do_X509_CRL_sign(X509_CRL *x, EVP_PKEY *pkey, const char *md,
                     STACK_OF(OPENSSL_STRING) *sigopts)
{
    int rv = 0;
    EVP_MD_CTX *mctx = EVP_MD_CTX_new();

    if (do_sign_init(mctx, pkey, md, sigopts) > 0)
        rv = (X509_CRL_sign_ctx(x, mctx) > 0);
    EVP_MD_CTX_free(mctx);
    return rv;
}

/*
 * do_X509_verify returns 1 if the signature is valid,
 * 0 if the signature check fails, or -1 if error occurs.
 */
int do_X509_verify(X509 *x, EVP_PKEY *pkey, STACK_OF(OPENSSL_STRING) *vfyopts)
{
    int rv = 0;

    if (do_x509_init(x, vfyopts) > 0)
        rv = X509_verify(x, pkey);
    else
        rv = -1;
    return rv;
}

/*
 * do_X509_REQ_verify returns 1 if the signature is valid,
 * 0 if the signature check fails, or -1 if error occurs.
 */
int do_X509_REQ_verify(X509_REQ *x, EVP_PKEY *pkey,
                       STACK_OF(OPENSSL_STRING) *vfyopts)
{
    int rv = 0;

    if (do_x509_req_init(x, vfyopts) > 0)
        rv = X509_REQ_verify_ex(x, pkey, app_get0_libctx(), app_get0_propq());
    else
        rv = -1;
    return rv;
}

/* Get first http URL from a DIST_POINT structure */

static const char *get_dp_url(DIST_POINT *dp)
{
    GENERAL_NAMES *gens;
    GENERAL_NAME *gen;
    int i, gtype;
    ASN1_STRING *uri;

    if (!dp->distpoint || dp->distpoint->type != 0)
        return NULL;
    gens = dp->distpoint->name.fullname;
    for (i = 0; i < sk_GENERAL_NAME_num(gens); i++) {
        gen = sk_GENERAL_NAME_value(gens, i);
        uri = GENERAL_NAME_get0_value(gen, &gtype);
        if (gtype == GEN_URI && ASN1_STRING_length(uri) > 6) {
            const char *uptr = (const char *)ASN1_STRING_get0_data(uri);

            if (IS_HTTP(uptr)) /* can/should not use HTTPS here */
                return uptr;
        }
    }
    return NULL;
}

/*
 * Look through a CRLDP structure and attempt to find an http URL to
 * downloads a CRL from.
 */

static X509_CRL *load_crl_crldp(STACK_OF(DIST_POINT) *crldp)
{
    int i;
    const char *urlptr = NULL;

    for (i = 0; i < sk_DIST_POINT_num(crldp); i++) {
        DIST_POINT *dp = sk_DIST_POINT_value(crldp, i);

        urlptr = get_dp_url(dp);
        if (urlptr != NULL)
            return load_crl(urlptr, FORMAT_UNDEF, 0, "CRL via CDP");
    }
    return NULL;
}

/*
 * Example of downloading CRLs from CRLDP:
 * not usable for real world as it always downloads and doesn't cache anything.
 */

static STACK_OF(X509_CRL) *crls_http_cb(const X509_STORE_CTX *ctx,
                                        const X509_NAME *nm)
{
    X509 *x;
    STACK_OF(X509_CRL) *crls = NULL;
    X509_CRL *crl;
    STACK_OF(DIST_POINT) *crldp;

    crls = sk_X509_CRL_new_null();
    if (!crls)
        return NULL;
    x = X509_STORE_CTX_get_current_cert(ctx);
    crldp = X509_get_ext_d2i(x, NID_crl_distribution_points, NULL, NULL);
    crl = load_crl_crldp(crldp);
    sk_DIST_POINT_pop_free(crldp, DIST_POINT_free);

    if (crl == NULL || !sk_X509_CRL_push(crls, crl))
        goto error;

    /* Try to download delta CRL */
    crldp = X509_get_ext_d2i(x, NID_freshest_crl, NULL, NULL);
    crl = load_crl_crldp(crldp);
    sk_DIST_POINT_pop_free(crldp, DIST_POINT_free);

    if (crl != NULL && !sk_X509_CRL_push(crls, crl))
        goto error;

    return crls;

error:
    X509_CRL_free(crl);
    sk_X509_CRL_free(crls);
    return NULL;
}

void store_setup_crl_download(X509_STORE *st)
{
    X509_STORE_set_lookup_crls_cb(st, crls_http_cb);
}

#if !defined(OPENSSL_NO_SOCK) && !defined(OPENSSL_NO_HTTP)
static const char *tls_error_hint(void)
{
    unsigned long err = ERR_peek_error();

    if (ERR_GET_LIB(err) != ERR_LIB_SSL)
        err = ERR_peek_last_error();
    if (ERR_GET_LIB(err) != ERR_LIB_SSL)
        return NULL; /* likely no TLS error */

    switch (ERR_GET_REASON(err)) {
    case SSL_R_WRONG_VERSION_NUMBER:
        return "The server does not support (a suitable version of) TLS";
    case SSL_R_UNKNOWN_PROTOCOL:
        return "The server does not support HTTPS";
    case SSL_R_CERTIFICATE_VERIFY_FAILED:
        return "Cannot authenticate server via its TLS certificate, likely due to mismatch with our trusted TLS certs or missing revocation status";
    case SSL_AD_REASON_OFFSET + TLS1_AD_UNKNOWN_CA:
        return "Server did not accept our TLS certificate, likely due to mismatch with server's trust anchor or missing revocation status";
    case SSL_AD_REASON_OFFSET + SSL3_AD_HANDSHAKE_FAILURE:
        return "TLS handshake failure. Possibly the server requires our TLS certificate but did not receive it";
    default:
        return NULL; /* no hint available for TLS error */
    }
}

static BIO *http_tls_shutdown(BIO *bio)
{
    if (bio != NULL) {
        BIO *cbio;
        const char *hint = tls_error_hint();

        if (hint != NULL)
            BIO_printf(bio_err, "%s\n", hint);
        (void)ERR_set_mark();
        BIO_ssl_shutdown(bio);
        cbio = BIO_pop(bio); /* connect+HTTP BIO */
        BIO_free(bio); /* SSL BIO */
        (void)ERR_pop_to_mark(); /* hide SSL_R_READ_BIO_NOT_SET etc. */
        bio = cbio;
    }
    return bio;
}

/* HTTP callback function that supports TLS connection also via HTTPS proxy */
BIO *app_http_tls_cb(BIO *bio, void *arg, int connect, int detail)
{
    APP_HTTP_TLS_INFO *info = (APP_HTTP_TLS_INFO *)arg;
    SSL_CTX *ssl_ctx = info->ssl_ctx;

    if (ssl_ctx == NULL) /* not using TLS */
        return bio;
    if (connect) {
        SSL *ssl;
        BIO *sbio = NULL;
        X509_STORE *ts = SSL_CTX_get_cert_store(ssl_ctx);
        X509_VERIFY_PARAM *vpm = X509_STORE_get0_param(ts);
        const char *host = vpm == NULL ? NULL :
            X509_VERIFY_PARAM_get0_host(vpm, 0 /* first hostname */);

        /* adapt after fixing callback design flaw, see #17088 */
        if ((info->use_proxy
             && !OSSL_HTTP_proxy_connect(bio, info->server, info->port,
                                         NULL, NULL, /* no proxy credentials */
                                         info->timeout, bio_err, opt_getprog()))
                || (sbio = BIO_new(BIO_f_ssl())) == NULL) {
            return NULL;
        }
        if ((ssl = SSL_new(ssl_ctx)) == NULL) {
            BIO_free(sbio);
            return NULL;
        }

        if (vpm != NULL)
            SSL_set_tlsext_host_name(ssl, host /* may be NULL */);

        SSL_set_connect_state(ssl);
        BIO_set_ssl(sbio, ssl, BIO_CLOSE);

        bio = BIO_push(sbio, bio);
    } else { /* disconnect from TLS */
        bio = http_tls_shutdown(bio);
    }
    return bio;
}

void APP_HTTP_TLS_INFO_free(APP_HTTP_TLS_INFO *info)
{
    if (info != NULL) {
        SSL_CTX_free(info->ssl_ctx);
        OPENSSL_free(info);
    }
}

ASN1_VALUE *app_http_get_asn1(const char *url, const char *proxy,
                              const char *no_proxy, SSL_CTX *ssl_ctx,
                              const STACK_OF(CONF_VALUE) *headers,
                              long timeout, const char *expected_content_type,
                              const ASN1_ITEM *it)
{
    APP_HTTP_TLS_INFO info;
    char *server;
    char *port;
    int use_ssl;
    BIO *mem;
    ASN1_VALUE *resp = NULL;

    if (url == NULL || it == NULL) {
        ERR_raise(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if (!OSSL_HTTP_parse_url(url, &use_ssl, NULL /* userinfo */, &server, &port,
                             NULL /* port_num, */, NULL, NULL, NULL))
        return NULL;
    if (use_ssl && ssl_ctx == NULL) {
        ERR_raise_data(ERR_LIB_HTTP, ERR_R_PASSED_NULL_PARAMETER,
                       "missing SSL_CTX");
        goto end;
    }
    if (!use_ssl && ssl_ctx != NULL) {
        ERR_raise_data(ERR_LIB_HTTP, ERR_R_PASSED_INVALID_ARGUMENT,
                       "SSL_CTX given but use_ssl == 0");
        goto end;
    }

    info.server = server;
    info.port = port;
    info.use_proxy = /* workaround for callback design flaw, see #17088 */
        OSSL_HTTP_adapt_proxy(proxy, no_proxy, server, use_ssl) != NULL;
    info.timeout = timeout;
    info.ssl_ctx = ssl_ctx;
    mem = OSSL_HTTP_get(url, proxy, no_proxy, NULL /* bio */, NULL /* rbio */,
                        app_http_tls_cb, &info, 0 /* buf_size */, headers,
                        expected_content_type, 1 /* expect_asn1 */,
                        OSSL_HTTP_DEFAULT_MAX_RESP_LEN, timeout);
    resp = ASN1_item_d2i_bio(it, mem, NULL);
    BIO_free(mem);

 end:
    OPENSSL_free(server);
    OPENSSL_free(port);
    return resp;

}

ASN1_VALUE *app_http_post_asn1(const char *host, const char *port,
                               const char *path, const char *proxy,
                               const char *no_proxy, SSL_CTX *ssl_ctx,
                               const STACK_OF(CONF_VALUE) *headers,
                               const char *content_type,
                               ASN1_VALUE *req, const ASN1_ITEM *req_it,
                               const char *expected_content_type,
                               long timeout, const ASN1_ITEM *rsp_it)
{
    int use_ssl = ssl_ctx != NULL;
    APP_HTTP_TLS_INFO info;
    BIO *rsp, *req_mem = ASN1_item_i2d_mem_bio(req_it, req);
    ASN1_VALUE *res;

    if (req_mem == NULL)
        return NULL;

    info.server = host;
    info.port = port;
    info.use_proxy = /* workaround for callback design flaw, see #17088 */
        OSSL_HTTP_adapt_proxy(proxy, no_proxy, host, use_ssl) != NULL;
    info.timeout = timeout;
    info.ssl_ctx = ssl_ctx;
    rsp = OSSL_HTTP_transfer(NULL, host, port, path, use_ssl,
                             proxy, no_proxy, NULL /* bio */, NULL /* rbio */,
                             app_http_tls_cb, &info,
                             0 /* buf_size */, headers, content_type, req_mem,
                             expected_content_type, 1 /* expect_asn1 */,
                             OSSL_HTTP_DEFAULT_MAX_RESP_LEN, timeout,
                             0 /* keep_alive */);
    BIO_free(req_mem);
    res = ASN1_item_d2i_bio(rsp_it, rsp, NULL);
    BIO_free(rsp);
    return res;
}

#endif

/*
 * Platform-specific sections
 */
#if defined(_WIN32)
# ifdef fileno
#  undef fileno
#  define fileno(a) (int)_fileno(a)
# endif

# include <windows.h>
# include <tchar.h>

static int WIN32_rename(const char *from, const char *to)
{
    TCHAR *tfrom = NULL, *tto;
    DWORD err;
    int ret = 0;

    if (sizeof(TCHAR) == 1) {
        tfrom = (TCHAR *)from;
        tto = (TCHAR *)to;
    } else {                    /* UNICODE path */
        size_t i, flen = strlen(from) + 1, tlen = strlen(to) + 1;

        tfrom = malloc(sizeof(*tfrom) * (flen + tlen));
        if (tfrom == NULL)
            goto err;
        tto = tfrom + flen;
# if !defined(_WIN32_WCE) || _WIN32_WCE >= 101
        if (!MultiByteToWideChar(CP_ACP, 0, from, flen, (WCHAR *)tfrom, flen))
# endif
            for (i = 0; i < flen; i++)
                tfrom[i] = (TCHAR)from[i];
# if !defined(_WIN32_WCE) || _WIN32_WCE >= 101
        if (!MultiByteToWideChar(CP_ACP, 0, to, tlen, (WCHAR *)tto, tlen))
# endif
            for (i = 0; i < tlen; i++)
                tto[i] = (TCHAR)to[i];
    }

    if (MoveFile(tfrom, tto))
        goto ok;
    err = GetLastError();
    if (err == ERROR_ALREADY_EXISTS || err == ERROR_FILE_EXISTS) {
        if (DeleteFile(tto) && MoveFile(tfrom, tto))
            goto ok;
        err = GetLastError();
    }
    if (err == ERROR_FILE_NOT_FOUND || err == ERROR_PATH_NOT_FOUND)
        errno = ENOENT;
    else if (err == ERROR_ACCESS_DENIED)
        errno = EACCES;
    else
        errno = EINVAL;         /* we could map more codes... */
 err:
    ret = -1;
 ok:
    if (tfrom != NULL && tfrom != (TCHAR *)from)
        free(tfrom);
    return ret;
}
#endif

/* app_tminterval section */
#if defined(_WIN32)
double app_tminterval(int stop, int usertime)
{
    FILETIME now;
    double ret = 0;
    static ULARGE_INTEGER tmstart;
    static int warning = 1;
    int use_GetSystemTime = 1;
# ifdef _WIN32_WINNT
    static HANDLE proc = NULL;

    if (proc == NULL) {
        if (check_winnt())
            proc = OpenProcess(PROCESS_QUERY_INFORMATION, FALSE,
                               GetCurrentProcessId());
        if (proc == NULL)
            proc = (HANDLE) - 1;
    }

    if (usertime && proc != (HANDLE) - 1) {
        FILETIME junk;

        GetProcessTimes(proc, &junk, &junk, &junk, &now);
        use_GetSystemTime = 0;
    }
# endif
    if (use_GetSystemTime) {
        SYSTEMTIME systime;

        if (usertime && warning) {
            BIO_printf(bio_err, "To get meaningful results, run "
                       "this program on idle system.\n");
            warning = 0;
        }
        GetSystemTime(&systime);
        SystemTimeToFileTime(&systime, &now);
    }

    if (stop == TM_START) {
        tmstart.u.LowPart = now.dwLowDateTime;
        tmstart.u.HighPart = now.dwHighDateTime;
    } else {
        ULARGE_INTEGER tmstop;

        tmstop.u.LowPart = now.dwLowDateTime;
        tmstop.u.HighPart = now.dwHighDateTime;

        ret = (__int64)(tmstop.QuadPart - tmstart.QuadPart) * 1e-7;
    }

    return ret;
}
#elif defined(OPENSSL_SYS_VXWORKS)
# include <time.h>

double app_tminterval(int stop, int usertime)
{
    double ret = 0;
# ifdef CLOCK_REALTIME
    static struct timespec tmstart;
    struct timespec now;
# else
    static unsigned long tmstart;
    unsigned long now;
# endif
    static int warning = 1;

    if (usertime && warning) {
        BIO_printf(bio_err, "To get meaningful results, run "
                   "this program on idle system.\n");
        warning = 0;
    }
# ifdef CLOCK_REALTIME
    clock_gettime(CLOCK_REALTIME, &now);
    if (stop == TM_START)
        tmstart = now;
    else
        ret = ((now.tv_sec + now.tv_nsec * 1e-9)
               - (tmstart.tv_sec + tmstart.tv_nsec * 1e-9));
# else
    now = tickGet();
    if (stop == TM_START)
        tmstart = now;
    else
        ret = (now - tmstart) / (double)sysClkRateGet();
# endif
    return ret;
}

#elif defined(_SC_CLK_TCK)      /* by means of unistd.h */
# include <sys/times.h>

double app_tminterval(int stop, int usertime)
{
    double ret = 0;
    struct tms rus;
    clock_t now = times(&rus);
    static clock_t tmstart;

    if (usertime)
        now = rus.tms_utime;

    if (stop == TM_START) {
        tmstart = now;
    } else {
        long int tck = sysconf(_SC_CLK_TCK);

        ret = (now - tmstart) / (double)tck;
    }

    return ret;
}

#else
# include <sys/time.h>
# include <sys/resource.h>

double app_tminterval(int stop, int usertime)
{
    double ret = 0;
    struct rusage rus;
    struct timeval now;
    static struct timeval tmstart;

    if (usertime)
        getrusage(RUSAGE_SELF, &rus), now = rus.ru_utime;
    else
        gettimeofday(&now, NULL);

    if (stop == TM_START)
        tmstart = now;
    else
        ret = ((now.tv_sec + now.tv_usec * 1e-6)
               - (tmstart.tv_sec + tmstart.tv_usec * 1e-6));

    return ret;
}
#endif

int app_access(const char *name, int flag)
{
#ifdef _WIN32
    return _access(name, flag);
#else
    return access(name, flag);
#endif
}

int app_isdir(const char *name)
{
    return opt_isdir(name);
}

/* raw_read|write section */
#if defined(__VMS)
# include "vms_term_sock.h"
static int stdin_sock = -1;

static void close_stdin_sock(void)
{
    TerminalSocket(TERM_SOCK_DELETE, &stdin_sock);
}

int fileno_stdin(void)
{
    if (stdin_sock == -1) {
        TerminalSocket(TERM_SOCK_CREATE, &stdin_sock);
        atexit(close_stdin_sock);
    }

    return stdin_sock;
}
#else
int fileno_stdin(void)
{
    return fileno(stdin);
}
#endif

int fileno_stdout(void)
{
    return fileno(stdout);
}

#if defined(_WIN32) && defined(STD_INPUT_HANDLE)
int raw_read_stdin(void *buf, int siz)
{
    DWORD n;

    if (ReadFile(GetStdHandle(STD_INPUT_HANDLE), buf, siz, &n, NULL))
        return n;
    else
        return -1;
}
#elif defined(__VMS)
# include <sys/socket.h>

int raw_read_stdin(void *buf, int siz)
{
    return recv(fileno_stdin(), buf, siz, 0);
}
#else
int raw_read_stdin(void *buf, int siz)
{
    return read(fileno_stdin(), buf, siz);
}
#endif

#if defined(_WIN32) && defined(STD_OUTPUT_HANDLE)
int raw_write_stdout(const void *buf, int siz)
{
    DWORD n;

    if (WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), buf, siz, &n, NULL))
        return n;
    else
        return -1;
}
#elif defined(OPENSSL_SYS_TANDEM) && defined(OPENSSL_THREADS) \
    && defined(_SPT_MODEL_)
int raw_write_stdout(const void *buf, int siz)
{
    return write(fileno(stdout), (void *)buf, siz);
}
#else
int raw_write_stdout(const void *buf, int siz)
{
    return write(fileno_stdout(), buf, siz);
}
#endif

/*
 * Centralized handling of input and output files with format specification
 * The format is meant to show what the input and output is supposed to be,
 * and is therefore a show of intent more than anything else.  However, it
 * does impact behavior on some platforms, such as differentiating between
 * text and binary input/output on non-Unix platforms
 */
BIO *dup_bio_in(int format)
{
    return BIO_new_fp(stdin,
                      BIO_NOCLOSE | (FMT_istext(format) ? BIO_FP_TEXT : 0));
}

BIO *dup_bio_out(int format)
{
    BIO *b = BIO_new_fp(stdout,
                        BIO_NOCLOSE | (FMT_istext(format) ? BIO_FP_TEXT : 0));
    void *prefix = NULL;

    if (b == NULL)
        return NULL;

#ifdef OPENSSL_SYS_VMS
    if (FMT_istext(format))
        b = BIO_push(BIO_new(BIO_f_linebuffer()), b);
#endif

    if (FMT_istext(format)
        && (prefix = getenv("HARNESS_OSSL_PREFIX")) != NULL) {
        b = BIO_push(BIO_new(BIO_f_prefix()), b);
        BIO_set_prefix(b, prefix);
    }

    return b;
}

BIO *dup_bio_err(int format)
{
    BIO *b = BIO_new_fp(stderr,
                        BIO_NOCLOSE | (FMT_istext(format) ? BIO_FP_TEXT : 0));

#ifdef OPENSSL_SYS_VMS
    if (b != NULL && FMT_istext(format))
        b = BIO_push(BIO_new(BIO_f_linebuffer()), b);
#endif
    return b;
}

void unbuffer(FILE *fp)
{
/*
 * On VMS, setbuf() will only take 32-bit pointers, and a compilation
 * with /POINTER_SIZE=64 will give off a MAYLOSEDATA2 warning here.
 * However, we trust that the C RTL will never give us a FILE pointer
 * above the first 4 GB of memory, so we simply turn off the warning
 * temporarily.
 */
#if defined(OPENSSL_SYS_VMS) && defined(__DECC)
# pragma environment save
# pragma message disable maylosedata2
#endif
    setbuf(fp, NULL);
#if defined(OPENSSL_SYS_VMS) && defined(__DECC)
# pragma environment restore
#endif
}

static const char *modestr(char mode, int format)
{
    OPENSSL_assert(mode == 'a' || mode == 'r' || mode == 'w');

    switch (mode) {
    case 'a':
        return FMT_istext(format) ? "a" : "ab";
    case 'r':
        return FMT_istext(format) ? "r" : "rb";
    case 'w':
        return FMT_istext(format) ? "w" : "wb";
    }
    /* The assert above should make sure we never reach this point */
    return NULL;
}

static const char *modeverb(char mode)
{
    switch (mode) {
    case 'a':
        return "appending";
    case 'r':
        return "reading";
    case 'w':
        return "writing";
    }
    return "(doing something)";
}

/*
 * Open a file for writing, owner-read-only.
 */
BIO *bio_open_owner(const char *filename, int format, int private)
{
    FILE *fp = NULL;
    BIO *b = NULL;
    int textmode, bflags;
#ifndef OPENSSL_NO_POSIX_IO
    int fd = -1, mode;
#endif

    if (!private || filename == NULL || strcmp(filename, "-") == 0)
        return bio_open_default(filename, 'w', format);

    textmode = FMT_istext(format);
#ifndef OPENSSL_NO_POSIX_IO
    mode = O_WRONLY;
# ifdef O_CREAT
    mode |= O_CREAT;
# endif
# ifdef O_TRUNC
    mode |= O_TRUNC;
# endif
    if (!textmode) {
# ifdef O_BINARY
        mode |= O_BINARY;
# elif defined(_O_BINARY)
        mode |= _O_BINARY;
# endif
    }

# ifdef OPENSSL_SYS_VMS
    /*
     * VMS doesn't have O_BINARY, it just doesn't make sense.  But,
     * it still needs to know that we're going binary, or fdopen()
     * will fail with "invalid argument"...  so we tell VMS what the
     * context is.
     */
    if (!textmode)
        fd = open(filename, mode, 0600, "ctx=bin");
    else
# endif
        fd = open(filename, mode, 0600);
    if (fd < 0)
        goto err;
    fp = fdopen(fd, modestr('w', format));
#else   /* OPENSSL_NO_POSIX_IO */
    /* Have stdio but not Posix IO, do the best we can */
    fp = fopen(filename, modestr('w', format));
#endif  /* OPENSSL_NO_POSIX_IO */
    if (fp == NULL)
        goto err;
    bflags = BIO_CLOSE;
    if (textmode)
        bflags |= BIO_FP_TEXT;
    b = BIO_new_fp(fp, bflags);
    if (b != NULL)
        return b;

 err:
    BIO_printf(bio_err, "%s: Can't open \"%s\" for writing, %s\n",
               opt_getprog(), filename, strerror(errno));
    ERR_print_errors(bio_err);
    /* If we have fp, then fdopen took over fd, so don't close both. */
    if (fp != NULL)
        fclose(fp);
#ifndef OPENSSL_NO_POSIX_IO
    else if (fd >= 0)
        close(fd);
#endif
    return NULL;
}

static BIO *bio_open_default_(const char *filename, char mode, int format,
                              int quiet)
{
    BIO *ret;

    if (filename == NULL || strcmp(filename, "-") == 0) {
        ret = mode == 'r' ? dup_bio_in(format) : dup_bio_out(format);
        if (quiet) {
            ERR_clear_error();
            return ret;
        }
        if (ret != NULL)
            return ret;
        BIO_printf(bio_err,
                   "Can't open %s, %s\n",
                   mode == 'r' ? "stdin" : "stdout", strerror(errno));
    } else {
        ret = BIO_new_file(filename, modestr(mode, format));
        if (quiet) {
            ERR_clear_error();
            return ret;
        }
        if (ret != NULL)
            return ret;
        BIO_printf(bio_err,
                   "Can't open \"%s\" for %s, %s\n",
                   filename, modeverb(mode), strerror(errno));
    }
    ERR_print_errors(bio_err);
    return NULL;
}

BIO *bio_open_default(const char *filename, char mode, int format)
{
    return bio_open_default_(filename, mode, format, 0);
}

BIO *bio_open_default_quiet(const char *filename, char mode, int format)
{
    return bio_open_default_(filename, mode, format, 1);
}

int mem_bio_to_file(BIO *in, const char *filename, int format, int private)
{
    int rv = 0, ret = 0;
    BIO *out = NULL;
    BUF_MEM *mem_buffer = NULL;

    rv = BIO_get_mem_ptr(in, &mem_buffer);
    if (rv <= 0) {
        BIO_puts(bio_err, "Error reading mem buffer\n");
        goto end;
    }
    out = bio_open_owner(filename, format, private);
    if (out == NULL)
        goto end;
    rv = BIO_write(out, mem_buffer->data, mem_buffer->length);
    if (rv < 0 || (size_t)rv != mem_buffer->length)
        BIO_printf(bio_err, "Error writing to output file: '%s'\n", filename);
    else
        ret = 1;
end:
    if (!ret)
        ERR_print_errors(bio_err);
    BIO_free_all(out);
    return ret;
}

void wait_for_async(SSL *s)
{
    /* On Windows select only works for sockets, so we simply don't wait  */
#ifndef OPENSSL_SYS_WINDOWS
    int width = 0;
    fd_set asyncfds;
    OSSL_ASYNC_FD *fds;
    size_t numfds;
    size_t i;

    if (!SSL_get_all_async_fds(s, NULL, &numfds))
        return;
    if (numfds == 0)
        return;
    fds = app_malloc(sizeof(OSSL_ASYNC_FD) * numfds, "allocate async fds");
    if (!SSL_get_all_async_fds(s, fds, &numfds)) {
        OPENSSL_free(fds);
        return;
    }

    FD_ZERO(&asyncfds);
    for (i = 0; i < numfds; i++) {
        if (width <= (int)fds[i])
            width = (int)fds[i] + 1;
        openssl_fdset((int)fds[i], &asyncfds);
    }
    select(width, (void *)&asyncfds, NULL, NULL, NULL);
    OPENSSL_free(fds);
#endif
}

/* if OPENSSL_SYS_WINDOWS is defined then so is OPENSSL_SYS_MSDOS */
#if defined(OPENSSL_SYS_MSDOS)
int has_stdin_waiting(void)
{
# if defined(OPENSSL_SYS_WINDOWS)
    HANDLE inhand = GetStdHandle(STD_INPUT_HANDLE);
    DWORD events = 0;
    INPUT_RECORD inputrec;
    DWORD insize = 1;
    BOOL peeked;

    if (inhand == INVALID_HANDLE_VALUE) {
        return 0;
    }

    peeked = PeekConsoleInput(inhand, &inputrec, insize, &events);
    if (!peeked) {
        /* Probably redirected input? _kbhit() does not work in this case */
        if (!feof(stdin)) {
            return 1;
        }
        return 0;
    }
# endif
    return _kbhit();
}
#endif

/* Corrupt a signature by modifying final byte */
void corrupt_signature(const ASN1_STRING *signature)
{
    unsigned char *s = signature->data;

    s[signature->length - 1] ^= 0x1;
}

int check_cert_time_string(const char *time, const char *desc)
{
    if (time == NULL || strcmp(time, "today") == 0
            || ASN1_TIME_set_string_X509(NULL, time))
        return 1;
    BIO_printf(bio_err,
               "%s is invalid, it should be \"today\" or have format [CC]YYMMDDHHMMSSZ\n",
               desc);
    return 0;
}

int set_cert_times(X509 *x, const char *startdate, const char *enddate,
                   int days, int strict_compare_times)
{
    if (!check_cert_time_string(startdate, "start date"))
        return 0;
    if (!check_cert_time_string(enddate, "end date"))
        return 0;
    if (startdate == NULL || strcmp(startdate, "today") == 0) {
        if (X509_gmtime_adj(X509_getm_notBefore(x), 0) == NULL) {
            BIO_printf(bio_err, "Error setting notBefore certificate field\n");
            return 0;
        }
    } else {
        if (!ASN1_TIME_set_string_X509(X509_getm_notBefore(x), startdate)) {
            BIO_printf(bio_err, "Error setting notBefore certificate field\n");
            return 0;
        }
    }
    if (enddate != NULL && strcmp(enddate, "today") == 0) {
        enddate = NULL;
        days = 0;
    }
    if (enddate == NULL) {
        if (X509_time_adj_ex(X509_getm_notAfter(x), days, 0, NULL) == NULL) {
            BIO_printf(bio_err, "Error setting notAfter certificate field\n");
            return 0;
        }
    } else if (!ASN1_TIME_set_string_X509(X509_getm_notAfter(x), enddate)) {
        BIO_printf(bio_err, "Error setting notAfter certificate field\n");
        return 0;
    }
    if (ASN1_TIME_compare(X509_get0_notAfter(x), X509_get0_notBefore(x)) < 0) {
        BIO_printf(bio_err, "%s: end date before start date\n",
                   strict_compare_times ? "Error" : "Warning");
        if (strict_compare_times)
            return 0;
    }
    return 1;
}

int set_crl_lastupdate(X509_CRL *crl, const char *lastupdate)
{
    int ret = 0;
    ASN1_TIME *tm = ASN1_TIME_new();

    if (tm == NULL)
        goto end;

    if (lastupdate == NULL) {
        if (X509_gmtime_adj(tm, 0) == NULL)
            goto end;
    } else {
        if (!ASN1_TIME_set_string_X509(tm, lastupdate))
            goto end;
    }

    if (!X509_CRL_set1_lastUpdate(crl, tm))
        goto end;

    ret = 1;
end:
    ASN1_TIME_free(tm);
    return ret;
}

int set_crl_nextupdate(X509_CRL *crl, const char *nextupdate,
                       long days, long hours, long secs)
{
    int ret = 0;
    ASN1_TIME *tm = ASN1_TIME_new();

    if (tm == NULL)
        goto end;

    if (nextupdate == NULL) {
        if (X509_time_adj_ex(tm, days, hours * 60 * 60 + secs, NULL) == NULL)
            goto end;
    } else {
        if (!ASN1_TIME_set_string_X509(tm, nextupdate))
            goto end;
    }

    if (!X509_CRL_set1_nextUpdate(crl, tm))
        goto end;

    ret = 1;
end:
    ASN1_TIME_free(tm);
    return ret;
}

void make_uppercase(char *string)
{
    int i;

    for (i = 0; string[i] != '\0'; i++)
        string[i] = toupper((unsigned char)string[i]);
}

OSSL_PARAM *app_params_new_from_opts(STACK_OF(OPENSSL_STRING) *opts,
                                     const OSSL_PARAM *paramdefs)
{
    OSSL_PARAM *params = NULL;
    size_t sz = (size_t)sk_OPENSSL_STRING_num(opts);
    size_t params_n;
    char *opt = "", *stmp, *vtmp = NULL;
    int found = 1;

    if (opts == NULL)
        return NULL;

    params = OPENSSL_zalloc(sizeof(OSSL_PARAM) * (sz + 1));
    if (params == NULL)
        return NULL;

    for (params_n = 0; params_n < sz; params_n++) {
        opt = sk_OPENSSL_STRING_value(opts, (int)params_n);
        if ((stmp = OPENSSL_strdup(opt)) == NULL
            || (vtmp = strchr(stmp, ':')) == NULL)
            goto err;
        /* Replace ':' with 0 to terminate the string pointed to by stmp */
        *vtmp = 0;
        /* Skip over the separator so that vmtp points to the value */
        vtmp++;
        if (!OSSL_PARAM_allocate_from_text(&params[params_n], paramdefs,
                                           stmp, vtmp, strlen(vtmp), &found))
            goto err;
        OPENSSL_free(stmp);
    }
    params[params_n] = OSSL_PARAM_construct_end();
    return params;
err:
    OPENSSL_free(stmp);
    BIO_printf(bio_err, "Parameter %s '%s'\n", found ? "error" : "unknown",
               opt);
    ERR_print_errors(bio_err);
    app_params_free(params);
    return NULL;
}

void app_params_free(OSSL_PARAM *params)
{
    int i;

    if (params != NULL) {
        for (i = 0; params[i].key != NULL; ++i)
            OPENSSL_free(params[i].data);
        OPENSSL_free(params);
    }
}

EVP_PKEY *app_keygen(EVP_PKEY_CTX *ctx, const char *alg, int bits, int verbose)
{
    EVP_PKEY *res = NULL;

    if (verbose && alg != NULL) {
        BIO_printf(bio_err, "Generating %s key", alg);
        if (bits > 0)
            BIO_printf(bio_err, " with %d bits\n", bits);
        else
            BIO_printf(bio_err, "\n");
    }
    if (!RAND_status())
        BIO_printf(bio_err, "Warning: generating random key material may take a long time\n"
                   "if the system has a poor entropy source\n");
    if (EVP_PKEY_keygen(ctx, &res) <= 0)
        BIO_printf(bio_err, "%s: Error generating %s key\n", opt_getprog(),
                   alg != NULL ? alg : "asymmetric");
    return res;
}

EVP_PKEY *app_paramgen(EVP_PKEY_CTX *ctx, const char *alg)
{
    EVP_PKEY *res = NULL;

    if (!RAND_status())
        BIO_printf(bio_err, "Warning: generating random key parameters may take a long time\n"
                   "if the system has a poor entropy source\n");
    if (EVP_PKEY_paramgen(ctx, &res) <= 0)
        BIO_printf(bio_err, "%s: Generating %s key parameters failed\n",
                   opt_getprog(), alg != NULL ? alg : "asymmetric");
    return res;
}

/*
 * Return non-zero if the legacy path is still an option.
 * This decision is based on the global command line operations and the
 * behaviour thus far.
 */
int opt_legacy_okay(void)
{
    int provider_options = opt_provider_option_given();
    int libctx = app_get0_libctx() != NULL || app_get0_propq() != NULL;

    /*
     * Having a provider option specified or a custom library context or
     * property query, is a sure sign we're not using legacy.
     */
    if (provider_options || libctx)
        return 0;
    return 1;
}
