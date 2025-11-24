/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/err.h>
#include <openssl/ui.h>
#include <openssl/params.h>
#include <openssl/encoder.h>
#include <openssl/core_names.h>
#include <openssl/provider.h>
#include <openssl/safestack.h>
#include <openssl/trace.h>
#include "internal/provider.h"
#include "internal/property.h"
#include "internal/namemap.h"
#include "crypto/evp.h"
#include "encoder_local.h"

DEFINE_STACK_OF(OSSL_ENCODER)

int OSSL_ENCODER_CTX_set_cipher(OSSL_ENCODER_CTX *ctx,
                                const char *cipher_name,
                                const char *propquery)
{
    OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END };

    params[0] =
        OSSL_PARAM_construct_utf8_string(OSSL_ENCODER_PARAM_CIPHER,
                                         (void *)cipher_name, 0);
    params[1] =
        OSSL_PARAM_construct_utf8_string(OSSL_ENCODER_PARAM_PROPERTIES,
                                         (void *)propquery, 0);

    return OSSL_ENCODER_CTX_set_params(ctx, params);
}

int OSSL_ENCODER_CTX_set_passphrase(OSSL_ENCODER_CTX *ctx,
                                    const unsigned char *kstr,
                                    size_t klen)
{
    return ossl_pw_set_passphrase(&ctx->pwdata, kstr, klen);
}

int OSSL_ENCODER_CTX_set_passphrase_ui(OSSL_ENCODER_CTX *ctx,
                                       const UI_METHOD *ui_method,
                                       void *ui_data)
{
    return ossl_pw_set_ui_method(&ctx->pwdata, ui_method, ui_data);
}

int OSSL_ENCODER_CTX_set_pem_password_cb(OSSL_ENCODER_CTX *ctx,
                                         pem_password_cb *cb, void *cbarg)
{
    return ossl_pw_set_pem_password_cb(&ctx->pwdata, cb, cbarg);
}

int OSSL_ENCODER_CTX_set_passphrase_cb(OSSL_ENCODER_CTX *ctx,
                                       OSSL_PASSPHRASE_CALLBACK *cb,
                                       void *cbarg)
{
    return ossl_pw_set_ossl_passphrase_cb(&ctx->pwdata, cb, cbarg);
}

/*
 * Support for OSSL_ENCODER_CTX_new_for_type:
 * finding a suitable encoder
 */

struct collected_encoder_st {
    STACK_OF(OPENSSL_CSTRING) *names;
    int *id_names;
    const char *output_structure;
    const char *output_type;

    const OSSL_PROVIDER *keymgmt_prov;
    OSSL_ENCODER_CTX *ctx;
    unsigned int flag_find_same_provider:1;

    int error_occurred;
};

static void collect_encoder(OSSL_ENCODER *encoder, void *arg)
{
    struct collected_encoder_st *data = arg;
    const OSSL_PROVIDER *prov;

    if (data->error_occurred)
        return;

    data->error_occurred = 1;     /* Assume the worst */

    prov = OSSL_ENCODER_get0_provider(encoder);
    /*
     * collect_encoder() is called in two passes, one where the encoders
     * from the same provider as the keymgmt are looked up, and one where
     * the other encoders are looked up.  |data->flag_find_same_provider|
     * tells us which pass we're in.
     */
    if ((data->keymgmt_prov == prov) == data->flag_find_same_provider) {
        void *provctx = OSSL_PROVIDER_get0_provider_ctx(prov);
        int i, end_i = sk_OPENSSL_CSTRING_num(data->names);
        int match;

        for (i = 0; i < end_i; i++) {
            if (data->flag_find_same_provider)
                match = (data->id_names[i] == encoder->base.id);
            else
                match = OSSL_ENCODER_is_a(encoder,
                                          sk_OPENSSL_CSTRING_value(data->names, i));
            if (!match
                || (encoder->does_selection != NULL
                    && !encoder->does_selection(provctx, data->ctx->selection))
                || (data->keymgmt_prov != prov
                    && encoder->import_object == NULL))
                continue;

            /* Only add each encoder implementation once */
            if (OSSL_ENCODER_CTX_add_encoder(data->ctx, encoder))
                break;
        }
    }

    data->error_occurred = 0;         /* All is good now */
}

struct collected_names_st {
    STACK_OF(OPENSSL_CSTRING) *names;
    unsigned int error_occurred:1;
};

static void collect_name(const char *name, void *arg)
{
    struct collected_names_st *data = arg;

    if (data->error_occurred)
        return;

    data->error_occurred = 1;         /* Assume the worst */

    if (sk_OPENSSL_CSTRING_push(data->names, name) <= 0)
        return;

    data->error_occurred = 0;         /* All is good now */
}

