/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include "internal/recordmethod.h"

/*****************************************************************************
 *                                                                           *
 * These structures should be considered PRIVATE to the record layer. No     *
 * non-record layer code should be using these structures in any way.        *
 *                                                                           *
 *****************************************************************************/

#define SEQ_NUM_SIZE                            8

typedef struct tls_record_st {
    void *rechandle;
    int version;
    uint8_t type;
    /* The data buffer containing bytes from the record */
    const unsigned char *data;
    /*
     * Buffer that we allocated to store data. If non NULL always the same as
     * data (but non-const)
     */
    unsigned char *allocdata;
    /* Number of remaining to be read in the data buffer */
    size_t length;
    /* Offset into the data buffer where to start reading */
    size_t off;
    /* epoch number. DTLS only */
    uint16_t epoch;
    /* sequence number. DTLS only */
    unsigned char seq_num[SEQ_NUM_SIZE];
#ifndef OPENSSL_NO_SCTP
    struct bio_dgram_sctp_rcvinfo recordinfo;
#endif
} TLS_RECORD;

typedef struct dtls_record_layer_st {
    /*
     * The current data and handshake epoch.  This is initially
     * undefined, and starts at zero once the initial handshake is
     * completed
     */
    uint16_t r_epoch;
    uint16_t w_epoch;

    /*
     * Buffered application records. Only for records between CCS and
     * Finished to prevent either protocol violation or unnecessary message
     * loss.
     */
    struct pqueue_st *buffered_app_data;
} DTLS_RECORD_LAYER;

/*****************************************************************************
 *                                                                           *
 * This structure should be considered "opaque" to anything outside of the   *
 * record layer. No non-record layer code should be accessing the members of *
 * this structure.                                                           *
 *                                                                           *
 *****************************************************************************/

typedef struct record_layer_st {
    /* The parent SSL_CONNECTION structure */
    SSL_CONNECTION *s;

    /* Custom record layer: always selected if set */
    const OSSL_RECORD_METHOD *custom_rlmethod;
    /* Record layer specific argument */
    void *rlarg;
    /* Method to use for the read record layer*/
    const OSSL_RECORD_METHOD *rrlmethod;
    /* Method to use for the write record layer*/
    const OSSL_RECORD_METHOD *wrlmethod;
    /* The read record layer object itself */
    OSSL_RECORD_LAYER *rrl;
    /* The write record layer object itself */
    OSSL_RECORD_LAYER *wrl;
    /* BIO to store data destined for the next read record layer epoch */
    BIO *rrlnext;
    /* Default read buffer length to be passed to the record layer */
    size_t default_read_buf_len;

    /*
     * Read as many input bytes as possible (for
     * non-blocking reads)
     */
    int read_ahead;

    /* number of bytes sent so far */
    size_t wnum;
    unsigned char handshake_fragment[4];
    size_t handshake_fragment_len;
    /* partial write - check the numbers match */
    /* number bytes written */
    size_t wpend_tot;
    uint8_t wpend_type;
    const unsigned char *wpend_buf;

    /* Count of the number of consecutive warning alerts received */
    unsigned int alert_count;
    DTLS_RECORD_LAYER *d;

    /* TLS1.3 padding callback */
    size_t (*record_padding_cb)(SSL *s, int type, size_t len, void *arg);
    void *record_padding_arg;
    size_t block_padding;
    size_t hs_padding;

    /* How many records we have read from the record layer */
    size_t num_recs;
    /* The next record from the record layer that we need to process */
    size_t curr_rec;
    /* Record layer data to be processed */
    TLS_RECORD tlsrecs[SSL_MAX_PIPELINES];

} RECORD_LAYER;

/*****************************************************************************
 *                                                                           *
 * The following macros/functions represent the libssl internal API to the   *
 * record layer. Any libssl code may call these functions/macros             *
 *                                                                           *
 *****************************************************************************/

