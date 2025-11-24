/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"

#include <stdio.h>
#include <stdlib.h>
#include "internal/common.h"
#include <openssl/bio.h>
#include <openssl/crypto.h>
#include <openssl/trace.h>
#include <openssl/lhash.h>
#include <openssl/conf.h>
#include <openssl/x509.h>
#include <openssl/pem.h>
#include <openssl/ssl.h>
#ifndef OPENSSL_NO_ENGINE
# include <openssl/engine.h>
#endif
#include <openssl/err.h>
/* Needed to get the other O_xxx flags. */
#ifdef OPENSSL_SYS_VMS
# include <unixio.h>
#endif
#include "apps.h"
#include "progs.h"

/*
 * The LHASH callbacks ("hash" & "cmp") have been replaced by functions with
 * the base prototypes (we cast each variable inside the function to the
 * required type of "FUNCTION*"). This removes the necessity for
 * macro-generated wrapper functions.
 */
static LHASH_OF(FUNCTION) *prog_init(void);
static int do_cmd(LHASH_OF(FUNCTION) *prog, int argc, char *argv[]);
char *default_config_file = NULL;

BIO *bio_in = NULL;
BIO *bio_out = NULL;
BIO *bio_err = NULL;

static void warn_deprecated(const FUNCTION *fp)
{
    if (fp->deprecated_version != NULL)
        BIO_printf(bio_err, "The command %s was deprecated in version %s.",
                   fp->name, fp->deprecated_version);
    else
        BIO_printf(bio_err, "The command %s is deprecated.", fp->name);
    if (strcmp(fp->deprecated_alternative, DEPRECATED_NO_ALTERNATIVE) != 0)
        BIO_printf(bio_err, " Use '%s' instead.", fp->deprecated_alternative);
    BIO_printf(bio_err, "\n");
}

static int apps_startup(void)
{
    const char *use_libctx = NULL;
#ifdef SIGPIPE
    signal(SIGPIPE, SIG_IGN);
#endif

    /* Set non-default library initialisation settings */
    if (!OPENSSL_init_ssl(OPENSSL_INIT_ENGINE_ALL_BUILTIN
                          | OPENSSL_INIT_LOAD_CONFIG, NULL))
        return 0;

    (void)setup_ui_method();
    (void)setup_engine_loader();

    /*
     * NOTE: This is an undocumented feature required for testing only.
     * There are no guarantees that it will exist in future builds.
     */
    use_libctx = getenv("OPENSSL_TEST_LIBCTX");
    if (use_libctx != NULL) {
        /* Set this to "1" to create a global libctx */
        if (strcmp(use_libctx, "1") == 0) {
            if (app_create_libctx() == NULL)
                return 0;
        }
    }

    return 1;
}

static void apps_shutdown(void)
{
    app_providers_cleanup();
    OSSL_LIB_CTX_free(app_get0_libctx());
    destroy_engine_loader();
    destroy_ui_method();
}


#ifndef OPENSSL_NO_TRACE
typedef struct tracedata_st {
    BIO *bio;
    unsigned int ingroup:1;
} tracedata;

static size_t internal_trace_cb(const char *buf, size_t cnt,
                                int category, int cmd, void *vdata)
{
    int ret = 0;
    tracedata *trace_data = vdata;
    char buffer[256], *hex;
    CRYPTO_THREAD_ID tid;

    switch (cmd) {
    case OSSL_TRACE_CTRL_BEGIN:
        if (trace_data->ingroup) {
            BIO_printf(bio_err, "ERROR: tracing already started\n");
            return 0;
        }
        trace_data->ingroup = 1;

        tid = CRYPTO_THREAD_get_current_id();
        hex = OPENSSL_buf2hexstr((const unsigned char *)&tid, sizeof(tid));
        BIO_snprintf(buffer, sizeof(buffer), "TRACE[%s]:%s: ",
                     hex == NULL ? "<null>" : hex,
                     OSSL_trace_get_category_name(category));
        OPENSSL_free(hex);
        BIO_set_prefix(trace_data->bio, buffer);
        break;
    case OSSL_TRACE_CTRL_WRITE:
        if (!trace_data->ingroup) {
            BIO_printf(bio_err, "ERROR: writing when tracing not started\n");
            return 0;
        }

        ret = BIO_write(trace_data->bio, buf, cnt);
        break;
    case OSSL_TRACE_CTRL_END:
        if (!trace_data->ingroup) {
            BIO_printf(bio_err, "ERROR: finishing when tracing not started\n");
            return 0;
        }
        trace_data->ingroup = 0;

        BIO_set_prefix(trace_data->bio, NULL);

        break;
    }

    return ret < 0 ? 0 : ret;
}

DEFINE_STACK_OF(tracedata)
static STACK_OF(tracedata) *trace_data_stack;

static void tracedata_free(tracedata *data)
{
    BIO_free_all(data->bio);
    OPENSSL_free(data);
}

static void cleanup_trace(void)
{
    sk_tracedata_pop_free(trace_data_stack, tracedata_free);
}

