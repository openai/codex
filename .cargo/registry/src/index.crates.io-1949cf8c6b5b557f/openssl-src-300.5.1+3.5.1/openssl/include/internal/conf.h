/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_CONF_H
# define OSSL_INTERNAL_CONF_H
# pragma once

# include <openssl/conf.h>

# define DEFAULT_CONF_MFLAGS \
    (CONF_MFLAGS_DEFAULT_SECTION | \
     CONF_MFLAGS_IGNORE_MISSING_FILE | \
     CONF_MFLAGS_IGNORE_RETURN_CODES)

struct ossl_init_settings_st {
    char *filename;
    char *appname;
    unsigned long flags;
};

int ossl_config_int(const OPENSSL_INIT_SETTINGS *);
void ossl_no_config_int(void);
void ossl_config_modules_free(void);

#endif
