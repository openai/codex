/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_SSL_H
# define OSSL_QUIC_SSL_H

# include <openssl/ssl.h>
# include <openssl/bio.h>
# include "internal/refcount.h"
# include "internal/quic_record_rx.h" /* OSSL_QRX */
# include "internal/quic_ackm.h"      /* OSSL_ACKM */
# include "internal/quic_channel.h"   /* QUIC_CHANNEL */
# include "internal/quic_predef.h"

# ifndef OPENSSL_NO_QUIC

__owur SSL *ossl_quic_new(SSL_CTX *ctx);
__owur SSL *ossl_quic_new_listener(SSL_CTX *ctx, uint64_t flags);
__owur SSL *ossl_quic_new_listener_from(SSL *ssl, uint64_t flags);
__owur SSL *ossl_quic_new_from_listener(SSL *ssl, uint64_t flags);
__owur SSL *ossl_quic_new_domain(SSL_CTX *ctx, uint64_t flags);

/*
 * Datatype returned from ossl_quic_get_peer_token
 */
typedef struct quic_token_st {
    CRYPTO_REF_COUNT references;
    uint8_t *hashkey;
    size_t hashkey_len;
    uint8_t *token;
    size_t token_len;
} QUIC_TOKEN;

SSL_TOKEN_STORE *ossl_quic_new_token_store(void);
void ossl_quic_free_token_store(SSL_TOKEN_STORE *hdl);
SSL_TOKEN_STORE *ossl_quic_get0_token_store(SSL_CTX *ctx);
int ossl_quic_set1_token_store(SSL_CTX *ctx, SSL_TOKEN_STORE *hdl);
int ossl_quic_set_peer_token(SSL_CTX *ctx, BIO_ADDR *peer,
                             const uint8_t *token, size_t token_len);
int ossl_quic_get_peer_token(SSL_CTX *ctx, BIO_ADDR *peer,
                             QUIC_TOKEN **token);
void ossl_quic_free_peer_token(QUIC_TOKEN *token);

__owur int ossl_quic_init(SSL *s);
void ossl_quic_deinit(SSL *s);
void ossl_quic_free(SSL *s);
int ossl_quic_reset(SSL *s);
int ossl_quic_clear(SSL *s);
__owur int ossl_quic_accept(SSL *s);
__owur int ossl_quic_connect(SSL *s);
__owur int ossl_quic_read(SSL *s, void *buf, size_t len, size_t *readbytes);
__owur int ossl_quic_peek(SSL *s, void *buf, size_t len, size_t *readbytes);
__owur int ossl_quic_write_flags(SSL *s, const void *buf, size_t len,
                                 uint64_t flags, size_t *written);
__owur int ossl_quic_write(SSL *s, const void *buf, size_t len, size_t *written);
__owur long ossl_quic_ctrl(SSL *s, int cmd, long larg, void *parg);
__owur long ossl_quic_ctx_ctrl(SSL_CTX *ctx, int cmd, long larg, void *parg);
__owur long ossl_quic_callback_ctrl(SSL *s, int cmd, void (*fp) (void));
__owur long ossl_quic_ctx_callback_ctrl(SSL_CTX *ctx, int cmd, void (*fp) (void));
__owur size_t ossl_quic_pending(const SSL *s);
__owur int ossl_quic_key_update(SSL *s, int update_type);
__owur int ossl_quic_get_key_update_type(const SSL *s);
__owur const SSL_CIPHER *ossl_quic_get_cipher_by_char(const unsigned char *p);
__owur int ossl_quic_num_ciphers(void);
__owur const SSL_CIPHER *ossl_quic_get_cipher(unsigned int u);
int ossl_quic_renegotiate_check(SSL *ssl, int initok);

int ossl_quic_do_handshake(SSL *s);
int ossl_quic_set_connect_state(SSL *s, int raiseerrs);
int ossl_quic_set_accept_state(SSL *s, int raiseerrs);

__owur int ossl_quic_has_pending(const SSL *s);
__owur int ossl_quic_handle_events(SSL *s);
__owur int ossl_quic_get_event_timeout(SSL *s, struct timeval *tv,
                                       int *is_infinite);
OSSL_TIME ossl_quic_get_event_deadline(SSL *s);
__owur int ossl_quic_get_rpoll_descriptor(SSL *s, BIO_POLL_DESCRIPTOR *d);
__owur int ossl_quic_get_wpoll_descriptor(SSL *s, BIO_POLL_DESCRIPTOR *d);
__owur int ossl_quic_get_net_read_desired(SSL *s);
__owur int ossl_quic_get_net_write_desired(SSL *s);
__owur int ossl_quic_get_error(const SSL *s, int i);
__owur int ossl_quic_want(const SSL *s);
__owur int ossl_quic_conn_get_blocking_mode(const SSL *s);
__owur int ossl_quic_conn_set_blocking_mode(SSL *s, int blocking);
__owur int ossl_quic_conn_shutdown(SSL *s, uint64_t flags,
                                   const SSL_SHUTDOWN_EX_ARGS *args,
                                   size_t args_len);
