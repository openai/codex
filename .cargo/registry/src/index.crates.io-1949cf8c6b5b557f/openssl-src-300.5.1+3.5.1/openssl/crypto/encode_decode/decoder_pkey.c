/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_names.h>
#include <openssl/core_object.h>
#include <openssl/provider.h>
#include <openssl/evp.h>
#include <openssl/ui.h>
#include <openssl/decoder.h>
#include <openssl/safestack.h>
#include <openssl/trace.h>
#include "crypto/evp.h"
#include "crypto/decoder.h"
#include "crypto/evp/evp_local.h"
#include "crypto/lhash.h"
#include "encoder_local.h"
#include "internal/namemap.h"
#include "internal/sizes.h"

int OSSL_DECODER_CTX_set_passphrase(OSSL_DECODER_CTX *ctx,
                                    const unsigned char *kstr,
                                    size_t klen)
{
    return ossl_pw_set_passphrase(&ctx->pwdata, kstr, klen);
}

int OSSL_DECODER_CTX_set_passphrase_ui(OSSL_DECODER_CTX *ctx,
                                       const UI_METHOD *ui_method,
                                       void *ui_data)
{
    return ossl_pw_set_ui_method(&ctx->pwdata, ui_method, ui_data);
}

int OSSL_DECODER_CTX_set_pem_password_cb(OSSL_DECODER_CTX *ctx,
                                         pem_password_cb *cb, void *cbarg)
{
    return ossl_pw_set_pem_password_cb(&ctx->pwdata, cb, cbarg);
}

int OSSL_DECODER_CTX_set_passphrase_cb(OSSL_DECODER_CTX *ctx,
                                       OSSL_PASSPHRASE_CALLBACK *cb,
                                       void *cbarg)
{
    return ossl_pw_set_ossl_passphrase_cb(&ctx->pwdata, cb, cbarg);
}

/*
 * Support for OSSL_DECODER_CTX_new_for_pkey:
 * The construct data, and collecting keymgmt information for it
 */

DEFINE_STACK_OF(EVP_KEYMGMT)

struct decoder_pkey_data_st {
    OSSL_LIB_CTX *libctx;
    char *propq;
    int selection;

    STACK_OF(EVP_KEYMGMT) *keymgmts;
    char *object_type;           /* recorded object data type, may be NULL */
    void **object;               /* Where the result should end up */
    OSSL_DECODER_CTX *ctx;       /* The parent decoder context */
};