static void setup_trace_category(int category)
{
    BIO *channel;
    tracedata *trace_data;
    BIO *bio = NULL;

    if (OSSL_trace_enabled(category))
        return;

    bio = BIO_new(BIO_f_prefix());
    channel = BIO_push(bio, dup_bio_err(FORMAT_TEXT));
    trace_data = OPENSSL_zalloc(sizeof(*trace_data));

    if (trace_data == NULL
        || bio == NULL
        || (trace_data->bio = channel) == NULL
        || OSSL_trace_set_callback(category, internal_trace_cb,
                                   trace_data) == 0
        || sk_tracedata_push(trace_data_stack, trace_data) == 0) {

        fprintf(stderr,
                "warning: unable to setup trace callback for category '%s'.\n",
                OSSL_trace_get_category_name(category));

        OSSL_trace_set_callback(category, NULL, NULL);
        BIO_free_all(channel);
    }
}

static void setup_trace(const char *str)
{
    char *val;

    /*
     * We add this handler as early as possible to ensure it's executed
     * as late as possible, i.e. after the TRACE code has done its cleanup
     * (which happens last in OPENSSL_cleanup).
     */
    atexit(cleanup_trace);

    trace_data_stack = sk_tracedata_new_null();
    val = OPENSSL_strdup(str);

    if (val != NULL) {
        char *valp = val;
        char *item;

        for (valp = val; (item = strtok(valp, ",")) != NULL; valp = NULL) {
            int category = OSSL_trace_get_category_num(item);

            if (category == OSSL_TRACE_CATEGORY_ALL) {
                while (++category < OSSL_TRACE_CATEGORY_NUM)
                    setup_trace_category(category);
                break;
            } else if (category > 0) {
                setup_trace_category(category);
            } else {
                fprintf(stderr,
                        "warning: unknown trace category: '%s'.\n", item);
            }
        }
    }

    OPENSSL_free(val);
}
#endif /* OPENSSL_NO_TRACE */

static char *help_argv[] = { "help", NULL };
static char *version_argv[] = { "version", NULL };

