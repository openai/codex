/*
 * Copyright 2000-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include <stdio.h>
#include <string.h>
#include "internal/conf.h"
#include "crypto/ctype.h"
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/conf.h>
#include <openssl/conf_api.h>
#include "conf_local.h"
#include <openssl/lhash.h>

static CONF_METHOD *default_CONF_method = NULL;

/* Init a 'CONF' structure from an old LHASH */

void CONF_set_nconf(CONF *conf, LHASH_OF(CONF_VALUE) *hash)
{
    if (default_CONF_method == NULL)
        default_CONF_method = NCONF_default();

    default_CONF_method->init(conf);
    conf->data = hash;
}

/*
 * The following section contains the "CONF classic" functions, rewritten in
 * terms of the new CONF interface.
 */

int CONF_set_default_method(CONF_METHOD *meth)
{
    default_CONF_method = meth;
    return 1;
}

LHASH_OF(CONF_VALUE) *CONF_load(LHASH_OF(CONF_VALUE) *conf, const char *file,
                                long *eline)
{
    LHASH_OF(CONF_VALUE) *ltmp;
    BIO *in = NULL;

#ifdef OPENSSL_SYS_VMS
    in = BIO_new_file(file, "r");
#else
    in = BIO_new_file(file, "rb");
#endif
    if (in == NULL) {
        ERR_raise(ERR_LIB_CONF, ERR_R_SYS_LIB);
        return NULL;
    }

    ltmp = CONF_load_bio(conf, in, eline);
    BIO_free(in);

    return ltmp;
}

#ifndef OPENSSL_NO_STDIO
LHASH_OF(CONF_VALUE) *CONF_load_fp(LHASH_OF(CONF_VALUE) *conf, FILE *fp,
                                   long *eline)
{
    BIO *btmp;
    LHASH_OF(CONF_VALUE) *ltmp;
    if ((btmp = BIO_new_fp(fp, BIO_NOCLOSE)) == NULL) {
        ERR_raise(ERR_LIB_CONF, ERR_R_BUF_LIB);
        return NULL;
    }
    ltmp = CONF_load_bio(conf, btmp, eline);
    BIO_free(btmp);
    return ltmp;
}
#endif

LHASH_OF(CONF_VALUE) *CONF_load_bio(LHASH_OF(CONF_VALUE) *conf, BIO *bp,
                                    long *eline)
{
    CONF ctmp;
    int ret;

    CONF_set_nconf(&ctmp, conf);

    ret = NCONF_load_bio(&ctmp, bp, eline);
    if (ret)
        return ctmp.data;
    return NULL;
}

STACK_OF(CONF_VALUE) *CONF_get_section(LHASH_OF(CONF_VALUE) *conf,
                                       const char *section)
{
    if (conf == NULL) {
        return NULL;
    } else {
        CONF ctmp;

        CONF_set_nconf(&ctmp, conf);
        return NCONF_get_section(&ctmp, section);
    }
}

char *CONF_get_string(LHASH_OF(CONF_VALUE) *conf, const char *group,
                      const char *name)
{
    if (conf == NULL) {
        return NCONF_get_string(NULL, group, name);
    } else {
        CONF ctmp;

        CONF_set_nconf(&ctmp, conf);
        return NCONF_get_string(&ctmp, group, name);
    }
}

long CONF_get_number(LHASH_OF(CONF_VALUE) *conf, const char *group,
                     const char *name)
{
    int status;
    long result = 0;

    ERR_set_mark();
    if (conf == NULL) {
        status = NCONF_get_number_e(NULL, group, name, &result);
    } else {
        CONF ctmp;

        CONF_set_nconf(&ctmp, conf);
        status = NCONF_get_number_e(&ctmp, group, name, &result);
    }
    ERR_pop_to_mark();
    return status == 0 ? 0L : result;
}

void CONF_free(LHASH_OF(CONF_VALUE) *conf)
{
    CONF ctmp;
    CONF_set_nconf(&ctmp, conf);
    NCONF_free_data(&ctmp);
}

#ifndef OPENSSL_NO_STDIO
int CONF_dump_fp(LHASH_OF(CONF_VALUE) *conf, FILE *out)
{
    BIO *btmp;
    int ret;

    if ((btmp = BIO_new_fp(out, BIO_NOCLOSE)) == NULL) {
        ERR_raise(ERR_LIB_CONF, ERR_R_BUF_LIB);
        return 0;
    }
    ret = CONF_dump_bio(conf, btmp);
    BIO_free(btmp);
    return ret;
}
#endif

