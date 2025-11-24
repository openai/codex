/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_names.h>
#include "internal/cryptlib.h"
#include "internal/nelem.h"
#include "crypto/evp.h"
#include "internal/core.h"
#include "internal/provider.h"
#include "evp_local.h"

/*
 * match_type() checks if two EVP_KEYMGMT are matching key types.  This
 * function assumes that the caller has made all the necessary NULL checks.
 */
static int match_type(const EVP_KEYMGMT *keymgmt1, const EVP_KEYMGMT *keymgmt2)
{
    const char *name2 = EVP_KEYMGMT_get0_name(keymgmt2);

    return EVP_KEYMGMT_is_a(keymgmt1, name2);
}

int evp_keymgmt_util_try_import(const OSSL_PARAM params[], void *arg)
{
    struct evp_keymgmt_util_try_import_data_st *data = arg;
    int delete_on_error = 0;

    /* Just in time creation of keydata */
    if (data->keydata == NULL) {
        if ((data->keydata = evp_keymgmt_newdata(data->keymgmt)) == NULL) {
            ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
            return 0;
        }
        delete_on_error = 1;
    }

    /*
     * It's fine if there was no data to transfer, we just end up with an
     * empty destination key.
     */
    if (params[0].key == NULL)
        return 1;

    if (evp_keymgmt_import(data->keymgmt, data->keydata, data->selection,
                           params))
        return 1;
    if (delete_on_error) {
        evp_keymgmt_freedata(data->keymgmt, data->keydata);
        data->keydata = NULL;
    }
    return 0;
}

