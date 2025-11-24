/*
 * Copyright 2002-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include "internal/cryptlib.h"
#include "internal/rcu.h"
#include <stdio.h>
#include <ctype.h>
#include <openssl/crypto.h>
#include "internal/conf.h"
#include <openssl/conf_api.h>
#include "internal/dso.h"
#include "internal/thread_once.h"
#include <openssl/x509.h>
#include <openssl/trace.h>
#include <openssl/engine.h>
#include "conf_local.h"

DEFINE_STACK_OF(CONF_MODULE)
DEFINE_STACK_OF(CONF_IMODULE)

#define DSO_mod_init_name "OPENSSL_init"
#define DSO_mod_finish_name "OPENSSL_finish"

/*
 * This structure contains a data about supported modules. entries in this
 * table correspond to either dynamic or static modules.
 */

struct conf_module_st {
    /* DSO of this module or NULL if static */
    DSO *dso;
    /* Name of the module */
    char *name;
    /* Init function */
    conf_init_func *init;
    /* Finish function */
    conf_finish_func *finish;
    /* Number of successfully initialized modules */
    int links;
    void *usr_data;
};

/*
 * This structure contains information about modules that have been
 * successfully initialized. There may be more than one entry for a given
 * module.
 */

struct conf_imodule_st {
    CONF_MODULE *pmod;
    char *name;
    char *value;
    unsigned long flags;
    void *usr_data;
};

static CRYPTO_ONCE init_module_list_lock = CRYPTO_ONCE_STATIC_INIT;
static CRYPTO_RCU_LOCK *module_list_lock = NULL;
static STACK_OF(CONF_MODULE) *supported_modules = NULL; /* protected by lock */
static STACK_OF(CONF_IMODULE) *initialized_modules = NULL; /* protected by lock */

static CRYPTO_ONCE load_builtin_modules = CRYPTO_ONCE_STATIC_INIT;

static void module_free(CONF_MODULE *md);
static void module_finish(CONF_IMODULE *imod);
static int module_run(const CONF *cnf, const char *name, const char *value,
                      unsigned long flags);
static CONF_MODULE *module_add(DSO *dso, const char *name,
                               conf_init_func *ifunc,
                               conf_finish_func *ffunc);
static CONF_MODULE *module_find(const char *name);
static int module_init(CONF_MODULE *pmod, const char *name, const char *value,
                       const CONF *cnf);
static CONF_MODULE *module_load_dso(const CONF *cnf, const char *name,
                                    const char *value);

static int conf_modules_finish_int(void);

static void module_lists_free(void)
{
    ossl_rcu_lock_free(module_list_lock);
    module_list_lock = NULL;

    sk_CONF_MODULE_free(supported_modules);
    supported_modules = NULL;

    sk_CONF_IMODULE_free(initialized_modules);
    initialized_modules = NULL;
}

DEFINE_RUN_ONCE_STATIC(do_init_module_list_lock)
{
    module_list_lock = ossl_rcu_lock_new(1, NULL);
    if (module_list_lock == NULL) {
        ERR_raise(ERR_LIB_CONF, ERR_R_CRYPTO_LIB);
        return 0;
    }

    return 1;
}

static int conf_diagnostics(const CONF *cnf)
{
    int status;
    long result = 0;

    ERR_set_mark();
    status = NCONF_get_number_e(cnf, NULL, "config_diagnostics", &result);
    ERR_pop_to_mark();
    if (status > 0) {
        OSSL_LIB_CTX_set_conf_diagnostics(cnf->libctx, result > 0);
        return result > 0;
    }
    return OSSL_LIB_CTX_get_conf_diagnostics(cnf->libctx);
}

/* Main function: load modules from a CONF structure */

