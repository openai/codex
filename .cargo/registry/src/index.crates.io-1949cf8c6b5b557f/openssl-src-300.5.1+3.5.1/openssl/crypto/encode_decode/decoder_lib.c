/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_names.h>
#include <openssl/bio.h>
#include <openssl/params.h>
#include <openssl/provider.h>
#include <openssl/evperr.h>
#include <openssl/ecerr.h>
#include <openssl/pkcs12err.h>
#include <openssl/x509err.h>
#include <openssl/trace.h>
#include "internal/bio.h"
#include "internal/provider.h"
#include "internal/namemap.h"
#include "crypto/decoder.h"
#include "encoder_local.h"
#include "internal/e_os.h"

struct decoder_process_data_st {
    OSSL_DECODER_CTX *ctx;

    /* Current BIO */
    BIO *bio;

    /* Index of the current decoder instance to be processed */
    size_t current_decoder_inst_index;
    /* For tracing, count recursion level */
    size_t recursion;

    /*-
     * Flags
     */
    unsigned int flag_next_level_called : 1;
    unsigned int flag_construct_called : 1;
    unsigned int flag_input_structure_checked : 1;
};

static int decoder_process(const OSSL_PARAM params[], void *arg);

int OSSL_DECODER_from_bio(OSSL_DECODER_CTX *ctx, BIO *in)
{
    struct decoder_process_data_st data;
    int ok = 0;
    BIO *new_bio = NULL;
    unsigned long lasterr;

    if (in == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (OSSL_DECODER_CTX_get_num_decoders(ctx) == 0) {
        ERR_raise_data(ERR_LIB_OSSL_DECODER, OSSL_DECODER_R_DECODER_NOT_FOUND,
                       "No decoders were found. For standard decoders you need "
                       "at least one of the default or base providers "
                       "available. Did you forget to load them?");
        return 0;
    }

    lasterr = ERR_peek_last_error();

    if (BIO_tell(in) < 0) {
        new_bio = BIO_new(BIO_f_readbuffer());
        if (new_bio == NULL)
            return 0;
        in = BIO_push(new_bio, in);
    }
    memset(&data, 0, sizeof(data));
    data.ctx = ctx;
    data.bio = in;

    /* Enable passphrase caching */
    (void)ossl_pw_enable_passphrase_caching(&ctx->pwdata);

    ok = decoder_process(NULL, &data);

    if (!data.flag_construct_called) {
        const char *spaces
            = ctx->start_input_type != NULL && ctx->input_structure != NULL
            ? " " : "";
        const char *input_type_label
            = ctx->start_input_type != NULL ? "Input type: " : "";
        const char *input_structure_label
            = ctx->input_structure != NULL ? "Input structure: " : "";
        const char *comma
            = ctx->start_input_type != NULL && ctx->input_structure != NULL
            ? ", " : "";
        const char *input_type
            = ctx->start_input_type != NULL ? ctx->start_input_type : "";
        const char *input_structure
            = ctx->input_structure != NULL ? ctx->input_structure : "";

        if (ERR_peek_last_error() == lasterr || ERR_peek_error() == 0)
            /* Prevent spurious decoding error but add at least something */
            ERR_raise_data(ERR_LIB_OSSL_DECODER, ERR_R_UNSUPPORTED,
                           "No supported data to decode. %s%s%s%s%s%s",
                           spaces, input_type_label, input_type, comma,
                           input_structure_label, input_structure);
        ok = 0;
    }

    /* Clear any internally cached passphrase */
    (void)ossl_pw_clear_passphrase_cache(&ctx->pwdata);

    if (new_bio != NULL) {
        BIO_pop(new_bio);
        BIO_free(new_bio);
    }
    return ok;
}

#ifndef OPENSSL_NO_STDIO
static BIO *bio_from_file(FILE *fp)
{
    BIO *b;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_BIO_LIB);
        return NULL;
    }
    BIO_set_fp(b, fp, BIO_NOCLOSE);
    return b;
}

int OSSL_DECODER_from_fp(OSSL_DECODER_CTX *ctx, FILE *fp)
{
    BIO *b = bio_from_file(fp);
    int ret = 0;

    if (b != NULL)
        ret = OSSL_DECODER_from_bio(ctx, b);

    BIO_free(b);
    return ret;
}
#endif

int OSSL_DECODER_from_data(OSSL_DECODER_CTX *ctx, const unsigned char **pdata,
                           size_t *pdata_len)
{
    BIO *membio;
    int ret = 0;

    if (pdata == NULL || *pdata == NULL || pdata_len == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    membio = BIO_new_mem_buf(*pdata, (int)*pdata_len);
    if (OSSL_DECODER_from_bio(ctx, membio)) {
        *pdata_len = (size_t)BIO_get_mem_data(membio, pdata);
        ret = 1;
    }
    BIO_free(membio);

    return ret;
}

int OSSL_DECODER_CTX_set_selection(OSSL_DECODER_CTX *ctx, int selection)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /*
     * 0 is a valid selection, and means that the caller leaves
     * it to code to discover what the selection is.
     */
    ctx->selection = selection;
    return 1;
}

