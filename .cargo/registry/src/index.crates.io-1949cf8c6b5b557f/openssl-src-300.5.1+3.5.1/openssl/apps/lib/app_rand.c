/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h" /* LIST_SEPARATOR_CHAR */
#include "apps.h"
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/conf.h>

static char *save_rand_file;
static STACK_OF(OPENSSL_STRING) *randfiles;

void app_RAND_load_conf(CONF *c, const char *section)
{
    const char *randfile = app_conf_try_string(c, section, "RANDFILE");

    if (randfile == NULL)
        return;
    if (RAND_load_file(randfile, -1) < 0) {
        BIO_printf(bio_err, "Can't load %s into RNG\n", randfile);
        ERR_print_errors(bio_err);
    }
    if (save_rand_file == NULL) {
        save_rand_file = OPENSSL_strdup(randfile);
        /* If some internal memory errors have occurred */
        if (save_rand_file == NULL) {
            BIO_printf(bio_err, "Can't duplicate %s\n", randfile);
            ERR_print_errors(bio_err);
        }
    }
}

static int loadfiles(char *name)
{
    char *p;
    int last, ret = 1;

    for (;;) {
        last = 0;
        for (p = name; *p != '\0' && *p != LIST_SEPARATOR_CHAR; p++)
            continue;
        if (*p == '\0')
            last = 1;
        *p = '\0';
        if (RAND_load_file(name, -1) < 0) {
            BIO_printf(bio_err, "Can't load %s into RNG\n", name);
            ERR_print_errors(bio_err);
            ret = 0;
        }
        if (last)
            break;
        name = p + 1;
        if (*name == '\0')
            break;
    }
    return ret;
}

int app_RAND_load(void)
{
    char *p;
    int i, ret = 1;

    for (i = 0; i < sk_OPENSSL_STRING_num(randfiles); i++) {
        p = sk_OPENSSL_STRING_value(randfiles, i);
        if (!loadfiles(p))
            ret = 0;
    }
    sk_OPENSSL_STRING_free(randfiles);
    return ret;
}

int app_RAND_write(void)
{
    int ret = 1;

    if (save_rand_file == NULL)
        return 1;
    if (RAND_write_file(save_rand_file) == -1) {
        BIO_printf(bio_err, "Cannot write random bytes:\n");
        ERR_print_errors(bio_err);
        ret = 0;
    }
    OPENSSL_free(save_rand_file);
    save_rand_file =  NULL;
    return ret;
}


/*
 * See comments in opt_verify for explanation of this.
 */
enum r_range { OPT_R_ENUM };

int opt_rand(int opt)
{
    switch ((enum r_range)opt) {
    case OPT_R__FIRST:
    case OPT_R__LAST:
        break;
    case OPT_R_RAND:
        if (randfiles == NULL
                && (randfiles = sk_OPENSSL_STRING_new_null()) == NULL)
            return 0;
        if (!sk_OPENSSL_STRING_push(randfiles, opt_arg()))
            return 0;
        break;
    case OPT_R_WRITERAND:
        OPENSSL_free(save_rand_file);
        save_rand_file = OPENSSL_strdup(opt_arg());
        if (save_rand_file == NULL)
            return 0;
        break;
    }
    return 1;
}
