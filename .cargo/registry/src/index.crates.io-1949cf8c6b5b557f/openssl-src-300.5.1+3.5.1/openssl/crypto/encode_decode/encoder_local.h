/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include <openssl/types.h>
#include <openssl/safestack.h>
#include <openssl/encoder.h>
#include <openssl/decoder.h>
#include "crypto/decoder.h"
#include "internal/cryptlib.h"
#include "internal/passphrase.h"
#include "internal/property.h"
#include "internal/refcount.h"

struct ossl_endecode_base_st {
    OSSL_PROVIDER *prov;
    int id;
    char *name;
    const OSSL_ALGORITHM *algodef;
    OSSL_PROPERTY_LIST *parsed_propdef;

    CRYPTO_REF_COUNT refcnt;
};

struct ossl_encoder_st {
    struct ossl_endecode_base_st base;
    OSSL_FUNC_encoder_newctx_fn *newctx;
    OSSL_FUNC_encoder_freectx_fn *freectx;
    OSSL_FUNC_encoder_get_params_fn *get_params;
    OSSL_FUNC_encoder_gettable_params_fn *gettable_params;
    OSSL_FUNC_encoder_set_ctx_params_fn *set_ctx_params;
    OSSL_FUNC_encoder_settable_ctx_params_fn *settable_ctx_params;
    OSSL_FUNC_encoder_does_selection_fn *does_selection;
    OSSL_FUNC_encoder_encode_fn *encode;
    OSSL_FUNC_encoder_import_object_fn *import_object;
    OSSL_FUNC_encoder_free_object_fn *free_object;
};

struct ossl_decoder_st {
    struct ossl_endecode_base_st base;
    OSSL_FUNC_decoder_newctx_fn *newctx;
    OSSL_FUNC_decoder_freectx_fn *freectx;
    OSSL_FUNC_decoder_get_params_fn *get_params;
    OSSL_FUNC_decoder_gettable_params_fn *gettable_params;
    OSSL_FUNC_decoder_set_ctx_params_fn *set_ctx_params;
    OSSL_FUNC_decoder_settable_ctx_params_fn *settable_ctx_params;
    OSSL_FUNC_decoder_does_selection_fn *does_selection;
    OSSL_FUNC_decoder_decode_fn *decode;
    OSSL_FUNC_decoder_export_object_fn *export_object;
};

struct ossl_encoder_instance_st {
    OSSL_ENCODER *encoder;        /* Never NULL */
    void *encoderctx;             /* Never NULL */
    const char *output_type;      /* Never NULL */
    const char *output_structure; /* May be NULL */
};

DEFINE_STACK_OF(OSSL_ENCODER_INSTANCE)

void ossl_encoder_instance_free(OSSL_ENCODER_INSTANCE *encoder_inst);

struct ossl_encoder_ctx_st {
    /*
     * Select what parts of an object will be encoded.  This selection is
     * bit encoded, and the bits correspond to selection bits available with
     * the provider side operation.  For example, when encoding an EVP_PKEY,
     * the OSSL_KEYMGMT_SELECT_ macros are used for this.
     */
    int selection;
    /*
     * The desired output type.  The encoder implementation must have a
     * gettable "output-type" parameter that this will match against.
     */
    const char *output_type;
    /*
     * The desired output structure, if that's relevant for the type of
     * object being encoded.  It may be used for selection of the starting
     * encoder implementations in a chain.
     */
    const char *output_structure;

    /*
     * Decoders that are components of any current decoding path.
     */
    STACK_OF(OSSL_ENCODER_INSTANCE) *encoder_insts;

    /*
     * The constructor and destructor of an object to pass to the first
     * encoder in a chain.
     */
    OSSL_ENCODER_CONSTRUCT *construct;
    OSSL_ENCODER_CLEANUP *cleanup;
    void *construct_data;

    /* For any function that needs a passphrase reader */
    struct ossl_passphrase_data_st pwdata;
};

struct ossl_decoder_instance_st {
    OSSL_DECODER *decoder;       /* Never NULL */
    void *decoderctx;            /* Never NULL */
    const char *input_type;      /* Never NULL */
    const char *input_structure; /* May be NULL */
    int input_type_id;

    unsigned int flag_input_structure_was_set : 1;
};

DEFINE_STACK_OF(OSSL_DECODER_INSTANCE)

struct ossl_decoder_ctx_st {
    /*
     * The caller may know the input type of the data they pass.  If not,
     * this will remain NULL and the decoding functionality will start
     * with trying to decode with any desencoder in |decoder_insts|,
     * regardless of their respective input type.
     */
    const char *start_input_type;
    /*
     * The desired input structure, if that's relevant for the type of
     * object being encoded.  It may be used for selection of the ending
     * decoder implementations in a chain, i.e. those chosen using the
     * expected output data type.
     */
    const char *input_structure;
    /*
     * Select what parts of an object are expected.  This may affect what
     * decoder implementations are selected, because there are structures
     * that look different depending on this selection; for example, EVP_PKEY
     * objects often have different encoding structures for private keys,
     * public keys and key parameters.
     * This selection is bit encoded, and the bits correspond to selection
     * bits available with the provider side operation.  For example, when
     * encoding an EVP_PKEY, the OSSL_KEYMGMT_SELECT_ macros are used for
     * this.
     */
    int selection;

    /*
     * Decoders that are components of any current decoding path.
     */
    STACK_OF(OSSL_DECODER_INSTANCE) *decoder_insts;

    /*
     * The constructors of a decoding, and its caller argument.
     */
    OSSL_DECODER_CONSTRUCT *construct;
    OSSL_DECODER_CLEANUP *cleanup;
    void *construct_data;

    /* For any function that needs a passphrase reader */
    struct ossl_passphrase_data_st pwdata;

    /* Signal that further processing should not continue. */
    int harderr;
};

const OSSL_PROPERTY_LIST *
ossl_decoder_parsed_properties(const OSSL_DECODER *decoder);
const OSSL_PROPERTY_LIST *
ossl_encoder_parsed_properties(const OSSL_ENCODER *encoder);

int ossl_decoder_fast_is_a(OSSL_DECODER *decoder,
                           const char *name, int *id_cache);
