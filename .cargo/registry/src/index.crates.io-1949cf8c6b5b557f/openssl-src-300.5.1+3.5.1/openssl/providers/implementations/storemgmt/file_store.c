/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* This file has quite some overlap with engines/e_loader_attic.c */

#include <string.h>
#include <sys/stat.h>
#include <ctype.h>  /* isdigit */
#include <assert.h>

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/core_object.h>
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/params.h>
#include <openssl/decoder.h>
#include <openssl/proverr.h>
#include <openssl/store.h>       /* The OSSL_STORE_INFO type numbers */
#include "internal/cryptlib.h"
#include "internal/o_dir.h"
#include "crypto/decoder.h"
#include "crypto/ctype.h"        /* ossl_isdigit() */
#include "prov/implementations.h"
#include "prov/bio.h"
#include "prov/providercommon.h"
#include "file_store_local.h"

DEFINE_STACK_OF(OSSL_STORE_INFO)

#ifdef _WIN32
# define stat _stat
#endif

#ifndef S_ISDIR
# define S_ISDIR(a) (((a) & S_IFMT) == S_IFDIR)
#endif

static OSSL_FUNC_store_open_fn file_open;
static OSSL_FUNC_store_attach_fn file_attach;
static OSSL_FUNC_store_settable_ctx_params_fn file_settable_ctx_params;
static OSSL_FUNC_store_set_ctx_params_fn file_set_ctx_params;
static OSSL_FUNC_store_load_fn file_load;
static OSSL_FUNC_store_eof_fn file_eof;
static OSSL_FUNC_store_close_fn file_close;

/*
 * This implementation makes full use of OSSL_DECODER, and then some.
 * It uses its own internal decoder implementation that reads DER and
 * passes that on to the data callback; this decoder is created with
 * internal OpenSSL functions, thereby bypassing the need for a surrounding
 * provider.  This is ok, since this is a local decoder, not meant for
 * public consumption.
 * Finally, it sets up its own construct and cleanup functions.
 *
 * Essentially, that makes this implementation a kind of glorified decoder.
 */

struct file_ctx_st {
    void *provctx;
    char *uri;                   /* The URI we currently try to load */
    enum {
        IS_FILE = 0,             /* Read file and pass results */
        IS_DIR                   /* Pass directory entry names */
    } type;

    union {
        /* Used with |IS_FILE| */
        struct {
            BIO *file;

            OSSL_DECODER_CTX *decoderctx;
            char *input_type;
            char *propq;    /* The properties we got as a parameter */
        } file;

        /* Used with |IS_DIR| */
        struct {
            OPENSSL_DIR_CTX *ctx;
            int end_reached;

            /*
             * When a search expression is given, these are filled in.
             * |search_name| contains the file basename to look for.
             * The string is exactly 8 characters long.
             */
            char search_name[9];

            /*
             * The directory reading utility we have combines opening with
             * reading the first name.  To make sure we can detect the end
             * at the right time, we read early and cache the name.
             */
            const char *last_entry;
            int last_errno;
        } dir;
    } _;

    /* Expected object type.  May be unspecified */
    int expected_type;
};

static void free_file_ctx(struct file_ctx_st *ctx)
{
    if (ctx == NULL)
        return;

    OPENSSL_free(ctx->uri);
    if (ctx->type != IS_DIR) {
        OSSL_DECODER_CTX_free(ctx->_.file.decoderctx);
        OPENSSL_free(ctx->_.file.propq);
        OPENSSL_free(ctx->_.file.input_type);
    }
    OPENSSL_free(ctx);
}

static struct file_ctx_st *new_file_ctx(int type, const char *uri,
                                        void *provctx)
{
    struct file_ctx_st *ctx = NULL;

    if ((ctx = OPENSSL_zalloc(sizeof(*ctx))) != NULL
        && (uri == NULL || (ctx->uri = OPENSSL_strdup(uri)) != NULL)) {
        ctx->type = type;
        ctx->provctx = provctx;
        return ctx;
    }
    free_file_ctx(ctx);
    return NULL;
}

static OSSL_DECODER_CONSTRUCT file_load_construct;
static OSSL_DECODER_CLEANUP file_load_cleanup;

/*-
 *  Opening / attaching streams and directories
 *  -------------------------------------------
 */

/*
 * Function to service both file_open() and file_attach()
 *
 *
 */
