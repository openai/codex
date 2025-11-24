/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <ctype.h>

#include <openssl/core_names.h>
#include <openssl/bio.h>
#include <openssl/encoder.h>
#include <openssl/buffer.h>
#include <openssl/params.h>
#include <openssl/provider.h>
#include <openssl/trace.h>
#include <crypto/bn.h>
#include "internal/bio.h"
#include "internal/ffc.h"
#include "internal/provider.h"
#include "internal/encoder.h"
#include "encoder_local.h"

/* Number of octets per line */
#define LABELED_BUF_PRINT_WIDTH    15

# ifdef SIXTY_FOUR_BIT_LONG
#  define BN_FMTu "%lu"
#  define BN_FMTx "%lx"
# endif

# ifdef SIXTY_FOUR_BIT
#  define BN_FMTu "%llu"
#  define BN_FMTx "%llx"
# endif

# ifdef THIRTY_TWO_BIT
#  define BN_FMTu "%u"
#  define BN_FMTx "%x"
# endif

struct encoder_process_data_st {
    OSSL_ENCODER_CTX *ctx;

    /* Current BIO */
    BIO *bio;

    /* Index of the current encoder instance to be processed */
    int current_encoder_inst_index;

    /* Processing data passed down through recursion */
    int level;                   /* Recursion level */
    OSSL_ENCODER_INSTANCE *next_encoder_inst;
    int count_output_structure;

    /* Processing data passed up through recursion */
    OSSL_ENCODER_INSTANCE *prev_encoder_inst;
    unsigned char *running_output;
    size_t running_output_length;
    /* Data type = the name of the first succeeding encoder implementation */
    const char *data_type;
};

static int encoder_process(struct encoder_process_data_st *data);

int OSSL_ENCODER_to_bio(OSSL_ENCODER_CTX *ctx, BIO *out)
{
    struct encoder_process_data_st data;

    memset(&data, 0, sizeof(data));
    data.ctx = ctx;
    data.bio = out;
    data.current_encoder_inst_index = OSSL_ENCODER_CTX_get_num_encoders(ctx);

    if (data.current_encoder_inst_index == 0) {
        ERR_raise_data(ERR_LIB_OSSL_ENCODER, OSSL_ENCODER_R_ENCODER_NOT_FOUND,
                       "No encoders were found. For standard encoders you need "
                       "at least one of the default or base providers "
                       "available. Did you forget to load them?");
        return 0;
    }

    if (ctx->cleanup == NULL || ctx->construct == NULL) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_INIT_FAIL);
        return 0;
    }

    return encoder_process(&data) > 0;
}

#ifndef OPENSSL_NO_STDIO
static BIO *bio_from_file(FILE *fp)
{
    BIO *b;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_BUF_LIB);
        return NULL;
    }
    BIO_set_fp(b, fp, BIO_NOCLOSE);
    return b;
}

int OSSL_ENCODER_to_fp(OSSL_ENCODER_CTX *ctx, FILE *fp)
{
    BIO *b = bio_from_file(fp);
    int ret = 0;

    if (b != NULL)
        ret = OSSL_ENCODER_to_bio(ctx, b);

    BIO_free(b);
    return ret;
}
#endif