static int decoder_construct_pkey(OSSL_DECODER_INSTANCE *decoder_inst,
                                  const OSSL_PARAM *params,
                                  void *construct_data)
{
    struct decoder_pkey_data_st *data = construct_data;
    OSSL_DECODER *decoder = OSSL_DECODER_INSTANCE_get_decoder(decoder_inst);
    void *decoderctx = OSSL_DECODER_INSTANCE_get_decoder_ctx(decoder_inst);
    const OSSL_PROVIDER *decoder_prov = OSSL_DECODER_get0_provider(decoder);
    EVP_KEYMGMT *keymgmt = NULL;
    const OSSL_PROVIDER *keymgmt_prov = NULL;
    int i, end;
    /*
     * |object_ref| points to a provider reference to an object, its exact
     * contents entirely opaque to us, but may be passed to any provider
     * function that expects this (such as OSSL_FUNC_keymgmt_load().
     *
     * This pointer is considered volatile, i.e. whatever it points at
     * is assumed to be freed as soon as this function returns.
     */
    void *object_ref = NULL;
    size_t object_ref_sz = 0;
    const OSSL_PARAM *p;

    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DATA_TYPE);
    if (p != NULL) {
        char *object_type = NULL;

        if (!OSSL_PARAM_get_utf8_string(p, &object_type, 0))
            return 0;
        OPENSSL_free(data->object_type);
        data->object_type = object_type;
    }

    /*
     * For stuff that should end up in an EVP_PKEY, we only accept an object
     * reference for the moment.  This enforces that the key data itself
     * remains with the provider.
     */
    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_REFERENCE);
    if (p == NULL || p->data_type != OSSL_PARAM_OCTET_STRING)
        return 0;
    object_ref = p->data;
    object_ref_sz = p->data_size;

    /*
     * First, we try to find a keymgmt that comes from the same provider as
     * the decoder that passed the params.
     */
    end = sk_EVP_KEYMGMT_num(data->keymgmts);
    for (i = 0; i < end; i++) {
        keymgmt = sk_EVP_KEYMGMT_value(data->keymgmts, i);
        keymgmt_prov = EVP_KEYMGMT_get0_provider(keymgmt);

        if (keymgmt_prov == decoder_prov
            && evp_keymgmt_has_load(keymgmt)
            && EVP_KEYMGMT_is_a(keymgmt, data->object_type))
            break;
    }
    if (i < end) {
        /* To allow it to be freed further down */
        if (!EVP_KEYMGMT_up_ref(keymgmt))
            return 0;
    } else if ((keymgmt = EVP_KEYMGMT_fetch(data->libctx,
                                            data->object_type,
                                            data->propq)) != NULL) {
        keymgmt_prov = EVP_KEYMGMT_get0_provider(keymgmt);
    }

    if (keymgmt != NULL) {
        EVP_PKEY *pkey = NULL;
        void *keydata = NULL;

        /*
         * If the EVP_KEYMGMT and the OSSL_DECODER are from the
         * same provider, we assume that the KEYMGMT has a key loading
         * function that can handle the provider reference we hold.
         *
         * Otherwise, we export from the decoder and import the
         * result in the keymgmt.
         */
        if (keymgmt_prov == decoder_prov) {
            keydata = evp_keymgmt_load(keymgmt, object_ref, object_ref_sz);
        } else {
            struct evp_keymgmt_util_try_import_data_st import_data;

            import_data.keymgmt = keymgmt;
            import_data.keydata = NULL;
            if (data->selection == 0)
                /* import/export functions do not tolerate 0 selection */
                import_data.selection = OSSL_KEYMGMT_SELECT_ALL;
            else
                import_data.selection = data->selection;

            /*
             * No need to check for errors here, the value of
             * |import_data.keydata| is as much an indicator.
             */
            (void)decoder->export_object(decoderctx,
                                         object_ref, object_ref_sz,
                                         &evp_keymgmt_util_try_import,
                                         &import_data);
            keydata = import_data.keydata;
            import_data.keydata = NULL;
        }
        /*
         * When load or import fails, because this is not an acceptable key
         * (despite the provided key material being syntactically valid), the
         * reason why the key is rejected would be lost, unless we signal a
         * hard error, and suppress resetting for another try.
         */
        if (keydata == NULL)
            ossl_decoder_ctx_set_harderr(data->ctx);

        if (keydata != NULL
            && (pkey = evp_keymgmt_util_make_pkey(keymgmt, keydata)) == NULL)
            evp_keymgmt_freedata(keymgmt, keydata);

        *data->object = pkey;

        /*
         * evp_keymgmt_util_make_pkey() increments the reference count when
         * assigning the EVP_PKEY, so we can free the keymgmt here.
         */
        EVP_KEYMGMT_free(keymgmt);
    }
    /*
     * We successfully looked through, |*ctx->object| determines if we
     * actually found something.
     */
    return (*data->object != NULL);
}

static void decoder_clean_pkey_construct_arg(void *construct_data)
{
    struct decoder_pkey_data_st *data = construct_data;

    if (data != NULL) {
        sk_EVP_KEYMGMT_pop_free(data->keymgmts, EVP_KEYMGMT_free);
        OPENSSL_free(data->propq);
        OPENSSL_free(data->object_type);
        OPENSSL_free(data);
    }
}

struct collect_data_st {
    OSSL_LIB_CTX *libctx;
    OSSL_DECODER_CTX *ctx;