static struct file_ctx_st *file_open_stream(BIO *source, const char *uri,
                                            void *provctx)
{
    struct file_ctx_st *ctx;

    if ((ctx = new_file_ctx(IS_FILE, uri, provctx)) == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PROV_LIB);
        goto err;
    }

    ctx->_.file.file = source;

    return ctx;
 err:
    free_file_ctx(ctx);
    return NULL;
}

static void *file_open_dir(const char *path, const char *uri, void *provctx)
{
    struct file_ctx_st *ctx;

    if ((ctx = new_file_ctx(IS_DIR, uri, provctx)) == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PROV_LIB);
        return NULL;
    }

    ctx->_.dir.last_entry = OPENSSL_DIR_read(&ctx->_.dir.ctx, path);
    ctx->_.dir.last_errno = errno;
    if (ctx->_.dir.last_entry == NULL) {
        if (ctx->_.dir.last_errno != 0) {
            ERR_raise_data(ERR_LIB_SYS, ctx->_.dir.last_errno,
                           "Calling OPENSSL_DIR_read(\"%s\")", path);
            goto err;
        }
        ctx->_.dir.end_reached = 1;
    }
    return ctx;
 err:
    file_close(ctx);
    return NULL;
}

static void *file_open(void *provctx, const char *uri)
{
    struct file_ctx_st *ctx = NULL;
    struct stat st;
    struct {
        const char *path;
        unsigned int check_absolute:1;
    } path_data[2];
    size_t path_data_n = 0, i;
    const char *path, *p = uri, *q;
    BIO *bio;

    ERR_set_mark();

    /*
     * First step, just take the URI as is.
     */
    path_data[path_data_n].check_absolute = 0;
    path_data[path_data_n++].path = uri;

    /*
     * Second step, if the URI appears to start with the "file" scheme,
     * extract the path and make that the second path to check.
     * There's a special case if the URI also contains an authority, then
     * the full URI shouldn't be used as a path anywhere.
     */
    if (CHECK_AND_SKIP_CASE_PREFIX(p, "file:")) {
        q = p;
        if (CHECK_AND_SKIP_CASE_PREFIX(q, "//")) {
            path_data_n--;           /* Invalidate using the full URI */
            if (CHECK_AND_SKIP_CASE_PREFIX(q, "localhost/")
                    || CHECK_AND_SKIP_CASE_PREFIX(q, "/")) {
                p = q - 1;
            } else {
                ERR_clear_last_mark();
                ERR_raise(ERR_LIB_PROV, PROV_R_URI_AUTHORITY_UNSUPPORTED);
                return NULL;
            }
        }

        path_data[path_data_n].check_absolute = 1;
#ifdef _WIN32
        /* Windows "file:" URIs with a drive letter start with a '/' */
        if (p[0] == '/' && p[2] == ':' && p[3] == '/') {
            char c = tolower((unsigned char)p[1]);

            if (c >= 'a' && c <= 'z') {
                p++;
                /* We know it's absolute, so no need to check */
                path_data[path_data_n].check_absolute = 0;
            }
        }
#endif
        path_data[path_data_n++].path = p;
    }


    for (i = 0, path = NULL; path == NULL && i < path_data_n; i++) {
        /*
         * If the scheme "file" was an explicit part of the URI, the path must
         * be absolute.  So says RFC 8089
         */
        if (path_data[i].check_absolute && path_data[i].path[0] != '/') {
            ERR_clear_last_mark();
            ERR_raise_data(ERR_LIB_PROV, PROV_R_PATH_MUST_BE_ABSOLUTE,
                           "Given path=%s", path_data[i].path);
            return NULL;
        }

        if (stat(path_data[i].path, &st) < 0) {
            ERR_raise_data(ERR_LIB_SYS, errno,
                           "calling stat(%s)",
                           path_data[i].path);
        } else {
            path = path_data[i].path;
        }
    }
    if (path == NULL) {
        ERR_clear_last_mark();
        return NULL;
    }

    /* Successfully found a working path, clear possible collected errors */
    ERR_pop_to_mark();

    if (S_ISDIR(st.st_mode))
        ctx = file_open_dir(path, uri, provctx);
    else if ((bio = BIO_new_file(path, "rb")) == NULL
             || (ctx = file_open_stream(bio, uri, provctx)) == NULL)
        BIO_free_all(bio);

    return ctx;
}