int OSSL_ENCODER_to_data(OSSL_ENCODER_CTX *ctx, unsigned char **pdata,
                         size_t *pdata_len)
{
    BIO *out;
    BUF_MEM *buf = NULL;
    int ret = 0;

    if (pdata_len == NULL) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    out = BIO_new(BIO_s_mem());

    if (out != NULL
        && OSSL_ENCODER_to_bio(ctx, out)
        && BIO_get_mem_ptr(out, &buf) > 0) {
        ret = 1; /* Hope for the best. A too small buffer will clear this */

        if (pdata != NULL && *pdata != NULL) {
            if (*pdata_len < buf->length)
                /*
                 * It's tempting to do |*pdata_len = (size_t)buf->length|
                 * However, it's believed to be confusing more than helpful,
                 * so we don't.
                 */
                ret = 0;
            else
                *pdata_len -= buf->length;
        } else {
            /* The buffer with the right size is already allocated for us */
            *pdata_len = (size_t)buf->length;
        }

        if (ret) {
            if (pdata != NULL) {
                if (*pdata != NULL) {
                    memcpy(*pdata, buf->data, buf->length);
                    *pdata += buf->length;
                } else {
                    /* In this case, we steal the data from BIO_s_mem() */
                    *pdata = (unsigned char *)buf->data;
                    buf->data = NULL;
                }
            }
        }
    }
    BIO_free(out);
    return ret;
}

int OSSL_ENCODER_CTX_set_selection(OSSL_ENCODER_CTX *ctx, int selection)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (!ossl_assert(selection != 0)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    ctx->selection = selection;
    return 1;
}