    const char *keytype; /* the keytype requested, if any */
    int keytype_id; /* if keytype_resolved is set, keymgmt name_id; else 0 */
    int sm2_id;     /* if keytype_resolved is set and EC, SM2 name_id; else 0 */
    int total;      /* number of matching results */
    char error_occurred;
    char keytype_resolved;

    STACK_OF(EVP_KEYMGMT) *keymgmts;
};

static void collect_decoder_keymgmt(EVP_KEYMGMT *keymgmt, OSSL_DECODER *decoder,
                                    void *provctx, struct collect_data_st *data)
{
    void *decoderctx = NULL;
    OSSL_DECODER_INSTANCE *di = NULL;

    /*
     * We already checked the EVP_KEYMGMT is applicable in check_keymgmt so we
     * don't check it again here.
     */

    if (keymgmt->name_id != decoder->base.id)
        /* Mismatch is not an error, continue. */
        return;

    if ((decoderctx = decoder->newctx(provctx)) == NULL) {
        data->error_occurred = 1;
        return;
    }

    if ((di = ossl_decoder_instance_new(decoder, decoderctx)) == NULL) {
        decoder->freectx(decoderctx);
        data->error_occurred = 1;
        return;
    }

    /*
     * Input types must be compatible, but we must accept DER encoders when the
     * start input type is "PEM".
     */
    if (data->ctx->start_input_type != NULL
        && di->input_type != NULL
        && OPENSSL_strcasecmp(di->input_type, data->ctx->start_input_type) != 0
        && (OPENSSL_strcasecmp(di->input_type, "DER") != 0
            || OPENSSL_strcasecmp(data->ctx->start_input_type, "PEM") != 0)) {
        /* Mismatch is not an error, continue. */
        ossl_decoder_instance_free(di);
        return;
    }

    OSSL_TRACE_BEGIN(DECODER) {
        BIO_printf(trc_out,
                   "(ctx %p) Checking out decoder %p:\n"
                   "    %s with %s\n",
                   (void *)data->ctx, (void *)decoder,
                   OSSL_DECODER_get0_name(decoder),
                   OSSL_DECODER_get0_properties(decoder));
    } OSSL_TRACE_END(DECODER);

    if (!ossl_decoder_ctx_add_decoder_inst(data->ctx, di)) {
        ossl_decoder_instance_free(di);
        data->error_occurred = 1;
        return;
    }

    ++data->total;
}

static void collect_decoder(OSSL_DECODER *decoder, void *arg)
{
    struct collect_data_st *data = arg;
    STACK_OF(EVP_KEYMGMT) *keymgmts = data->keymgmts;
    int i, end_i;
    EVP_KEYMGMT *keymgmt;
    const OSSL_PROVIDER *prov;
    void *provctx;

    if (data->error_occurred)
        return;

    prov = OSSL_DECODER_get0_provider(decoder);
    provctx = OSSL_PROVIDER_get0_provider_ctx(prov);

    /*
     * Either the caller didn't give us a selection, or if they did, the decoder
     * must tell us if it supports that selection to be accepted. If the decoder
     * doesn't have |does_selection|, it's seen as taking anything.
     */
    if (decoder->does_selection != NULL
            && !decoder->does_selection(provctx, data->ctx->selection))
        return;

    OSSL_TRACE_BEGIN(DECODER) {
        BIO_printf(trc_out,
                   "(ctx %p) Checking out decoder %p:\n"
                   "    %s with %s\n",
                   (void *)data->ctx, (void *)decoder,
                   OSSL_DECODER_get0_name(decoder),
                   OSSL_DECODER_get0_properties(decoder));
    } OSSL_TRACE_END(DECODER);

    end_i = sk_EVP_KEYMGMT_num(keymgmts);
    for (i = 0; i < end_i; ++i) {
        keymgmt = sk_EVP_KEYMGMT_value(keymgmts, i);

        collect_decoder_keymgmt(keymgmt, decoder, provctx, data);
        if (data->error_occurred)
            return;
    }
}

/*
 * Is this EVP_KEYMGMT applicable given the key type given in the call to
 * ossl_decoder_ctx_setup_for_pkey (if any)?
 */