void *file_attach(void *provctx, OSSL_CORE_BIO *cin)
{
    struct file_ctx_st *ctx;
    BIO *new_bio = ossl_bio_new_from_core_bio(provctx, cin);

    if (new_bio == NULL)
        return NULL;

    ctx = file_open_stream(new_bio, NULL, provctx);
    if (ctx == NULL)
        BIO_free(new_bio);
    return ctx;
}

/*-
 *  Setting parameters
 *  ------------------
 */

static const OSSL_PARAM *file_settable_ctx_params(void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_STORE_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_int(OSSL_STORE_PARAM_EXPECT, NULL),
        OSSL_PARAM_octet_string(OSSL_STORE_PARAM_SUBJECT, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_STORE_PARAM_INPUT_TYPE, NULL, 0),
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int file_set_ctx_params(void *loaderctx, const OSSL_PARAM params[])
{
    struct file_ctx_st *ctx = loaderctx;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    if (ctx->type != IS_DIR) {
        /* these parameters are ignored for directories */
        p = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_PROPERTIES);
        if (p != NULL) {
            OPENSSL_free(ctx->_.file.propq);
            ctx->_.file.propq = NULL;
            if (!OSSL_PARAM_get_utf8_string(p, &ctx->_.file.propq, 0))
                return 0;
        }
        p = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_INPUT_TYPE);
        if (p != NULL) {
            OPENSSL_free(ctx->_.file.input_type);
            ctx->_.file.input_type = NULL;
            if (!OSSL_PARAM_get_utf8_string(p, &ctx->_.file.input_type, 0))
                return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_EXPECT);
    if (p != NULL && !OSSL_PARAM_get_int(p, &ctx->expected_type))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_SUBJECT);
    if (p != NULL) {
        const unsigned char *der = NULL;
        size_t der_len = 0;
        X509_NAME *x509_name;
        unsigned long hash;
        int ok;

        if (ctx->type != IS_DIR) {
            ERR_raise(ERR_LIB_PROV,
                      PROV_R_SEARCH_ONLY_SUPPORTED_FOR_DIRECTORIES);
            return 0;
        }

        if (!OSSL_PARAM_get_octet_string_ptr(p, (const void **)&der, &der_len)
            || (x509_name = d2i_X509_NAME(NULL, &der, der_len)) == NULL)
            return 0;
        hash = X509_NAME_hash_ex(x509_name,
                                 ossl_prov_ctx_get0_libctx(ctx->provctx), NULL,
                                 &ok);
        BIO_snprintf(ctx->_.dir.search_name, sizeof(ctx->_.dir.search_name),
                     "%08lx", hash);
        X509_NAME_free(x509_name);
        if (ok == 0)
            return 0;
    }
    return 1;
}

/*-
 *  Loading an object from a stream
 *  -------------------------------
 */

struct file_load_data_st {
    OSSL_CALLBACK *object_cb;
    void *object_cbarg;
};

static int file_load_construct(OSSL_DECODER_INSTANCE *decoder_inst,
                               const OSSL_PARAM *params, void *construct_data)
{
    struct file_load_data_st *data = construct_data;

    /*
     * At some point, we may find it justifiable to recognise PKCS#12 and
     * handle it specially here, making |file_load()| return pass its
     * contents one piece at ta time, like |e_loader_attic.c| does.
     *
     * However, that currently means parsing them out, which converts the
     * DER encoded PKCS#12 into a bunch of EVP_PKEYs and X509s, just to
     * have to re-encode them into DER to create an object abstraction for
     * each of them.
     * It's much simpler (less churn) to pass on the object abstraction we
     * get to the load_result callback and leave it to that one to do the
     * work.  If that's libcrypto code, we know that it has much better
     * possibilities to handle the EVP_PKEYs and X509s without the extra
     * churn.
     */

    return data->object_cb(params, data->object_cbarg);
}

void file_load_cleanup(void *construct_data)
{
    /* Nothing to do */
}

