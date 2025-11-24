/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>

/* tls_pad.c */
int ssl3_cbc_remove_padding_and_mac(size_t *reclen,
                                    size_t origreclen,
                                    unsigned char *recdata,
                                    unsigned char **mac,
                                    int *alloced,
                                    size_t block_size, size_t mac_size,
                                    OSSL_LIB_CTX *libctx);

int tls1_cbc_remove_padding_and_mac(size_t *reclen,
                                    size_t origreclen,
                                    unsigned char *recdata,
                                    unsigned char **mac,
                                    int *alloced,
                                    size_t block_size, size_t mac_size,
                                    int aead,
                                    OSSL_LIB_CTX *libctx);

/* ssl3_cbc.c */
__owur char ssl3_cbc_record_digest_supported(const EVP_MD_CTX *ctx);
__owur int ssl3_cbc_digest_record(const EVP_MD *md,
                                  unsigned char *md_out,
                                  size_t *md_out_size,
                                  const unsigned char *header,
                                  const unsigned char *data,
                                  size_t data_size,
                                  size_t data_plus_mac_plus_padding_size,
                                  const unsigned char *mac_secret,
                                  size_t mac_secret_length, char is_sslv3);