int OSSL_DECODER_CTX_set_input_type(OSSL_DECODER_CTX *ctx,
                                    const char *input_type)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /*
     * NULL is a valid starting input type, and means that the caller leaves
     * it to code to discover what the starting input type is.
     */
    ctx->start_input_type = input_type;
    return 1;
}

int OSSL_DECODER_CTX_set_input_structure(OSSL_DECODER_CTX *ctx,
                                         const char *input_structure)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /*
     * NULL is a valid starting input structure, and means that the caller
     * leaves it to code to discover what the starting input structure is.
     */
    ctx->input_structure = input_structure;
    return 1;
}

OSSL_DECODER_INSTANCE *
ossl_decoder_instance_new_forprov(OSSL_DECODER *decoder, void *provctx,
                                  const char *input_structure)
{
    void *decoderctx;

    if (!ossl_assert(decoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    decoderctx = decoder->newctx(provctx);
    if (decoderctx == NULL)
        return 0;
    if (input_structure != NULL && decoder->set_ctx_params != NULL) {
        OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END };

        params[0] =
            OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_STRUCTURE,
                                             (char *)input_structure, 0);
        if (!decoder->set_ctx_params(decoderctx, params)) {
            decoder->freectx(decoderctx);
            return 0;
        }
    }
    return ossl_decoder_instance_new(decoder, decoderctx);
}