static int file_setup_decoders(struct file_ctx_st *ctx)
{
    OSSL_LIB_CTX *libctx = ossl_prov_ctx_get0_libctx(ctx->provctx);
    const OSSL_ALGORITHM *to_algo = NULL;
    const char *input_structure = NULL;
    int ok = 0;

    /* Setup for this session, so only if not already done */
    if (ctx->_.file.decoderctx == NULL) {
        if ((ctx->_.file.decoderctx = OSSL_DECODER_CTX_new()) == NULL) {
            ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
            goto err;
        }

        /* Make sure the input type is set */
        if (!OSSL_DECODER_CTX_set_input_type(ctx->_.file.decoderctx,
                                             ctx->_.file.input_type)) {
            ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
            goto err;
        }

        /*
         * Where applicable, set the outermost structure name.
         * The goal is to avoid the STORE object types that are
         * potentially password protected but aren't interesting
         * for this load.
         */
        switch (ctx->expected_type) {
        case OSSL_STORE_INFO_PUBKEY:
            input_structure = "SubjectPublicKeyInfo";
            if (!OSSL_DECODER_CTX_set_input_structure(ctx->_.file.decoderctx,
                                                      input_structure)) {
                ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
                goto err;
            }
            break;
        case OSSL_STORE_INFO_PKEY:
            /*
             * The user's OSSL_STORE_INFO_PKEY covers PKCS#8, whether encrypted
             * or not.  The decoder will figure out whether decryption is
             * applicable and fall back as necessary.  We just need to indicate
             * that it is OK to try and encrypt, which may involve a password
             * prompt, so not done unless the data type is explicit, as we
             * might then get a password prompt for a key when reading only
             * certs from a file.
             */
            input_structure = "EncryptedPrivateKeyInfo";
            if (!OSSL_DECODER_CTX_set_input_structure(ctx->_.file.decoderctx,
                                                      input_structure)) {
                ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
                goto err;
            }
            break;
        case OSSL_STORE_INFO_CERT:
            input_structure = "Certificate";
            if (!OSSL_DECODER_CTX_set_input_structure(ctx->_.file.decoderctx,
                                                      input_structure)) {
                ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
                goto err;
            }
            break;
        case OSSL_STORE_INFO_CRL:
            input_structure = "CertificateList";
            if (!OSSL_DECODER_CTX_set_input_structure(ctx->_.file.decoderctx,
                                                      input_structure)) {
                ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
                goto err;
            }
            break;
        default:
            break;
        }

        for (to_algo = ossl_any_to_obj_algorithm;
             to_algo->algorithm_names != NULL;
             to_algo++) {
            OSSL_DECODER *to_obj = NULL;
            OSSL_DECODER_INSTANCE *to_obj_inst = NULL;
            const char *input_type;

            /*
             * Create the internal last resort decoder implementation
             * together with a "decoder instance".
             * The decoder doesn't need any identification or to be
             * attached to any provider, since it's only used locally.
             */
            to_obj = ossl_decoder_from_algorithm(0, to_algo, NULL);
            if (to_obj != NULL)
                to_obj_inst =
                    ossl_decoder_instance_new_forprov(to_obj, ctx->provctx,
                                                      input_structure);
            OSSL_DECODER_free(to_obj);
            if (to_obj_inst == NULL)
                goto err;
            /*
             * The input type has to match unless, the input type is PEM
             * and the decoder input type is DER, in which case we'll pick
             * up additional decoders.
             */
            input_type = OSSL_DECODER_INSTANCE_get_input_type(to_obj_inst);
            if (ctx->_.file.input_type != NULL
                && OPENSSL_strcasecmp(input_type, ctx->_.file.input_type) != 0
                && (OPENSSL_strcasecmp(ctx->_.file.input_type, "PEM") != 0
                    || OPENSSL_strcasecmp(input_type, "der") != 0)) {
                ossl_decoder_instance_free(to_obj_inst);
                continue;
            }

            if (!ossl_decoder_ctx_add_decoder_inst(ctx->_.file.decoderctx,
                                                   to_obj_inst)) {
                ossl_decoder_instance_free(to_obj_inst);
                ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
                goto err;
            }
        }
        /* Add on the usual extra decoders */
        if (!OSSL_DECODER_CTX_add_extra(ctx->_.file.decoderctx,
                                        libctx, ctx->_.file.propq)) {
            ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
            goto err;
        }

        /*
         * Then install our constructor hooks, which just passes decoded
         * data to the load callback
         */
        if (!OSSL_DECODER_CTX_set_construct(ctx->_.file.decoderctx,
                                            file_load_construct)
            || !OSSL_DECODER_CTX_set_cleanup(ctx->_.file.decoderctx,
                                             file_load_cleanup)) {
            ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
            goto err;
        }
    }

    ok = 1;
 err:
    return ok;
}

