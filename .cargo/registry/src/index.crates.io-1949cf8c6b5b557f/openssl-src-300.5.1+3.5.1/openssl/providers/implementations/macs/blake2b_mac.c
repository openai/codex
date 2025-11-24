/*
 * Copyright 2018-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Constants */
#define BLAKE2_CTX BLAKE2B_CTX
#define BLAKE2_PARAM BLAKE2B_PARAM
#define BLAKE2_KEYBYTES BLAKE2B_KEYBYTES
#define BLAKE2_OUTBYTES BLAKE2B_OUTBYTES
#define BLAKE2_PERSONALBYTES BLAKE2B_PERSONALBYTES
#define BLAKE2_SALTBYTES BLAKE2B_SALTBYTES
#define BLAKE2_BLOCKBYTES BLAKE2B_BLOCKBYTES

/* Function names */
#define BLAKE2_PARAM_INIT ossl_blake2b_param_init
#define BLAKE2_INIT_KEY ossl_blake2b_init_key
#define BLAKE2_UPDATE ossl_blake2b_update
#define BLAKE2_FINAL ossl_blake2b_final
#define BLAKE2_PARAM_SET_DIGEST_LENGTH ossl_blake2b_param_set_digest_length
#define BLAKE2_PARAM_SET_KEY_LENGTH ossl_blake2b_param_set_key_length
#define BLAKE2_PARAM_SET_PERSONAL ossl_blake2b_param_set_personal
#define BLAKE2_PARAM_SET_SALT ossl_blake2b_param_set_salt

/* OSSL_DISPATCH symbol */
#define BLAKE2_FUNCTIONS ossl_blake2bmac_functions

#include "blake2_mac_impl.c"

