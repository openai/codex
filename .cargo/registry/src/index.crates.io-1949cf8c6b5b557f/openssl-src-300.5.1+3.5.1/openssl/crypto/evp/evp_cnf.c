/*
 * Copyright 2012-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/crypto.h>
#include "internal/cryptlib.h"
#include <openssl/conf.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/trace.h>
#include "crypto/evp.h"

/* Algorithm configuration module. */

static int alg_module_init(CONF_IMODULE *md, const CONF *cnf)
{
    int i;
    const char *oid_section;
    STACK_OF(CONF_VALUE) *sktmp;
    CONF_VALUE *oval;

    OSSL_TRACE2(CONF, "Loading EVP module: name %s, value %s\n",
                CONF_imodule_get_name(md), CONF_imodule_get_value(md));

    oid_section = CONF_imodule_get_value(md);
    if ((sktmp = NCONF_get_section(cnf, oid_section)) == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_ERROR_LOADING_SECTION);
        return 0;
    }
    for (i = 0; i < sk_CONF_VALUE_num(sktmp); i++) {
        oval = sk_CONF_VALUE_value(sktmp, i);
        if (strcmp(oval->name, "fips_mode") == 0) {
            int m;

            /* Detailed error already reported. */
            if (!X509V3_get_value_bool(oval, &m))
                return 0;

            /*
             * fips_mode is deprecated and should not be used in new
             * configurations.
             */
            if (!evp_default_properties_enable_fips_int(
                    NCONF_get0_libctx((CONF *)cnf), m > 0, 0)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_SET_DEFAULT_PROPERTY_FAILURE);
                return 0;
            }
        } else if (strcmp(oval->name, "default_properties") == 0) {
            if (!evp_set_default_properties_int(NCONF_get0_libctx((CONF *)cnf),
                        oval->value, 0, 0)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_SET_DEFAULT_PROPERTY_FAILURE);
                return 0;
            }
        } else {
            ERR_raise_data(ERR_LIB_EVP, EVP_R_UNKNOWN_OPTION,
                           "name=%s, value=%s", oval->name, oval->value);
            return 0;
        }

    }
    return 1;
}

void EVP_add_alg_module(void)
{
    OSSL_TRACE(CONF, "Adding config module 'alg_section'\n");
    CONF_module_add("alg_section", alg_module_init, 0);
}