int CONF_dump_bio(LHASH_OF(CONF_VALUE) *conf, BIO *out)
{
    CONF ctmp;

    CONF_set_nconf(&ctmp, conf);
    return NCONF_dump_bio(&ctmp, out);
}

/*
 * The following section contains the "New CONF" functions.  They are
 * completely centralised around a new CONF structure that may contain
 * basically anything, but at least a method pointer and a table of data.
 * These functions are also written in terms of the bridge functions used by
 * the "CONF classic" functions, for consistency.
 */

CONF *NCONF_new_ex(OSSL_LIB_CTX *libctx, CONF_METHOD *meth)
{
    CONF *ret;

    if (meth == NULL)
        meth = NCONF_default();

    ret = meth->create(meth);
    if (ret == NULL) {
        ERR_raise(ERR_LIB_CONF, ERR_R_CONF_LIB);
        return NULL;
    }
    ret->libctx = libctx;

    return ret;
}

CONF *NCONF_new(CONF_METHOD *meth)
{
    return NCONF_new_ex(NULL, meth);
}

void NCONF_free(CONF *conf)
{
    if (conf == NULL)
        return;
    conf->meth->destroy(conf);
}

void NCONF_free_data(CONF *conf)
{
    if (conf == NULL)
        return;
    conf->meth->destroy_data(conf);
}

OSSL_LIB_CTX *NCONF_get0_libctx(const CONF *conf)
{
    return conf->libctx;
}

typedef STACK_OF(OPENSSL_CSTRING) SECTION_NAMES;

IMPLEMENT_LHASH_DOALL_ARG_CONST(CONF_VALUE, SECTION_NAMES);

static void collect_section_name(const CONF_VALUE *v, SECTION_NAMES *names)
{
    /* A section is a CONF_VALUE with name == NULL */
    if (v->name == NULL)
        /* A failure to push cannot be handled so we ignore the result. */
        (void)sk_OPENSSL_CSTRING_push(names, v->section);
}

static int section_name_cmp(OPENSSL_CSTRING const *a, OPENSSL_CSTRING const *b)
{
    return strcmp(*a, *b);
}

STACK_OF(OPENSSL_CSTRING) *NCONF_get_section_names(const CONF *cnf)
{
    SECTION_NAMES *names;

    if ((names = sk_OPENSSL_CSTRING_new(section_name_cmp)) == NULL)
        return NULL;
    lh_CONF_VALUE_doall_SECTION_NAMES(cnf->data, collect_section_name, names);
    sk_OPENSSL_CSTRING_sort(names);
    return names;
}

int NCONF_load(CONF *conf, const char *file, long *eline)
{
    if (conf == NULL) {
        ERR_raise(ERR_LIB_CONF, CONF_R_NO_CONF);
        return 0;
    }

    return conf->meth->load(conf, file, eline);
}

#ifndef OPENSSL_NO_STDIO
int NCONF_load_fp(CONF *conf, FILE *fp, long *eline)
{
    BIO *btmp;
    int ret;
    if ((btmp = BIO_new_fp(fp, BIO_NOCLOSE)) == NULL) {
        ERR_raise(ERR_LIB_CONF, ERR_R_BUF_LIB);
        return 0;
    }
    ret = NCONF_load_bio(conf, btmp, eline);
    BIO_free(btmp);
    return ret;
}
#endif

int NCONF_load_bio(CONF *conf, BIO *bp, long *eline)
{
    if (conf == NULL) {
        ERR_raise(ERR_LIB_CONF, CONF_R_NO_CONF);
        return 0;
    }

    return conf->meth->load_bio(conf, bp, eline);
}

STACK_OF(CONF_VALUE) *NCONF_get_section(const CONF *conf, const char *section)
{
    if (conf == NULL) {
        ERR_raise(ERR_LIB_CONF, CONF_R_NO_CONF);
        return NULL;
    }

    if (section == NULL) {
        ERR_raise(ERR_LIB_CONF, CONF_R_NO_SECTION);
        return NULL;
    }

    return _CONF_get_section_values(conf, section);
}

char *NCONF_get_string(const CONF *conf, const char *group, const char *name)
{
    char *s = _CONF_get_string(conf, group, name);

    /*
     * Since we may get a value from an environment variable even if conf is
     * NULL, let's check the value first
     */
    if (s)
        return s;

    if (conf == NULL) {
        ERR_raise(ERR_LIB_CONF, CONF_R_NO_CONF_OR_ENVIRONMENT_VARIABLE);
        return NULL;
    }
    ERR_raise_data(ERR_LIB_CONF, CONF_R_NO_VALUE,
                   "group=%s name=%s", group, name);
    return NULL;
}