int evp_keymgmt_util_assign_pkey(EVP_PKEY *pkey, EVP_KEYMGMT *keymgmt,
                                 void *keydata)
{
    if (pkey == NULL || keymgmt == NULL || keydata == NULL
        || !EVP_PKEY_set_type_by_keymgmt(pkey, keymgmt)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    pkey->keydata = keydata;
    evp_keymgmt_util_cache_keyinfo(pkey);
    return 1;
}

EVP_PKEY *evp_keymgmt_util_make_pkey(EVP_KEYMGMT *keymgmt, void *keydata)
{
    EVP_PKEY *pkey = NULL;

    if (keymgmt == NULL
        || keydata == NULL
        || (pkey = EVP_PKEY_new()) == NULL
        || !evp_keymgmt_util_assign_pkey(pkey, keymgmt, keydata)) {
        EVP_PKEY_free(pkey);
        return NULL;
    }
    return pkey;
}

int evp_keymgmt_util_export(const EVP_PKEY *pk, int selection,
                            OSSL_CALLBACK *export_cb, void *export_cbarg)
{
    if (pk == NULL || export_cb == NULL)
        return 0;
    return evp_keymgmt_export(pk->keymgmt, pk->keydata, selection,
                              export_cb, export_cbarg);
}

void *evp_keymgmt_util_export_to_provider(EVP_PKEY *pk, EVP_KEYMGMT *keymgmt,
                                          int selection)
{
    struct evp_keymgmt_util_try_import_data_st import_data;
    OP_CACHE_ELEM *op;

    /* Export to where? */
    if (keymgmt == NULL)
        return NULL;

    /* If we have an unassigned key, give up */
    if (pk->keydata == NULL)
        return NULL;

    /*
     * If |keymgmt| matches the "origin" |keymgmt|, there is no more to do.
     * The "origin" is determined by the |keymgmt| pointers being identical
     * or when the provider and the name ID match.  The latter case handles the
     * situation where the fetch cache is flushed and a "new" key manager is
     * created.
     */
    if (pk->keymgmt == keymgmt
        || (pk->keymgmt->name_id == keymgmt->name_id
            && pk->keymgmt->prov == keymgmt->prov))
        return pk->keydata;

    if (!CRYPTO_THREAD_read_lock(pk->lock))
        return NULL;
    /*
     * If the provider native "origin" hasn't changed since last time, we
     * try to find our keymgmt in the operation cache.  If it has changed
     * and our keymgmt isn't found, we will clear the cache further down.
     */
    if (pk->dirty_cnt == pk->dirty_cnt_copy) {
        /* If this key is already exported to |keymgmt|, no more to do */
        op = evp_keymgmt_util_find_operation_cache(pk, keymgmt, selection);
        if (op != NULL && op->keymgmt != NULL) {
            void *ret = op->keydata;

            CRYPTO_THREAD_unlock(pk->lock);
            return ret;
        }
    }
    CRYPTO_THREAD_unlock(pk->lock);

    /* If the "origin" |keymgmt| doesn't support exporting, give up */
    if (pk->keymgmt->export == NULL)
        return NULL;

    /*
     * Make sure that the type of the keymgmt to export to matches the type
     * of the "origin"
     */
    if (!ossl_assert(match_type(pk->keymgmt, keymgmt)))
        return NULL;

    /*
     * We look at the already cached provider keys, and import from the
     * first that supports it (i.e. use its export function), and export
     * the imported data to the new provider.
     */

    /* Setup for the export callback */
    import_data.keydata = NULL;  /* evp_keymgmt_util_try_import will create it */
    import_data.keymgmt = keymgmt;
    import_data.selection = selection;

    /*
     * The export function calls the callback (evp_keymgmt_util_try_import),
     * which does the import for us.  If successful, we're done.
     */
    if (!evp_keymgmt_util_export(pk, selection,
                                 &evp_keymgmt_util_try_import, &import_data))
        /* If there was an error, bail out */
        return NULL;

    if (!CRYPTO_THREAD_write_lock(pk->lock)) {
        evp_keymgmt_freedata(keymgmt, import_data.keydata);
        return NULL;
    }
    /* Check to make sure some other thread didn't get there first */
    op = evp_keymgmt_util_find_operation_cache(pk, keymgmt, selection);
    if (op != NULL && op->keydata != NULL) {
        void *ret = op->keydata;

        CRYPTO_THREAD_unlock(pk->lock);

        /*
         * Another thread seemms to have already exported this so we abandon
         * all the work we just did.
         */
        evp_keymgmt_freedata(keymgmt, import_data.keydata);

        return ret;
    }

    /*
     * If the dirty counter changed since last time, then clear the
     * operation cache.  In that case, we know that |i| is zero.
     */
    if (pk->dirty_cnt != pk->dirty_cnt_copy)
        evp_keymgmt_util_clear_operation_cache(pk);

    /* Add the new export to the operation cache */
    if (!evp_keymgmt_util_cache_keydata(pk, keymgmt, import_data.keydata,
                                        selection)) {
        CRYPTO_THREAD_unlock(pk->lock);
        evp_keymgmt_freedata(keymgmt, import_data.keydata);
        return NULL;
    }

    /* Synchronize the dirty count */
    pk->dirty_cnt_copy = pk->dirty_cnt;

    CRYPTO_THREAD_unlock(pk->lock);

    return import_data.keydata;
}

static void op_cache_free(OP_CACHE_ELEM *e)
{
    evp_keymgmt_freedata(e->keymgmt, e->keydata);
    EVP_KEYMGMT_free(e->keymgmt);
    OPENSSL_free(e);
}

int evp_keymgmt_util_clear_operation_cache(EVP_PKEY *pk)
{
    if (pk != NULL) {
        sk_OP_CACHE_ELEM_pop_free(pk->operation_cache, op_cache_free);
        pk->operation_cache = NULL;
    }

    return 1;
}

OP_CACHE_ELEM *evp_keymgmt_util_find_operation_cache(EVP_PKEY *pk,
                                                     EVP_KEYMGMT *keymgmt,
                                                     int selection)
{
    int i, end = sk_OP_CACHE_ELEM_num(pk->operation_cache);
    OP_CACHE_ELEM *p;

    /*
     * A comparison and sk_P_CACHE_ELEM_find() are avoided to not cause
     * problems when we've only a read lock.
     * A keymgmt is a match if the |keymgmt| pointers are identical or if the
     * provider and the name ID match
     */
    for (i = 0; i < end; i++) {
        p = sk_OP_CACHE_ELEM_value(pk->operation_cache, i);
        if ((p->selection & selection) == selection
                && (keymgmt == p->keymgmt
                    || (keymgmt->name_id == p->keymgmt->name_id
                        && keymgmt->prov == p->keymgmt->prov)))
            return p;
    }
    return NULL;
}

int evp_keymgmt_util_cache_keydata(EVP_PKEY *pk, EVP_KEYMGMT *keymgmt,
                                   void *keydata, int selection)
{
    OP_CACHE_ELEM *p = NULL;

    if (keydata != NULL) {
        if (pk->operation_cache == NULL) {
            pk->operation_cache = sk_OP_CACHE_ELEM_new_null();
            if (pk->operation_cache == NULL)
                return 0;
        }

        p = OPENSSL_malloc(sizeof(*p));
        if (p == NULL)
            return 0;
        p->keydata = keydata;
        p->keymgmt = keymgmt;
        p->selection = selection;

        if (!EVP_KEYMGMT_up_ref(keymgmt)) {
            OPENSSL_free(p);
            return 0;
        }

        if (!sk_OP_CACHE_ELEM_push(pk->operation_cache, p)) {
            EVP_KEYMGMT_free(keymgmt);
            OPENSSL_free(p);
            return 0;
        }
    }
    return 1;
}

void evp_keymgmt_util_cache_keyinfo(EVP_PKEY *pk)
{
    /*
     * Cache information about the provider "origin" key.
     *
     * This services functions like EVP_PKEY_get_size, EVP_PKEY_get_bits, etc
     */
    if (pk->keydata != NULL) {
        int bits = 0;
        int security_bits = 0;
        int size = 0;
        OSSL_PARAM params[4];

        params[0] = OSSL_PARAM_construct_int(OSSL_PKEY_PARAM_BITS, &bits);
        params[1] = OSSL_PARAM_construct_int(OSSL_PKEY_PARAM_SECURITY_BITS,
                                             &security_bits);
        params[2] = OSSL_PARAM_construct_int(OSSL_PKEY_PARAM_MAX_SIZE, &size);
        params[3] = OSSL_PARAM_construct_end();
        if (evp_keymgmt_get_params(pk->keymgmt, pk->keydata, params)) {
            pk->cache.size = size;
            pk->cache.bits = bits;
            pk->cache.security_bits = security_bits;
        }
    }
}

void *evp_keymgmt_util_fromdata(EVP_PKEY *target, EVP_KEYMGMT *keymgmt,
                                int selection, const OSSL_PARAM params[])
{
    void *keydata = NULL;

    if ((keydata = evp_keymgmt_newdata(keymgmt)) == NULL
        || !evp_keymgmt_import(keymgmt, keydata, selection, params)
        || !evp_keymgmt_util_assign_pkey(target, keymgmt, keydata)) {
        evp_keymgmt_freedata(keymgmt, keydata);
        keydata = NULL;
    }
    return keydata;
}

int evp_keymgmt_util_has(EVP_PKEY *pk, int selection)
{
    /* Check if key is even assigned */
    if (pk->keymgmt == NULL)
        return 0;

    return evp_keymgmt_has(pk->keymgmt, pk->keydata, selection);
}

/*
 * evp_keymgmt_util_match() doesn't just look at the provider side "origin",
 * but also in the operation cache to see if there's any common keymgmt that
 * supplies OP_keymgmt_match.
 *
 * evp_keymgmt_util_match() adheres to the return values that EVP_PKEY_eq()
 * and EVP_PKEY_parameters_eq() return, i.e.:
 *
 *  1   same key
 *  0   not same key
 * -1   not same key type
 * -2   unsupported operation
 */
int evp_keymgmt_util_match(EVP_PKEY *pk1, EVP_PKEY *pk2, int selection)
{
    EVP_KEYMGMT *keymgmt1 = NULL, *keymgmt2 = NULL;
    void *keydata1 = NULL, *keydata2 = NULL;

    if (pk1 == NULL || pk2 == NULL) {
        if (pk1 == NULL && pk2 == NULL)
            return 1;
        return 0;
    }

    keymgmt1 = pk1->keymgmt;
    keydata1 = pk1->keydata;
    keymgmt2 = pk2->keymgmt;
    keydata2 = pk2->keydata;

    if (keymgmt1 != keymgmt2) {
        /*
         * The condition for a successful cross export is that the
         * keydata to be exported is NULL (typed, but otherwise empty
         * EVP_PKEY), or that it was possible to export it with
         * evp_keymgmt_util_export_to_provider().
         *
         * We use |ok| to determine if it's ok to cross export one way,
         * but also to determine if we should attempt a cross export
         * the other way.  There's no point doing it both ways.
         */
        int ok = 0;

        /* Complex case, where the keymgmt differ */
        if (keymgmt1 != NULL
            && keymgmt2 != NULL
            && !match_type(keymgmt1, keymgmt2)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_DIFFERENT_KEY_TYPES);
            return -1;           /* Not the same type */
        }

        /*
         * The key types are determined to match, so we try cross export,
         * but only to keymgmt's that supply a matching function.
         */
        if (keymgmt2 != NULL
            && keymgmt2->match != NULL) {
            void *tmp_keydata = NULL;

            ok = 1;
            if (keydata1 != NULL) {
                tmp_keydata =
                    evp_keymgmt_util_export_to_provider(pk1, keymgmt2,
                                                        selection);
                ok = (tmp_keydata != NULL);
            }
            if (ok) {
                keymgmt1 = keymgmt2;
                keydata1 = tmp_keydata;
            }
        }
        /*
         * If we've successfully cross exported one way, there's no point
         * doing it the other way, hence the |!ok| check.
         */
        if (!ok
            && keymgmt1 != NULL
            && keymgmt1->match != NULL) {
            void *tmp_keydata = NULL;

            ok = 1;
            if (keydata2 != NULL) {
                tmp_keydata =
                    evp_keymgmt_util_export_to_provider(pk2, keymgmt1,
                                                        selection);
                ok = (tmp_keydata != NULL);
            }
            if (ok) {
                keymgmt2 = keymgmt1;
                keydata2 = tmp_keydata;
            }
        }
    }

    /* If we still don't have matching keymgmt implementations, we give up */
    if (keymgmt1 != keymgmt2)
        return -2;

    /* If both keydata are NULL, then they're the same key */
    if (keydata1 == NULL && keydata2 == NULL)
        return 1;
    /* If only one of the keydata is NULL, then they're different keys */
    if (keydata1 == NULL || keydata2 == NULL)
        return 0;
    /* If both keydata are non-NULL, we let the backend decide */
    return evp_keymgmt_match(keymgmt1, keydata1, keydata2, selection);
}