#define RECORD_LAYER_set_read_ahead(rl, ra)     ((rl)->read_ahead = (ra))
#define RECORD_LAYER_get_read_ahead(rl)         ((rl)->read_ahead)

void RECORD_LAYER_init(RECORD_LAYER *rl, SSL_CONNECTION *s);
int RECORD_LAYER_clear(RECORD_LAYER *rl);
int RECORD_LAYER_reset(RECORD_LAYER *rl);
int RECORD_LAYER_read_pending(const RECORD_LAYER *rl);
int RECORD_LAYER_processed_read_pending(const RECORD_LAYER *rl);
int RECORD_LAYER_write_pending(const RECORD_LAYER *rl);
int RECORD_LAYER_is_sslv2_record(RECORD_LAYER *rl);
__owur size_t ssl3_pending(const SSL *s);
__owur int ssl3_write_bytes(SSL *s, uint8_t type, const void *buf, size_t len,
                            size_t *written);
__owur int ssl3_read_bytes(SSL *s, uint8_t type, uint8_t *recvd_type,
                           unsigned char *buf, size_t len, int peek,
                           size_t *readbytes);

int DTLS_RECORD_LAYER_new(RECORD_LAYER *rl);
void DTLS_RECORD_LAYER_free(RECORD_LAYER *rl);
void DTLS_RECORD_LAYER_clear(RECORD_LAYER *rl);
__owur int dtls1_read_bytes(SSL *s, uint8_t type, uint8_t *recvd_type,
                            unsigned char *buf, size_t len, int peek,
                            size_t *readbytes);
__owur int dtls1_write_bytes(SSL_CONNECTION *s, uint8_t type, const void *buf,
                             size_t len, size_t *written);
int do_dtls1_write(SSL_CONNECTION *s, uint8_t type, const unsigned char *buf,
                   size_t len, size_t *written);
void dtls1_increment_epoch(SSL_CONNECTION *s, int rw);
uint16_t dtls1_get_epoch(SSL_CONNECTION *s, int rw);
int ssl_release_record(SSL_CONNECTION *s, TLS_RECORD *rr, size_t length);

# define HANDLE_RLAYER_READ_RETURN(s, ret) \
    ossl_tls_handle_rlayer_return(s, 0, ret, OPENSSL_FILE, OPENSSL_LINE)

# define HANDLE_RLAYER_WRITE_RETURN(s, ret) \
    ossl_tls_handle_rlayer_return(s, 1, ret, OPENSSL_FILE, OPENSSL_LINE)

int ossl_tls_handle_rlayer_return(SSL_CONNECTION *s, int writing, int ret,
                                  char *file, int line);

int ssl_set_new_record_layer(SSL_CONNECTION *s, int version,
                             int direction, int level,
                             unsigned char *secret, size_t secretlen,
                             unsigned char *key, size_t keylen,
                             unsigned char *iv,  size_t ivlen,
                             unsigned char *mackey, size_t mackeylen,
                             const EVP_CIPHER *ciph, size_t taglen,
                             int mactype, const EVP_MD *md,
                             const SSL_COMP *comp, const EVP_MD *kdfdigest);
int ssl_set_record_protocol_version(SSL_CONNECTION *s, int vers);

# define OSSL_FUNC_RLAYER_SKIP_EARLY_DATA        1
OSSL_CORE_MAKE_FUNC(int, rlayer_skip_early_data, (void *cbarg))
# define OSSL_FUNC_RLAYER_MSG_CALLBACK           2
OSSL_CORE_MAKE_FUNC(void, rlayer_msg_callback, (int write_p, int version,
                                                int content_type,
                                                const void *buf, size_t len,
                                                void *cbarg))
# define OSSL_FUNC_RLAYER_SECURITY               3
OSSL_CORE_MAKE_FUNC(int, rlayer_security, (void *cbarg, int op, int bits,
                                           int nid, void *other))
# define OSSL_FUNC_RLAYER_PADDING                4
OSSL_CORE_MAKE_FUNC(size_t, rlayer_padding, (void *cbarg, int type, size_t len))