static int file_load_file(struct file_ctx_st *ctx,
                          OSSL_CALLBACK *object_cb, void *object_cbarg,
                          OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct file_load_data_st data;
    int ret, err;

    /* Setup the decoders (one time shot per session */

    if (!file_setup_decoders(ctx))
        return 0;

    /* Setup for this object */

    data.object_cb = object_cb;
    data.object_cbarg = object_cbarg;
    OSSL_DECODER_CTX_set_construct_data(ctx->_.file.decoderctx, &data);
    OSSL_DECODER_CTX_set_passphrase_cb(ctx->_.file.decoderctx, pw_cb, pw_cbarg);

    /* Launch */

    ERR_set_mark();
    ret = OSSL_DECODER_from_bio(ctx->_.file.decoderctx, ctx->_.file.file);
    if (BIO_eof(ctx->_.file.file)
        && ((err = ERR_peek_last_error()) != 0)
        && ERR_GET_LIB(err) == ERR_LIB_OSSL_DECODER
        && ERR_GET_REASON(err) == ERR_R_UNSUPPORTED)
        ERR_pop_to_mark();
    else
        ERR_clear_last_mark();
    return ret;
}

/*-
 *  Loading a name object from a directory
 *  --------------------------------------
 */

static char *file_name_to_uri(struct file_ctx_st *ctx, const char *name)
{
    char *data = NULL;

    assert(name != NULL);
    {
        const char *pathsep = ossl_ends_with_dirsep(ctx->uri) ? "" : "/";
        long calculated_length = strlen(ctx->uri) + strlen(pathsep)
            + strlen(name) + 1 /* \0 */;

        data = OPENSSL_zalloc(calculated_length);
        if (data == NULL)
            return NULL;

        OPENSSL_strlcat(data, ctx->uri, calculated_length);
        OPENSSL_strlcat(data, pathsep, calculated_length);
        OPENSSL_strlcat(data, name, calculated_length);
    }
    return data;
}

static int file_name_check(struct file_ctx_st *ctx, const char *name)
{
    const char *p = NULL;
    size_t len = strlen(ctx->_.dir.search_name);

    /* If there are no search criteria, all names are accepted */
    if (ctx->_.dir.search_name[0] == '\0')
        return 1;

    /* If the expected type isn't supported, no name is accepted */
    if (ctx->expected_type != 0
        && ctx->expected_type != OSSL_STORE_INFO_CERT
        && ctx->expected_type != OSSL_STORE_INFO_CRL)
        return 0;

    /*
     * First, check the basename
     */
    if (OPENSSL_strncasecmp(name, ctx->_.dir.search_name, len) != 0
        || name[len] != '.')
        return 0;
    p = &name[len + 1];

    /*
     * Then, if the expected type is a CRL, check that the extension starts
     * with 'r'
     */
    if (*p == 'r') {
        p++;
        if (ctx->expected_type != 0
            && ctx->expected_type != OSSL_STORE_INFO_CRL)
            return 0;
    } else if (ctx->expected_type == OSSL_STORE_INFO_CRL) {
        return 0;
    }

    /*
     * Last, check that the rest of the extension is a decimal number, at
     * least one digit long.
     */
    if (!isdigit((unsigned char)*p))
        return 0;
    while (isdigit((unsigned char)*p))
        p++;

#ifdef __VMS
    /*
     * One extra step here, check for a possible generation number.
     */
    if (*p == ';')
        for (p++; *p != '\0'; p++)
            if (!ossl_isdigit((unsigned char)*p))
                break;
#endif

    /*
     * If we've reached the end of the string at this point, we've successfully
     * found a fitting file name.
     */
    return *p == '\0';
}