int evp_keymgmt_util_copy(EVP_PKEY *to, EVP_PKEY *from, int selection)
{
    /* Save copies of pointers we want to play with without affecting |to| */
    EVP_KEYMGMT *to_keymgmt = to->keymgmt;
    void *to_keydata = to->keydata, *alloc_keydata = NULL;

    /* An unassigned key can't be copied */
    if (from == NULL || from->keydata == NULL)
        return 0;

    /*
     * If |to| is unassigned, ensure it gets the same KEYMGMT as |from|,
     * Note that the final setting of KEYMGMT is done further down, with
     * EVP_PKEY_set_type_by_keymgmt(); we don't want to do that prematurely.
     */
    if (to_keymgmt == NULL)
        to_keymgmt = from->keymgmt;

    if (to_keymgmt == from->keymgmt && to_keymgmt->dup != NULL
        && to_keydata == NULL) {
        to_keydata = alloc_keydata = evp_keymgmt_dup(to_keymgmt,
                                                     from->keydata,
                                                     selection);
        if (to_keydata == NULL)
            return 0;
    } else if (match_type(to_keymgmt, from->keymgmt)) {
        struct evp_keymgmt_util_try_import_data_st import_data;

        import_data.keymgmt = to_keymgmt;
        import_data.keydata = to_keydata;
        import_data.selection = selection;

        if (!evp_keymgmt_util_export(from, selection,
                                     &evp_keymgmt_util_try_import,
                                     &import_data))
            return 0;

        /*
         * In case to_keydata was previously unallocated,
         * evp_keymgmt_util_try_import() may have created it for us.
         */
        if (to_keydata == NULL)
            to_keydata = alloc_keydata = import_data.keydata;
    } else {
        ERR_raise(ERR_LIB_EVP, EVP_R_DIFFERENT_KEY_TYPES);
        return 0;
    }

    /*
     * We only need to set the |to| type when its |keymgmt| isn't set.
     * We can then just set its |keydata| to what we have, which might
     * be exactly what it had when entering this function.
     * This is a bit different from using evp_keymgmt_util_assign_pkey(),
     * which isn't as careful with |to|'s original |keymgmt|, since it's
     * meant to forcibly reassign an EVP_PKEY no matter what, which is
     * why we don't use that one here.
     */
    if (to->keymgmt == NULL
        && !EVP_PKEY_set_type_by_keymgmt(to, to_keymgmt)) {
        evp_keymgmt_freedata(to_keymgmt, alloc_keydata);
        return 0;
    }
    to->keydata = to_keydata;
    evp_keymgmt_util_cache_keyinfo(to);

    return 1;
}