static int check_keymgmt(EVP_KEYMGMT *keymgmt, struct collect_data_st *data)
{
    /* If no keytype was specified, everything matches. */
    if (data->keytype == NULL)
        return 1;

    if (!data->keytype_resolved) {
        /* We haven't cached the IDs from the keytype string yet. */
        OSSL_NAMEMAP *namemap = ossl_namemap_stored(data->libctx);
        data->keytype_id = ossl_namemap_name2num(namemap, data->keytype);

        /*
         * If keytype is a value ambiguously used for both EC and SM2,
         * collect the ID for SM2 as well.
         */
        if (data->keytype_id != 0
            && (strcmp(data->keytype, "id-ecPublicKey") == 0
                || strcmp(data->keytype, "1.2.840.10045.2.1") == 0))
            data->sm2_id = ossl_namemap_name2num(namemap, "SM2");

        /*
         * If keytype_id is zero the name was not found, but we still
         * set keytype_resolved to avoid trying all this again.
         */
        data->keytype_resolved = 1;
    }

    /* Specified keytype could not be resolved, so nothing matches. */
    if (data->keytype_id == 0)
        return 0;

    /* Does not match the keytype specified, so skip. */
    if (keymgmt->name_id != data->keytype_id
        && keymgmt->name_id != data->sm2_id)
        return 0;

    return 1;
}

static void collect_keymgmt(EVP_KEYMGMT *keymgmt, void *arg)
{
    struct collect_data_st *data = arg;

    if (!check_keymgmt(keymgmt, data))
        return;

    /*
     * We have to ref EVP_KEYMGMT here because in the success case,
     * data->keymgmts is referenced by the constructor we register in the
     * OSSL_DECODER_CTX. The registered cleanup function
     * (decoder_clean_pkey_construct_arg) unrefs every element of the stack and
     * frees it.
     */
    if (!EVP_KEYMGMT_up_ref(keymgmt))
        return;

    if (sk_EVP_KEYMGMT_push(data->keymgmts, keymgmt) <= 0) {
        EVP_KEYMGMT_free(keymgmt);
        data->error_occurred = 1;
    }
}

/*
 * This function does the actual binding of decoders to the OSSL_DECODER_CTX. It
 * searches for decoders matching 'keytype', which is a string like "RSA", "DH",
 * etc. If 'keytype' is NULL, decoders for all keytypes are bound.
 */
