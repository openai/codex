/*
 * Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Here is a set of wrappers for the ENGINE API, which are no-ops when the
 * ENGINE API is disabled / removed.
 * We need to suppress deprecation warnings to make this work.
 */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <string.h> /* strcmp */

#include <openssl/types.h> /* Ensure we have the ENGINE type, regardless */
#include <openssl/err.h>
#ifndef OPENSSL_NO_ENGINE
# include <openssl/engine.h>
#endif
#include "apps.h"

#ifndef OPENSSL_NO_ENGINE
/* Try to load an engine in a shareable library */
static ENGINE *try_load_engine(const char *engine)
{
    ENGINE *e = NULL;

    if ((e = ENGINE_by_id("dynamic")) != NULL) {
        if (!ENGINE_ctrl_cmd_string(e, "SO_PATH", engine, 0)
            || !ENGINE_ctrl_cmd_string(e, "LOAD", NULL, 0)) {
            ENGINE_free(e);
            e = NULL;
        }
    }
    return e;
}
#endif

ENGINE *setup_engine_methods(const char *id, unsigned int methods, int debug)
{
    ENGINE *e = NULL;

#ifndef OPENSSL_NO_ENGINE
    if (id != NULL) {
        if (strcmp(id, "auto") == 0) {
            BIO_printf(bio_err, "Enabling auto ENGINE support\n");
            ENGINE_register_all_complete();
            return NULL;
        }
        if ((e = ENGINE_by_id(id)) == NULL
            && (e = try_load_engine(id)) == NULL) {
            BIO_printf(bio_err, "Invalid engine \"%s\"\n", id);
            ERR_print_errors(bio_err);
            return NULL;
        }
        if (debug)
            (void)ENGINE_ctrl(e, ENGINE_CTRL_SET_LOGSTREAM, 0, bio_err, 0);
        if (!ENGINE_ctrl_cmd(e, "SET_USER_INTERFACE", 0,
                             (void *)get_ui_method(), 0, 1)
                || !ENGINE_set_default(e, methods)) {
            BIO_printf(bio_err, "Cannot use engine \"%s\"\n", ENGINE_get_id(e));
            ERR_print_errors(bio_err);
            ENGINE_free(e);
            return NULL;
        }

        BIO_printf(bio_err, "Engine \"%s\" set.\n", ENGINE_get_id(e));
    }
#endif
    return e;
}

void release_engine(ENGINE *e)
{
#ifndef OPENSSL_NO_ENGINE
    /* Free our "structural" reference. */
    ENGINE_free(e);
#endif
}

int init_engine(ENGINE *e)
{
    int rv = 1;

#ifndef OPENSSL_NO_ENGINE
    rv = ENGINE_init(e);
#endif
    return rv;
}

int finish_engine(ENGINE *e)
{
    int rv = 1;

#ifndef OPENSSL_NO_ENGINE
    rv = ENGINE_finish(e);
#endif
    return rv;
}

char *make_engine_uri(ENGINE *e, const char *key_id, const char *desc)
{
    char *new_uri = NULL;

#ifndef OPENSSL_NO_ENGINE
    if (e == NULL) {
        BIO_printf(bio_err, "No engine specified for loading %s\n", desc);
    } else if (key_id == NULL) {
        BIO_printf(bio_err, "No engine key id specified for loading %s\n", desc);
    } else {
        const char *engineid = ENGINE_get_id(e);
        size_t uri_sz =
            sizeof(ENGINE_SCHEME_COLON) - 1
            + strlen(engineid)
            + 1 /* : */
            + strlen(key_id)
            + 1 /* \0 */
            ;

        new_uri = OPENSSL_malloc(uri_sz);
        if (new_uri != NULL) {
            OPENSSL_strlcpy(new_uri, ENGINE_SCHEME_COLON, uri_sz);
            OPENSSL_strlcat(new_uri, engineid, uri_sz);
            OPENSSL_strlcat(new_uri, ":", uri_sz);
            OPENSSL_strlcat(new_uri, key_id, uri_sz);
        }
    }
#else
    BIO_printf(bio_err, "Engines not supported for loading %s\n", desc);
#endif
    return new_uri;
}

int get_legacy_pkey_id(OSSL_LIB_CTX *libctx, const char *algname, ENGINE *e)
{
    const EVP_PKEY_ASN1_METHOD *ameth;
    ENGINE *tmpeng = NULL;
    int pkey_id = NID_undef;

    ERR_set_mark();
    ameth = EVP_PKEY_asn1_find_str(&tmpeng, algname, -1);

#if !defined(OPENSSL_NO_ENGINE)
    ENGINE_finish(tmpeng);

    if (ameth == NULL && e != NULL)
        ameth = ENGINE_get_pkey_asn1_meth_str(e, algname, -1);
    else
#endif
    /* We're only interested if it comes from an ENGINE */
    if (tmpeng == NULL)
        ameth = NULL;

    ERR_pop_to_mark();
    if (ameth == NULL)
        return NID_undef;

    EVP_PKEY_asn1_get0_info(&pkey_id, NULL, NULL, NULL, NULL, ameth);

    return pkey_id;
}

const EVP_MD *get_digest_from_engine(const char *name)
{
#ifndef OPENSSL_NO_ENGINE
    ENGINE *eng;

    eng = ENGINE_get_digest_engine(OBJ_sn2nid(name));
    if (eng != NULL) {
        ENGINE_finish(eng);
        return EVP_get_digestbyname(name);
    }
#endif
    return NULL;
}

const EVP_CIPHER *get_cipher_from_engine(const char *name)
{
#ifndef OPENSSL_NO_ENGINE
    ENGINE *eng;

    eng = ENGINE_get_cipher_engine(OBJ_sn2nid(name));
    if (eng != NULL) {
        ENGINE_finish(eng);
        return EVP_get_cipherbyname(name);
    }
#endif
    return NULL;
}
