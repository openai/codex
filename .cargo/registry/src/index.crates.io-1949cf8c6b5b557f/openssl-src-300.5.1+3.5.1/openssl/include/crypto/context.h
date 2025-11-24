/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>

void *ossl_provider_store_new(OSSL_LIB_CTX *);
void *ossl_property_string_data_new(OSSL_LIB_CTX *);
void *ossl_stored_namemap_new(OSSL_LIB_CTX *);
void *ossl_property_defns_new(OSSL_LIB_CTX *);
void *ossl_ctx_global_properties_new(OSSL_LIB_CTX *);
void *ossl_rand_ctx_new(OSSL_LIB_CTX *);
void *ossl_prov_conf_ctx_new(OSSL_LIB_CTX *);
void *ossl_bio_core_globals_new(OSSL_LIB_CTX *);
void *ossl_child_prov_ctx_new(OSSL_LIB_CTX *);
void *ossl_prov_drbg_nonce_ctx_new(OSSL_LIB_CTX *);
void *ossl_self_test_set_callback_new(OSSL_LIB_CTX *);
void *ossl_indicator_set_callback_new(OSSL_LIB_CTX *);
void *ossl_rand_crng_ctx_new(OSSL_LIB_CTX *);
int ossl_thread_register_fips(OSSL_LIB_CTX *);
void *ossl_thread_event_ctx_new(OSSL_LIB_CTX *);
void *ossl_fips_prov_ossl_ctx_new(OSSL_LIB_CTX *);
#if defined(OPENSSL_THREADS)
void *ossl_threads_ctx_new(OSSL_LIB_CTX *);
#endif

void ossl_provider_store_free(void *);
void ossl_property_string_data_free(void *);
void ossl_stored_namemap_free(void *);
void ossl_property_defns_free(void *);
void ossl_ctx_global_properties_free(void *);
void ossl_rand_ctx_free(void *);
void ossl_prov_conf_ctx_free(void *);
void ossl_bio_core_globals_free(void *);
void ossl_child_prov_ctx_free(void *);
void ossl_prov_drbg_nonce_ctx_free(void *);
void ossl_indicator_set_callback_free(void *cb);
void ossl_self_test_set_callback_free(void *);
void ossl_rand_crng_ctx_free(void *);
void ossl_thread_event_ctx_free(void *);
void ossl_fips_prov_ossl_ctx_free(void *);
void ossl_release_default_drbg_ctx(void);
#if defined(OPENSSL_THREADS)
void ossl_threads_ctx_free(void *);
#endif