int CONF_modules_load(const CONF *cnf, const char *appname,
                      unsigned long flags)
{
    STACK_OF(CONF_VALUE) *values;
    CONF_VALUE *vl;
    char *vsection = NULL;
    int ret, i;

    if (!cnf)
        return 1;

    if (conf_diagnostics(cnf))
        flags &= ~(CONF_MFLAGS_IGNORE_ERRORS
                   | CONF_MFLAGS_IGNORE_RETURN_CODES
                   | CONF_MFLAGS_SILENT
                   | CONF_MFLAGS_IGNORE_MISSING_FILE);

    ERR_set_mark();
    if (appname)
        vsection = NCONF_get_string(cnf, NULL, appname);

    if (!appname || (!vsection && (flags & CONF_MFLAGS_DEFAULT_SECTION)))
        vsection = NCONF_get_string(cnf, NULL, "openssl_conf");

    if (!vsection) {
        ERR_pop_to_mark();
        return 1;
    }

    OSSL_TRACE1(CONF, "Configuration in section %s\n", vsection);
    values = NCONF_get_section(cnf, vsection);

    if (values == NULL) {
        if (!(flags & CONF_MFLAGS_SILENT)) {
            ERR_clear_last_mark();
            ERR_raise_data(ERR_LIB_CONF,
                           CONF_R_OPENSSL_CONF_REFERENCES_MISSING_SECTION,
                           "openssl_conf=%s", vsection);
        } else {
            ERR_pop_to_mark();
        }
        return 0;
    }
    ERR_pop_to_mark();

    for (i = 0; i < sk_CONF_VALUE_num(values); i++) {
        vl = sk_CONF_VALUE_value(values, i);
        ERR_set_mark();
        ret = module_run(cnf, vl->name, vl->value, flags);
        OSSL_TRACE3(CONF, "Running module %s (%s) returned %d\n",
                    vl->name, vl->value, ret);
        if (ret <= 0)
            if (!(flags & CONF_MFLAGS_IGNORE_ERRORS)) {
                ERR_clear_last_mark();
                return ret;
            }
        ERR_pop_to_mark();
    }

    return 1;

}

int CONF_modules_load_file_ex(OSSL_LIB_CTX *libctx, const char *filename,
                              const char *appname, unsigned long flags)
{
    char *file = NULL;
    CONF *conf = NULL;
    int ret = 0, diagnostics = OSSL_LIB_CTX_get_conf_diagnostics(libctx);

    ERR_set_mark();

    if (filename == NULL) {
        file = CONF_get1_default_config_file();
        if (file == NULL)
            goto err;
        if (*file == '\0') {
            /* Do not try to load an empty file name but do not error out */
            ret = 1;
            goto err;
        }
    } else {
        file = (char *)filename;
    }

    conf = NCONF_new_ex(libctx, NULL);
    if (conf == NULL)
        goto err;

    if (NCONF_load(conf, file, NULL) <= 0) {
        if ((flags & CONF_MFLAGS_IGNORE_MISSING_FILE) &&
            (ERR_GET_REASON(ERR_peek_last_error()) == CONF_R_NO_SUCH_FILE)) {
            ret = 1;
        }
        goto err;
    }

    ret = CONF_modules_load(conf, appname, flags);
    /* CONF_modules_load() might change the diagnostics setting, reread it. */
    diagnostics = OSSL_LIB_CTX_get_conf_diagnostics(libctx);

 err:
    if (filename == NULL)
        OPENSSL_free(file);
    NCONF_free(conf);

    if ((flags & CONF_MFLAGS_IGNORE_RETURN_CODES) != 0 && !diagnostics)
        ret = 1;

    if (ret > 0)
        ERR_pop_to_mark();
    else
        ERR_clear_last_mark();

    return ret;
}

int CONF_modules_load_file(const char *filename,
                           const char *appname, unsigned long flags)
{
    return CONF_modules_load_file_ex(NULL, filename, appname, flags);
}

DEFINE_RUN_ONCE_STATIC(do_load_builtin_modules)
{
    OPENSSL_load_builtin_modules();
#ifndef OPENSSL_NO_ENGINE
    /* Need to load ENGINEs */
    ENGINE_load_builtin_engines();
#endif
    return 1;
}

static int module_run(const CONF *cnf, const char *name, const char *value,
                      unsigned long flags)
{
    CONF_MODULE *md;
    int ret;

    if (!RUN_ONCE(&load_builtin_modules, do_load_builtin_modules))
        return -1;

    md = module_find(name);

    /* Module not found: try to load DSO */
    if (!md && !(flags & CONF_MFLAGS_NO_DSO))
        md = module_load_dso(cnf, name, value);

    if (!md) {
        if (!(flags & CONF_MFLAGS_SILENT)) {
            ERR_raise_data(ERR_LIB_CONF, CONF_R_UNKNOWN_MODULE_NAME,
                           "module=%s", name);
        }
        return -1;
    }

    ret = module_init(md, name, value, cnf);

    if (ret <= 0) {
        if (!(flags & CONF_MFLAGS_SILENT))
            ERR_raise_data(ERR_LIB_CONF, CONF_R_MODULE_INITIALIZATION_ERROR,
                           "module=%s, value=%s retcode=%-8d",
                           name, value, ret);
    }

    return ret;
}

