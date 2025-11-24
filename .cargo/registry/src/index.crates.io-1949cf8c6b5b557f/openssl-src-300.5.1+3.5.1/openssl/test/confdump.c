/*
 * Copyright 1999-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include <openssl/bio.h>
#include <openssl/conf.h>
#include <openssl/safestack.h>
#include <openssl/err.h>

static void dump_section(const char *name, const CONF *cnf)
{
    STACK_OF(CONF_VALUE) *sect = NCONF_get_section(cnf, name);
    int i;

    printf("[ %s ]\n", name);
    for (i = 0; i < sk_CONF_VALUE_num(sect); i++) {
        CONF_VALUE *cv = sk_CONF_VALUE_value(sect, i);

        printf("%s = %s\n", cv->name, cv->value);
    }
}

int main(int argc, char **argv)
{
    long eline;
    CONF *conf = NCONF_new(NCONF_default());
    int ret = 1;
    STACK_OF(OPENSSL_CSTRING) *section_names = NULL;

    if (conf != NULL && NCONF_load(conf, argv[1], &eline)) {
        int i;

        section_names = NCONF_get_section_names(conf);
        for (i = 0; i < sk_OPENSSL_CSTRING_num(section_names); i++) {
            dump_section(sk_OPENSSL_CSTRING_value(section_names, i), conf);
        }
        sk_OPENSSL_CSTRING_free(section_names);
        ret = 0;
    } else {
        ERR_print_errors_fp(stderr);
    }
    NCONF_free(conf);
    return ret;
}
