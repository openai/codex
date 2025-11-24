/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/crypto.h>
#include <openssl/x509.h>

int X509_STORE_set_default_paths_ex(X509_STORE *ctx, OSSL_LIB_CTX *libctx,
                                    const char *propq)
{
    X509_LOOKUP *lookup;

    lookup = X509_STORE_add_lookup(ctx, X509_LOOKUP_file());
    if (lookup == NULL)
        return 0;
    X509_LOOKUP_load_file_ex(lookup, NULL, X509_FILETYPE_DEFAULT, libctx, propq);

    lookup = X509_STORE_add_lookup(ctx, X509_LOOKUP_hash_dir());
    if (lookup == NULL)
        return 0;
    X509_LOOKUP_add_dir(lookup, NULL, X509_FILETYPE_DEFAULT);

    lookup = X509_STORE_add_lookup(ctx, X509_LOOKUP_store());
    if (lookup == NULL)
        return 0;
    /*
     * The NULL URI argument will activate any default URIs (presently none),
     * DO NOT pass the default CApath or CAfile, they're already handled above,
     * likely much more efficiently.
     */
    X509_LOOKUP_add_store_ex(lookup, NULL, libctx, propq);

    /* clear any errors */
    ERR_clear_error();

    return 1;
}
int X509_STORE_set_default_paths(X509_STORE *ctx)
{
    return X509_STORE_set_default_paths_ex(ctx, NULL, NULL);
}

int X509_STORE_load_file_ex(X509_STORE *ctx, const char *file,
                            OSSL_LIB_CTX *libctx, const char *propq)
{
    X509_LOOKUP *lookup;

    if (file == NULL
        || (lookup = X509_STORE_add_lookup(ctx, X509_LOOKUP_file())) == NULL
        || X509_LOOKUP_load_file_ex(lookup, file, X509_FILETYPE_PEM, libctx,
                                    propq) <= 0)
        return 0;

    return 1;
}

int X509_STORE_load_file(X509_STORE *ctx, const char *file)
{
    return X509_STORE_load_file_ex(ctx, file, NULL, NULL);
}

int X509_STORE_load_path(X509_STORE *ctx, const char *path)
{
    X509_LOOKUP *lookup;

    if (path == NULL
        || (lookup = X509_STORE_add_lookup(ctx, X509_LOOKUP_hash_dir())) == NULL
        || X509_LOOKUP_add_dir(lookup, path, X509_FILETYPE_PEM) <= 0)
        return 0;

    return 1;
}

int X509_STORE_load_store_ex(X509_STORE *ctx, const char *uri,
                             OSSL_LIB_CTX *libctx, const char *propq)
{
    X509_LOOKUP *lookup;

    if (uri == NULL
        || (lookup = X509_STORE_add_lookup(ctx, X509_LOOKUP_store())) == NULL
        || X509_LOOKUP_add_store_ex(lookup, uri, libctx, propq) == 0)
        return 0;

    return 1;
}

int X509_STORE_load_store(X509_STORE *ctx, const char *uri)
{
    return X509_STORE_load_store_ex(ctx, uri, NULL, NULL);
}

int X509_STORE_load_locations_ex(X509_STORE *ctx, const char *file,
                                 const char *path, OSSL_LIB_CTX *libctx,
                                 const char *propq)
{
    if (file == NULL && path == NULL)
        return 0;
    if (file != NULL && !X509_STORE_load_file_ex(ctx, file, libctx, propq))
        return 0;
    if (path != NULL && !X509_STORE_load_path(ctx, path))
        return 0;
    return 1;
}

int X509_STORE_load_locations(X509_STORE *ctx, const char *file,
                              const char *path)
{
    return X509_STORE_load_locations_ex(ctx, file, path, NULL, NULL);
}