void *evp_keymgmt_util_gen(EVP_PKEY *target, EVP_KEYMGMT *keymgmt,
                           void *genctx, OSSL_CALLBACK *cb, void *cbarg)
{
    void *keydata = NULL;

    if ((keydata = evp_keymgmt_gen(keymgmt, genctx, cb, cbarg)) == NULL
        || !evp_keymgmt_util_assign_pkey(target, keymgmt, keydata)) {
        evp_keymgmt_freedata(keymgmt, keydata);
        keydata = NULL;
    }

    return keydata;
}

/*
 * Returns the same numbers as EVP_PKEY_get_default_digest_name()
 * When the string from the EVP_KEYMGMT implementation is "", we use
 * SN_undef, since that corresponds to what EVP_PKEY_get_default_nid()
 * returns for no digest.
 */
int evp_keymgmt_util_get_deflt_digest_name(EVP_KEYMGMT *keymgmt,
                                           void *keydata,
                                           char *mdname, size_t mdname_sz)
{
    OSSL_PARAM params[3];
    char mddefault[100] = "";
    char mdmandatory[100] = "";
    char *result = NULL;
    int rv = -2;

    params[0] =
        OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_DEFAULT_DIGEST,
                                         mddefault, sizeof(mddefault));
    params[1] =
        OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_MANDATORY_DIGEST,
                                         mdmandatory,
                                         sizeof(mdmandatory));
    params[2] = OSSL_PARAM_construct_end();

    if (!evp_keymgmt_get_params(keymgmt, keydata, params))
        return 0;

    if (OSSL_PARAM_modified(params + 1)) {
        if (params[1].return_size <= 1) /* Only a NUL byte */
            result = SN_undef;
        else
            result = mdmandatory;
        rv = 2;
    } else if (OSSL_PARAM_modified(params)) {
        if (params[0].return_size <= 1) /* Only a NUL byte */
            result = SN_undef;
        else
            result = mddefault;
        rv = 1;
    }
    if (rv > 0)
        OPENSSL_strlcpy(mdname, result, mdname_sz);
    return rv;
}

/*
 * If |keymgmt| has the method function |query_operation_name|, use it to get
 * the name of a supported operation identity.  Otherwise, return the keytype,
 * assuming that it works as a default operation name.
 */
const char *evp_keymgmt_util_query_operation_name(EVP_KEYMGMT *keymgmt,
                                                  int op_id)
{
    const char *name = NULL;

    if (keymgmt != NULL) {
        if (keymgmt->query_operation_name != NULL)
            name = keymgmt->query_operation_name(op_id);
        if (name == NULL)
            name = EVP_KEYMGMT_get0_name(keymgmt);
    }
    return name;
}