int OSSL_ENCODER_CTX_set_output_type(OSSL_ENCODER_CTX *ctx,
                                     const char *output_type)
{
    if (!ossl_assert(ctx != NULL) || !ossl_assert(output_type != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    ctx->output_type = output_type;
    return 1;
}

int OSSL_ENCODER_CTX_set_output_structure(OSSL_ENCODER_CTX *ctx,
                                          const char *output_structure)
{
    if (!ossl_assert(ctx != NULL) || !ossl_assert(output_structure != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    ctx->output_structure = output_structure;
    return 1;
}

static OSSL_ENCODER_INSTANCE *ossl_encoder_instance_new(OSSL_ENCODER *encoder,
                                                        void *encoderctx)
{
    OSSL_ENCODER_INSTANCE *encoder_inst = NULL;
    const OSSL_PROVIDER *prov;
    OSSL_LIB_CTX *libctx;
    const OSSL_PROPERTY_LIST *props;
    const OSSL_PROPERTY_DEFINITION *prop;

    if (!ossl_assert(encoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if ((encoder_inst = OPENSSL_zalloc(sizeof(*encoder_inst))) == NULL)
        return 0;

    if (!OSSL_ENCODER_up_ref(encoder)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    prov = OSSL_ENCODER_get0_provider(encoder);
    libctx = ossl_provider_libctx(prov);
    props = ossl_encoder_parsed_properties(encoder);
    if (props == NULL) {
        ERR_raise_data(ERR_LIB_OSSL_DECODER, ERR_R_INVALID_PROPERTY_DEFINITION,
                       "there are no property definitions with encoder %s",
                       OSSL_ENCODER_get0_name(encoder));
        goto err;
    }

    /* The "output" property is mandatory */
    prop = ossl_property_find_property(props, libctx, "output");
    encoder_inst->output_type = ossl_property_get_string_value(libctx, prop);
    if (encoder_inst->output_type == NULL) {
        ERR_raise_data(ERR_LIB_OSSL_DECODER, ERR_R_INVALID_PROPERTY_DEFINITION,
                       "the mandatory 'output' property is missing "
                       "for encoder %s (properties: %s)",
                       OSSL_ENCODER_get0_name(encoder),
                       OSSL_ENCODER_get0_properties(encoder));
        goto err;
    }

    /* The "structure" property is optional */
    prop = ossl_property_find_property(props, libctx, "structure");
    if (prop != NULL)
        encoder_inst->output_structure
            = ossl_property_get_string_value(libctx, prop);

    encoder_inst->encoder = encoder;
    encoder_inst->encoderctx = encoderctx;
    return encoder_inst;
 err:
    ossl_encoder_instance_free(encoder_inst);
    return NULL;
}

void ossl_encoder_instance_free(OSSL_ENCODER_INSTANCE *encoder_inst)
{
    if (encoder_inst != NULL) {
        if (encoder_inst->encoder != NULL)
            encoder_inst->encoder->freectx(encoder_inst->encoderctx);
        encoder_inst->encoderctx = NULL;
        OSSL_ENCODER_free(encoder_inst->encoder);
        encoder_inst->encoder = NULL;
        OPENSSL_free(encoder_inst);
    }
}

static int ossl_encoder_ctx_add_encoder_inst(OSSL_ENCODER_CTX *ctx,
                                             OSSL_ENCODER_INSTANCE *ei)
{
    int ok;

    if (ctx->encoder_insts == NULL
        && (ctx->encoder_insts =
            sk_OSSL_ENCODER_INSTANCE_new_null()) == NULL) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_CRYPTO_LIB);
        return 0;
    }

    ok = (sk_OSSL_ENCODER_INSTANCE_push(ctx->encoder_insts, ei) > 0);
    if (ok) {
        OSSL_TRACE_BEGIN(ENCODER) {
            BIO_printf(trc_out,
                       "(ctx %p) Added encoder instance %p (encoder %p):\n"
                       "    %s with %s\n",
                       (void *)ctx, (void *)ei, (void *)ei->encoder,
                       OSSL_ENCODER_get0_name(ei->encoder),
                       OSSL_ENCODER_get0_properties(ei->encoder));
        } OSSL_TRACE_END(ENCODER);
    }
    return ok;
}

int OSSL_ENCODER_CTX_add_encoder(OSSL_ENCODER_CTX *ctx, OSSL_ENCODER *encoder)
{
    OSSL_ENCODER_INSTANCE *encoder_inst = NULL;
    const OSSL_PROVIDER *prov = NULL;
    void *encoderctx = NULL;
    void *provctx = NULL;

    if (!ossl_assert(ctx != NULL) || !ossl_assert(encoder != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    prov = OSSL_ENCODER_get0_provider(encoder);
    provctx = OSSL_PROVIDER_get0_provider_ctx(prov);

    if ((encoderctx = encoder->newctx(provctx)) == NULL
        || (encoder_inst =
            ossl_encoder_instance_new(encoder, encoderctx)) == NULL)
        goto err;
    /* Avoid double free of encoderctx on further errors */
    encoderctx = NULL;

    if (!ossl_encoder_ctx_add_encoder_inst(ctx, encoder_inst))
        goto err;

    return 1;
 err:
    ossl_encoder_instance_free(encoder_inst);
    if (encoderctx != NULL)
        encoder->freectx(encoderctx);
    return 0;
}

int OSSL_ENCODER_CTX_add_extra(OSSL_ENCODER_CTX *ctx,
                               OSSL_LIB_CTX *libctx, const char *propq)
{
    return 1;
}

int OSSL_ENCODER_CTX_get_num_encoders(OSSL_ENCODER_CTX *ctx)
{
    if (ctx == NULL || ctx->encoder_insts == NULL)
        return 0;
    return sk_OSSL_ENCODER_INSTANCE_num(ctx->encoder_insts);
}

int OSSL_ENCODER_CTX_set_construct(OSSL_ENCODER_CTX *ctx,
                                   OSSL_ENCODER_CONSTRUCT *construct)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ctx->construct = construct;
    return 1;
}

int OSSL_ENCODER_CTX_set_construct_data(OSSL_ENCODER_CTX *ctx,
                                        void *construct_data)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ctx->construct_data = construct_data;
    return 1;
}

int OSSL_ENCODER_CTX_set_cleanup(OSSL_ENCODER_CTX *ctx,
                                 OSSL_ENCODER_CLEANUP *cleanup)
{
    if (!ossl_assert(ctx != NULL)) {
        ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ctx->cleanup = cleanup;
    return 1;
}

OSSL_ENCODER *
OSSL_ENCODER_INSTANCE_get_encoder(OSSL_ENCODER_INSTANCE *encoder_inst)
{
    if (encoder_inst == NULL)
        return NULL;
    return encoder_inst->encoder;
}

void *
OSSL_ENCODER_INSTANCE_get_encoder_ctx(OSSL_ENCODER_INSTANCE *encoder_inst)
{
    if (encoder_inst == NULL)
        return NULL;
    return encoder_inst->encoderctx;
}

const char *
OSSL_ENCODER_INSTANCE_get_output_type(OSSL_ENCODER_INSTANCE *encoder_inst)
{
    if (encoder_inst == NULL)
        return NULL;
    return encoder_inst->output_type;
}

const char *
OSSL_ENCODER_INSTANCE_get_output_structure(OSSL_ENCODER_INSTANCE *encoder_inst)
{
    if (encoder_inst == NULL)
        return NULL;
    return encoder_inst->output_structure;
}

static int encoder_process(struct encoder_process_data_st *data)
{
    OSSL_ENCODER_INSTANCE *current_encoder_inst = NULL;
    OSSL_ENCODER *current_encoder = NULL;
    OSSL_ENCODER_CTX *current_encoder_ctx = NULL;
    BIO *allocated_out = NULL;
    const void *original_data = NULL;
    OSSL_PARAM abstract[10];
    const OSSL_PARAM *current_abstract = NULL;
    int i;
    int ok = -1;  /* -1 signifies that the lookup loop gave nothing */
    int top = 0;

    if (data->next_encoder_inst == NULL) {
        /* First iteration, where we prepare for what is to come */

        data->count_output_structure =
            data->ctx->output_structure == NULL ? -1 : 0;
        top = 1;
    }

    for (i = data->current_encoder_inst_index; i-- > 0;) {
        OSSL_ENCODER *next_encoder = NULL;
        const char *current_output_type;
        const char *current_output_structure;
        struct encoder_process_data_st new_data;

        if (!top)
            next_encoder =
                OSSL_ENCODER_INSTANCE_get_encoder(data->next_encoder_inst);

        current_encoder_inst =
            sk_OSSL_ENCODER_INSTANCE_value(data->ctx->encoder_insts, i);
        current_encoder =
            OSSL_ENCODER_INSTANCE_get_encoder(current_encoder_inst);
        current_encoder_ctx =
            OSSL_ENCODER_INSTANCE_get_encoder_ctx(current_encoder_inst);
        current_output_type =
            OSSL_ENCODER_INSTANCE_get_output_type(current_encoder_inst);
        current_output_structure =
            OSSL_ENCODER_INSTANCE_get_output_structure(current_encoder_inst);
        memset(&new_data, 0, sizeof(new_data));
        new_data.ctx = data->ctx;
        new_data.current_encoder_inst_index = i;
        new_data.next_encoder_inst = current_encoder_inst;
        new_data.count_output_structure = data->count_output_structure;
        new_data.level = data->level + 1;

        OSSL_TRACE_BEGIN(ENCODER) {
            BIO_printf(trc_out,
                       "[%d] (ctx %p) Considering encoder instance %p (encoder %p)\n",
                       data->level, (void *)data->ctx,
                       (void *)current_encoder_inst, (void *)current_encoder);
        } OSSL_TRACE_END(ENCODER);

        /*
         * If this is the top call, we check if the output type of the current
         * encoder matches the desired output type.
         * If this isn't the top call, i.e. this is deeper in the recursion,
         * we instead check if the output type of the current encoder matches
         * the name of the next encoder (the one found by the parent call).
         */
        if (top) {
            if (data->ctx->output_type != NULL
                && OPENSSL_strcasecmp(current_output_type,
                                      data->ctx->output_type) != 0) {
                OSSL_TRACE_BEGIN(ENCODER) {
                    BIO_printf(trc_out,
                               "[%d]    Skipping because current encoder output type (%s) != desired output type (%s)\n",
                               data->level,
                               current_output_type, data->ctx->output_type);
                } OSSL_TRACE_END(ENCODER);
                continue;
            }
        } else {
            if (!OSSL_ENCODER_is_a(next_encoder, current_output_type)) {
                OSSL_TRACE_BEGIN(ENCODER) {
                    BIO_printf(trc_out,
                               "[%d]    Skipping because current encoder output type (%s) != name of encoder %p\n",
                               data->level,
                               current_output_type, (void *)next_encoder);
                } OSSL_TRACE_END(ENCODER);
                continue;
            }
        }

        /*
         * If the caller and the current encoder specify an output structure,
         * Check if they match.  If they do, count the match, otherwise skip
         * the current encoder.
         */
        if (data->ctx->output_structure != NULL
            && current_output_structure != NULL) {
            if (OPENSSL_strcasecmp(data->ctx->output_structure,
                                   current_output_structure) != 0) {
                OSSL_TRACE_BEGIN(ENCODER) {
                    BIO_printf(trc_out,
                               "[%d]    Skipping because current encoder output structure (%s) != ctx output structure (%s)\n",
                               data->level,
                               current_output_structure,
                               data->ctx->output_structure);
                } OSSL_TRACE_END(ENCODER);
                continue;
            }

            data->count_output_structure++;
        }

        /*
         * Recurse to process the encoder implementations before the current
         * one.
         */
        ok = encoder_process(&new_data);

        data->prev_encoder_inst = new_data.prev_encoder_inst;
        data->running_output = new_data.running_output;
        data->running_output_length = new_data.running_output_length;

        /*
         * ok == -1     means that the recursion call above gave no further
         *              encoders, and that the one we're currently at should
         *              be tried.
         * ok == 0      means that something failed in the recursion call
         *              above, making the result unsuitable for a chain.
         *              In this case, we simply continue to try finding a
         *              suitable encoder at this recursion level.
         * ok == 1      means that the recursion call was successful, and we
         *              try to use the result at this recursion level.
         */
        if (ok != 0)
            break;

        OSSL_TRACE_BEGIN(ENCODER) {
            BIO_printf(trc_out,
                       "[%d]    Skipping because recursion level %d failed\n",
                       data->level, new_data.level);
        } OSSL_TRACE_END(ENCODER);
    }

    /*
     * If |i < 0|, we didn't find any useful encoder in this recursion, so
     * we do the rest of the process only if |i >= 0|.
     */
    if (i < 0) {
        ok = -1;

        OSSL_TRACE_BEGIN(ENCODER) {
            BIO_printf(trc_out,
                       "[%d] (ctx %p) No suitable encoder found\n",
                       data->level, (void *)data->ctx);
        } OSSL_TRACE_END(ENCODER);
    } else {
        /* Preparations */

        switch (ok) {
        case 0:
            break;
        case -1:
            /*
             * We have reached the beginning of the encoder instance sequence,
             * so we prepare the object to be encoded.
             */

            /*
             * |data->count_output_structure| is one of these values:
             *
             * -1       There is no desired output structure
             *  0       There is a desired output structure, and it wasn't
             *          matched by any of the encoder instances that were
             *          considered
             * >0       There is a desired output structure, and at least one
             *          of the encoder instances matched it
             */
            if (data->count_output_structure == 0)
                return 0;

            original_data =
                data->ctx->construct(current_encoder_inst,
                                     data->ctx->construct_data);

            /* Also set the data type, using the encoder implementation name */
            data->data_type = OSSL_ENCODER_get0_name(current_encoder);

            /* Assume that the constructor recorded an error */
            if (original_data != NULL)
                ok = 1;
            else
                ok = 0;
            break;
        case 1:
            if (!ossl_assert(data->running_output != NULL)) {
                ERR_raise(ERR_LIB_OSSL_ENCODER, ERR_R_INTERNAL_ERROR);
                ok = 0;
                break;
            }

            {
                /*
                 * Create an object abstraction from the latest output, which
                 * was stolen from the previous round.
                 */

                OSSL_PARAM *abstract_p = abstract;
                const char *prev_output_structure =
                    OSSL_ENCODER_INSTANCE_get_output_structure(data->prev_encoder_inst);

                *abstract_p++ =
                    OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_TYPE,
                                                     (char *)data->data_type, 0);
                if (prev_output_structure != NULL)
                    *abstract_p++ =
                        OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_STRUCTURE,
                                                         (char *)prev_output_structure,
                                                         0);
                *abstract_p++ =
                    OSSL_PARAM_construct_octet_string(OSSL_OBJECT_PARAM_DATA,
                                                      data->running_output,
                                                      data->running_output_length);
                *abstract_p = OSSL_PARAM_construct_end();
                current_abstract = abstract;
            }
            break;
        }

        /* Calling the encoder implementation */

        if (ok) {
            OSSL_CORE_BIO *cbio = NULL;
            BIO *current_out = NULL;

            /*
             * If we're at the last encoder instance to use, we're setting up
             * final output.  Otherwise, set up an intermediary memory output.
             */
            if (top)
                current_out = data->bio;
            else if ((current_out = allocated_out = BIO_new(BIO_s_mem()))
                     == NULL)
                ok = 0;     /* Assume BIO_new() recorded an error */

            if (ok)
                ok = (cbio = ossl_core_bio_new_from_bio(current_out)) != NULL;
            if (ok) {
                ok = current_encoder->encode(current_encoder_ctx, cbio,
                                             original_data, current_abstract,
                                             data->ctx->selection,
                                             ossl_pw_passphrase_callback_enc,
                                             &data->ctx->pwdata);
                OSSL_TRACE_BEGIN(ENCODER) {
                    BIO_printf(trc_out,
                               "[%d] (ctx %p) Running encoder instance %p => %d\n",
                               data->level, (void *)data->ctx,
                               (void *)current_encoder_inst, ok);
                } OSSL_TRACE_END(ENCODER);
            }

            ossl_core_bio_free(cbio);
            data->prev_encoder_inst = current_encoder_inst;
        }
    }

    /* Cleanup and collecting the result */

    OPENSSL_free(data->running_output);
    data->running_output = NULL;

    /*
     * Steal the output from the BIO_s_mem, if we did allocate one.
     * That'll be the data for an object abstraction in the next round.
     */
    if (allocated_out != NULL) {
        BUF_MEM *buf;

        BIO_get_mem_ptr(allocated_out, &buf);
        data->running_output = (unsigned char *)buf->data;
        data->running_output_length = buf->length;
        memset(buf, 0, sizeof(*buf));
    }

    BIO_free(allocated_out);
    if (original_data != NULL)
        data->ctx->cleanup(data->ctx->construct_data);
    return ok;
}

int ossl_bio_print_labeled_bignum(BIO *out, const char *label, const BIGNUM *bn)
{
    int ret = 0, use_sep = 0;
    char *hex_str = NULL, *p;
    const char spaces[] = "    ";
    const char *post_label_spc = " ";

    const char *neg = "";
    int bytes;

    if (bn == NULL)
        return 0;
    if (label == NULL) {
        label = "";
        post_label_spc = "";
    }

    if (BN_is_zero(bn))
        return BIO_printf(out, "%s%s0\n", label, post_label_spc);

    if (BN_num_bytes(bn) <= BN_BYTES) {
        BN_ULONG *words = bn_get_words(bn);

        if (BN_is_negative(bn))
            neg = "-";

        return BIO_printf(out, "%s%s%s" BN_FMTu " (%s0x" BN_FMTx ")\n",
                          label, post_label_spc, neg, words[0], neg, words[0]);
    }

    hex_str = BN_bn2hex(bn);
    if (hex_str == NULL)
        return 0;

    p = hex_str;
    if (*p == '-') {
        ++p;
        neg = " (Negative)";
    }
    if (BIO_printf(out, "%s%s\n", label, neg) <= 0)
        goto err;

    /* Keep track of how many bytes we have printed out so far */
    bytes = 0;

    if (BIO_printf(out, "%s", spaces) <= 0)
        goto err;

    /* Add a leading 00 if the top bit is set */
    if (*p >= '8') {
        if (BIO_printf(out, "%02x", 0) <= 0)
            goto err;
        ++bytes;
        use_sep = 1;
    }
    while (*p != '\0') {
        /* Do a newline after every 15 hex bytes + add the space indent */
        if ((bytes % 15) == 0 && bytes > 0) {
            if (BIO_printf(out, ":\n%s", spaces) <= 0)
                goto err;
            use_sep = 0; /* The first byte on the next line doesn't have a : */
        }
        if (BIO_printf(out, "%s%c%c", use_sep ? ":" : "",
                       tolower((unsigned char)p[0]),
                       tolower((unsigned char)p[1])) <= 0)
            goto err;
        ++bytes;
        p += 2;
        use_sep = 1;
    }
    if (BIO_printf(out, "\n") <= 0)
        goto err;
    ret = 1;
err:
    OPENSSL_free(hex_str);
    return ret;
}

int ossl_bio_print_labeled_buf(BIO *out, const char *label,
                           const unsigned char *buf, size_t buflen)
{
    size_t i;

    if (BIO_printf(out, "%s\n", label) <= 0)
        return 0;

    for (i = 0; i < buflen; i++) {
        if ((i % LABELED_BUF_PRINT_WIDTH) == 0) {
            if (i > 0 && BIO_printf(out, "\n") <= 0)
                return 0;
            if (BIO_printf(out, "    ") <= 0)
                return 0;
        }

        if (BIO_printf(out, "%02x%s", buf[i],
                                 (i == buflen - 1) ? "" : ":") <= 0)
            return 0;
    }
    if (BIO_printf(out, "\n") <= 0)
        return 0;

    return 1;
}

#if !defined(OPENSSL_NO_DH) || !defined(OPENSSL_NO_DSA)
int ossl_bio_print_ffc_params(BIO *out, const FFC_PARAMS *ffc)
{
    if (ffc->nid != NID_undef) {
#ifndef OPENSSL_NO_DH
        const DH_NAMED_GROUP *group = ossl_ffc_uid_to_dh_named_group(ffc->nid);
        const char *name = ossl_ffc_named_group_get_name(group);

        if (name == NULL)
            goto err;
        if (BIO_printf(out, "GROUP: %s\n", name) <= 0)
            goto err;
        return 1;
#else
        /* How could this be? We should not have a nid in a no-dh build. */
        goto err;
#endif
    }

    if (!ossl_bio_print_labeled_bignum(out, "P:   ", ffc->p))
        goto err;
    if (ffc->q != NULL) {
        if (!ossl_bio_print_labeled_bignum(out, "Q:   ", ffc->q))
            goto err;
    }
    if (!ossl_bio_print_labeled_bignum(out, "G:   ", ffc->g))
        goto err;
    if (ffc->j != NULL) {
        if (!ossl_bio_print_labeled_bignum(out, "J:   ", ffc->j))
            goto err;
    }
    if (ffc->seed != NULL) {
        if (!ossl_bio_print_labeled_buf(out, "SEED:", ffc->seed, ffc->seedlen))
            goto err;
    }
    if (ffc->gindex != -1) {
        if (BIO_printf(out, "gindex: %d\n", ffc->gindex) <= 0)
            goto err;
    }
    if (ffc->pcounter != -1) {
        if (BIO_printf(out, "pcounter: %d\n", ffc->pcounter) <= 0)
            goto err;
    }
    if (ffc->h != 0) {
        if (BIO_printf(out, "h: %d\n", ffc->h) <= 0)
            goto err;
    }
    return 1;
err:
    return 0;
}

#endif
