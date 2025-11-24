/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/ssl.h>
#include <internal/quic_tserver.h>

/* Type to represent the Fault Injector */
typedef struct qtest_fault QTEST_FAULT;

typedef struct bio_qtest_data {
    size_t short_conn_id_len;
    struct qtest_fault *fault;
} QTEST_DATA;

/*
 * Structure representing a parsed EncryptedExtension message. Listeners can
 * make changes to the contents of structure objects as required and the fault
 * injector will reconstruct the message to be sent on
 */
typedef struct qtest_fault_encrypted_extensions {
    /* EncryptedExtension messages just have an extensions block */
    unsigned char *extensions;
    size_t extensionslen;
} QTEST_ENCRYPTED_EXTENSIONS;

/* Flags for use with qtest_create_quic_objects() */

/* Indicates whether we are using blocking mode or not */
#define QTEST_FLAG_BLOCK        (1 << 0)
/* Use fake time rather than real time */
#define QTEST_FLAG_FAKE_TIME    (1 << 1)
/* Introduce noise in the BIO */
#define QTEST_FLAG_NOISE        (1 << 2)
/* Split datagrams such that each datagram contains one packet */
#define QTEST_FLAG_PACKET_SPLIT (1 << 3)
/* Turn on client side tracing */
#define QTEST_FLAG_CLIENT_TRACE (1 << 4)
/*
 * Given an SSL_CTX for the client and filenames for the server certificate and
 * keyfile, create a server and client instances as well as a fault injector
 * instance. |flags| is the logical or of flags defined above, or 0 if none.
 */
int qtest_create_quic_objects(OSSL_LIB_CTX *libctx, SSL_CTX *clientctx,
                              SSL_CTX *serverctx, char *certfile, char *keyfile,
                              int flags, QUIC_TSERVER **qtserv, SSL **cssl,
                              QTEST_FAULT **fault, BIO **tracebio);

/* Where QTEST_FLAG_FAKE_TIME is used, add millis to the current time */
void qtest_add_time(uint64_t millis);

/* Starts time measurement */
void qtest_start_stopwatch(void);
/* Returns the duration from the start in millis */
uint64_t qtest_get_stopwatch_time(void);

QTEST_FAULT *qtest_create_injector(QUIC_TSERVER *ts);

BIO_METHOD *qtest_get_bio_method(void);

/*
 * Free up a Fault Injector instance
 */
void qtest_fault_free(QTEST_FAULT *fault);

/* Returns 1 if the quictestlib supports blocking tests */
int qtest_supports_blocking(void);

/*
 * Run the TLS handshake to create a QUIC connection between the client and
 * server.
 */
int qtest_create_quic_connection(QUIC_TSERVER *qtserv, SSL *clientssl);

/*
 * Check if both client and server have no data to read and are waiting on a
 * timeout. If so, wait until the timeout has expired.
 */
int qtest_wait_for_timeout(SSL *s, QUIC_TSERVER *qtserv);

/*
 * Same as qtest_create_quic_connection but will stop (successfully) if the
 * clientssl indicates SSL_ERROR_WANT_XXX as specified by |wanterr|
 */
int qtest_create_quic_connection_ex(QUIC_TSERVER *qtserv, SSL *clientssl,
                                    int wanterr);

/*
 * Shutdown the client SSL object gracefully
 */
int qtest_shutdown(QUIC_TSERVER *qtserv, SSL *clientssl);

/*
 * Confirm that the server has received the given transport error code.
 */
int qtest_check_server_transport_err(QUIC_TSERVER *qtserv, uint64_t code);

/*
 * Confirm the server has received a protocol error. Equivalent to calling
 * qtest_check_server_transport_err with a code of QUIC_ERR_PROTOCOL_VIOLATION
 */
int qtest_check_server_protocol_err(QUIC_TSERVER *qtserv);

/*
 * Confirm the server has received a frame encoding error. Equivalent to calling
 * qtest_check_server_transport_err with a code of QUIC_ERR_FRAME_ENCODING_ERROR
 */
int qtest_check_server_frame_encoding_err(QUIC_TSERVER *qtserv);

/*
 * Enable tests to listen for pre-encryption QUIC packets being sent
 */
typedef int (*qtest_fault_on_packet_plain_cb)(QTEST_FAULT *fault,
                                              QUIC_PKT_HDR *hdr,
                                              unsigned char *buf,
                                              size_t len,
                                              void *cbarg);

int qtest_fault_set_packet_plain_listener(QTEST_FAULT *fault,
                                          qtest_fault_on_packet_plain_cb pplaincb,
                                          void *pplaincbarg);


/*
 * Helper function to be called from a packet_plain_listener callback if it
 * wants to resize the packet (either to add new data to it, or to truncate it).
 * The buf provided to packet_plain_listener is over allocated, so this just
 * changes the logical size and never changes the actual address of the buf.
 * This will fail if a large resize is attempted that exceeds the over
 * allocation.
 */
int qtest_fault_resize_plain_packet(QTEST_FAULT *fault, size_t newlen);

/*
 * Prepend frame data into a packet. To be called from a packet_plain_listener
 * callback
 */
int qtest_fault_prepend_frame(QTEST_FAULT *fault, const unsigned char *frame,
                              size_t frame_len);

/*
 * The general handshake message listener is sent the entire handshake message
 * data block, including the handshake header itself
 */