__owur int ossl_quic_conn_stream_conclude(SSL *s);
void ossl_quic_conn_set0_net_rbio(SSL *s, BIO *net_wbio);
void ossl_quic_conn_set0_net_wbio(SSL *s, BIO *net_wbio);
BIO *ossl_quic_conn_get_net_rbio(const SSL *s);
BIO *ossl_quic_conn_get_net_wbio(const SSL *s);
__owur int ossl_quic_conn_set_initial_peer_addr(SSL *s,
                                                const BIO_ADDR *peer_addr);
__owur SSL *ossl_quic_conn_stream_new(SSL *s, uint64_t flags);
__owur SSL *ossl_quic_get0_connection(SSL *s);
__owur SSL *ossl_quic_get0_listener(SSL *s);
__owur SSL *ossl_quic_get0_domain(SSL *s);
__owur int ossl_quic_get_domain_flags(const SSL *s, uint64_t *domain_flags);
__owur int ossl_quic_get_stream_type(SSL *s);
__owur uint64_t ossl_quic_get_stream_id(SSL *s);
__owur int ossl_quic_is_stream_local(SSL *s);
__owur int ossl_quic_set_default_stream_mode(SSL *s, uint32_t mode);
__owur SSL *ossl_quic_detach_stream(SSL *s);
__owur int ossl_quic_attach_stream(SSL *conn, SSL *stream);
__owur int ossl_quic_set_incoming_stream_policy(SSL *s, int policy,
                                                uint64_t aec);
__owur SSL *ossl_quic_accept_stream(SSL *s, uint64_t flags);
__owur size_t ossl_quic_get_accept_stream_queue_len(SSL *s);
__owur int ossl_quic_get_value_uint(SSL *s, uint32_t class_, uint32_t id,
                                    uint64_t *value);
__owur int ossl_quic_set_value_uint(SSL *s, uint32_t class_, uint32_t id,
                                    uint64_t value);
__owur SSL *ossl_quic_accept_connection(SSL *ssl, uint64_t flags);
__owur size_t ossl_quic_get_accept_connection_queue_len(SSL *ssl);
__owur int ossl_quic_listen(SSL *ssl);

__owur int ossl_quic_stream_reset(SSL *ssl,
                                  const SSL_STREAM_RESET_ARGS *args,
                                  size_t args_len);

__owur int ossl_quic_get_stream_read_state(SSL *ssl);
__owur int ossl_quic_get_stream_write_state(SSL *ssl);
__owur int ossl_quic_get_stream_read_error_code(SSL *ssl,
                                                uint64_t *app_error_code);
__owur int ossl_quic_get_stream_write_error_code(SSL *ssl,
                                                 uint64_t *app_error_code);
__owur int ossl_quic_get_conn_close_info(SSL *ssl,
                                         SSL_CONN_CLOSE_INFO *info,
                                         size_t info_len);

uint64_t ossl_quic_set_options(SSL *s, uint64_t opts);
uint64_t ossl_quic_clear_options(SSL *s, uint64_t opts);
uint64_t ossl_quic_get_options(const SSL *s);

/* Modifies write buffer size for a stream. */
__owur int ossl_quic_set_write_buffer_size(SSL *s, size_t size);

/*
 * Used to override ossl_time_now() for debug purposes. While this may be
 * overridden at any time, expect strange results if you change it after
 * connecting.
 */
int ossl_quic_set_override_now_cb(SSL *s,
                                  OSSL_TIME (*now_cb)(void *arg),
                                  void *now_cb_arg);

/*
 * Condvar waiting in the assist thread doesn't support time faking as it relies
 * on the OS's notion of time, thus this is used in test code to force a
 * spurious wakeup instead.
 */
void ossl_quic_conn_force_assist_thread_wake(SSL *s);

/* For use by tests only. */
QUIC_CHANNEL *ossl_quic_conn_get_channel(SSL *s);

int ossl_quic_has_pending(const SSL *s);
int ossl_quic_get_shutdown(const SSL *s);

/*
 * Set qlog diagnostic title. String is copied internally on success and need
 * not remain allocated. Only has any effect if logging has not already begun.
 * For use by tests only. Setting this on a context affects any QCSO created
 * after this is called but does not affect QCSOs already created from a
 * context.
 */
int ossl_quic_set_diag_title(SSL_CTX *ctx, const char *title);

/* APIs used by the polling infrastructure */
int ossl_quic_conn_poll_events(SSL *ssl, uint64_t events, int do_tick,
                               uint64_t *revents);
int ossl_quic_get_notifier_fd(SSL *ssl);
void ossl_quic_enter_blocking_section(SSL *ssl, QUIC_REACTOR_WAIT_CTX *wctx);
void ossl_quic_leave_blocking_section(SSL *ssl, QUIC_REACTOR_WAIT_CTX *wctx);

# endif

#endif