int main(int argc, char *argv[])
{
    FUNCTION f, *fp;
    LHASH_OF(FUNCTION) *prog = NULL;
    char *pname;
    const char *fname;
    ARGS arg;
    int global_help = 0;
    int global_version = 0;
    int ret = 0;

    arg.argv = NULL;
    arg.size = 0;

    /* Set up some of the environment. */
    bio_in = dup_bio_in(FORMAT_TEXT);
    bio_out = dup_bio_out(FORMAT_TEXT);
    bio_err = dup_bio_err(FORMAT_TEXT);

#if defined(OPENSSL_SYS_VMS) && defined(__DECC)
    argv = copy_argv(&argc, argv);
#elif defined(_WIN32)
    /* Replace argv[] with UTF-8 encoded strings. */
    win32_utf8argv(&argc, &argv);
#endif

#ifndef OPENSSL_NO_TRACE
    setup_trace(getenv("OPENSSL_TRACE"));
#endif

    if ((fname = "apps_startup", !apps_startup())
            || (fname = "prog_init", (prog = prog_init()) == NULL)) {
        BIO_printf(bio_err,
                   "FATAL: Startup failure (dev note: %s()) for %s\n",
                   fname, argv[0]);
        ERR_print_errors(bio_err);
        ret = 1;
        goto end;
    }
    pname = opt_progname(argv[0]);

    default_config_file = CONF_get1_default_config_file();
    if (default_config_file == NULL)
        app_bail_out("%s: could not get default config file\n", pname);

    /* first check the program name */
    f.name = pname;
    fp = lh_FUNCTION_retrieve(prog, &f);
    if (fp == NULL) {
        /* We assume we've been called as 'openssl ...' */
        global_help = argc > 1
            && (strcmp(argv[1], "-help") == 0 || strcmp(argv[1], "--help") == 0
                || strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--h") == 0);
        global_version = argc > 1
            && (strcmp(argv[1], "-version") == 0 || strcmp(argv[1], "--version") == 0
                || strcmp(argv[1], "-v") == 0 || strcmp(argv[1], "--v") == 0);

        argc--;
        argv++;
        opt_appname(argc == 1 || global_help ? "help" : global_version ? "version" : argv[0]);
    } else {
        argv[0] = pname;
    }

    /*
     * If there's no command, assume "help". If there's an override for help
     * or version run those, otherwise run the command given.
     */
    ret =  (argc == 0) || global_help
            ? do_cmd(prog, 1, help_argv)
            : global_version
                ? do_cmd(prog, 1, version_argv)
                : do_cmd(prog, argc, argv);

 end:
    OPENSSL_free(default_config_file);
    lh_FUNCTION_free(prog);
    OPENSSL_free(arg.argv);
    if (!app_RAND_write())
        ret = EXIT_FAILURE;

    BIO_free(bio_in);
    BIO_free_all(bio_out);
    apps_shutdown();
    BIO_free_all(bio_err);
    EXIT(ret);
}

typedef enum HELP_CHOICE {
    OPT_hERR = -1, OPT_hEOF = 0, OPT_hHELP
} HELP_CHOICE;

const OPTIONS help_options[] = {
    {OPT_HELP_STR, 1, '-', "Usage: help [options] [command]\n"},

    OPT_SECTION("General"),
    {"help", OPT_hHELP, '-', "Display this summary"},

    OPT_PARAMETERS(),
    {"command", 0, 0, "Name of command to display help (optional)"},
    {NULL}
};

int help_main(int argc, char **argv)
{
    FUNCTION *fp;
    int i, nl;
    FUNC_TYPE tp;
    char *prog;
    HELP_CHOICE o;
    DISPLAY_COLUMNS dc;
    char *new_argv[3];

    prog = opt_init(argc, argv, help_options);
    while ((o = opt_next()) != OPT_hEOF) {
        switch (o) {
        case OPT_hERR:
        case OPT_hEOF:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            return 1;
        case OPT_hHELP:
            opt_help(help_options);
            return 0;
        }
    }

    /* One optional argument, the command to get help for. */
    if (opt_num_rest() == 1) {
        new_argv[0] = opt_rest()[0];
        new_argv[1] = "--help";
        new_argv[2] = NULL;
        return do_cmd(prog_init(), 2, new_argv);
    }
    if (!opt_check_rest_arg(NULL)) {
        BIO_printf(bio_err, "Usage: %s\n", prog);
        return 1;
    }

    calculate_columns(functions, &dc);
    BIO_printf(bio_err, "%s:\n\nStandard commands", prog);
    i = 0;
    tp = FT_none;
    for (fp = functions; fp->name != NULL; fp++) {
        nl = 0;
        if (i++ % dc.columns == 0) {
            BIO_printf(bio_err, "\n");
            nl = 1;
        }
        if (fp->type != tp) {
            tp = fp->type;
            if (!nl)
                BIO_printf(bio_err, "\n");
            if (tp == FT_md) {
                i = 1;
                BIO_printf(bio_err,
                           "\nMessage Digest commands (see the `dgst' command for more details)\n");
            } else if (tp == FT_cipher) {
                i = 1;
                BIO_printf(bio_err,
                           "\nCipher commands (see the `enc' command for more details)\n");
            }
        }
        BIO_printf(bio_err, "%-*s", dc.width, fp->name);
    }
    BIO_printf(bio_err, "\n\n");
    return 0;
}

static int do_cmd(LHASH_OF(FUNCTION) *prog, int argc, char *argv[])
{
    FUNCTION f, *fp;

    if (argc <= 0 || argv[0] == NULL)
        return 0;
    memset(&f, 0, sizeof(f));
    f.name = argv[0];
    fp = lh_FUNCTION_retrieve(prog, &f);
    if (fp == NULL) {
        if (EVP_get_digestbyname(argv[0])) {
            f.type = FT_md;
            f.func = dgst_main;
            fp = &f;
        } else if (EVP_get_cipherbyname(argv[0])) {
            f.type = FT_cipher;
            f.func = enc_main;
            fp = &f;
        }
    }
    if (fp != NULL) {
        if (fp->deprecated_alternative != NULL)
            warn_deprecated(fp);
        return fp->func(argc, argv);
    }
    f.name = argv[0];
    if (CHECK_AND_SKIP_PREFIX(f.name, "no-")) {
        /*
         * User is asking if foo is unsupported, by trying to "run" the
         * no-foo command.  Strange.
         */
        if (lh_FUNCTION_retrieve(prog, &f) == NULL) {
            BIO_printf(bio_out, "%s\n", argv[0]);
            return 0;
        }
        BIO_printf(bio_out, "%s\n", argv[0] + 3);
        return 1;
    }

    BIO_printf(bio_err, "Invalid command '%s'; type \"help\" for a list.\n",
               argv[0]);
    return 1;
}

static int function_cmp(const FUNCTION *a, const FUNCTION *b)
{
    return strncmp(a->name, b->name, 8);
}

static unsigned long function_hash(const FUNCTION *a)
{
    return OPENSSL_LH_strhash(a->name);
}

static int SortFnByName(const void *_f1, const void *_f2)
{
    const FUNCTION *f1 = _f1;
    const FUNCTION *f2 = _f2;

    if (f1->type != f2->type)
        return f1->type - f2->type;
    return strcmp(f1->name, f2->name);
}

static LHASH_OF(FUNCTION) *prog_init(void)
{
    static LHASH_OF(FUNCTION) *ret = NULL;
    static int prog_inited = 0;
    FUNCTION *f;
    size_t i;

    if (prog_inited)
        return ret;

    prog_inited = 1;

    /* Sort alphabetically within category. For nicer help displays. */
    for (i = 0, f = functions; f->name != NULL; ++f, ++i)
        ;
    qsort(functions, i, sizeof(*functions), SortFnByName);

    if ((ret = lh_FUNCTION_new(function_hash, function_cmp)) == NULL)
        return NULL;

    for (f = functions; f->name != NULL; f++)
        (void)lh_FUNCTION_insert(ret, f);
    return ret;
}
