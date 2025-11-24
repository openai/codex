/*
 * Copyright 2002-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include "eng_local.h"
#include <openssl/conf.h>
#include <openssl/trace.h>

/* ENGINE config module */

static const char *skip_dot(const char *name)
{
    const char *p = strchr(name, '.');

    if (p != NULL)
        return p + 1;
    return name;
}

static STACK_OF(ENGINE) *initialized_engines = NULL;

static int int_engine_init(ENGINE *e)
{
    if (!ENGINE_init(e))
        return 0;
    if (!initialized_engines)
        initialized_engines = sk_ENGINE_new_null();
    if (!initialized_engines || !sk_ENGINE_push(initialized_engines, e)) {
        ENGINE_finish(e);
        return 0;
    }
    return 1;
}

static int int_engine_configure(const char *name, const char *value, const CONF *cnf)
{
    int i;
    int ret = 0;
    long do_init = -1;
    STACK_OF(CONF_VALUE) *ecmds;
    CONF_VALUE *ecmd = NULL;
    const char *ctrlname, *ctrlvalue;
    ENGINE *e = NULL;
    int soft = 0;

    name = skip_dot(name);
    OSSL_TRACE1(CONF, "Configuring engine %s\n", name);
    /* Value is a section containing ENGINE commands */
    ecmds = NCONF_get_section(cnf, value);

    if (!ecmds) {
        ERR_raise(ERR_LIB_ENGINE, ENGINE_R_ENGINE_SECTION_ERROR);
        return 0;
    }

    for (i = 0; i < sk_CONF_VALUE_num(ecmds); i++) {
        ecmd = sk_CONF_VALUE_value(ecmds, i);
        ctrlname = skip_dot(ecmd->name);
        ctrlvalue = ecmd->value;
        OSSL_TRACE2(CONF, "ENGINE: doing ctrl(%s,%s)\n",
                    ctrlname, ctrlvalue);

        /* First handle some special pseudo ctrls */

        /* Override engine name to use */
        if (strcmp(ctrlname, "engine_id") == 0)
            name = ctrlvalue;
        else if (strcmp(ctrlname, "soft_load") == 0)
            soft = 1;
        /* Load a dynamic ENGINE */
        else if (strcmp(ctrlname, "dynamic_path") == 0) {
            e = ENGINE_by_id("dynamic");
            if (!e)
                goto err;
            if (!ENGINE_ctrl_cmd_string(e, "SO_PATH", ctrlvalue, 0))
                goto err;
            if (!ENGINE_ctrl_cmd_string(e, "LIST_ADD", "2", 0))
                goto err;
            if (!ENGINE_ctrl_cmd_string(e, "LOAD", NULL, 0))
                goto err;
        }
        /* ... add other pseudos here ... */
        else {
            /*
             * At this point we need an ENGINE structural reference if we
             * don't already have one.
             */
            if (!e) {
                e = ENGINE_by_id(name);
                if (!e && soft) {
                    ERR_clear_error();
                    return 1;
                }
                if (!e)
                    goto err;
            }
            /*
             * Allow "EMPTY" to mean no value: this allows a valid "value" to
             * be passed to ctrls of type NO_INPUT
             */
            if (strcmp(ctrlvalue, "EMPTY") == 0)
                ctrlvalue = NULL;
            if (strcmp(ctrlname, "init") == 0) {
                if (!NCONF_get_number_e(cnf, value, "init", &do_init))
                    goto err;
                if (do_init == 1) {
                    if (!int_engine_init(e))
                        goto err;
                } else if (do_init != 0) {
                    ERR_raise(ERR_LIB_ENGINE, ENGINE_R_INVALID_INIT_VALUE);
                    goto err;
                }
            } else if (strcmp(ctrlname, "default_algorithms") == 0) {
                if (!ENGINE_set_default_string(e, ctrlvalue))
                    goto err;
            } else if (!ENGINE_ctrl_cmd_string(e, ctrlname, ctrlvalue, 0))
                goto err;
        }

    }
    if (e && (do_init == -1) && !int_engine_init(e)) {
        ecmd = NULL;
        goto err;
    }
    ret = 1;
 err:
    if (ret != 1) {
        if (ecmd == NULL)
            ERR_raise(ERR_LIB_ENGINE, ENGINE_R_ENGINE_CONFIGURATION_ERROR);
        else
            ERR_raise_data(ERR_LIB_ENGINE, ENGINE_R_ENGINE_CONFIGURATION_ERROR,
                           "section=%s, name=%s, value=%s",
                           ecmd->section, ecmd->name, ecmd->value);
    }
    ENGINE_free(e);
    return ret;
}

static int int_engine_module_init(CONF_IMODULE *md, const CONF *cnf)
{
    STACK_OF(CONF_VALUE) *elist;
    CONF_VALUE *cval;
    int i;
    OSSL_TRACE2(CONF, "Called engine module: name %s, value %s\n",
                CONF_imodule_get_name(md), CONF_imodule_get_value(md));
    /* Value is a section containing ENGINEs to configure */
    elist = NCONF_get_section(cnf, CONF_imodule_get_value(md));

    if (!elist) {
        ERR_raise(ERR_LIB_ENGINE, ENGINE_R_ENGINES_SECTION_ERROR);
        return 0;
    }

    for (i = 0; i < sk_CONF_VALUE_num(elist); i++) {
        cval = sk_CONF_VALUE_value(elist, i);
        if (!int_engine_configure(cval->name, cval->value, cnf))
            return 0;
    }

    return 1;
}

static void int_engine_module_finish(CONF_IMODULE *md)
{
    ENGINE *e;

    while ((e = sk_ENGINE_pop(initialized_engines)))
        ENGINE_finish(e);
    sk_ENGINE_free(initialized_engines);
    initialized_engines = NULL;
}

void ENGINE_add_conf_module(void)
{
    CONF_module_add("engines",
                    int_engine_module_init, int_engine_module_finish);
}