static int ossl_decoder_ctx_setup_for_pkey(OSSL_DECODER_CTX *ctx,
                                           const char *keytype,
                                           OSSL_LIB_CTX *libctx,
                                           const char *propquery)
{
    int ok = 0;
    struct decoder_pkey_data_st *process_data = NULL;
    struct collect_data_st collect_data = { NULL };
    STACK_OF(EVP_KEYMGMT) *keymgmts = NULL;

    OSSL_TRACE_BEGIN(DECODER) {
        const char *input_type = ctx->start_input_type;
        const char *input_structure = ctx->input_structure;

        BIO_printf(trc_out,
                   "(ctx %p) Looking for decoders producing %s%s%s%s%s%s\n",
                   (void *)ctx,
                   keytype != NULL ? keytype : "",
                   keytype != NULL ? " keys" : "keys of any type",
                   input_type != NULL ? " from " : "",
                   input_type != NULL ? input_type : "",
                   input_structure != NULL ? " with " : "",
                   input_structure != NULL ? input_structure : "");
    } OSSL_TRACE_END(DECODER);

    /* Allocate data. */
    if ((process_data = OPENSSL_zalloc(sizeof(*process_data))) == NULL)
        goto err;
    if ((propquery != NULL
            && (process_data->propq = OPENSSL_strdup(propquery)) == NULL))
        goto err;

    /* Allocate our list of EVP_KEYMGMTs. */
    keymgmts = sk_EVP_KEYMGMT_new_null();
    if (keymgmts == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_CRYPTO_LIB);
        goto err;
    }

    process_data->object    = NULL;
    process_data->libctx    = libctx;
    process_data->selection = ctx->selection;
    process_data->keymgmts  = keymgmts;

    /*
     * Enumerate all keymgmts into a stack.
     *
     * We could nest EVP_KEYMGMT_do_all_provided inside
     * OSSL_DECODER_do_all_provided or vice versa but these functions become
     * bottlenecks if called repeatedly, which is why we collect the
     * EVP_KEYMGMTs into a stack here and call both functions only once.
     *
     * We resolve the keytype string to a name ID so we don't have to resolve it
     * multiple times, avoiding repeated calls to EVP_KEYMGMT_is_a, which is a
     * performance bottleneck. However, we do this lazily on the first call to
     * collect_keymgmt made by EVP_KEYMGMT_do_all_provided, rather than do it
     * upfront, as this ensures that the names for all loaded providers have
     * been registered by the time we try to resolve the keytype string.
     */
    collect_data.ctx        = ctx;
    collect_data.libctx     = libctx;
    collect_data.keymgmts   = keymgmts;
    collect_data.keytype    = keytype;
    EVP_KEYMGMT_do_all_provided(libctx, collect_keymgmt, &collect_data);

    if (collect_data.error_occurred)
        goto err;

    /* Enumerate all matching decoders. */
    OSSL_DECODER_do_all_provided(libctx, collect_decoder, &collect_data);

    if (collect_data.error_occurred)
        goto err;

    OSSL_TRACE_BEGIN(DECODER) {
        BIO_printf(trc_out,
                   "(ctx %p) Got %d decoders producing keys\n",
                   (void *)ctx, collect_data.total);
    } OSSL_TRACE_END(DECODER);

    /*
     * Finish initializing the decoder context. If one or more decoders matched
     * above then the number of decoders attached to the OSSL_DECODER_CTX will
     * be nonzero. Else nothing was found and we do nothing.
     */
    if (OSSL_DECODER_CTX_get_num_decoders(ctx) != 0) {
        if (!OSSL_DECODER_CTX_set_construct(ctx, decoder_construct_pkey)
            || !OSSL_DECODER_CTX_set_construct_data(ctx, process_data)
            || !OSSL_DECODER_CTX_set_cleanup(ctx,
                                             decoder_clean_pkey_construct_arg))
            goto err;

        process_data = NULL; /* Avoid it being freed */
    }

    ok = 1;
 err:
    decoder_clean_pkey_construct_arg(process_data);
    return ok;
}

/* Only const here because deep_copy requires it */
static EVP_KEYMGMT *keymgmt_dup(const EVP_KEYMGMT *keymgmt)
{
    if (!EVP_KEYMGMT_up_ref((EVP_KEYMGMT *)keymgmt))
        return NULL;

    return (EVP_KEYMGMT *)keymgmt;
}

/*
 * Duplicates a template OSSL_DECODER_CTX that has been setup for an EVP_PKEY
 * operation and sets up the duplicate for a new operation.
 * It does not duplicate the pwdata on the assumption that this does not form
 * part of the template. That is set up later.
 */