static int default_is_number(const CONF *conf, char c)
{
    return ossl_isdigit(c);
}

static int default_to_int(const CONF *conf, char c)
{
    return (int)(c - '0');
}

int NCONF_get_number_e(const CONF *conf, const char *group, const char *name,
                       long *result)
{
    char *str;
    long res;
    int (*is_number)(const CONF *, char) = &default_is_number;
    int (*to_int)(const CONF *, char) = &default_to_int;

    if (result == NULL) {
        ERR_raise(ERR_LIB_CONF, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    str = NCONF_get_string(conf, group, name);

    if (str == NULL)
        return 0;

    if (conf != NULL) {
        if (conf->meth->is_number != NULL)
            is_number = conf->meth->is_number;
        if (conf->meth->to_int != NULL)
            to_int = conf->meth->to_int;
    }
    for (res = 0; is_number(conf, *str); str++) {
        const int d = to_int(conf, *str);

        if (res > (LONG_MAX - d) / 10L) {
            ERR_raise(ERR_LIB_CONF, CONF_R_NUMBER_TOO_LARGE);
            return 0;
        }
        res = res * 10 + d;
    }

    *result = res;
    return 1;
}

long _CONF_get_number(const CONF *conf, const char *section,
                      const char *name)
{
    int status;
    long result = 0;

    ERR_set_mark();
    status = NCONF_get_number_e(conf, section, name, &result);
    ERR_pop_to_mark();
    return status == 0 ? 0L : result;
}

#ifndef OPENSSL_NO_STDIO
int NCONF_dump_fp(const CONF *conf, FILE *out)
{
    BIO *btmp;
    int ret;
    if ((btmp = BIO_new_fp(out, BIO_NOCLOSE)) == NULL) {
        ERR_raise(ERR_LIB_CONF, ERR_R_BUF_LIB);
        return 0;
    }
    ret = NCONF_dump_bio(conf, btmp);
    BIO_free(btmp);
    return ret;
}
#endif

int NCONF_dump_bio(const CONF *conf, BIO *out)
{
    if (conf == NULL) {
        ERR_raise(ERR_LIB_CONF, CONF_R_NO_CONF);
        return 0;
    }

    return conf->meth->dump(conf, out);
}

/*
 * These routines call the C malloc/free, to avoid intermixing with
 * OpenSSL function pointers before the library is initialized.
 */
OPENSSL_INIT_SETTINGS *OPENSSL_INIT_new(void)
{
    OPENSSL_INIT_SETTINGS *ret = malloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;

    memset(ret, 0, sizeof(*ret));
    ret->flags = DEFAULT_CONF_MFLAGS;

    return ret;
}


#ifndef OPENSSL_NO_STDIO
/*
 * If CRYPTO_set_mem_functions is called after this, then
 * memory allocation and deallocation in this function can
 * become disjointed. Avoid this by always using standard
 * strdup & free instead of OPENSSL_strdup & OPENSSL_free.
 */
int OPENSSL_INIT_set_config_filename(OPENSSL_INIT_SETTINGS *settings,
                                     const char *filename)
{
    char *newfilename = NULL;

    if (filename != NULL) {
        newfilename = strdup(filename);
        if (newfilename == NULL)
            return 0;
    }

    free(settings->filename);
    settings->filename = newfilename;

    return 1;
}

void OPENSSL_INIT_set_config_file_flags(OPENSSL_INIT_SETTINGS *settings,
                                        unsigned long flags)
{
    settings->flags = flags;
}

/*
 * If CRYPTO_set_mem_functions is called after this, then
 * memory allocation and deallocation in this function can
 * become disjointed. Avoid this by always using standard
 * strdup & free instead of OPENSSL_strdup & OPENSSL_free.
 */
int OPENSSL_INIT_set_config_appname(OPENSSL_INIT_SETTINGS *settings,
                                    const char *appname)
{
    char *newappname = NULL;

    if (appname != NULL) {
        newappname = strdup(appname);
        if (newappname == NULL)
            return 0;
    }

    free(settings->appname);
    settings->appname = newappname;

    return 1;
}
#endif

void OPENSSL_INIT_free(OPENSSL_INIT_SETTINGS *settings)
{
    if (settings == NULL)
        return;

    free(settings->filename);
    free(settings->appname);
    free(settings);
}