OSSL_DECODER_INSTANCE *ossl_decoder_instance_new(OSSL_DECODER *decoder,
                                                 void *decoderctx)
{
    OSSL_DECODER_INSTANCE *decoder_inst = NULL;
    const OSSL_PROVIDER *prov;
    OSSL_LIB_CTX *libctx;
    const OSSL_PROPERTY_LIST *props;
    const OSSL_PROPERTY_DEFINITION *prop;

    if (!ossl_assert(decoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if ((decoder_inst = OPENSSL_zalloc(sizeof(*decoder_inst))) == NULL)
        return 0;

    prov = OSSL_DECODER_get0_provider(decoder);
    libctx = ossl_provider_libctx(prov);
    props = ossl_decoder_parsed_properties(decoder);
    if (props == NULL) {
        ERR_raise_data(ERR_LIB_OSSL_DECODER, ERR_R_INVALID_PROPERTY_DEFINITION,
                       "there are no property definitions with decoder %s",
                       OSSL_DECODER_get0_name(decoder));
        goto err;
    }

    /* The "input" property is mandatory */
    prop = ossl_property_find_property(props, libctx, "input");
    decoder_inst->input_type = ossl_property_get_string_value(libctx, prop);
    decoder_inst->input_type_id = 0;
    if (decoder_inst->input_type == NULL) {
        ERR_raise_data(ERR_LIB_OSSL_DECODER, ERR_R_INVALID_PROPERTY_DEFINITION,
                       "the mandatory 'input' property is missing "
                       "for decoder %s (properties: %s)",
                       OSSL_DECODER_get0_name(decoder),
                       OSSL_DECODER_get0_properties(decoder));
        goto err;
    }

    /* The "structure" property is optional */
    prop = ossl_property_find_property(props, libctx, "structure");
    if (prop != NULL) {
        decoder_inst->input_structure
            = ossl_property_get_string_value(libctx, prop);
    }

    if (!OSSL_DECODER_up_ref(decoder)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    decoder_inst->decoder = decoder;
    decoder_inst->decoderctx = decoderctx;
    return decoder_inst;
 err:
    ossl_decoder_instance_free(decoder_inst);
    return NULL;
}

void ossl_decoder_instance_free(OSSL_DECODER_INSTANCE *decoder_inst)
{
    if (decoder_inst != NULL) {
        if (decoder_inst->decoder != NULL)
            decoder_inst->decoder->freectx(decoder_inst->decoderctx);
        decoder_inst->decoderctx = NULL;
        OSSL_DECODER_free(decoder_inst->decoder);
        decoder_inst->decoder = NULL;
        OPENSSL_free(decoder_inst);
    }
}

OSSL_DECODER_INSTANCE *ossl_decoder_instance_dup(const OSSL_DECODER_INSTANCE *src)
{
    OSSL_DECODER_INSTANCE *dest;
    const OSSL_PROVIDER *prov;
    void *provctx;

    if ((dest = OPENSSL_zalloc(sizeof(*dest))) == NULL)
        return NULL;

    *dest = *src;
    if (!OSSL_DECODER_up_ref(dest->decoder)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    prov = OSSL_DECODER_get0_provider(dest->decoder);
    provctx = OSSL_PROVIDER_get0_provider_ctx(prov);

    dest->decoderctx = dest->decoder->newctx(provctx);
    if (dest->decoderctx == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_INTERNAL_ERROR);
        OSSL_DECODER_free(dest->decoder);
        goto err;
    }

    return dest;

 err:
    OPENSSL_free(dest);
    return NULL;
}

void ossl_decoder_ctx_set_harderr(OSSL_DECODER_CTX *ctx)
{
    ctx->harderr = 1;
}

int ossl_decoder_ctx_get_harderr(const OSSL_DECODER_CTX *ctx)
{
    return ctx->harderr;
}

int ossl_decoder_ctx_add_decoder_inst(OSSL_DECODER_CTX *ctx,
                                      OSSL_DECODER_INSTANCE *di)
{
    int ok;

    if (ctx->decoder_insts == NULL
        && (ctx->decoder_insts =
            sk_OSSL_DECODER_INSTANCE_new_null()) == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_CRYPTO_LIB);
        return 0;
    }

    ok = (sk_OSSL_DECODER_INSTANCE_push(ctx->decoder_insts, di) > 0);
    if (ok) {
        OSSL_TRACE_BEGIN(DECODER) {
            BIO_printf(trc_out,
                       "(ctx %p) Added decoder instance %p for decoder %p\n"
                       "    %s with %s\n",
                       (void *)ctx, (void *)di, (void *)di->decoder,
                       OSSL_DECODER_get0_name(di->decoder),
                       OSSL_DECODER_get0_properties(di->decoder));
        } OSSL_TRACE_END(DECODER);
    }
    return ok;
}

int OSSL_DECODER_CTX_add_decoder(OSSL_DECODER_CTX *ctx, OSSL_DECODER *decoder)
{
    OSSL_DECODER_INSTANCE *decoder_inst = NULL;
    const OSSL_PROVIDER *prov = NULL;
    void *decoderctx = NULL;
    void *provctx = NULL;

    if (!ossl_assert(ctx != NULL) || !ossl_assert(decoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    prov = OSSL_DECODER_get0_provider(decoder);
    provctx = OSSL_PROVIDER_get0_provider_ctx(prov);

    if ((decoderctx = decoder->newctx(provctx)) == NULL
        || (decoder_inst =
            ossl_decoder_instance_new(decoder, decoderctx)) == NULL)
        goto err;
    /* Avoid double free of decoderctx on further errors */
    decoderctx = NULL;

    if (!ossl_decoder_ctx_add_decoder_inst(ctx, decoder_inst))
        goto err;

    return 1;
 err:
    ossl_decoder_instance_free(decoder_inst);
    if (decoderctx != NULL)
        decoder->freectx(decoderctx);
    return 0;
}

struct collect_extra_decoder_data_st {
    OSSL_DECODER_CTX *ctx;
    const char *output_type;
    int output_type_id;

    /*
     * 0 to check that the decoder's input type is the same as the decoder name
     * 1 to check that the decoder's input type differs from the decoder name
     */
    enum { IS_SAME = 0, IS_DIFFERENT = 1 } type_check;
    size_t w_prev_start, w_prev_end; /* "previous" decoders */
    size_t w_new_start, w_new_end;   /* "new" decoders */
};

DEFINE_STACK_OF(OSSL_DECODER)

static void collect_all_decoders(OSSL_DECODER *decoder, void *arg)
{
    STACK_OF(OSSL_DECODER) *skdecoders = arg;

    if (OSSL_DECODER_up_ref(decoder)
            && !sk_OSSL_DECODER_push(skdecoders, decoder))
        OSSL_DECODER_free(decoder);
}

static void collect_extra_decoder(OSSL_DECODER *decoder, void *arg)
{
    struct collect_extra_decoder_data_st *data = arg;
    size_t j;
    const OSSL_PROVIDER *prov = OSSL_DECODER_get0_provider(decoder);
    void *provctx = OSSL_PROVIDER_get0_provider_ctx(prov);

    if (ossl_decoder_fast_is_a(decoder, data->output_type, &data->output_type_id)) {
        void *decoderctx = NULL;
        OSSL_DECODER_INSTANCE *di = NULL;

        OSSL_TRACE_BEGIN(DECODER) {
            BIO_printf(trc_out,
                       "(ctx %p) [%d] Checking out decoder %p:\n"
                       "    %s with %s\n",
                       (void *)data->ctx, data->type_check, (void *)decoder,
                       OSSL_DECODER_get0_name(decoder),
                       OSSL_DECODER_get0_properties(decoder));
        } OSSL_TRACE_END(DECODER);

        /*
         * Check that we don't already have this decoder in our stack,
         * starting with the previous windows but also looking at what
         * we have added in the current window.
         */
        for (j = data->w_prev_start; j < data->w_new_end; j++) {
            OSSL_DECODER_INSTANCE *check_inst =
                sk_OSSL_DECODER_INSTANCE_value(data->ctx->decoder_insts, j);

            if (decoder->base.algodef == check_inst->decoder->base.algodef) {
                /* We found it, so don't do anything more */
                OSSL_TRACE_BEGIN(DECODER) {
                    BIO_printf(trc_out,
                               "    REJECTED: already exists in the chain\n");
                } OSSL_TRACE_END(DECODER);
                return;
            }
        }

        if ((decoderctx = decoder->newctx(provctx)) == NULL)
            return;

        if (decoder->set_ctx_params != NULL
            && data->ctx->input_structure != NULL) {
            OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END };
            const char *str = data->ctx->input_structure;

            params[0] =
                OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_STRUCTURE,
                                                 (char *)str, 0);
            if (!decoder->set_ctx_params(decoderctx, params)) {
                decoder->freectx(decoderctx);
                return;
            }
        }

        if ((di = ossl_decoder_instance_new(decoder, decoderctx)) == NULL) {
            decoder->freectx(decoderctx);
            return;
        }

        switch (data->type_check) {
        case IS_SAME:
            /* If it differs, this is not a decoder to add for now. */
            if (!ossl_decoder_fast_is_a(decoder,
                                        OSSL_DECODER_INSTANCE_get_input_type(di),
                                        &di->input_type_id)) {
                ossl_decoder_instance_free(di);
                OSSL_TRACE_BEGIN(DECODER) {
                    BIO_printf(trc_out,
                               "    REJECTED: input type doesn't match output type\n");
                } OSSL_TRACE_END(DECODER);
                return;
            }
            break;
        case IS_DIFFERENT:
            /* If it's the same, this is not a decoder to add for now. */
            if (ossl_decoder_fast_is_a(decoder,
                                       OSSL_DECODER_INSTANCE_get_input_type(di),
                                       &di->input_type_id)) {
                ossl_decoder_instance_free(di);
                OSSL_TRACE_BEGIN(DECODER) {
                    BIO_printf(trc_out,
                               "    REJECTED: input type matches output type\n");
                } OSSL_TRACE_END(DECODER);
                return;
            }
            break;
        }

        /*
         * Apart from keeping w_new_end up to date, We don't care about
         * errors here.  If it doesn't collect, then it doesn't...
         */
        if (!ossl_decoder_ctx_add_decoder_inst(data->ctx, di)) {
            ossl_decoder_instance_free(di);
            return;
        }

        data->w_new_end++;
    }
}

int OSSL_DECODER_CTX_add_extra(OSSL_DECODER_CTX *ctx,
                               OSSL_LIB_CTX *libctx, const char *propq)
{
    /*
     * This function goes through existing decoder methods in
     * |ctx->decoder_insts|, and tries to fetch new decoders that produce
     * what the existing ones want as input, and push those newly fetched
     * decoders on top of the same stack.
     * Then it does the same again, but looping over the newly fetched
     * decoders, until there are no more decoders to be fetched, or
     * when we have done this 10 times.
     *
     * we do this with sliding windows on the stack by keeping track of indexes
     * and of the end.
     *
     * +----------------+
     * |   DER to RSA   | <--- w_prev_start
     * +----------------+
     * |   DER to DSA   |
     * +----------------+
     * |   DER to DH    |
     * +----------------+
     * |   PEM to DER   | <--- w_prev_end, w_new_start
     * +----------------+
     *                    <--- w_new_end
     */
    struct collect_extra_decoder_data_st data;
    size_t depth = 0; /* Counts the number of iterations */
    size_t count; /* Calculates how many were added in each iteration */
    size_t numdecoders;
    STACK_OF(OSSL_DECODER) *skdecoders;

    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /*
     * If there is no stack of OSSL_DECODER_INSTANCE, we have nothing
     * more to add.  That's fine.
     */
    if (ctx->decoder_insts == NULL)
        return 1;

    OSSL_TRACE_BEGIN(DECODER) {
        BIO_printf(trc_out, "(ctx %p) Looking for extra decoders\n",
                   (void *)ctx);
    } OSSL_TRACE_END(DECODER);


    skdecoders = sk_OSSL_DECODER_new_null();
    if (skdecoders == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_CRYPTO_LIB);
        return 0;
    }
    OSSL_DECODER_do_all_provided(libctx, collect_all_decoders, skdecoders);
    numdecoders = sk_OSSL_DECODER_num(skdecoders);

    memset(&data, 0, sizeof(data));
    data.ctx = ctx;
    data.w_prev_start = 0;
    data.w_prev_end = sk_OSSL_DECODER_INSTANCE_num(ctx->decoder_insts);
    do {
        size_t i, j;

        data.w_new_start = data.w_new_end = data.w_prev_end;

        /*
         * Two iterations:
         * 0.  All decoders that have the same name as their input type.
         *     This allows for decoders that unwrap some data in a specific
         *     encoding, and pass the result on with the same encoding.
         * 1.  All decoders that a different name than their input type.
         */
        for (data.type_check = IS_SAME;
             data.type_check <= IS_DIFFERENT;
             data.type_check++) {
            for (i = data.w_prev_start; i < data.w_prev_end; i++) {
                OSSL_DECODER_INSTANCE *decoder_inst =
                    sk_OSSL_DECODER_INSTANCE_value(ctx->decoder_insts, i);

                data.output_type
                    = OSSL_DECODER_INSTANCE_get_input_type(decoder_inst);

                data.output_type_id = 0;

                for (j = 0; j < numdecoders; j++)
                    collect_extra_decoder(sk_OSSL_DECODER_value(skdecoders, j),
                                          &data);
            }
        }
        /* How many were added in this iteration */
        count = data.w_new_end - data.w_new_start;

        /* Slide the "previous decoder" windows */
        data.w_prev_start = data.w_new_start;
        data.w_prev_end = data.w_new_end;

        depth++;
    } while (count != 0 && depth <= 10);

    sk_OSSL_DECODER_pop_free(skdecoders, OSSL_DECODER_free);
    return 1;
}

int OSSL_DECODER_CTX_get_num_decoders(OSSL_DECODER_CTX *ctx)
{
    if (ctx == NULL || ctx->decoder_insts == NULL)
        return 0;
    return sk_OSSL_DECODER_INSTANCE_num(ctx->decoder_insts);
}

int OSSL_DECODER_CTX_set_construct(OSSL_DECODER_CTX *ctx,
                                   OSSL_DECODER_CONSTRUCT *construct)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ctx->construct = construct;
    return 1;
}