/*
 * Support for OSSL_ENCODER_to_bio:
 * writing callback for the OSSL_PARAM (the implementation doesn't have
 * intimate knowledge of the provider side object)
 */

struct construct_data_st {
    const EVP_PKEY *pk;
    int selection;

    OSSL_ENCODER_INSTANCE *encoder_inst;
    const void *obj;
    void *constructed_obj;
};

static int encoder_import_cb(const OSSL_PARAM params[], void *arg)
{
    struct construct_data_st *construct_data = arg;
    OSSL_ENCODER_INSTANCE *encoder_inst = construct_data->encoder_inst;
    OSSL_ENCODER *encoder = OSSL_ENCODER_INSTANCE_get_encoder(encoder_inst);
    void *encoderctx = OSSL_ENCODER_INSTANCE_get_encoder_ctx(encoder_inst);

    construct_data->constructed_obj =
        encoder->import_object(encoderctx, construct_data->selection, params);

    return (construct_data->constructed_obj != NULL);
}

static const void *
encoder_construct_pkey(OSSL_ENCODER_INSTANCE *encoder_inst, void *arg)
{
    struct construct_data_st *data = arg;

    if (data->obj == NULL) {
        OSSL_ENCODER *encoder =
            OSSL_ENCODER_INSTANCE_get_encoder(encoder_inst);
        const EVP_PKEY *pk = data->pk;
        const OSSL_PROVIDER *k_prov = EVP_KEYMGMT_get0_provider(pk->keymgmt);
        const OSSL_PROVIDER *e_prov = OSSL_ENCODER_get0_provider(encoder);

        if (k_prov != e_prov) {
            int selection = data->selection;

            if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
                selection |= OSSL_KEYMGMT_SELECT_PUBLIC_KEY;
            data->encoder_inst = encoder_inst;

            if (!evp_keymgmt_export(pk->keymgmt, pk->keydata, selection,
                                    &encoder_import_cb, data))
                return NULL;
            data->obj = data->constructed_obj;
        } else {
            data->obj = pk->keydata;
        }
    }

    return data->obj;
}

static void encoder_destruct_pkey(void *arg)
{
    struct construct_data_st *data = arg;
    int match = (data->obj == data->constructed_obj);

    if (data->encoder_inst != NULL) {
        OSSL_ENCODER *encoder =
            OSSL_ENCODER_INSTANCE_get_encoder(data->encoder_inst);

        encoder->free_object(data->constructed_obj);
    }
    data->constructed_obj = NULL;
    if (match)
        data->obj = NULL;
}

/*
 * OSSL_ENCODER_CTX_new_for_pkey() returns a ctx with no encoder if
 * it couldn't find a suitable encoder.  This allows a caller to detect if
 * a suitable encoder was found, with OSSL_ENCODER_CTX_get_num_encoder(),
 * and to use fallback methods if the result is NULL.
 */
