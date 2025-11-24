/*
 * Copyright 2021-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Low level APIs are deprecated for public use, but still ok for internal use.
 */
#include "internal/deprecated.h"

#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/ec.h>
#include "internal/passphrase.h"
#include "internal/nelem.h"
#include "prov/implementations.h"
#include "prov/bio.h"
#include "prov/provider_ctx.h"
#include "endecoder_local.h"

static int write_blob(void *provctx, OSSL_CORE_BIO *cout,
                      void *data, int len)
{
    BIO *out = ossl_bio_new_from_core_bio(provctx, cout);
    int ret;

    if (out == NULL)
        return 0;
    ret = BIO_write(out, data, len);

    BIO_free(out);
    return ret;
}

static OSSL_FUNC_encoder_newctx_fn key2blob_newctx;
static OSSL_FUNC_encoder_freectx_fn key2blob_freectx;

static void *key2blob_newctx(void *provctx)
{
    return provctx;
}

static void key2blob_freectx(void *vctx)
{
}

static int key2blob_check_selection(int selection, int selection_mask)
{
    /*
     * The selections are kinda sorta "levels", i.e. each selection given
     * here is assumed to include those following.
     */
    int checks[] = {
        OSSL_KEYMGMT_SELECT_PRIVATE_KEY,
        OSSL_KEYMGMT_SELECT_PUBLIC_KEY,
        OSSL_KEYMGMT_SELECT_ALL_PARAMETERS
    };
    size_t i;

    /* The decoder implementations made here support guessing */
    if (selection == 0)
        return 1;

    for (i = 0; i < OSSL_NELEM(checks); i++) {
        int check1 = (selection & checks[i]) != 0;
        int check2 = (selection_mask & checks[i]) != 0;

        /*
         * If the caller asked for the currently checked bit(s), return
         * whether the decoder description says it's supported.
         */
        if (check1)
            return check2;
    }

    /* This should be dead code, but just to be safe... */
    return 0;
}

static int key2blob_encode(void *vctx, const void *key, int selection,
                           OSSL_CORE_BIO *cout)
{
    int pubkey_len = 0, ok = 0;
    unsigned char *pubkey = NULL;

    pubkey_len = i2o_ECPublicKey(key, &pubkey);
    if (pubkey_len > 0 && pubkey != NULL)
        ok = write_blob(vctx, cout, pubkey, pubkey_len);
    OPENSSL_free(pubkey);
    return ok;
}

/*
 * MAKE_BLOB_ENCODER() Makes an OSSL_DISPATCH table for a particular key->blob
 * encoder
 *
 * impl:                The keytype to encode
 * type:                The C structure type holding the key data
 * selection_name:      The acceptable selections.  This translates into
 *                      the macro EVP_PKEY_##selection_name.
 *
 * The selection is understood as a "level" rather than an exact set of
 * requests from the caller.  The encoder has to decide what contents fit
 * the encoded format.  For example, the EC public key blob will only contain
 * the encoded public key itself, no matter if the selection bits include
 * OSSL_KEYMGMT_SELECT_PARAMETERS or not.  However, if the selection includes
 * OSSL_KEYMGMT_SELECT_PRIVATE_KEY, the same encoder will simply refuse to
 * cooperate, because it cannot output the private key.
 *
 * EVP_PKEY_##selection_name are convenience macros that combine "typical"
 * OSSL_KEYMGMT_SELECT_ macros for a certain type of EVP_PKEY content.
 */
#define MAKE_BLOB_ENCODER(impl, type, selection_name)                   \
    static OSSL_FUNC_encoder_import_object_fn                           \
    impl##2blob_import_object;                                          \
    static OSSL_FUNC_encoder_free_object_fn impl##2blob_free_object;    \
    static OSSL_FUNC_encoder_does_selection_fn                          \
    impl##2blob_does_selection;                                         \
    static OSSL_FUNC_encoder_encode_fn impl##2blob_encode;              \
                                                                        \
    static void *impl##2blob_import_object(void *ctx, int selection,    \
                                           const OSSL_PARAM params[])   \
    {                                                                   \
        return ossl_prov_import_key(ossl_##impl##_keymgmt_functions,    \
                                    ctx, selection, params);            \
    }                                                                   \
    static void impl##2blob_free_object(void *key)                      \
    {                                                                   \
        ossl_prov_free_key(ossl_##impl##_keymgmt_functions, key);       \
    }                                                                   \
    static int impl##2blob_does_selection(void *ctx, int selection)     \
    {                                                                   \
        return key2blob_check_selection(selection,                      \
                                        EVP_PKEY_##selection_name);     \
    }                                                                   \
    static int impl##2blob_encode(void *vctx, OSSL_CORE_BIO *cout,      \
                                  const void *key,                      \
                                  const OSSL_PARAM key_abstract[],      \
                                  int selection,                        \
                                  OSSL_PASSPHRASE_CALLBACK *cb,         \
                                  void *cbarg)                          \
    {                                                                   \
        /* We don't deal with abstract objects */                       \
        if (key_abstract != NULL) {                                     \
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);     \
            return 0;                                                   \
        }                                                               \
        return key2blob_encode(vctx, key, selection, cout);             \
    }                                                                   \
    const OSSL_DISPATCH ossl_##impl##_to_blob_encoder_functions[] = {   \
        { OSSL_FUNC_ENCODER_NEWCTX,                                     \
          (void (*)(void))key2blob_newctx },                            \
        { OSSL_FUNC_ENCODER_FREECTX,                                    \
          (void (*)(void))key2blob_freectx },                           \
        { OSSL_FUNC_ENCODER_DOES_SELECTION,                             \
          (void (*)(void))impl##2blob_does_selection },                 \
        { OSSL_FUNC_ENCODER_IMPORT_OBJECT,                              \
          (void (*)(void))impl##2blob_import_object },                  \
        { OSSL_FUNC_ENCODER_FREE_OBJECT,                                \
          (void (*)(void))impl##2blob_free_object },                    \
        { OSSL_FUNC_ENCODER_ENCODE,                                     \
          (void (*)(void))impl##2blob_encode },                         \
        OSSL_DISPATCH_END                                               \
    }

#ifndef OPENSSL_NO_EC
MAKE_BLOB_ENCODER(ec, ec, PUBLIC_KEY);
# ifndef OPENSSL_NO_SM2
MAKE_BLOB_ENCODER(sm2, ec, PUBLIC_KEY);
# endif
#endif