static OSSL_DECODER_CTX *
ossl_decoder_ctx_for_pkey_dup(OSSL_DECODER_CTX *src,
                              EVP_PKEY **pkey,
                              const char *input_type,
                              const char *input_structure)
{
    OSSL_DECODER_CTX *dest;
    struct decoder_pkey_data_st *process_data_src, *process_data_dest = NULL;

    if (src == NULL)
        return NULL;

    if ((dest = OSSL_DECODER_CTX_new()) == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
        return NULL;
    }

    if (!OSSL_DECODER_CTX_set_input_type(dest, input_type)
            || !OSSL_DECODER_CTX_set_input_structure(dest, input_structure)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
        goto err;
    }
    dest->selection = src->selection;

    if (src->decoder_insts != NULL) {
        dest->decoder_insts
            = sk_OSSL_DECODER_INSTANCE_deep_copy(src->decoder_insts,
                                                 ossl_decoder_instance_dup,
                                                 ossl_decoder_instance_free);
        if (dest->decoder_insts == NULL) {
            ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
            goto err;
        }
    }

    if (!OSSL_DECODER_CTX_set_construct(dest,
                                        OSSL_DECODER_CTX_get_construct(src))) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
        goto err;
    }

    process_data_src = OSSL_DECODER_CTX_get_construct_data(src);
    if (process_data_src != NULL) {
        process_data_dest = OPENSSL_zalloc(sizeof(*process_data_dest));
        if (process_data_dest == NULL) {
            ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_CRYPTO_LIB);
            goto err;
        }
        if (process_data_src->propq != NULL) {
            process_data_dest->propq = OPENSSL_strdup(process_data_src->propq);
            if (process_data_dest->propq == NULL) {
                ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_CRYPTO_LIB);
                goto err;
            }
        }

        if (process_data_src->keymgmts != NULL) {
            process_data_dest->keymgmts
                = sk_EVP_KEYMGMT_deep_copy(process_data_src->keymgmts,
                                           keymgmt_dup,
                                           EVP_KEYMGMT_free);
            if (process_data_dest->keymgmts == NULL) {
                ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_EVP_LIB);
                goto err;
            }
        }

        process_data_dest->object    = (void **)pkey;
        process_data_dest->libctx    = process_data_src->libctx;
        process_data_dest->selection = process_data_src->selection;
        process_data_dest->ctx       = dest;
        if (!OSSL_DECODER_CTX_set_construct_data(dest, process_data_dest)) {
            ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
            goto err;
        }
        process_data_dest = NULL;
    }

    if (!OSSL_DECODER_CTX_set_cleanup(dest,
                                      OSSL_DECODER_CTX_get_cleanup(src))) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
        goto err;
    }

    return dest;
 err:
    decoder_clean_pkey_construct_arg(process_data_dest);
    OSSL_DECODER_CTX_free(dest);
    return NULL;
}

typedef struct {
    char *input_type;
    char *input_structure;
    char *keytype;
    int selection;
    char *propquery;
    OSSL_DECODER_CTX *template;
} DECODER_CACHE_ENTRY;

DEFINE_LHASH_OF_EX(DECODER_CACHE_ENTRY);

typedef struct {
    CRYPTO_RWLOCK *lock;
    LHASH_OF(DECODER_CACHE_ENTRY) *hashtable;
} DECODER_CACHE;

static void decoder_cache_entry_free(DECODER_CACHE_ENTRY *entry)
{
    if (entry == NULL)
        return;
    OPENSSL_free(entry->input_type);
    OPENSSL_free(entry->input_structure);
    OPENSSL_free(entry->keytype);
    OPENSSL_free(entry->propquery);
    OSSL_DECODER_CTX_free(entry->template);
    OPENSSL_free(entry);
}

static unsigned long decoder_cache_entry_hash(const DECODER_CACHE_ENTRY *cache)
{
    unsigned long hash = 17;

    hash = (hash * 23)
           + (cache->propquery == NULL
              ? 0 : ossl_lh_strcasehash(cache->propquery));
    hash = (hash * 23)
           + (cache->input_structure == NULL
              ? 0  : ossl_lh_strcasehash(cache->input_structure));
    hash = (hash * 23)
           + (cache->input_type == NULL
              ? 0  : ossl_lh_strcasehash(cache->input_type));
    hash = (hash * 23)
           + (cache->keytype == NULL
              ? 0  : ossl_lh_strcasehash(cache->keytype));

    hash ^= cache->selection;

    return hash;
}

static ossl_inline int nullstrcmp(const char *a, const char *b, int casecmp)
{
    if (a == NULL || b == NULL) {
        if (a == NULL) {
            if (b == NULL)
                return 0;
            else
                return 1;
        } else {
            return -1;
        }
    } else {
        if (casecmp)
            return OPENSSL_strcasecmp(a, b);
        else
            return strcmp(a, b);
    }
}