int OSSL_DECODER_CTX_set_construct_data(OSSL_DECODER_CTX *ctx,
                                        void *construct_data)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ctx->construct_data = construct_data;
    return 1;
}

int OSSL_DECODER_CTX_set_cleanup(OSSL_DECODER_CTX *ctx,
                                 OSSL_DECODER_CLEANUP *cleanup)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ctx->cleanup = cleanup;
    return 1;
}

OSSL_DECODER_CONSTRUCT *
OSSL_DECODER_CTX_get_construct(OSSL_DECODER_CTX *ctx)
{
    if (ctx == NULL)
        return NULL;
    return ctx->construct;
}

void *OSSL_DECODER_CTX_get_construct_data(OSSL_DECODER_CTX *ctx)
{
    if (ctx == NULL)
        return NULL;
    return ctx->construct_data;
}

OSSL_DECODER_CLEANUP *
OSSL_DECODER_CTX_get_cleanup(OSSL_DECODER_CTX *ctx)
{
    if (ctx == NULL)
        return NULL;
    return ctx->cleanup;
}

int OSSL_DECODER_export(OSSL_DECODER_INSTANCE *decoder_inst,
                        void *reference, size_t reference_sz,
                        OSSL_CALLBACK *export_cb, void *export_cbarg)
{
    OSSL_DECODER *decoder = NULL;
    void *decoderctx = NULL;

    if (!(ossl_assert(decoder_inst != NULL)
          && ossl_assert(reference != NULL)
          && ossl_assert(export_cb != NULL)
          && ossl_assert(export_cbarg != NULL))) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    decoder = OSSL_DECODER_INSTANCE_get_decoder(decoder_inst);
    decoderctx = OSSL_DECODER_INSTANCE_get_decoder_ctx(decoder_inst);
    return decoder->export_object(decoderctx, reference, reference_sz,
                                  export_cb, export_cbarg);
}