/* Load a module from a DSO */
static CONF_MODULE *module_load_dso(const CONF *cnf,
                                    const char *name, const char *value)
{
    DSO *dso = NULL;
    conf_init_func *ifunc;
    conf_finish_func *ffunc;
    const char *path = NULL;
    int errcode = 0;
    CONF_MODULE *md;

    /* Look for alternative path in module section */
    path = _CONF_get_string(cnf, value, "path");
    if (path == NULL) {
        path = name;
    }
    dso = DSO_load(NULL, path, NULL, 0);
    if (dso == NULL) {
        errcode = CONF_R_ERROR_LOADING_DSO;
        goto err;
    }
    ifunc = (conf_init_func *)DSO_bind_func(dso, DSO_mod_init_name);
    if (ifunc == NULL) {
        errcode = CONF_R_MISSING_INIT_FUNCTION;
        goto err;
    }
    ffunc = (conf_finish_func *)DSO_bind_func(dso, DSO_mod_finish_name);
    /* All OK, add module */
    md = module_add(dso, name, ifunc, ffunc);

    if (md == NULL)
        goto err;

    return md;

 err:
    DSO_free(dso);
    ERR_raise_data(ERR_LIB_CONF, errcode, "module=%s, path=%s", name, path);
    return NULL;
}

/* add module to list */
static CONF_MODULE *module_add(DSO *dso, const char *name,
                               conf_init_func *ifunc, conf_finish_func *ffunc)
{
    CONF_MODULE *tmod = NULL;
    STACK_OF(CONF_MODULE) *old_modules;
    STACK_OF(CONF_MODULE) *new_modules;

    if (!RUN_ONCE(&init_module_list_lock, do_init_module_list_lock))
        return NULL;

    ossl_rcu_write_lock(module_list_lock);

    old_modules = ossl_rcu_deref(&supported_modules);

    if (old_modules == NULL)
        new_modules = sk_CONF_MODULE_new_null();
    else
        new_modules = sk_CONF_MODULE_dup(old_modules);

    if (new_modules == NULL)
        goto err;

    if ((tmod = OPENSSL_zalloc(sizeof(*tmod))) == NULL)
        goto err;

    tmod->dso = dso;
    tmod->name = OPENSSL_strdup(name);
    tmod->init = ifunc;
    tmod->finish = ffunc;
    if (tmod->name == NULL)
        goto err;

    if (!sk_CONF_MODULE_push(new_modules, tmod))
        goto err;

    ossl_rcu_assign_ptr(&supported_modules, &new_modules);
    ossl_rcu_write_unlock(module_list_lock);
    ossl_synchronize_rcu(module_list_lock);

    sk_CONF_MODULE_free(old_modules);
    return tmod;

 err:
    ossl_rcu_write_unlock(module_list_lock);
    if (tmod != NULL) {
        OPENSSL_free(tmod->name);
        OPENSSL_free(tmod);
    }
    sk_CONF_MODULE_free(new_modules);
    return NULL;
}

/*
 * Find a module from the list. We allow module names of the form
 * modname.XXXX to just search for modname to allow the same module to be
 * initialized more than once.
 */

static CONF_MODULE *module_find(const char *name)
{
    CONF_MODULE *tmod;
    int i, nchar;
    char *p;
    STACK_OF(CONF_MODULE) *mods;

    p = strrchr(name, '.');

    if (p)
        nchar = p - name;
    else
        nchar = strlen(name);

    if (!RUN_ONCE(&init_module_list_lock, do_init_module_list_lock))
        return NULL;

    ossl_rcu_read_lock(module_list_lock);
    mods = ossl_rcu_deref(&supported_modules);

    for (i = 0; i < sk_CONF_MODULE_num(mods); i++) {
        tmod = sk_CONF_MODULE_value(mods, i);
        if (strncmp(tmod->name, name, nchar) == 0) {
            ossl_rcu_read_unlock(module_list_lock);
            return tmod;
        }
    }

    ossl_rcu_read_unlock(module_list_lock);
    return NULL;
}

