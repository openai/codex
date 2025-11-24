/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_LOCAL_H
# define OSSL_QUIC_LOCAL_H

# include <openssl/ssl.h>
# include "internal/quic_ssl.h"       /* QUIC_CONNECTION */
# include "internal/quic_txp.h"
# include "internal/quic_statm.h"
# include "internal/quic_demux.h"
# include "internal/quic_record_rx.h"
# include "internal/quic_tls.h"
# include "internal/quic_fc.h"
# include "internal/quic_stream.h"
# include "internal/quic_channel.h"
# include "internal/quic_reactor.h"
# include "internal/quic_thread_assist.h"
# include "../ssl_local.h"
# include "quic_obj_local.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC stream SSL object (QSSO) type. This implements the API personality layer
 * for QSSO objects, wrapping the QUIC-native QUIC_STREAM object and tracking
 * state required by the libssl API personality.
 */
struct quic_xso_st {
    /* QUIC_OBJ common header, including SSL object common header. */
    QUIC_OBJ                        obj;

    /* The connection this stream is associated with. Always non-NULL. */
    QUIC_CONNECTION                 *conn;

    /* The stream object. Always non-NULL for as long as the XSO exists. */
    QUIC_STREAM                     *stream;

    /* The application has retired a FIN (i.e. SSL_ERROR_ZERO_RETURN). */
    unsigned int                    retired_fin             : 1;

    /*
     * The application has requested a reset. Not set for reflexive
     * STREAM_RESETs caused by peer STOP_SENDING.
     */
    unsigned int                    requested_reset         : 1;

    /*
     * This state tracks SSL_write all-or-nothing (AON) write semantics
     * emulation.
     *
     * Example chronology:
     *
     *   t=0:  aon_write_in_progress=0
     *   t=1:  SSL_write(ssl, b1, l1) called;
     *         too big to enqueue into sstream at once, SSL_ERROR_WANT_WRITE;
     *         aon_write_in_progress=1; aon_buf_base=b1; aon_buf_len=l1;
     *         aon_buf_pos < l1 (depends on how much room was in sstream);
     *   t=2:  SSL_write(ssl, b2, l2);
     *         b2 must equal b1 (validated unless ACCEPT_MOVING_WRITE_BUFFER)
     *         l2 must equal l1 (always validated)
     *         append into sstream from [b2 + aon_buf_pos, b2 + aon_buf_len)
     *         if done, aon_write_in_progress=0
     *
     */
    /* Is an AON write in progress? */
    unsigned int                    aon_write_in_progress   : 1;

    /*
     * The base buffer pointer the caller passed us for the initial AON write
     * call. We use this for validation purposes unless
     * ACCEPT_MOVING_WRITE_BUFFER is enabled.
     *
     * NOTE: We never dereference this, as the caller might pass a different
     * (but identical) buffer if using ACCEPT_MOVING_WRITE_BUFFER. It is for
     * validation by pointer comparison only.
     */
    const unsigned char             *aon_buf_base;
    /* The total length of the AON buffer being sent, in bytes. */
    size_t                          aon_buf_len;
    /*
     * The position in the AON buffer up to which we have successfully sent data
     * so far.
     */
    size_t                          aon_buf_pos;

    /* SSL_set_mode */
    uint32_t                        ssl_mode;

    /* SSL_set_options */
    uint64_t                        ssl_options;

    /*
     * Last 'normal' error during an app-level I/O operation, used by
     * SSL_get_error(); used to track data-path errors like SSL_ERROR_WANT_READ
     * and SSL_ERROR_WANT_WRITE.
     */
    int                             last_error;
};

/*
 * QUIC connection SSL object (QCSO) type. This implements the API personality
 * layer for QCSO objects, wrapping the QUIC-native QUIC_CHANNEL object.
 */
struct quic_conn_st {
    /*
     * QUIC_OBJ is a common header for QUIC APL objects, allowing objects of
     * these different types to be disambiguated at runtime and providing some
     * common fields.
     *
     * Note: This must come first in the QUIC_CONNECTION structure.
     */
    QUIC_OBJ                        obj;

    SSL                             *tls;

    /* The QLSO this connection belongs to, if any. */
    QUIC_LISTENER                   *listener;

    /* The QDSO this connection belongs to, if any. */
    QUIC_DOMAIN                     *domain;

    /* The QUIC engine representing the QUIC event domain. */
    QUIC_ENGINE                     *engine;

    /* The QUIC port representing the QUIC listener and socket. */
    QUIC_PORT                       *port;

    /*
     * The QUIC channel providing the core QUIC connection implementation. Note
     * that this is not instantiated until we actually start trying to do the
     * handshake. This is to allow us to gather information like whether we are
     * going to be in client or server mode before committing to instantiating
     * the channel, since we want to determine the channel arguments based on
     * that.
     *
     * The channel remains available after connection termination until the SSL
     * object is freed, thus (ch != NULL) iff (started == 1).
     */
    QUIC_CHANNEL                    *ch;

    /*
     * The mutex used to synchronise access to the QUIC_CHANNEL. We own this but
     * provide it to the channel.
     */
#if defined(OPENSSL_THREADS)
    CRYPTO_MUTEX                    *mutex;
#endif

    /*
     * If we have a default stream attached, this is the internal XSO
     * object. If there is no default stream, this is NULL.
     */
    QUIC_XSO                        *default_xso;

    /* Initial peer L4 address. */
    BIO_ADDR                        init_peer_addr;

#  ifndef OPENSSL_NO_QUIC_THREAD_ASSIST
    /* Manages thread for QUIC thread assisted mode. */
    QUIC_THREAD_ASSIST              thread_assist;
#  endif

    /* Number of XSOs allocated. Includes the default XSO, if any. */
    size_t                          num_xso;

