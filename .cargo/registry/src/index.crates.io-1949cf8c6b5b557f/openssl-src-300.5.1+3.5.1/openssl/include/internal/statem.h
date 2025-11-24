/*
 * Copyright 2015-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_INTERNAL_STATEM_H
# define OSSL_INTERNAL_STATEM_H

/*****************************************************************************
 *                                                                           *
 * These enums should be considered PRIVATE to the state machine. No         *
 * non-state machine code should need to use these                           *
 *                                                                           *
 *****************************************************************************/
/*
 * Valid return codes used for functions performing work prior to or after
 * sending or receiving a message
 */
typedef enum {
    /* Something went wrong */
    WORK_ERROR,
    /* We're done working and there shouldn't be anything else to do after */
    WORK_FINISHED_STOP,
    /* We're done working move onto the next thing */
    WORK_FINISHED_CONTINUE,
    /* We're done writing, start reading (or vice versa) */
    WORK_FINISHED_SWAP,
    /* We're working on phase A */
    WORK_MORE_A,
    /* We're working on phase B */
    WORK_MORE_B,
    /* We're working on phase C */
    WORK_MORE_C
} WORK_STATE;

/* Write transition return codes */
typedef enum {
    /* Something went wrong */
    WRITE_TRAN_ERROR,
    /* A transition was successfully completed and we should continue */
    WRITE_TRAN_CONTINUE,
    /* There is no more write work to be done */
    WRITE_TRAN_FINISHED
} WRITE_TRAN;

/* Message flow states */
typedef enum {
    /* No handshake in progress */
    MSG_FLOW_UNINITED,
    /* A permanent error with this connection */
    MSG_FLOW_ERROR,
    /* We are reading messages */
    MSG_FLOW_READING,
    /* We are writing messages */
    MSG_FLOW_WRITING,
    /* Handshake has finished */
    MSG_FLOW_FINISHED
} MSG_FLOW_STATE;

/* Read states */
typedef enum {
    READ_STATE_HEADER,
    READ_STATE_BODY,
    READ_STATE_POST_PROCESS
} READ_STATE;

/* Write states */
typedef enum {
    WRITE_STATE_TRANSITION,
    WRITE_STATE_PRE_WORK,
    WRITE_STATE_SEND,
    WRITE_STATE_POST_WORK
} WRITE_STATE;

typedef enum {
    CON_FUNC_ERROR = 0,
    CON_FUNC_SUCCESS,
    CON_FUNC_DONT_SEND
} CON_FUNC_RETURN;

typedef int (*ossl_statem_mutate_handshake_cb)(const unsigned char *msgin,
                                               size_t inlen,
                                               unsigned char **msgout,
                                               size_t *outlen,
                                               void *arg);

typedef void (*ossl_statem_finish_mutate_handshake_cb)(void *arg);

/*****************************************************************************
 *                                                                           *
 * This structure should be considered "opaque" to anything outside of the   *
 * state machine. No non-state machine code should be accessing the members  *
 * of this structure.                                                        *
 *                                                                           *
 *****************************************************************************/

struct ossl_statem_st {
    MSG_FLOW_STATE state;
    WRITE_STATE write_state;
    WORK_STATE write_state_work;
    READ_STATE read_state;
    WORK_STATE read_state_work;
    OSSL_HANDSHAKE_STATE hand_state;
    /* The handshake state requested by an API call (e.g. HelloRequest) */
    OSSL_HANDSHAKE_STATE request_state;
    int in_init;
    int read_state_first_init;
    /* true when we are actually in SSL_accept() or SSL_connect() */
    int in_handshake;
    /*
     * True when are processing a "real" handshake that needs cleaning up (not
     * just a HelloRequest or similar).
     */
    int cleanuphand;
    /* Should we skip the CertificateVerify message? */
    unsigned int no_cert_verify;
    int use_timer;

    /* Test harness message mutator callbacks */
    ossl_statem_mutate_handshake_cb mutate_handshake_cb;
    ossl_statem_finish_mutate_handshake_cb finish_mutate_handshake_cb;
    void *mutatearg;
    unsigned int write_in_progress : 1;
};
typedef struct ossl_statem_st OSSL_STATEM;

/*****************************************************************************
 *                                                                           *
 * The following macros/functions represent the libssl internal API to the   *
 * state machine. Any libssl code may call these functions/macros            *
 *                                                                           *
 *****************************************************************************/

typedef struct ssl_connection_st SSL_CONNECTION;

__owur int ossl_statem_accept(SSL *s);
__owur int ossl_statem_connect(SSL *s);
OSSL_HANDSHAKE_STATE ossl_statem_get_state(SSL_CONNECTION *s);
void ossl_statem_clear(SSL_CONNECTION *s);
void ossl_statem_set_renegotiate(SSL_CONNECTION *s);
void ossl_statem_send_fatal(SSL_CONNECTION *s, int al);
void ossl_statem_fatal(SSL_CONNECTION *s, int al, int reason,
                       const char *fmt, ...);
# define SSLfatal_alert(s, al) ossl_statem_send_fatal((s), (al))
# define SSLfatal(s, al, r) SSLfatal_data((s), (al), (r), NULL)
# define SSLfatal_data                                          \
    (ERR_new(),                                                 \
     ERR_set_debug(OPENSSL_FILE, OPENSSL_LINE, OPENSSL_FUNC),   \
     ossl_statem_fatal)

int ossl_statem_in_error(const SSL_CONNECTION *s);
void ossl_statem_set_in_init(SSL_CONNECTION *s, int init);
int ossl_statem_get_in_handshake(SSL_CONNECTION *s);
void ossl_statem_set_in_handshake(SSL_CONNECTION *s, int inhand);
__owur int ossl_statem_skip_early_data(SSL_CONNECTION *s);
int ossl_statem_check_finish_init(SSL_CONNECTION *s, int send);
void ossl_statem_set_hello_verify_done(SSL_CONNECTION *s);
__owur int ossl_statem_app_data_allowed(SSL_CONNECTION *s);
__owur int ossl_statem_export_allowed(SSL_CONNECTION *s);
__owur int ossl_statem_export_early_allowed(SSL_CONNECTION *s);

/* Flush the write BIO */
int statem_flush(SSL_CONNECTION *s);

int ossl_statem_set_mutator(SSL *s,
                            ossl_statem_mutate_handshake_cb mutate_handshake_cb,
                            ossl_statem_finish_mutate_handshake_cb finish_mutate_handshake_cb,
                            void *mutatearg);

#endif