static int file_load_dir_entry(struct file_ctx_st *ctx,
                               OSSL_CALLBACK *object_cb, void *object_cbarg,
                               OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    /* Prepare as much as possible in advance */
    static const int object_type = OSSL_OBJECT_NAME;
    OSSL_PARAM object[] = {
        OSSL_PARAM_int(OSSL_OBJECT_PARAM_TYPE, (int *)&object_type),
        OSSL_PARAM_utf8_string(OSSL_OBJECT_PARAM_DATA, NULL, 0),
        OSSL_PARAM_END
    };
    char *newname = NULL;
    int ok;

    /* Loop until we get an error or until we have a suitable name */
    do {
        if (ctx->_.dir.last_entry == NULL) {
            if (!ctx->_.dir.end_reached) {
                assert(ctx->_.dir.last_errno != 0);
                ERR_raise(ERR_LIB_SYS, ctx->_.dir.last_errno);
            }
            /* file_eof() will tell if EOF was reached */
            return 0;
        }

        /* flag acceptable names */
        if (ctx->_.dir.last_entry[0] != '.'
            && file_name_check(ctx, ctx->_.dir.last_entry)) {

            /* If we can't allocate the new name, we fail */
            if ((newname =
                 file_name_to_uri(ctx, ctx->_.dir.last_entry)) == NULL)
                return 0;
        }

        /*
         * On the first call (with a NULL context), OPENSSL_DIR_read()
         * cares about the second argument.  On the following calls, it
         * only cares that it isn't NULL.  Therefore, we can safely give
         * it our URI here.
         */
        ctx->_.dir.last_entry = OPENSSL_DIR_read(&ctx->_.dir.ctx, ctx->uri);
        ctx->_.dir.last_errno = errno;
        if (ctx->_.dir.last_entry == NULL && ctx->_.dir.last_errno == 0)
            ctx->_.dir.end_reached = 1;
    } while (newname == NULL);

    object[1].data = newname;
    object[1].data_size = strlen(newname);
    ok = object_cb(object, object_cbarg);
    OPENSSL_free(newname);
    return ok;
}

/*-
 *  Loading, local dispatcher
 *  -------------------------
 */

static int file_load(void *loaderctx,
                     OSSL_CALLBACK *object_cb, void *object_cbarg,
                     OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct file_ctx_st *ctx = loaderctx;

    switch (ctx->type) {
    case IS_FILE:
        return file_load_file(ctx, object_cb, object_cbarg, pw_cb, pw_cbarg);
    case IS_DIR:
        return
            file_load_dir_entry(ctx, object_cb, object_cbarg, pw_cb, pw_cbarg);
    default:
        break;
    }

    /* ctx->type has an unexpected value */
    assert(0);
    return 0;
}

/*-
 *  Eof detection and closing
 *  -------------------------
 */

static int file_eof(void *loaderctx)
{
    struct file_ctx_st *ctx = loaderctx;

    switch (ctx->type) {
    case IS_DIR:
        return ctx->_.dir.end_reached;
    case IS_FILE:
        /*
         * BIO_pending() checks any filter BIO.
         * BIO_eof() checks the source BIO.
         */
        return !BIO_pending(ctx->_.file.file)
            && BIO_eof(ctx->_.file.file);
    }

    /* ctx->type has an unexpected value */
    assert(0);
    return 1;
}

static int file_close_dir(struct file_ctx_st *ctx)
{
    if (ctx->_.dir.ctx != NULL)
        OPENSSL_DIR_end(&ctx->_.dir.ctx);
    free_file_ctx(ctx);
    return 1;
}

static int file_close_stream(struct file_ctx_st *ctx)
{
    /*
     * This frees either the provider BIO filter (for file_attach()) OR
     * the allocated file BIO (for file_open()).
     */
    BIO_free(ctx->_.file.file);
    ctx->_.file.file = NULL;

    free_file_ctx(ctx);
    return 1;
}

static int file_close(void *loaderctx)
{
    struct file_ctx_st *ctx = loaderctx;

    switch (ctx->type) {
    case IS_DIR:
        return file_close_dir(ctx);
    case IS_FILE:
        return file_close_stream(ctx);
    }

    /* ctx->type has an unexpected value */
    assert(0);
    return 1;
}

const OSSL_DISPATCH ossl_file_store_functions[] = {
    { OSSL_FUNC_STORE_OPEN, (void (*)(void))file_open },
    { OSSL_FUNC_STORE_ATTACH, (void (*)(void))file_attach },
    { OSSL_FUNC_STORE_SETTABLE_CTX_PARAMS,
      (void (*)(void))file_settable_ctx_params },
    { OSSL_FUNC_STORE_SET_CTX_PARAMS, (void (*)(void))file_set_ctx_params },
    { OSSL_FUNC_STORE_LOAD, (void (*)(void))file_load },
    { OSSL_FUNC_STORE_EOF, (void (*)(void))file_eof },
    { OSSL_FUNC_STORE_CLOSE, (void (*)(void))file_close },
    OSSL_DISPATCH_END,
};