/* initialize a module */
static int module_init(CONF_MODULE *pmod, const char *name, const char *value,
                       const CONF *cnf)
{
    int ret = 1;
    int init_called = 0;
    CONF_IMODULE *imod = NULL;
    STACK_OF(CONF_IMODULE) *old_modules;
    STACK_OF(CONF_IMODULE) *new_modules;

    /* Otherwise add initialized module to list */
    imod = OPENSSL_malloc(sizeof(*imod));
    if (imod == NULL)
        goto err;

    imod->pmod = pmod;
    imod->name = OPENSSL_strdup(name);
    imod->value = OPENSSL_strdup(value);
    imod->usr_data = NULL;

    if (!imod->name || !imod->value)
        goto memerr;

    /* Try to initialize module */
    if (pmod->init) {
        ret = pmod->init(imod, cnf);
        init_called = 1;
        /* Error occurred, exit */
        if (ret <= 0)
            goto err;
    }

    if (!RUN_ONCE(&init_module_list_lock, do_init_module_list_lock))
        goto err;

    ossl_rcu_write_lock(module_list_lock);

    old_modules = ossl_rcu_deref(&initialized_modules);

    if (old_modules == NULL)
        new_modules = sk_CONF_IMODULE_new_null();
    else
        new_modules = sk_CONF_IMODULE_dup(old_modules);

    if (new_modules == NULL) {
        ossl_rcu_write_unlock(module_list_lock);
        ERR_raise(ERR_LIB_CONF, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if (!sk_CONF_IMODULE_push(new_modules, imod)) {
        ossl_rcu_write_unlock(module_list_lock);
        sk_CONF_IMODULE_free(new_modules);
        ERR_raise(ERR_LIB_CONF, ERR_R_CRYPTO_LIB);
        goto err;
    }

    pmod->links++;

    ossl_rcu_assign_ptr(&initialized_modules, &new_modules);
    ossl_rcu_write_unlock(module_list_lock);
    ossl_synchronize_rcu(module_list_lock);
    sk_CONF_IMODULE_free(old_modules);
    return ret;

 err:

    /* We've started the module so we'd better finish it */
    if (pmod->finish && init_called)
        pmod->finish(imod);

 memerr:
    if (imod) {
        OPENSSL_free(imod->name);
        OPENSSL_free(imod->value);
        OPENSSL_free(imod);
    }

    return -1;

}

/*
 * Unload any dynamic modules that have a link count of zero: i.e. have no
 * active initialized modules. If 'all' is set then all modules are unloaded
 * including static ones.
 */

void CONF_modules_unload(int all)
{
    int i;
    CONF_MODULE *md;
    STACK_OF(CONF_MODULE) *old_modules;
    STACK_OF(CONF_MODULE) *new_modules;
    STACK_OF(CONF_MODULE) *to_delete;

    if (!conf_modules_finish_int()) /* also inits module list lock */
        return;

    ossl_rcu_write_lock(module_list_lock);

    old_modules = ossl_rcu_deref(&supported_modules);
    new_modules = sk_CONF_MODULE_dup(old_modules);

    if (new_modules == NULL) {
        ossl_rcu_write_unlock(module_list_lock);
        return;
    }

    to_delete = sk_CONF_MODULE_new_null();

    /* unload modules in reverse order */
    for (i = sk_CONF_MODULE_num(new_modules) - 1; i >= 0; i--) {
        md = sk_CONF_MODULE_value(new_modules, i);
        /* If static or in use and 'all' not set ignore it */
        if (((md->links > 0) || !md->dso) && !all)
            continue;
        /* Since we're working in reverse this is OK */
        (void)sk_CONF_MODULE_delete(new_modules, i);
        sk_CONF_MODULE_push(to_delete, md);
    }

    if (sk_CONF_MODULE_num(new_modules) == 0) {
        sk_CONF_MODULE_free(new_modules);
        new_modules = NULL;
    }

    ossl_rcu_assign_ptr(&supported_modules, &new_modules);
    ossl_rcu_write_unlock(module_list_lock);
    ossl_synchronize_rcu(module_list_lock);
    sk_CONF_MODULE_free(old_modules);
    sk_CONF_MODULE_pop_free(to_delete, module_free);

}

/* unload a single module */
static void module_free(CONF_MODULE *md)
{
    DSO_free(md->dso);
    OPENSSL_free(md->name);
    OPENSSL_free(md);
}

/* finish and free up all modules instances */

static int conf_modules_finish_int(void)
{
    CONF_IMODULE *imod;
    STACK_OF(CONF_IMODULE) *old_modules;
    STACK_OF(CONF_IMODULE) *new_modules = NULL;

    if (!RUN_ONCE(&init_module_list_lock, do_init_module_list_lock))
        return 0;

    /* If module_list_lock is NULL here it means we were already unloaded */
    if (module_list_lock == NULL)
        return 0;

    ossl_rcu_write_lock(module_list_lock);
    old_modules = ossl_rcu_deref(&initialized_modules);
    ossl_rcu_assign_ptr(&initialized_modules, &new_modules);
    ossl_rcu_write_unlock(module_list_lock);
    ossl_synchronize_rcu(module_list_lock);

    while (sk_CONF_IMODULE_num(old_modules) > 0) {
        imod = sk_CONF_IMODULE_pop(old_modules);
        module_finish(imod);
    }
    sk_CONF_IMODULE_free(old_modules);

    return 1;
}

void CONF_modules_finish(void)
{
    conf_modules_finish_int();
}

/* finish a module instance */

static void module_finish(CONF_IMODULE *imod)
{
    if (!imod)
        return;
    if (imod->pmod->finish)
        imod->pmod->finish(imod);
    imod->pmod->links--;
    OPENSSL_free(imod->name);
    OPENSSL_free(imod->value);
    OPENSSL_free(imod);
}

/* Add a static module to OpenSSL */

int CONF_module_add(const char *name, conf_init_func *ifunc,
                    conf_finish_func *ffunc)
{
    if (module_add(NULL, name, ifunc, ffunc))
        return 1;
    else
        return 0;
}

void ossl_config_modules_free(void)
{
    CONF_modules_unload(1); /* calls CONF_modules_finish */
    module_lists_free();
}

/* Utility functions */

const char *CONF_imodule_get_name(const CONF_IMODULE *md)
{
    return md->name;
}

const char *CONF_imodule_get_value(const CONF_IMODULE *md)
{
    return md->value;
}

void *CONF_imodule_get_usr_data(const CONF_IMODULE *md)
{
    return md->usr_data;
}

void CONF_imodule_set_usr_data(CONF_IMODULE *md, void *usr_data)
{
    md->usr_data = usr_data;
}

CONF_MODULE *CONF_imodule_get_module(const CONF_IMODULE *md)
{
    return md->pmod;
}

unsigned long CONF_imodule_get_flags(const CONF_IMODULE *md)
{
    return md->flags;
}

void CONF_imodule_set_flags(CONF_IMODULE *md, unsigned long flags)
{
    md->flags = flags;
}

void *CONF_module_get_usr_data(CONF_MODULE *pmod)
{
    return pmod->usr_data;
}

void CONF_module_set_usr_data(CONF_MODULE *pmod, void *usr_data)
{
    pmod->usr_data = usr_data;
}

/* Return default config file name */
char *CONF_get1_default_config_file(void)
{
    const char *t;
    char *file, *sep = "";
    size_t size;

    if ((file = ossl_safe_getenv("OPENSSL_CONF")) != NULL)
        return OPENSSL_strdup(file);

    t = X509_get_default_cert_area();
    /*
     * On windows systems with -DOSSL_WINCTX set, if the needed registry
     * keys are not yet set, openssl applets will return, due to an inability
     * to locate various directories, like the default cert area.  In that
     * event, clone an empty string here, so that commands like openssl version
     * continue to operate properly without needing to set OPENSSL_CONF.
     * Applets like cms will fail gracefully later when they try to parse an
     * empty config file
     */
    if (t == NULL)
        return OPENSSL_strdup("");

#ifndef OPENSSL_SYS_VMS
    sep = "/";
#endif
    size = strlen(t) + strlen(sep) + strlen(OPENSSL_CONF) + 1;
    file = OPENSSL_malloc(size);

    if (file == NULL)
        return NULL;
    BIO_snprintf(file, size, "%s%s%s", t, sep, OPENSSL_CONF);

    return file;
}

/*
 * This function takes a list separated by 'sep' and calls the callback
 * function giving the start and length of each member optionally stripping
 * leading and trailing whitespace. This can be used to parse comma separated
 * lists for example.
 */

int CONF_parse_list(const char *list_, int sep, int nospc,
                    int (*list_cb) (const char *elem, int len, void *usr),
                    void *arg)
{
    int ret;
    const char *lstart, *tmpend, *p;

    if (list_ == NULL) {
        ERR_raise(ERR_LIB_CONF, CONF_R_LIST_CANNOT_BE_NULL);
        return 0;
    }

    lstart = list_;
    for (;;) {
        if (nospc) {
            while (*lstart && isspace((unsigned char)*lstart))
                lstart++;
        }
        p = strchr(lstart, sep);
        if (p == lstart || *lstart == '\0')
            ret = list_cb(NULL, 0, arg);
        else {
            if (p)
                tmpend = p - 1;
            else
                tmpend = lstart + strlen(lstart) - 1;
            if (nospc) {
                while (isspace((unsigned char)*tmpend))
                    tmpend--;
            }
            ret = list_cb(lstart, tmpend - lstart + 1, arg);
        }
        if (ret <= 0)
            return ret;
        if (p == NULL)
            return 1;
        lstart = p + 1;
    }
}
