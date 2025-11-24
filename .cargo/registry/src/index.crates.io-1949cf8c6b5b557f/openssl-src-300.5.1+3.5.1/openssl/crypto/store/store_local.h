/*
 * Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include "internal/thread_once.h"
#include "internal/refcount.h"
#include <openssl/dsa.h>
#include <openssl/engine.h>
#include <openssl/evp.h>
#include <openssl/lhash.h>
#include <openssl/x509.h>
#include <openssl/store.h>
#include "internal/passphrase.h"

/*-
 *  OSSL_STORE_INFO stuff
 *  ---------------------
 */

struct ossl_store_info_st {
    int type;
    union {
        void *data;              /* used internally as generic pointer */

        struct {
            char *name;
            char *desc;
        } name;                  /* when type == OSSL_STORE_INFO_NAME */

        EVP_PKEY *params;        /* when type == OSSL_STORE_INFO_PARAMS */
        EVP_PKEY *pubkey;        /* when type == OSSL_STORE_INFO_PUBKEY */
        EVP_PKEY *pkey;          /* when type == OSSL_STORE_INFO_PKEY */
        X509 *x509;              /* when type == OSSL_STORE_INFO_CERT */
        X509_CRL *crl;           /* when type == OSSL_STORE_INFO_CRL */
    } _;
};
DEFINE_STACK_OF(OSSL_STORE_INFO)

/*-
 *  OSSL_STORE_SEARCH stuff
 *  -----------------------
 */

struct ossl_store_search_st {
    int search_type;

    /*
     * Used by OSSL_STORE_SEARCH_BY_NAME and
     * OSSL_STORE_SEARCH_BY_ISSUER_SERIAL
     */
    X509_NAME *name;

    /* Used by OSSL_STORE_SEARCH_BY_ISSUER_SERIAL */
    const ASN1_INTEGER *serial;

    /* Used by OSSL_STORE_SEARCH_BY_KEY_FINGERPRINT */
    const EVP_MD *digest;

    /*
     * Used by OSSL_STORE_SEARCH_BY_KEY_FINGERPRINT and
     * OSSL_STORE_SEARCH_BY_ALIAS
     */
    const unsigned char *string;
    size_t stringlength;
};

/*-
 *  OSSL_STORE_LOADER stuff
 *  -----------------------
 */

int ossl_store_register_loader_int(OSSL_STORE_LOADER *loader);
OSSL_STORE_LOADER *ossl_store_unregister_loader_int(const char *scheme);

/* loader stuff */
struct ossl_store_loader_st {
#ifndef OPENSSL_NO_DEPRECATED_3_0
    /* Legacy stuff */
    const char *scheme;
    ENGINE *engine;
    OSSL_STORE_open_fn open;
    OSSL_STORE_attach_fn attach;
    OSSL_STORE_ctrl_fn ctrl;
    OSSL_STORE_expect_fn expect;
    OSSL_STORE_find_fn find;
    OSSL_STORE_load_fn load;
    OSSL_STORE_eof_fn eof;
    OSSL_STORE_error_fn error;
    OSSL_STORE_close_fn closefn;
    OSSL_STORE_open_ex_fn open_ex;
#endif

    /* Provider stuff */
    OSSL_PROVIDER *prov;
    int scheme_id;
    const char *propdef;
    const char *description;

    CRYPTO_REF_COUNT refcnt;

    OSSL_FUNC_store_open_fn *p_open;
    OSSL_FUNC_store_attach_fn *p_attach;
    OSSL_FUNC_store_settable_ctx_params_fn *p_settable_ctx_params;
    OSSL_FUNC_store_set_ctx_params_fn *p_set_ctx_params;
    OSSL_FUNC_store_load_fn *p_load;
    OSSL_FUNC_store_eof_fn *p_eof;
    OSSL_FUNC_store_close_fn *p_close;
    OSSL_FUNC_store_export_object_fn *p_export_object;
    OSSL_FUNC_store_delete_fn *p_delete;
    OSSL_FUNC_store_open_ex_fn *p_open_ex;
};
DEFINE_LHASH_OF_EX(OSSL_STORE_LOADER);

const OSSL_STORE_LOADER *ossl_store_get0_loader_int(const char *scheme);
void ossl_store_destroy_loaders_int(void);

#ifdef OPENSSL_NO_DEPRECATED_3_0
/* struct ossl_store_loader_ctx_st is defined differently by each loader */
typedef struct ossl_store_loader_ctx_st OSSL_STORE_LOADER_CTX;
#endif

/*-
 *  OSSL_STORE_CTX stuff
 *  ---------------------
 */

struct ossl_store_ctx_st {
    const OSSL_STORE_LOADER *loader; /* legacy */
    OSSL_STORE_LOADER *fetched_loader;
    OSSL_STORE_LOADER_CTX *loader_ctx;
    OSSL_STORE_post_process_info_fn post_process;
    void *post_process_data;
    int expected_type;

    char *properties;

    /* 0 before the first STORE_load(), 1 otherwise */
    int loading;
    /* 1 on load error, only valid for fetched loaders */
    int error_flag;

    /*
     * Cache of stuff, to be able to return the contents of a PKCS#12
     * blob, one object at a time.
     */
    STACK_OF(OSSL_STORE_INFO) *cached_info;

    struct ossl_passphrase_data_st pwdata;
};

/*-
 *  'file' scheme stuff
 *  -------------------
 */

OSSL_STORE_LOADER_CTX *ossl_store_file_attach_pem_bio_int(BIO *bp);
int ossl_store_file_detach_pem_bio_int(OSSL_STORE_LOADER_CTX *ctx);

/*-
 * Provider stuff
 * -------------------
 */
OSSL_STORE_LOADER *ossl_store_loader_fetch(OSSL_LIB_CTX *libctx,
                                           const char *scheme,
                                           const char *properties);

/* Standard function to handle the result from OSSL_FUNC_store_load() */
struct ossl_load_result_data_st {
    OSSL_STORE_INFO *v;          /* To be filled in */
    OSSL_STORE_CTX *ctx;
};
OSSL_CALLBACK ossl_store_handle_load_result;
