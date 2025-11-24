/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_DECODER_H
# define OSSL_CRYPTO_DECODER_H
# pragma once

# include <openssl/decoder.h>

/*
 * These are specially made for the 'file:' provider-native loader, which
 * uses this to install a DER to anything decoder, which doesn't do much
 * except read a DER blob and pass it on as a provider object abstraction
 * (provider-object(7)).
 */
void *ossl_decoder_from_algorithm(int id, const OSSL_ALGORITHM *algodef,
                                  OSSL_PROVIDER *prov);

OSSL_DECODER_INSTANCE *
ossl_decoder_instance_new_forprov(OSSL_DECODER *decoder, void *provctx,
                                  const char *input_structure);
OSSL_DECODER_INSTANCE *
ossl_decoder_instance_new(OSSL_DECODER *decoder, void *decoderctx);
void ossl_decoder_instance_free(OSSL_DECODER_INSTANCE *decoder_inst);
int ossl_decoder_ctx_get_harderr(const OSSL_DECODER_CTX *ctx);
void ossl_decoder_ctx_set_harderr(OSSL_DECODER_CTX *ctx);
OSSL_DECODER_INSTANCE *ossl_decoder_instance_dup(const OSSL_DECODER_INSTANCE *src);
int ossl_decoder_ctx_add_decoder_inst(OSSL_DECODER_CTX *ctx,
                                      OSSL_DECODER_INSTANCE *di);

int ossl_decoder_get_number(const OSSL_DECODER *encoder);
int ossl_decoder_store_cache_flush(OSSL_LIB_CTX *libctx);
int ossl_decoder_store_remove_all_provided(const OSSL_PROVIDER *prov);

void *ossl_decoder_cache_new(OSSL_LIB_CTX *ctx);
void ossl_decoder_cache_free(void *vcache);
int ossl_decoder_cache_flush(OSSL_LIB_CTX *libctx);

#endif