typedef int (*qtest_fault_on_handshake_cb)(QTEST_FAULT *fault,
                                           unsigned char *msg,
                                           size_t msglen,
                                           void *handshakecbarg);

int qtest_fault_set_handshake_listener(QTEST_FAULT *fault,
                                       qtest_fault_on_handshake_cb handshakecb,
                                       void *handshakecbarg);

/*
 * Helper function to be called from a handshake_listener callback if it wants
 * to resize the handshake message (either to add new data to it, or to truncate
 * it). newlen must include the length of the handshake message header. The
 * handshake message buffer is over allocated, so this just changes the logical
 * size and never changes the actual address of the buf.
 * This will fail if a large resize is attempted that exceeds the over
 * allocation.
 */
int qtest_fault_resize_handshake(QTEST_FAULT *fault, size_t newlen);

/*
 * Add listeners for specific types of frame here. E.g. we might
 * expect to see an "ACK" frame listener which will be passed pre-parsed ack
 * data that can be modified as required.
 */

/*
 * Handshake message specific listeners. Unlike the general handshake message
 * listener these messages are pre-parsed and supplied with message specific
 * data and exclude the handshake header
 */
typedef int (*qtest_fault_on_enc_ext_cb)(QTEST_FAULT *fault,
                                         QTEST_ENCRYPTED_EXTENSIONS *ee,
                                         size_t eelen,
                                         void *encextcbarg);

int qtest_fault_set_hand_enc_ext_listener(QTEST_FAULT *fault,
                                          qtest_fault_on_enc_ext_cb encextcb,
                                          void *encextcbarg);

/* Add listeners for other types of handshake message here */


/*
 * Helper function to be called from message specific listener callbacks. newlen
 * is the new length of the specific message excluding the handshake message
 * header.  The buffers provided to the message specific listeners are over
 * allocated, so this just changes the logical size and never changes the actual
 * address of the buffer. This will fail if a large resize is attempted that
 * exceeds the over allocation.
 */
int qtest_fault_resize_message(QTEST_FAULT *fault, size_t newlen);

/*
 * Helper function to delete an extension from an extension block. |exttype| is
 * the type of the extension to be deleted. |ext| points to the extension block.
 * On entry |*extlen| contains the length of the extension block. It is updated
 * with the new length on exit. If old_ext is non-NULL, the deleted extension
 * is appended to the given BUF_MEM.
 */
int qtest_fault_delete_extension(QTEST_FAULT *fault,
                                 unsigned int exttype, unsigned char *ext,
                                 size_t *extlen,
                                 BUF_MEM *old_ext);

/*
 * Add additional helper functions for querying extensions here (e.g.
 * finding or adding them). We could also provide a "listener" API for listening
 * for specific extension types
 */

/*
 * Enable tests to listen for post-encryption QUIC packets being sent
 */
typedef int (*qtest_fault_on_packet_cipher_cb)(QTEST_FAULT *fault,
                                               /* The parsed packet header */
                                               QUIC_PKT_HDR *hdr,
                                               /* The packet payload data */
                                               unsigned char *buf,
                                               /* Length of the payload */
                                               size_t len,
                                               void *cbarg);

int qtest_fault_set_packet_cipher_listener(QTEST_FAULT *fault,
                                           qtest_fault_on_packet_cipher_cb pciphercb,
                                           void *picphercbarg);

/*
 * Enable tests to listen for datagrams being sent
 */
typedef int (*qtest_fault_on_datagram_cb)(QTEST_FAULT *fault,
                                          BIO_MSG *m,
                                          size_t stride,
                                          void *cbarg);

int qtest_fault_set_datagram_listener(QTEST_FAULT *fault,
                                      qtest_fault_on_datagram_cb datagramcb,
                                      void *datagramcbarg);

/*
 * To be called from a datagram_listener callback. The datagram buffer is over
 * allocated, so this just changes the logical size and never changes the actual
 * address of the buffer. This will fail if a large resize is attempted that
 * exceeds the over allocation.
 */
int qtest_fault_resize_datagram(QTEST_FAULT *fault, size_t newlen);

/*
 * Set bandwidth and noise rate on noisy dgram filter.
 * Arguments with values of 0 mean no limit/no noise.
 */

int qtest_fault_set_bw_limit(QTEST_FAULT *fault,
                             size_t ctos_bw, size_t stoc_bw,
                             int noise_rate);

/* Copy a BIO_MSG */
int bio_msg_copy(BIO_MSG *dst, BIO_MSG *src);

#define BIO_CTRL_NOISE_BACK_OFF       1001
#define BIO_CTRL_NOISE_RATE           1002
#define BIO_CTRL_NOISE_RECV_BANDWIDTH 1003
#define BIO_CTRL_NOISE_SEND_BANDWIDTH 1004
#define BIO_CTRL_NOISE_SET_NOW_CB     1005

struct bio_noise_now_cb_st {
    OSSL_TIME (*now_cb)(void *);
    void *now_cb_arg;
};

/* BIO filter for simulating a noisy UDP socket */
const BIO_METHOD *bio_f_noisy_dgram_filter(void);

/* Free the BIO filter method object */
void bio_f_noisy_dgram_filter_free(void);

/*
 * BIO filter for splitting QUIC datagrams containing multiple packets into
 * individual datagrams.
 */
const BIO_METHOD *bio_f_pkt_split_dgram_filter(void);

/* Free the BIO filter method object */
void bio_f_pkt_split_dgram_filter_free(void);