static int ossl_encoder_ctx_setup_for_pkey(OSSL_ENCODER_CTX *ctx,
                                           const EVP_PKEY *pkey,
                                           int selection,
                                           const char *propquery)
{
    struct construct_data_st *data = NULL;
    const OSSL_PROVIDER *prov = NULL;
    OSSL_LIB_CTX *libctx = NULL;
    int ok = 0, i, end;
    OSSL_NAMEMAP *namemap;

    if (!ossl_assert(ctx != NULL) || !ossl_assert(pkey != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (evp_pkey_is_provided(pkey)) {
        prov = EVP_KEYMGMT_get0_provider(pkey->keymgmt);
        libctx = ossl_provider_libctx(prov);
    }

    if (pkey->keymgmt != NULL) {
        struct collected_encoder_st encoder_data;
        struct collected_names_st keymgmt_data;

        if ((data = OPENSSL_zalloc(sizeof(*data))) == NULL)
            goto err;

        /*
         * Select the first encoder implementations in two steps.
         * First, collect the keymgmt names, then the encoders that match.
         */
        keymgmt_data.names = sk_OPENSSL_CSTRING_new_null();
        if (keymgmt_data.names == NULL) {
            ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_CRYPTO_LIB);
            goto err;
        }

        keymgmt_data.error_occurred = 0;
        EVP_KEYMGMT_names_do_all(pkey->keymgmt, collect_name, &keymgmt_data);
        if (keymgmt_data.error_occurred) {
            sk_OPENSSL_CSTRING_free(keymgmt_data.names);
            goto err;
        }

        encoder_data.names = keymgmt_data.names;
        encoder_data.output_type = ctx->output_type;
        encoder_data.output_structure = ctx->output_structure;
        encoder_data.error_occurred = 0;
        encoder_data.keymgmt_prov = prov;
        encoder_data.ctx = ctx;
        encoder_data.id_names = NULL;

        /*
         * collect_encoder() is called many times, and for every call it converts all encoder_data.names
         * into namemap ids if it calls OSSL_ENCODER_is_a(). We cache the ids here instead,
         * and can use them for encoders with the same provider as the keymgmt.
         */
        namemap = ossl_namemap_stored(libctx);
        end = sk_OPENSSL_CSTRING_num(encoder_data.names);
        if (end > 0) {
            encoder_data.id_names = OPENSSL_malloc(end * sizeof(int));
            if (encoder_data.id_names == NULL) {
                sk_OPENSSL_CSTRING_free(keymgmt_data.names);
                goto err;
            }
            for (i = 0; i < end; ++i) {
                const char *name = sk_OPENSSL_CSTRING_value(keymgmt_data.names, i);

                encoder_data.id_names[i] = ossl_namemap_name2num(namemap, name);
            }
        }
        /*
         * Place the encoders with the a different provider as the keymgmt
         * last (the chain is processed in reverse order)
         */
        encoder_data.flag_find_same_provider = 0;
        OSSL_ENCODER_do_all_provided(libctx, collect_encoder, &encoder_data);

        /*
         * Place the encoders with the same provider as the keymgmt first
         * (the chain is processed in reverse order)
         */
        encoder_data.flag_find_same_provider = 1;
        OSSL_ENCODER_do_all_provided(libctx, collect_encoder, &encoder_data);

        OPENSSL_free(encoder_data.id_names);
        sk_OPENSSL_CSTRING_free(keymgmt_data.names);
        if (encoder_data.error_occurred) {
            ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    if (data != NULL && OSSL_ENCODER_CTX_get_num_encoders(ctx) != 0) {
        if (!OSSL_ENCODER_CTX_set_construct(ctx, encoder_construct_pkey)
            || !OSSL_ENCODER_CTX_set_construct_data(ctx, data)
            || !OSSL_ENCODER_CTX_set_cleanup(ctx, encoder_destruct_pkey))
            goto err;

        data->pk = pkey;
        data->selection = selection;

        data = NULL;             /* Avoid it being freed */
    }

    ok = 1;
 err:
    if (data != NULL) {
        OSSL_ENCODER_CTX_set_construct_data(ctx, NULL);
        OPENSSL_free(data);
    }
    return ok;
}

OSSL_ENCODER_CTX *OSSL_ENCODER_CTX_new_for_pkey(const EVP_PKEY *pkey,
                                                int selection,
                                                const char *output_type,
                                                const char *output_struct,
                                                const char *propquery)
{
    OSSL_ENCODER_CTX *ctx = NULL;
    OSSL_LIB_CTX *libctx = NULL;

    if (pkey == NULL) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if (!evp_pkey_is_assigned(pkey)) {
        ERR_raise_data(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_INVALID_ARGUMENT,
                       "The passed EVP_PKEY must be assigned a key");
        return NULL;
    }

    if ((ctx = OSSL_ENCODER_CTX_new()) == NULL) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_OSSL_ENCODER_LIB);
        return NULL;
    }

    if (evp_pkey_is_provided(pkey)) {
        const OSSL_PROVIDER *prov = EVP_KEYMGMT_get0_provider(pkey->keymgmt);

        libctx = ossl_provider_libctx(prov);
    }

    OSSL_TRACE_BEGIN(ENCODER) {
        BIO_printf(trc_out,
                   "(ctx %p) Looking for %s encoders with selection %d\n",
                   (void *)ctx, EVP_PKEY_get0_type_name(pkey), selection);
        BIO_printf(trc_out, "    output type: %s, output structure: %s\n",
                   output_type, output_struct);
    } OSSL_TRACE_END(ENCODER);

    if (OSSL_ENCODER_CTX_set_output_type(ctx, output_type)
        && (output_struct == NULL
            || OSSL_ENCODER_CTX_set_output_structure(ctx, output_struct))
        && OSSL_ENCODER_CTX_set_selection(ctx, selection)
        && ossl_encoder_ctx_setup_for_pkey(ctx, pkey, selection, propquery)
        && OSSL_ENCODER_CTX_add_extra(ctx, libctx, propquery)) {
        OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
        int save_parameters = pkey->save_parameters;

        params[0] = OSSL_PARAM_construct_int(OSSL_ENCODER_PARAM_SAVE_PARAMETERS,
                                             &save_parameters);
        /* ignoring error as this is only auxiliary parameter */
        (void)OSSL_ENCODER_CTX_set_params(ctx, params);

        OSSL_TRACE_BEGIN(ENCODER) {
            BIO_printf(trc_out, "(ctx %p) Got %d encoders\n",
                       (void *)ctx, OSSL_ENCODER_CTX_get_num_encoders(ctx));
        } OSSL_TRACE_END(ENCODER);
        return ctx;
    }

    OSSL_ENCODER_CTX_free(ctx);
    return NULL;
}