OSSL_DECODER *
OSSL_DECODER_INSTANCE_get_decoder(OSSL_DECODER_INSTANCE *decoder_inst)
{
    if (decoder_inst == NULL)
        return NULL;
    return decoder_inst->decoder;
}

void *
OSSL_DECODER_INSTANCE_get_decoder_ctx(OSSL_DECODER_INSTANCE *decoder_inst)
{
    if (decoder_inst == NULL)
        return NULL;
    return decoder_inst->decoderctx;
}

const char *
OSSL_DECODER_INSTANCE_get_input_type(OSSL_DECODER_INSTANCE *decoder_inst)
{
    if (decoder_inst == NULL)
        return NULL;
    return decoder_inst->input_type;
}

const char *
OSSL_DECODER_INSTANCE_get_input_structure(OSSL_DECODER_INSTANCE *decoder_inst,
                                          int *was_set)
{
    if (decoder_inst == NULL)
        return NULL;
    *was_set = decoder_inst->flag_input_structure_was_set;
    return decoder_inst->input_structure;
}

static int decoder_process(const OSSL_PARAM params[], void *arg)
{
    struct decoder_process_data_st *data = arg;
    OSSL_DECODER_CTX *ctx = data->ctx;
    OSSL_DECODER_INSTANCE *decoder_inst = NULL;
    OSSL_DECODER *decoder = NULL;
    OSSL_CORE_BIO *cbio = NULL;
    BIO *bio = data->bio;
    long loc;
    size_t i;
    int ok = 0;
    /* For recursions */
    struct decoder_process_data_st new_data;
    const char *data_type = NULL;
    const char *data_structure = NULL;
    /* Saved to restore on return, mutated in PEM->DER transition. */
    const char *start_input_type = ctx->start_input_type;

    /*
     * This is an indicator up the call stack that something was indeed
     * decoded, leading to a recursive call of this function.
     */
    data->flag_next_level_called = 1;

    memset(&new_data, 0, sizeof(new_data));
    new_data.ctx = data->ctx;
    new_data.recursion = data->recursion + 1;

#define LEVEL_STR ">>>>>>>>>>>>>>>>"
#define LEVEL (new_data.recursion < sizeof(LEVEL_STR)                   \
               ? &LEVEL_STR[sizeof(LEVEL_STR) - new_data.recursion - 1] \
               : LEVEL_STR "...")

    if (params == NULL) {
        /* First iteration, where we prepare for what is to come */

        OSSL_TRACE_BEGIN(DECODER) {
            BIO_printf(trc_out,
                       "(ctx %p) starting to walk the decoder chain\n",
                       (void *)new_data.ctx);
        } OSSL_TRACE_END(DECODER);

        data->current_decoder_inst_index =
            OSSL_DECODER_CTX_get_num_decoders(ctx);

        bio = data->bio;
    } else {
        const OSSL_PARAM *p;
        const char *trace_data_structure;

        decoder_inst =
            sk_OSSL_DECODER_INSTANCE_value(ctx->decoder_insts,
                                           data->current_decoder_inst_index);
        decoder = OSSL_DECODER_INSTANCE_get_decoder(decoder_inst);

        data->flag_construct_called = 0;
        if (ctx->construct != NULL) {
            int rv;

            OSSL_TRACE_BEGIN(DECODER) {
                BIO_printf(trc_out,
                           "(ctx %p) %s Running constructor\n",
                           (void *)new_data.ctx, LEVEL);
            } OSSL_TRACE_END(DECODER);

            rv = ctx->construct(decoder_inst, params, ctx->construct_data);

            OSSL_TRACE_BEGIN(DECODER) {
                BIO_printf(trc_out,
                           "(ctx %p) %s Running constructor => %d\n",
                           (void *)new_data.ctx, LEVEL, rv);
            } OSSL_TRACE_END(DECODER);

            ok = (rv > 0);
            if (ok) {
                data->flag_construct_called = 1;
                goto end;
            }
        }

        /* The constructor didn't return success */

        /*
         * so we try to use the object we got and feed it to any next
         * decoder that will take it.  Object references are not
         * allowed for this.
         * If this data isn't present, decoding has failed.
         */

        p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DATA);
        if (p == NULL || p->data_type != OSSL_PARAM_OCTET_STRING)
            goto end;
        new_data.bio = BIO_new_mem_buf(p->data, (int)p->data_size);
        if (new_data.bio == NULL)
            goto end;
        bio = new_data.bio;

        /* Get the data type if there is one */
        p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DATA_TYPE);
        if (p != NULL && !OSSL_PARAM_get_utf8_string_ptr(p, &data_type))
            goto end;

        /* Get the data structure if there is one */
        p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DATA_STRUCTURE);
        if (p != NULL && !OSSL_PARAM_get_utf8_string_ptr(p, &data_structure))
            goto end;

        /* Get the new input type if there is one */
        p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_INPUT_TYPE);
        if (p != NULL) {
            if (!OSSL_PARAM_get_utf8_string_ptr(p, &ctx->start_input_type))
                goto end;
            /*
             * When switching PKCS8 from PEM to DER we decrypt the data if needed
             * and then determine the algorithm OID.  Likewise, with SPKI, only
             * this time sans decryption.
             */
            if (ctx->input_structure != NULL
                && (OPENSSL_strcasecmp(ctx->input_structure, "SubjectPublicKeyInfo") == 0
                    || OPENSSL_strcasecmp(data_structure, "PrivateKeyInfo") == 0
                    || OPENSSL_strcasecmp(ctx->input_structure, "PrivateKeyInfo") == 0))
                data->flag_input_structure_checked = 1;
        }

        /*
         * If the data structure is "type-specific" and the data type is
         * given, we drop the data structure.  The reasoning is that the
         * data type is already enough to find the applicable next decoder,
         * so an additional "type-specific" data structure is extraneous.
         *
         * Furthermore, if the OSSL_DECODER caller asked for a type specific
         * structure under another name, such as "DH", we get a mismatch
         * if the data structure we just received is "type-specific".
         * There's only so much you can do without infusing this code with
         * too special knowledge.
         */
        trace_data_structure = data_structure;
        if (data_type != NULL && data_structure != NULL
            && OPENSSL_strcasecmp(data_structure, "type-specific") == 0)
            data_structure = NULL;

        OSSL_TRACE_BEGIN(DECODER) {
            BIO_printf(trc_out,
                       "(ctx %p) %s incoming from previous decoder (%p):\n"
                       "    data type: %s, data structure: %s%s\n",
                       (void *)new_data.ctx, LEVEL, (void *)decoder,
                       data_type, trace_data_structure,
                       (trace_data_structure == data_structure
                        ? "" : " (dropped)"));
        } OSSL_TRACE_END(DECODER);
    }

    /*
     * If we have no more decoders to look through at this point,
     * we failed
     */
    if (data->current_decoder_inst_index == 0)
        goto end;

    if ((loc = BIO_tell(bio)) < 0) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_BIO_LIB);
        goto end;
    }

    if ((cbio = ossl_core_bio_new_from_bio(bio)) == NULL) {
        ERR_raise(ERR_LIB_OSSL_DECODER, ERR_R_BIO_LIB);
        goto end;
    }

    for (i = data->current_decoder_inst_index; i-- > 0;) {
        OSSL_DECODER_INSTANCE *new_decoder_inst =
            sk_OSSL_DECODER_INSTANCE_value(ctx->decoder_insts, i);
        OSSL_DECODER *new_decoder =
            OSSL_DECODER_INSTANCE_get_decoder(new_decoder_inst);
        const char *new_decoder_name = NULL;
        void *new_decoderctx =
            OSSL_DECODER_INSTANCE_get_decoder_ctx(new_decoder_inst);
        const char *new_input_type =
            OSSL_DECODER_INSTANCE_get_input_type(new_decoder_inst);
        int n_i_s_was_set = 0;   /* We don't care here */
        const char *new_input_structure =
            OSSL_DECODER_INSTANCE_get_input_structure(new_decoder_inst,
                                                      &n_i_s_was_set);

        OSSL_TRACE_BEGIN(DECODER) {
            new_decoder_name = OSSL_DECODER_get0_name(new_decoder);
            BIO_printf(trc_out,
                       "(ctx %p) %s [%u] Considering decoder instance %p (decoder %p):\n"
                       "    %s with %s\n",
                       (void *)new_data.ctx, LEVEL, (unsigned int)i,
                       (void *)new_decoder_inst, (void *)new_decoder,
                       new_decoder_name,
                       OSSL_DECODER_get0_properties(new_decoder));
        } OSSL_TRACE_END(DECODER);

        /*
         * If |decoder| is NULL, it means we've just started, and the caller
         * may have specified what it expects the initial input to be.  If
         * that's the case, we do this extra check.
         */
        if (decoder == NULL && ctx->start_input_type != NULL
            && OPENSSL_strcasecmp(ctx->start_input_type, new_input_type) != 0) {
            OSSL_TRACE_BEGIN(DECODER) {
                BIO_printf(trc_out,
                           "(ctx %p) %s [%u] the start input type '%s' doesn't match the input type of the considered decoder, skipping...\n",
                           (void *)new_data.ctx, LEVEL, (unsigned int)i,
                           ctx->start_input_type);
            } OSSL_TRACE_END(DECODER);
            continue;
        }

        /*
         * If we have a previous decoder, we check that the input type
         * of the next to be used matches the type of this previous one.
         * |new_input_type| holds the value of the "input-type" parameter
         * for the decoder we're currently considering.
         */
        if (decoder != NULL && !ossl_decoder_fast_is_a(decoder, new_input_type,
                                                       &new_decoder_inst->input_type_id)) {
            OSSL_TRACE_BEGIN(DECODER) {
                BIO_printf(trc_out,
                           "(ctx %p) %s [%u] the input type doesn't match the name of the previous decoder (%p), skipping...\n",
                           (void *)new_data.ctx, LEVEL, (unsigned int)i,
                           (void *)decoder);
            } OSSL_TRACE_END(DECODER);
            continue;
        }

        /*
         * If the previous decoder gave us a data type, we check to see
         * if that matches the decoder we're currently considering.
         */
        if (data_type != NULL && !OSSL_DECODER_is_a(new_decoder, data_type)) {
            OSSL_TRACE_BEGIN(DECODER) {
                BIO_printf(trc_out,
                           "(ctx %p) %s [%u] the previous decoder's data type doesn't match the name of the considered decoder, skipping...\n",
                           (void *)new_data.ctx, LEVEL, (unsigned int)i);
            } OSSL_TRACE_END(DECODER);
            continue;
        }

        /*
         * If the previous decoder gave us a data structure name, we check
         * to see that it matches the input data structure of the decoder
         * we're currently considering.
         */
        if (data_structure != NULL
            && (new_input_structure == NULL
                || OPENSSL_strcasecmp(data_structure,
                                      new_input_structure) != 0)) {
            OSSL_TRACE_BEGIN(DECODER) {
                BIO_printf(trc_out,
                           "(ctx %p) %s [%u] the previous decoder's data structure doesn't match the input structure of the considered decoder, skipping...\n",
                           (void *)new_data.ctx, LEVEL, (unsigned int)i);
            } OSSL_TRACE_END(DECODER);
            continue;
        }

        /*
         * If the decoder we're currently considering specifies a structure,
         * and this check hasn't already been done earlier in this chain of
         * decoder_process() calls, check that it matches the user provided
         * input structure, if one is given.
         */
        if (!data->flag_input_structure_checked
            && ctx->input_structure != NULL
            && new_input_structure != NULL) {
            data->flag_input_structure_checked = 1;
            if (OPENSSL_strcasecmp(new_input_structure,
                                   ctx->input_structure) != 0) {
                OSSL_TRACE_BEGIN(DECODER) {
                    BIO_printf(trc_out,
                               "(ctx %p) %s [%u] the previous decoder's data structure doesn't match the input structure given by the user, skipping...\n",
                               (void *)new_data.ctx, LEVEL, (unsigned int)i);
                } OSSL_TRACE_END(DECODER);
                continue;
            }
        }

        /*
         * Checking the return value of BIO_reset() or BIO_seek() is unsafe.
         * Furthermore, BIO_reset() is unsafe to use if the source BIO happens
         * to be a BIO_s_mem(), because the earlier BIO_tell() gives us zero
         * no matter where we are in the underlying buffer we're reading from.
         *
         * So, we simply do a BIO_seek(), and use BIO_tell() that we're back
         * at the same position.  This is a best effort attempt, but BIO_seek()
         * and BIO_tell() should come as a pair...
         */
        (void)BIO_seek(bio, loc);
        if (BIO_tell(bio) != loc)
            goto end;

        /* Recurse */
        OSSL_TRACE_BEGIN(DECODER) {
            BIO_printf(trc_out,
                       "(ctx %p) %s [%u] Running decoder instance %s (%p)\n",
                       (void *)new_data.ctx, LEVEL, (unsigned int)i,
                       new_decoder_name, (void *)new_decoder_inst);
        } OSSL_TRACE_END(DECODER);

        /*
         * We only care about errors reported from decoder implementations
         * if it returns false (i.e. there was a fatal error).
         */
        ERR_set_mark();

        new_data.current_decoder_inst_index = i;
        new_data.flag_input_structure_checked
            = data->flag_input_structure_checked;
        ok = new_decoder->decode(new_decoderctx, cbio,
                                 new_data.ctx->selection,
                                 decoder_process, &new_data,
                                 ossl_pw_passphrase_callback_dec,
                                 &new_data.ctx->pwdata);

        OSSL_TRACE_BEGIN(DECODER) {
            BIO_printf(trc_out,
                       "(ctx %p) %s [%u] Running decoder instance %s (%p) => %d"
                       " (recursed further: %s, construct called: %s)\n",
                       (void *)new_data.ctx, LEVEL, (unsigned int)i,
                       new_decoder_name, (void *)new_decoder_inst, ok,
                       new_data.flag_next_level_called ? "yes" : "no",
                       new_data.flag_construct_called ? "yes" : "no");
        } OSSL_TRACE_END(DECODER);

        data->flag_construct_called = new_data.flag_construct_called;

        /* Break on error or if we tried to construct an object already */
        if (!ok || data->flag_construct_called) {
            ERR_clear_last_mark();
            break;
        }
        ERR_pop_to_mark();

        /*
         * Break if the decoder implementation that we called recursed, since
         * that indicates that it successfully decoded something.
         */
        if (new_data.flag_next_level_called)
            break;
    }

 end:
    ossl_core_bio_free(cbio);
    BIO_free(new_data.bio);
    ctx->start_input_type = start_input_type;
    return ok;
}
