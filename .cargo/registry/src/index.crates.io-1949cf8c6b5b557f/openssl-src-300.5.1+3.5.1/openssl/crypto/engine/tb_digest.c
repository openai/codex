/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include "eng_local.h"

static ENGINE_TABLE *digest_table = NULL;

void ENGINE_unregister_digests(ENGINE *e)
{
    engine_table_unregister(&digest_table, e);
}

static void engine_unregister_all_digests(void)
{
    engine_table_cleanup(&digest_table);
}

int ENGINE_register_digests(ENGINE *e)
{
    if (e->digests) {
        const int *nids;
        int num_nids = e->digests(e, NULL, &nids, 0);
        if (num_nids > 0)
            return engine_table_register(&digest_table,
                                         engine_unregister_all_digests, e,
                                         nids, num_nids, 0);
    }
    return 1;
}

void ENGINE_register_all_digests(void)
{
    ENGINE *e;

    for (e = ENGINE_get_first(); e; e = ENGINE_get_next(e))
        ENGINE_register_digests(e);
}

int ENGINE_set_default_digests(ENGINE *e)
{
    if (e->digests) {
        const int *nids;
        int num_nids = e->digests(e, NULL, &nids, 0);
        if (num_nids > 0)
            return engine_table_register(&digest_table,
                                         engine_unregister_all_digests, e,
                                         nids, num_nids, 1);
    }
    return 1;
}

/*
 * Exposed API function to get a functional reference from the implementation
 * table (ie. try to get a functional reference from the tabled structural
 * references) for a given digest 'nid'
 */
ENGINE *ENGINE_get_digest_engine(int nid)
{
    return ossl_engine_table_select(&digest_table, nid,
                                    OPENSSL_FILE, OPENSSL_LINE);
}

/* Obtains a digest implementation from an ENGINE functional reference */
const EVP_MD *ENGINE_get_digest(ENGINE *e, int nid)
{
    const EVP_MD *ret;
    ENGINE_DIGESTS_PTR fn = ENGINE_get_digests(e);
    if (!fn || !fn(e, &ret, NULL, nid)) {
        ERR_raise(ERR_LIB_ENGINE, ENGINE_R_UNIMPLEMENTED_DIGEST);
        return NULL;
    }
    return ret;
}

/* Gets the digest callback from an ENGINE structure */
ENGINE_DIGESTS_PTR ENGINE_get_digests(const ENGINE *e)
{
    return e->digests;
}

/* Sets the digest callback in an ENGINE structure */
int ENGINE_set_digests(ENGINE *e, ENGINE_DIGESTS_PTR f)
{
    e->digests = f;
    return 1;
}