    /* Have we started? */
    unsigned int                    started                 : 1;

    /*
     * This is 1 if we were instantiated using a QUIC server method
     * (for future use).
     */
    unsigned int                    as_server               : 1;

    /*
     * Has the application called SSL_set_accept_state? We require this to be
     * congruent with the value of as_server.
     */
    unsigned int                    as_server_state         : 1;

    /* Are we using thread assisted mode? Never changes after init. */
    unsigned int                    is_thread_assisted      : 1;

    /* Have we created a default XSO yet? */
    unsigned int                    default_xso_created     : 1;

    /*
     * Pre-TERMINATING shutdown phase in which we are flushing streams.
     * Monotonically transitions to 1.
     * New streams cannot be created in this state.
     */
    unsigned int                    shutting_down           : 1;

    /* Have we probed the BIOs for addressing support? */
    unsigned int                    addressing_probe_done   : 1;

    /* Are we using addressed mode (BIO_sendmmsg with non-NULL peer)? */
    unsigned int                    addressed_mode_w        : 1;
    unsigned int                    addressed_mode_r        : 1;

    /* Flag to indicate waiting on accept queue */
    unsigned int                    pending                 : 1;

    /* Default stream type. Defaults to SSL_DEFAULT_STREAM_MODE_AUTO_BIDI. */
    uint32_t                        default_stream_mode;

    /* SSL_set_mode. This is not used directly but inherited by new XSOs. */
    uint32_t                        default_ssl_mode;

    /* SSL_set_options. This is not used directly but inherited by new XSOs. */
    uint64_t                        default_ssl_options;

    /* SSL_set_incoming_stream_policy. */
    int                             incoming_stream_policy;
    uint64_t                        incoming_stream_aec;

    /*
     * Last 'normal' error during an app-level I/O operation, used by
     * SSL_get_error(); used to track data-path errors like SSL_ERROR_WANT_READ
     * and SSL_ERROR_WANT_WRITE.
     */
    int                             last_error;
};

/*
 * QUIC listener SSL object (QLSO) type. This implements the API personality
 * layer for QLSO objects, wrapping the QUIC-native QUIC_PORT object.
 */
struct quic_listener_st {
    /* QUIC_OBJ common header, including SSL object common header. */
    QUIC_OBJ                        obj;

    /* The QDSO this connection belongs to, if any. */
    QUIC_DOMAIN                     *domain;

    /* The QUIC engine representing the QUIC event domain. */
    QUIC_ENGINE                     *engine;

    /* The QUIC port representing the QUIC listener and socket. */
    QUIC_PORT                       *port;

#if defined(OPENSSL_THREADS)
    /*
     * The mutex used to synchronise access to the QUIC_ENGINE. We own this but
     * provide it to the engine.
     */
    CRYPTO_MUTEX                    *mutex;
#endif

    /* Have we started listening yet? */
    unsigned int                    listening               : 1;
};

/*
 * QUIC domain SSL object (QDSO) type. This implements the API personality layer
 * for QDSO objects, wrapping the QUIC-native QUIC_ENGINE object.
 */
struct quic_domain_st {
     /* QUIC_OBJ common header, including SSL object common header. */
    QUIC_OBJ                        obj;

    /* The QUIC engine representing the QUIC event domain. */
    QUIC_ENGINE                     *engine;

#if defined(OPENSSL_THREADS)
    /*
     * The mutex used to synchronise access to the QUIC_ENGINE. We own this but
     * provide it to the engine.
     */
    CRYPTO_MUTEX                    *mutex;
#endif
};

/* Internal calls to the QUIC CSM which come from various places. */
int ossl_quic_conn_on_handshake_confirmed(QUIC_CONNECTION *qc);

/*
 * To be called when a protocol violation occurs. The connection is torn down
 * with the given error code, which should be a OSSL_QUIC_ERR_* value. Reason
 * string is optional and copied if provided. frame_type should be 0 if not
 * applicable.
 */
void ossl_quic_conn_raise_protocol_error(QUIC_CONNECTION *qc,
                                         uint64_t error_code,
                                         uint64_t frame_type,
                                         const char *reason);

void ossl_quic_conn_on_remote_conn_close(QUIC_CONNECTION *qc,
                                         OSSL_QUIC_FRAME_CONN_CLOSE *f);

#  define OSSL_QUIC_ANY_VERSION 0xFFFFF
# endif

# define IMPLEMENT_quic_meth_func(version, func_name, q_accept, \
                                 q_connect, enc_data) \
const SSL_METHOD *func_name(void)  \
        { \
        static const SSL_METHOD func_name##_data= { \
                version, \
                0, \
                0, \
                ossl_quic_new, \
                ossl_quic_free, \
                ossl_quic_reset, \
                ossl_quic_init, \
                NULL /* clear */, \
                ossl_quic_deinit, \
                q_accept, \
                q_connect, \
                ossl_quic_read, \
                ossl_quic_peek, \
                ossl_quic_write, \
                NULL /* shutdown */, \
                NULL /* renegotiate */, \
                ossl_quic_renegotiate_check, \
                NULL /* read_bytes */, \
                NULL /* write_bytes */, \
                NULL /* dispatch_alert */, \
                ossl_quic_ctrl, \
                ossl_quic_ctx_ctrl, \
                ossl_quic_get_cipher_by_char, \
                NULL /* put_cipher_by_char */, \
                ossl_quic_pending, \
                ossl_quic_num_ciphers, \
                ossl_quic_get_cipher, \
                tls1_default_timeout, \
                &enc_data, \
                ssl_undefined_void_function, \
                ossl_quic_callback_ctrl, \
                ossl_quic_ctx_callback_ctrl, \
        }; \
        return &func_name##_data; \
        }

#endif
