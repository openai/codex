/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdarg.h>
#include <openssl/bio.h>
#include <openssl/safestack.h>
#include "opt.h"

static BIO *bio_in = NULL;
static BIO *bio_out = NULL;
static BIO *bio_err = NULL;

/*-
 * This program sets up a chain of BIO_f_filter() on top of bio_out, how
 * many is governed by the user through -n.  It allows the user to set the
 * indentation for each individual filter using -i and -p.  Then it reads
 * text from bio_in and prints it out through the BIO chain.
 *
 * The filter index is counted from the source/sink, i.e. index 0 is closest
 * to it.
 *
 * Example:
 *
 * $ echo foo | ./bio_prefix_text -n 2 -i 1:32 -p 1:FOO -i 0:3
 *    FOO                                foo
 * ^^^   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
 *  |                   |
 *  |                   +------ 32 spaces from filter 1
 *  +-------------------------- 3 spaces from filter 0
 */

static size_t amount = 0;
static BIO **chain = NULL;

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_AMOUNT,
    OPT_INDENT,
    OPT_PREFIX
} OPTION_CHOICE;

static const OPTIONS options[] = {
    { "n", OPT_AMOUNT, 'p', "Amount of BIO_f_prefix() filters" },
    /*
     * idx is the index to the BIO_f_filter chain(), where 0 is closest
     * to the source/sink BIO.  If idx isn't given, 0 is assumed
     */
    { "i", OPT_INDENT, 's', "Indentation in form '[idx:]indent'" },
    { "p", OPT_PREFIX, 's', "Prefix in form '[idx:]prefix'" },
    { NULL }
};

int opt_printf_stderr(const char *fmt, ...)
{
    va_list ap;
    int ret;

    va_start(ap, fmt);
    ret = BIO_vprintf(bio_err, fmt, ap);
    va_end(ap);
    return ret;
}

static int run_pipe(void)
{
    char buf[4096];

    while (!BIO_eof(bio_in)) {
        size_t bytes_in;
        size_t bytes_out;

        if (!BIO_read_ex(bio_in, buf, sizeof(buf), &bytes_in))
            return 0;
        bytes_out = 0;
        while (bytes_out < bytes_in) {
            size_t bytes;

            if (!BIO_write_ex(chain[amount - 1], buf, bytes_in, &bytes))
                return 0;
            bytes_out += bytes;
        }
    }
    return 1;
}

static int setup_bio_chain(const char *progname)
{
    BIO *next = NULL;
    size_t n = amount;

    chain = OPENSSL_zalloc(sizeof(*chain) * n);

    if (chain != NULL) {
        size_t i;

        if (!BIO_up_ref(bio_out)) /* Protection against freeing */
            goto err;

        next = bio_out;

        for (i = 0; n > 0; i++, n--) {
            BIO *curr = BIO_new(BIO_f_prefix());

            if (curr == NULL)
                goto err;
            chain[i] = BIO_push(curr, next);
            if (chain[i] == NULL)
                goto err;
            next = chain[i];
        }
    }
    return chain != NULL;
 err:
    /* Free the chain we built up */
    BIO_free_all(next);
    OPENSSL_free(chain);
    return 0;
}

static void cleanup(void)
{
    if (chain != NULL) {
        BIO_free_all(chain[amount - 1]);
        OPENSSL_free(chain);
    }

    BIO_free_all(bio_in);
    BIO_free_all(bio_out);
    BIO_free_all(bio_err);
}

static int setup(void)
{
    OPTION_CHOICE o;
    char *arg;
    char *colon;
    char *endptr;
    size_t idx, indent;
    const char *progname = opt_getprog();

    bio_in = BIO_new_fp(stdin, BIO_NOCLOSE | BIO_FP_TEXT);
    bio_out = BIO_new_fp(stdout, BIO_NOCLOSE | BIO_FP_TEXT);
    bio_err = BIO_new_fp(stderr, BIO_NOCLOSE | BIO_FP_TEXT);
#ifdef __VMS
    bio_out = BIO_push(BIO_new(BIO_f_linebuffer()), bio_out);
    bio_err = BIO_push(BIO_new(BIO_f_linebuffer()), bio_err);
#endif

    OPENSSL_assert(bio_in != NULL);
    OPENSSL_assert(bio_out != NULL);
    OPENSSL_assert(bio_err != NULL);


    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_AMOUNT:
            arg = opt_arg();
            amount = strtoul(arg, &endptr, 10);
            if (endptr[0] != '\0') {
                BIO_printf(bio_err,
                           "%s: -n argument isn't a decimal number: %s",
                           progname, arg);
                return 0;
            }
            if (amount < 1) {
                BIO_printf(bio_err, "%s: must set up at least one filter",
                           progname);
                return 0;
            }
            if (!setup_bio_chain(progname)) {
                BIO_printf(bio_err, "%s: failed setting up filter chain",
                           progname);
                return 0;
            }
            break;
        case OPT_INDENT:
            if (chain == NULL) {
                BIO_printf(bio_err, "%s: -i given before -n", progname);
                return 0;
            }
            arg = opt_arg();
            colon = strchr(arg, ':');
            idx = 0;
            if (colon != NULL) {
                idx = strtoul(arg, &endptr, 10);
                if (endptr[0] != ':') {
                    BIO_printf(bio_err,
                               "%s: -i index isn't a decimal number: %s",
                               progname, arg);
                    return 0;
                }
                colon++;
            } else {
                colon = arg;
            }
            indent = strtoul(colon, &endptr, 10);
            if (endptr[0] != '\0') {
                BIO_printf(bio_err,
                           "%s: -i value isn't a decimal number: %s",
                           progname, arg);
                return 0;
            }
            if (idx >= amount) {
                BIO_printf(bio_err, "%s: index (%zu) not within range 0..%zu",
                           progname, idx, amount - 1);
                return 0;
            }
            if (BIO_set_indent(chain[idx], (long)indent) <= 0) {
                BIO_printf(bio_err, "%s: failed setting indentation: %s",
                           progname, arg);
                return 0;
            }
            break;
        case OPT_PREFIX:
            if (chain == NULL) {
                BIO_printf(bio_err, "%s: -p given before -n", progname);
                return 0;
            }
            arg = opt_arg();
            colon = strchr(arg, ':');
            idx = 0;
            if (colon != NULL) {
                idx = strtoul(arg, &endptr, 10);
                if (endptr[0] != ':') {
                    BIO_printf(bio_err,
                               "%s: -p index isn't a decimal number: %s",
                               progname, arg);
                    return 0;
                }
                colon++;
            } else {
                colon = arg;
            }
            if (idx >= amount) {
                BIO_printf(bio_err, "%s: index (%zu) not within range 0..%zu",
                           progname, idx, amount - 1);
                return 0;
            }
            if (BIO_set_prefix(chain[idx], colon) <= 0) {
                BIO_printf(bio_err, "%s: failed setting prefix: %s",
                           progname, arg);
                return 0;
            }
            break;
        default:
        case OPT_ERR:
            return 0;
        }
    }
    return 1;
}

int main(int argc, char **argv)
{
    int rv = EXIT_SUCCESS;

    opt_init(argc, argv, options);
    rv = (setup() && run_pipe()) ? EXIT_SUCCESS : EXIT_FAILURE;
    cleanup();
    return rv;
}
