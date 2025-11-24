/*
 * Copyright 2012-2024 The OpenSSL Project Authors. All Rights Reserved.
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
#include <openssl/x509v3.h>

/* Multi string module: add table entries from a given section */

static int do_tcreate(const char *value, const char *name);

static int stbl_module_init(CONF_IMODULE *md, const CONF *cnf)
{
    int i;
    const char *stbl_section;
    STACK_OF(CONF_VALUE) *sktmp;
    CONF_VALUE *mval;

    stbl_section = CONF_imodule_get_value(md);
    if ((sktmp = NCONF_get_section(cnf, stbl_section)) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_ERROR_LOADING_SECTION);
        return 0;
    }
    for (i = 0; i < sk_CONF_VALUE_num(sktmp); i++) {
        mval = sk_CONF_VALUE_value(sktmp, i);
        if (!do_tcreate(mval->value, mval->name)) {
            ERR_raise(ERR_LIB_ASN1, ASN1_R_INVALID_VALUE);
            return 0;
        }
    }
    return 1;
}

static void stbl_module_finish(CONF_IMODULE *md)
{
    ASN1_STRING_TABLE_cleanup();
}

void ASN1_add_stable_module(void)
{
    CONF_module_add("stbl_section", stbl_module_init, stbl_module_finish);
}

/*
 * Create an table entry based on a name value pair. format is oid_name =
 * n1:v1, n2:v2,... where name is "min", "max", "mask" or "flags".
 */

static int do_tcreate(const char *value, const char *name)
{
    char *eptr;
    int nid, i, rv = 0;
    long tbl_min = -1, tbl_max = -1;
    unsigned long tbl_mask = 0, tbl_flags = 0;
    STACK_OF(CONF_VALUE) *lst = NULL;
    CONF_VALUE *cnf = NULL;
    nid = OBJ_sn2nid(name);
    if (nid == NID_undef)
        nid = OBJ_ln2nid(name);
    if (nid == NID_undef)
        goto err;
    lst = X509V3_parse_list(value);
    if (!lst)
        goto err;
    for (i = 0; i < sk_CONF_VALUE_num(lst); i++) {
        cnf = sk_CONF_VALUE_value(lst, i);
        if (cnf->value == NULL)
            goto err;
        if (strcmp(cnf->name, "min") == 0) {
            tbl_min = strtoul(cnf->value, &eptr, 0);
            if (*eptr)
                goto err;
        } else if (strcmp(cnf->name, "max") == 0) {
            tbl_max = strtoul(cnf->value, &eptr, 0);
            if (*eptr)
                goto err;
        } else if (strcmp(cnf->name, "mask") == 0) {
            if (!ASN1_str2mask(cnf->value, &tbl_mask) || !tbl_mask)
                goto err;
        } else if (strcmp(cnf->name, "flags") == 0) {
            if (strcmp(cnf->value, "nomask") == 0)
                tbl_flags = STABLE_NO_MASK;
            else if (strcmp(cnf->value, "none") == 0)
                tbl_flags = STABLE_FLAGS_CLEAR;
            else
                goto err;
        } else
            goto err;
    }
    rv = 1;
 err:
    if (rv == 0) {
        if (cnf)
            ERR_raise_data(ERR_LIB_ASN1, ASN1_R_INVALID_STRING_TABLE_VALUE,
                           "field=%s, value=%s", cnf->name,
                                                 cnf->value != NULL ? cnf->value
                                                 : value);
        else
            ERR_raise_data(ERR_LIB_ASN1, ASN1_R_INVALID_STRING_TABLE_VALUE,
                           "name=%s, value=%s", name, value);
    } else {
        rv = ASN1_STRING_TABLE_add(nid, tbl_min, tbl_max,
                                   tbl_mask, tbl_flags);
        if (!rv)
            ERR_raise(ERR_LIB_ASN1, ERR_R_ASN1_LIB);
    }
    sk_CONF_VALUE_pop_free(lst, X509V3_conf_free);
    return rv;
}
