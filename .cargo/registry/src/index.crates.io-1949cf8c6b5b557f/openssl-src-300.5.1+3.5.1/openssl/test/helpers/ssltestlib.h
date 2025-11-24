/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_TEST_SSLTESTLIB_H
# define OSSL_TEST_SSLTESTLIB_H

# include <openssl/ssl.h>

#define TLS13_AES_128_GCM_SHA256_BYTES  ((const unsigned char *)"\x13\x01")
#define TLS13_AES_256_GCM_SHA384_BYTES  ((const unsigned char *)"\x13\x02")
#define TLS13_CHACHA20_POLY1305_SHA256_BYTES ((const unsigned char *)"\x13\x03")
#define TLS13_AES_128_CCM_SHA256_BYTES ((const unsigned char *)"\x13\x04")
#define TLS13_AES_128_CCM_8_SHA256_BYTES ((const unsigned char *)"\x13\05")
#define TLS13_SHA256_SHA256_BYTES ((const unsigned char *)"\xC0\xB4")
#define TLS13_SHA384_SHA384_BYTES ((const unsigned char *)"\xC0\xB5")

int create_ssl_ctx_pair(OSSL_LIB_CTX *libctx, const SSL_METHOD *sm,
                        const SSL_METHOD *cm, int min_proto_version,
                        int max_proto_version, SSL_CTX **sctx, SSL_CTX **cctx,
                        char *certfile, char *privkeyfile);
int create_ssl_objects(SSL_CTX *serverctx, SSL_CTX *clientctx, SSL **sssl,
                       SSL **cssl, BIO *s_to_c_fbio, BIO *c_to_s_fbio);
int create_bare_ssl_connection(SSL *serverssl, SSL *clientssl, int want,
                               int read, int listen);
int create_bare_ssl_connection_ex(SSL *serverssl, SSL *clientssl, int want,
                                  int read, int listen, int *cm_count, int *sm_count);
int create_ssl_objects2(SSL_CTX *serverctx, SSL_CTX *clientctx, SSL **sssl,
                       SSL **cssl, int sfd, int cfd);
int wait_until_sock_readable(int sock);
int create_test_sockets(int *cfdp, int *sfdp, int socktype, BIO_ADDR *saddr);
int create_ssl_connection(SSL *serverssl, SSL *clientssl, int want);
int create_ssl_connection_ex(SSL *serverssl, SSL *clientssl, int want,
                             int *cm_count, int *sm_count);
void shutdown_ssl_connection(SSL *serverssl, SSL *clientssl);

/* Note: Not thread safe! */
const BIO_METHOD *bio_f_tls_dump_filter(void);
void bio_f_tls_dump_filter_free(void);

const BIO_METHOD *bio_s_mempacket_test(void);
void bio_s_mempacket_test_free(void);

const BIO_METHOD *bio_s_always_retry(void);
void bio_s_always_retry_free(void);
void set_always_retry_err_val(int err);

/*
 * Maybe retry BIO ctrls. We make them large enough to not clash with standard
 * BIO ctrl codes.
 */
#define MAYBE_RETRY_CTRL_SET_RETRY_AFTER_CNT       (1 << 15)

const BIO_METHOD *bio_s_maybe_retry(void);
void bio_s_maybe_retry_free(void);

/* Packet types - value 0 is reserved */
#define INJECT_PACKET                   1
#define INJECT_PACKET_IGNORE_REC_SEQ    2

/*
 * Mempacket BIO ctrls. We make them large enough to not clash with standard BIO
 * ctrl codes.
 */
#define MEMPACKET_CTRL_SET_DROP_EPOCH       (1 << 15)
#define MEMPACKET_CTRL_SET_DROP_REC         (2 << 15)
#define MEMPACKET_CTRL_GET_DROP_REC         (3 << 15)
#define MEMPACKET_CTRL_SET_DUPLICATE_REC    (4 << 15)

int mempacket_swap_epoch(BIO *bio);
int mempacket_move_packet(BIO *bio, int d, int s);
int mempacket_dup_last_packet(BIO *bio);
int mempacket_test_inject(BIO *bio, const char *in, int inl, int pktnum,
                          int type);

typedef struct mempacket_st MEMPACKET;

DEFINE_STACK_OF(MEMPACKET)

SSL_SESSION *create_a_psk(SSL *ssl, size_t mdsize);

/* Add cert from `cert_file` multiple times to create large extra cert chain */
int ssl_ctx_add_large_cert_chain(OSSL_LIB_CTX *libctx, SSL_CTX *sctx,
                                 const char *cert_file);

ENGINE *load_dasync(void);

#endif /* OSSL_TEST_SSLTESTLIB_H */