static int decoder_cache_entry_cmp(const DECODER_CACHE_ENTRY *a,
                                   const DECODER_CACHE_ENTRY *b)
{
    int cmp;

    if (a->selection != b->selection)
        return (a->selection < b->selection) ? -1 : 1;

    cmp = nullstrcmp(a->keytype, b->keytype, 1);
    if (cmp != 0)
        return cmp;

    cmp = nullstrcmp(a->input_type, b->input_type, 1);
    if (cmp != 0)
        return cmp;

    cmp = nullstrcmp(a->input_structure, b->input_structure, 1);
    if (cmp != 0)
        return cmp;

    cmp = nullstrcmp(a->propquery, b->propquery, 0);

    return cmp;
}

void *ossl_decoder_cache_new(OSSL_LIB_CTX *ctx)
{
    DECODER_CACHE *cache = OPENSSL_malloc(sizeof(*cache));

    if (cache == NULL)
        return NULL;

    cache->lock = CRYPTO_THREAD_lock_new();
    if (cache->lock == NULL) {
        OPENSSL_free(cache);
        return NULL;
    }
    cache->hashtable = lh_DECODER_CACHE_ENTRY_new(decoder_cache_entry_hash,
                                                  decoder_cache_entry_cmp);
    if (cache->hashtable == NULL) {
        CRYPTO_THREAD_lock_free(cache->lock);
        OPENSSL_free(cache);
        return NULL;
    }

    return cache;
}

void ossl_decoder_cache_free(void *vcache)
{
    DECODER_CACHE *cache = (DECODER_CACHE *)vcache;

    lh_DECODER_CACHE_ENTRY_doall(cache->hashtable, decoder_cache_entry_free);
    lh_DECODER_CACHE_ENTRY_free(cache->hashtable);
    CRYPTO_THREAD_lock_free(cache->lock);
    OPENSSL_free(cache);
}

/*
 * Called whenever a provider gets activated/deactivated. In that case the
 * decoders that are available might change so we flush our cache.
 */
int ossl_decoder_cache_flush(OSSL_LIB_CTX *libctx)
{
    DECODER_CACHE *cache
        = ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_DECODER_CACHE_INDEX);

    if (cache == NULL)
        return 0;


    if (!CRYPTO_THREAD_write_lock(cache->lock)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
        return 0;
    }

    lh_DECODER_CACHE_ENTRY_doall(cache->hashtable, decoder_cache_entry_free);
    lh_DECODER_CACHE_ENTRY_flush(cache->hashtable);

    CRYPTO_THREAD_unlock(cache->lock);
    return 1;
}

