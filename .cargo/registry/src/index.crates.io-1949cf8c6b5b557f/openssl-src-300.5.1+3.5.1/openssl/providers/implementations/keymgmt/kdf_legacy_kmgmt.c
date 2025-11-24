/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This implemments a dummy key manager for legacy KDFs that still support the
 * old way of performing a KDF via EVP_PKEY_derive(). New KDFs should not be
 * implemented this way. In reality there is no key data for such KDFs, so this
 * key manager does very little.
 */

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/kdfexchange.h"

static OSSL_FUNC_keymgmt_new_fn kdf_newdata;
static OSSL_FUNC_keymgmt_free_fn kdf_freedata;
static OSSL_FUNC_keymgmt_has_fn kdf_has;

KDF_DATA *ossl_kdf_data_new(void *provctx)
{
    KDF_DATA *kdfdata;

    if (!ossl_prov_is_running())
        return NULL;

    kdfdata = OPENSSL_zalloc(sizeof(*kdfdata));
    if (kdfdata == NULL)
        return NULL;

    if (!CRYPTO_NEW_REF(&kdfdata->refcnt, 1)) {
        OPENSSL_free(kdfdata);
        return NULL;
    }
    kdfdata->libctx = PROV_LIBCTX_OF(provctx);

    return kdfdata;
}

void ossl_kdf_data_free(KDF_DATA *kdfdata)
{
    int ref = 0;

    if (kdfdata == NULL)
        return;

    CRYPTO_DOWN_REF(&kdfdata->refcnt, &ref);
    if (ref > 0)
        return;

    CRYPTO_FREE_REF(&kdfdata->refcnt);
    OPENSSL_free(kdfdata);
}

int ossl_kdf_data_up_ref(KDF_DATA *kdfdata)
{
    int ref = 0;

    /* This is effectively doing a new operation on the KDF_DATA and should be
     * adequately guarded again modules' error states.  However, both current
     * calls here are guarded properly in exchange/kdf_exch.c.  Thus, it
     * could be removed here.  The concern is that something in the future
     * might call this function without adequate guards.  It's a cheap call,
     * it seems best to leave it even though it is currently redundant.
     */
    if (!ossl_prov_is_running())
        return 0;

    CRYPTO_UP_REF(&kdfdata->refcnt, &ref);
    return 1;
}

static void *kdf_newdata(void *provctx)
{
    return ossl_kdf_data_new(provctx);
}

static void kdf_freedata(void *kdfdata)
{
    ossl_kdf_data_free(kdfdata);
}

static int kdf_has(const void *keydata, int selection)
{
    return 1; /* nothing is missing */
}

const OSSL_DISPATCH ossl_kdf_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))kdf_newdata },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))kdf_freedata },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))kdf_has },
    OSSL_DISPATCH_END
};