OSSL_DECODER_CTX *
OSSL_DECODER_CTX_new_for_pkey(EVP_PKEY **pkey,
                              const char *input_type,
                              const char *input_structure,
                              const char *keytype, int selection,
                              OSSL_LIB_CTX *libctx, const char *propquery)
{
    OSSL_DECODER_CTX *ctx = NULL;
    OSSL_PARAM decoder_params[] = {
        OSSL_PARAM_END,
        OSSL_PARAM_END,
        OSSL_PARAM_END
    };
    DECODER_CACHE *cache
        = ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_DECODER_CACHE_INDEX);
    DECODER_CACHE_ENTRY cacheent, *res, *newcache = NULL;
    int i = 0;

    if (cache == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
        return NULL;
    }
    if (input_structure != NULL)
        decoder_params[i++] =
            OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_STRUCTURE,
                                             (char *)input_structure, 0);
    if (propquery != NULL)
        decoder_params[i++] =
            OSSL_PARAM_construct_utf8_string(OSSL_DECODER_PARAM_PROPERTIES,
                                             (char *)propquery, 0);

    /* It is safe to cast away the const here */
    cacheent.input_type = (char *)input_type;
    cacheent.input_structure = (char *)input_structure;
    cacheent.keytype = (char *)keytype;
    cacheent.selection = selection;
    cacheent.propquery = (char *)propquery;

    if (!CRYPTO_THREAD_read_lock(cache->lock)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_CRYPTO_LIB);
        return NULL;
    }

    /* First see if we have a template OSSL_DECODER_CTX */
    res = lh_DECODER_CACHE_ENTRY_retrieve(cache->hashtable, &cacheent);

    if (res == NULL) {
        /*
         * There is no template so we will have to construct one. This will be
         * time consuming so release the lock and we will later upgrade it to a
         * write lock.
         */
        CRYPTO_THREAD_unlock(cache->lock);

        if ((ctx = OSSL_DECODER_CTX_new()) == NULL) {
            ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
            return NULL;
        }

        OSSL_TRACE_BEGIN(DECODER) {
            BIO_printf(trc_out,
                    "(ctx %p) Looking for %s decoders with selection %d\n",
                    (void *)ctx, keytype, selection);
            BIO_printf(trc_out, "    input type: %s, input structure: %s\n",
                    input_type, input_structure);
        } OSSL_TRACE_END(DECODER);

        if (OSSL_DECODER_CTX_set_input_type(ctx, input_type)
            && OSSL_DECODER_CTX_set_input_structure(ctx, input_structure)
            && OSSL_DECODER_CTX_set_selection(ctx, selection)
            && ossl_decoder_ctx_setup_for_pkey(ctx, keytype, libctx, propquery)
            && OSSL_DECODER_CTX_add_extra(ctx, libctx, propquery)
            && (propquery == NULL
                || OSSL_DECODER_CTX_set_params(ctx, decoder_params))) {
            OSSL_TRACE_BEGIN(DECODER) {
                BIO_printf(trc_out, "(ctx %p) Got %d decoders\n",
                        (void *)ctx, OSSL_DECODER_CTX_get_num_decoders(ctx));
            } OSSL_TRACE_END(DECODER);
        } else {
            ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_OSSL_DECODER_LIB);
            OSSL_DECODER_CTX_free(ctx);
            return NULL;
        }

        newcache = OPENSSL_zalloc(sizeof(*newcache));
        if (newcache == NULL) {
            OSSL_DECODER_CTX_free(ctx);
            return NULL;
        }

        if (input_type != NULL) {
            newcache->input_type = OPENSSL_strdup(input_type);
            if (newcache->input_type == NULL)
                goto err;
        }
        if (input_structure != NULL) {
            newcache->input_structure = OPENSSL_strdup(input_structure);
            if (newcache->input_structure == NULL)
                goto err;
        }
        if (keytype != NULL) {
            newcache->keytype = OPENSSL_strdup(keytype);
            if (newcache->keytype == NULL)
                goto err;
        }
        if (propquery != NULL) {
            newcache->propquery = OPENSSL_strdup(propquery);
            if (newcache->propquery == NULL)
                goto err;
        }
        newcache->selection = selection;
        newcache->template = ctx;

        if (!CRYPTO_THREAD_write_lock(cache->lock)) {
            ctx = NULL;
            ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_CRYPTO_LIB);
            goto err;
        }
        res = lh_DECODER_CACHE_ENTRY_retrieve(cache->hashtable, &cacheent);
        if (res == NULL) {
            (void)lh_DECODER_CACHE_ENTRY_insert(cache->hashtable, newcache);
            if (lh_DECODER_CACHE_ENTRY_error(cache->hashtable)) {
                ctx = NULL;
                ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_CRYPTO_LIB);
                goto err;
            }
        } else {
            /*
             * We raced with another thread to construct this and lost. Free
             * what we just created and use the entry from the hashtable instead
             */
            decoder_cache_entry_free(newcache);
            ctx = res->template;
        }
    } else {
        ctx = res->template;
    }

    ctx = ossl_decoder_ctx_for_pkey_dup(ctx, pkey, input_type, input_structure);
    CRYPTO_THREAD_unlock(cache->lock);

    return ctx;
 err:
    decoder_cache_entry_free(newcache);
    OSSL_DECODER_CTX_free(ctx);
    return NULL;
}
