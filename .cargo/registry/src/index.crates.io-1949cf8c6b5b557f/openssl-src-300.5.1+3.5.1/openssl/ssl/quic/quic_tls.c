/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <openssl/ssl.h>
#include "internal/recordmethod.h"
#include "internal/quic_tls.h"
#include "../ssl_local.h"
#include "internal/quic_record_util.h"
#include "internal/quic_error.h"
#include "internal/quic_types.h"
#include "internal/ssl_unwrap.h"

#define QUIC_TLS_FATAL(rl, ad, err) \
    do { \
        if ((rl) != NULL) (rl)->alert = (ad); \
        ERR_raise(ERR_LIB_SSL, (err)); \
        if ((rl) != NULL) (rl)->qtls->inerror = 1; \
    } while(0)

struct quic_tls_st {
    QUIC_TLS_ARGS args;

    /*
     * Transport parameters which client should send. Buffer lifetime must
     * exceed the lifetime of the QUIC_TLS object.
     */
    const unsigned char *local_transport_params;
    size_t local_transport_params_len;

    ERR_STATE *error_state;

    /*
     * QUIC error code (usually in the TLS Alert-mapped CRYPTO_ERR range). Valid
     * only if inerror is 1.
     */
    uint64_t error_code;

    /*
     * Error message with static storage duration. Valid only if inerror is 1.
     * Should be suitable for encapsulation in a CONNECTION_CLOSE frame.
     */
    const char *error_msg;

    /* Whether our SSL object for TLS has been configured for use in QUIC */
    unsigned int configured : 1;

    /* Set if we have hit any error state */
    unsigned int inerror : 1;

    /* Set if the handshake has completed */
    unsigned int complete : 1;

    /* Set if we have consumed the local transport parameters yet. */
    unsigned int local_transport_params_consumed : 1;
};

struct ossl_record_layer_st {
    QUIC_TLS *qtls;

    /* Protection level */
    int level;

    /* Only used for retry flags */
    BIO *dummybio;

    /* Number of bytes written so far if we are part way through a write */
    size_t written;

    /* If we are part way through a write, a copy of the template */
    OSSL_RECORD_TEMPLATE template;

    /*
     * If we hit an error, what alert code should be used
     */
    int alert;

    /* Amount of crypto stream data we read in the last call to quic_read_record */
    size_t recread;

    /* Amount of crypto stream data read but not yet released */
    size_t recunreleased;

    /* Callbacks */
    OSSL_FUNC_rlayer_msg_callback_fn *msg_callback;
    void *cbarg;
};

static int quic_set1_bio(OSSL_RECORD_LAYER *rl, BIO *bio);
static int quic_free(OSSL_RECORD_LAYER *r);

static int
quic_new_record_layer(OSSL_LIB_CTX *libctx, const char *propq, int vers,
                      int role, int direction, int level, uint16_t epoch,
                      unsigned char *secret, size_t secretlen,
                      unsigned char *key, size_t keylen, unsigned char *iv,
                      size_t ivlen, unsigned char *mackey, size_t mackeylen,
                      const EVP_CIPHER *ciph, size_t taglen,
                      int mactype,
                      const EVP_MD *md, COMP_METHOD *comp,
                      const EVP_MD *kdfdigest, BIO *prev, BIO *transport,
                      BIO *next, BIO_ADDR *local, BIO_ADDR *peer,
                      const OSSL_PARAM *settings, const OSSL_PARAM *options,
                      const OSSL_DISPATCH *fns, void *cbarg, void *rlarg,
                      OSSL_RECORD_LAYER **retrl)
{
    OSSL_RECORD_LAYER *rl = OPENSSL_zalloc(sizeof(*rl));
    int qdir;
    uint32_t suite_id = 0;

    if (rl == NULL) {
        QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    rl->qtls = (QUIC_TLS *)rlarg;
    rl->level = level;
    if (!quic_set1_bio(rl, transport)) {
        QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    rl->cbarg = cbarg;
    *retrl = rl;

    if (fns != NULL) {
        for (; fns->function_id != 0; fns++) {
            switch (fns->function_id) {
                break;
            case OSSL_FUNC_RLAYER_MSG_CALLBACK:
                rl->msg_callback = OSSL_FUNC_rlayer_msg_callback(fns);
                break;
            default:
                /* Just ignore anything we don't understand */
                break;
            }
        }
    }

    if (level == OSSL_RECORD_PROTECTION_LEVEL_NONE)
        return 1;

    if (direction == OSSL_RECORD_DIRECTION_READ)
        qdir = 0;
    else
        qdir = 1;

    if (rl->qtls->args.ossl_quic) {
#ifndef OPENSSL_NO_QUIC
        /*
         * We only look up the suite_id/MD for internal callers. Not used in the
         * public API. We assume that a 3rd party QUIC stack will want to
         * figure this out by itself (e.g. so that they could add new
         * ciphersuites at a different pace to us)
         */
        if (EVP_CIPHER_is_a(ciph, "AES-128-GCM")) {
            suite_id = QRL_SUITE_AES128GCM;
        } else if (EVP_CIPHER_is_a(ciph, "AES-256-GCM")) {
            suite_id = QRL_SUITE_AES256GCM;
        } else if (EVP_CIPHER_is_a(ciph, "CHACHA20-POLY1305")) {
            suite_id = QRL_SUITE_CHACHA20POLY1305;
        } else {
            QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, SSL_R_UNKNOWN_CIPHER_TYPE);
            goto err;
        }

        /* We pass a ref to the md in a successful yield_secret_cb call */
        /* TODO(QUIC FUTURE): This cast is horrible. We should try and remove it */
        if (!EVP_MD_up_ref((EVP_MD *)kdfdigest)) {
            QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            goto err;
        }
#else
        if (!ossl_assert("Should not happen" == NULL))
            goto err;
#endif
    } else {
        kdfdigest = NULL;
    }

    if (!rl->qtls->args.yield_secret_cb(level, qdir, suite_id,
                                        (EVP_MD *)kdfdigest, secret, secretlen,
                                        rl->qtls->args.yield_secret_cb_arg)) {
        QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        EVP_MD_free((EVP_MD *)kdfdigest);
        goto err;
    }

    return 1;
 err:
    *retrl = NULL;
    quic_free(rl);
    return 0;
}

static int quic_free(OSSL_RECORD_LAYER *rl)
{
    if (rl == NULL)
        return 1;

    BIO_free(rl->dummybio);
    OPENSSL_free(rl);
    return 1;
}

static int quic_unprocessed_read_pending(OSSL_RECORD_LAYER *rl)
{
    /*
     * Read ahead isn't really a thing for QUIC so we never have unprocessed
     * data pending
     */
    return 0;
}

static int quic_processed_read_pending(OSSL_RECORD_LAYER *rl)
{
    /*
     * This is currently only ever used by:
     * - SSL_has_pending()
     * - to check whether we have more records that we want to supply to the
     *   upper layers
     *
     * We only ever supply 1 record at a time to the upper layers, and
     * SSL_has_pending() will go via the QUIC method not the TLS method so that
     * use case doesn't apply here.
     * Therefore we can ignore this for now and always return 0. We might
     * eventually want to change this to check in the receive buffers to see if
     * we have any more data pending.
     */
    return 0;
}

static size_t quic_get_max_records(OSSL_RECORD_LAYER *rl, uint8_t type,
                                   size_t len,
                                   size_t maxfrag, size_t *preffrag)
{
    return 1;
}

static int quic_write_records(OSSL_RECORD_LAYER *rl,
                              OSSL_RECORD_TEMPLATE *template,
                              size_t numtempl)
{
    size_t consumed;
    unsigned char alert;

    if (!ossl_assert(numtempl == 1)) {
        /* How could this be? quic_get_max_records() always returns 1 */
        QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    BIO_clear_retry_flags(rl->dummybio);

    if (rl->msg_callback != NULL) {
        unsigned char dummyrec[SSL3_RT_HEADER_LENGTH];

        /*
         * For the purposes of the callback we "pretend" to be normal TLS,
         * and manufacture a dummy record header
         */
        dummyrec[0] = (rl->level == OSSL_RECORD_PROTECTION_LEVEL_NONE)
                        ? template->type
                        : SSL3_RT_APPLICATION_DATA;
        dummyrec[1] = (unsigned char)((template->version >> 8) & 0xff);
        dummyrec[2] = (unsigned char)(template->version & 0xff);
        /*
         * We assume that buflen is always <= UINT16_MAX. Since this is
         * generated by libssl itself we actually expect it to never
         * exceed SSL3_RT_MAX_PLAIN_LENGTH - so it should be a safe
         * assumption
         */
        dummyrec[3] = (unsigned char)((template->buflen >> 8) & 0xff);
        dummyrec[4] = (unsigned char)(template->buflen & 0xff);

        rl->msg_callback(1, TLS1_3_VERSION, SSL3_RT_HEADER, dummyrec,
                            SSL3_RT_HEADER_LENGTH, rl->cbarg);

        if (rl->level != OSSL_RECORD_PROTECTION_LEVEL_NONE) {
            rl->msg_callback(1, TLS1_3_VERSION, SSL3_RT_INNER_CONTENT_TYPE,
                             &template->type, 1, rl->cbarg);
        }
    }

    switch (template->type) {
    case SSL3_RT_ALERT:
        if (template->buflen != 2) {
            /*
             * We assume that libssl always sends both bytes of an alert to
             * us in one go, and never fragments it. If we ever get more
             * or less bytes than exactly 2 then this is very unexpected.
             */
            QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, SSL_R_BAD_VALUE);
            return OSSL_RECORD_RETURN_FATAL;
        }
        /*
         * Byte 0 is the alert level (we ignore it) and byte 1 is the alert
         * description that we are actually interested in.
         */
        alert = template->buf[1];

        if (!rl->qtls->args.alert_cb(rl->qtls->args.alert_cb_arg, alert)) {
            QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return OSSL_RECORD_RETURN_FATAL;
        }
        break;

    case SSL3_RT_HANDSHAKE:
        /*
         * We expect this to only fail on some fatal error (e.g. malloc
         * failure)
         */
        if (!rl->qtls->args.crypto_send_cb(template->buf + rl->written,
                                           template->buflen - rl->written,
                                           &consumed,
                                           rl->qtls->args.crypto_send_cb_arg)) {
            QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return OSSL_RECORD_RETURN_FATAL;
        }
        /*
         * We might have written less than we wanted to if we have filled the
         * send stream buffer.
         */
        if (consumed + rl->written != template->buflen) {
            if (!ossl_assert(consumed + rl->written < template->buflen)) {
                QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
                return OSSL_RECORD_RETURN_FATAL;
            }

            /*
             * We've not written everything we wanted to. Take a copy of the
             * template, remember how much we wrote so far and signal a retry.
             * The buffer supplied in the template is guaranteed to be the same
             * on a retry for handshake data
             */
            rl->written += consumed;
            rl->template = *template;
            BIO_set_retry_write(rl->dummybio);

            return OSSL_RECORD_RETURN_RETRY;
        }
        rl->written = 0;
        break;

    default:
        /* Anything else is unexpected and an error */
        QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    return OSSL_RECORD_RETURN_SUCCESS;
}

static int quic_retry_write_records(OSSL_RECORD_LAYER *rl)
{
    return quic_write_records(rl, &rl->template, 1);
}

static int quic_read_record(OSSL_RECORD_LAYER *rl, void **rechandle,
                            int *rversion, uint8_t *type, const unsigned char **data,
                            size_t *datalen, uint16_t *epoch,
                            unsigned char *seq_num)
{
    if (rl->recread != 0 || rl->recunreleased != 0)
        return OSSL_RECORD_RETURN_FATAL;

    BIO_clear_retry_flags(rl->dummybio);

    if (!rl->qtls->args.crypto_recv_rcd_cb(data, datalen,
                                           rl->qtls->args.crypto_recv_rcd_cb_arg)) {
        QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    if (*datalen == 0) {
        BIO_set_retry_read(rl->dummybio);
        return OSSL_RECORD_RETURN_RETRY;
    }

    *rechandle = rl;
    *rversion = TLS1_3_VERSION;
    *type = SSL3_RT_HANDSHAKE;
    rl->recread = rl->recunreleased = *datalen;
    /* epoch/seq_num are not relevant for TLS */

    if (rl->msg_callback != NULL) {
        unsigned char dummyrec[SSL3_RT_HEADER_LENGTH];

        /*
         * For the purposes of the callback we "pretend" to be normal TLS,
         * and manufacture a dummy record header
         */
        dummyrec[0] = (rl->level == OSSL_RECORD_PROTECTION_LEVEL_NONE)
                      ? SSL3_RT_HANDSHAKE
                      : SSL3_RT_APPLICATION_DATA;
        dummyrec[1] = (unsigned char)((TLS1_2_VERSION >> 8) & 0xff);
        dummyrec[2] = (unsigned char)(TLS1_2_VERSION & 0xff);
        /*
         * *datalen will always fit into 2 bytes because our original buffer
         * size is less than that.
         */
        dummyrec[3] = (unsigned char)((*datalen >> 8) & 0xff);
        dummyrec[4] = (unsigned char)(*datalen & 0xff);

        rl->msg_callback(0, TLS1_3_VERSION, SSL3_RT_HEADER, dummyrec,
                         SSL3_RT_HEADER_LENGTH, rl->cbarg);
        rl->msg_callback(0, TLS1_3_VERSION, SSL3_RT_INNER_CONTENT_TYPE, type, 1,
                         rl->cbarg);
    }

    return OSSL_RECORD_RETURN_SUCCESS;
}

static int quic_release_record(OSSL_RECORD_LAYER *rl, void *rechandle,
                               size_t length)
{
    if (!ossl_assert(rl->recread > 0)
            || !ossl_assert(rl->recunreleased <= rl->recread)
            || !ossl_assert(rl == rechandle)
            || !ossl_assert(length <= rl->recunreleased)) {
        QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return OSSL_RECORD_RETURN_FATAL;
    }

    if (rl->recunreleased == length) {
        if (!rl->qtls->args.crypto_release_rcd_cb(rl->recread,
                                                  rl->qtls->args.crypto_release_rcd_cb_arg)) {
            QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
            return OSSL_RECORD_RETURN_FATAL;
        }
        rl->recread = 0;
    }
    rl->recunreleased -= length;
    return OSSL_RECORD_RETURN_SUCCESS;
}

static int quic_get_alert_code(OSSL_RECORD_LAYER *rl)
{
    return rl->alert;
}

static int quic_set_protocol_version(OSSL_RECORD_LAYER *rl, int version)
{
    /* We only support TLSv1.3, so its bad if we negotiate anything else */
    if (!ossl_assert(version == TLS1_3_VERSION)) {
        QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    return 1;
}

static void quic_set_plain_alerts(OSSL_RECORD_LAYER *rl, int allow)
{
    /* We don't care */
}

static void quic_set_first_handshake(OSSL_RECORD_LAYER *rl, int first)
{
    /* We don't care */
}

static void quic_set_max_pipelines(OSSL_RECORD_LAYER *rl, size_t max_pipelines)
{
    /* We don't care */
}

static void quic_get_state(OSSL_RECORD_LAYER *rl, const char **shortstr,
                    const char **longstr)
{
    /*
     * According to the docs, valid read state strings are: "RH"/"read header",
     * "RB"/"read body", and "unknown"/"unknown". We don't read records in quite
     * that way, so we report every "normal" state as "read header". In the
     * event of error then we report "unknown".
     */

    if (rl->qtls->inerror) {
        if (shortstr != NULL)
            *shortstr = "unknown";
        if (longstr != NULL)
            *longstr = "unknown";
    } else {
        if (shortstr != NULL)
            *shortstr = "RH";
        if (longstr != NULL)
            *longstr = "read header";
    }
}

static int quic_set_options(OSSL_RECORD_LAYER *rl, const OSSL_PARAM *options)
{
    /*
     * We don't support any options yet - but we might do at some point so
     * this could be useful.
     */
    return 1;
}

static const COMP_METHOD *quic_get_compression(OSSL_RECORD_LAYER *rl)
{
    /* We only support TLSv1.3 which doesn't have compression */
    return NULL;
}

static void quic_set_max_frag_len(OSSL_RECORD_LAYER *rl, size_t max_frag_len)
{
    /* This really doesn't make any sense for QUIC. Ignore it */
}

static int quic_alloc_buffers(OSSL_RECORD_LAYER *rl)
{
    /*
     * This is a hint only. We don't support it (yet), so just ignore the
     * request
     */
    return 1;
}

static int quic_free_buffers(OSSL_RECORD_LAYER *rl)
{
    /*
     * This is a hint only. We don't support it (yet), so just ignore the
     * request
     */
    return 1;
}

static int quic_set1_bio(OSSL_RECORD_LAYER *rl, BIO *bio)
{
    if (bio != NULL && !BIO_up_ref(bio))
        return 0;
    BIO_free(rl->dummybio);
    rl->dummybio = bio;

    return 1;
}

/*
 * Never called functions
 *
 * Due to the way we are configured and used we never expect any of the next set
 * of functions to be called. Therefore we set them to always fail.
 */

static size_t quic_app_data_pending(OSSL_RECORD_LAYER *rl)
{
    QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    return (size_t)ossl_assert(0);
}

static size_t quic_get_max_record_overhead(OSSL_RECORD_LAYER *rl)
{
    QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    return (size_t)ossl_assert(0);
}

static int quic_increment_sequence_ctr(OSSL_RECORD_LAYER *rl)
{
    QUIC_TLS_FATAL(rl, SSL_AD_INTERNAL_ERROR, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    return ossl_assert(0);
}

/* End of never called functions */

static const OSSL_RECORD_METHOD quic_tls_record_method = {
    quic_new_record_layer,
    quic_free,
    quic_unprocessed_read_pending,
    quic_processed_read_pending,
    quic_app_data_pending, /* Never called */
    quic_get_max_records,
    quic_write_records,
    quic_retry_write_records,
    quic_read_record,
    quic_release_record,
    quic_get_alert_code,
    quic_set1_bio,
    quic_set_protocol_version,
    quic_set_plain_alerts,
    quic_set_first_handshake,
    quic_set_max_pipelines,
    NULL, /* set_in_init: Optional - we don't need it */
    quic_get_state,
    quic_set_options,
    quic_get_compression,
    quic_set_max_frag_len,
    quic_get_max_record_overhead, /* Never called */
    quic_increment_sequence_ctr, /* Never called */
    quic_alloc_buffers,
    quic_free_buffers
};

static int add_transport_params_cb(SSL *s, unsigned int ext_type,
                                   unsigned int context,
                                   const unsigned char **out, size_t *outlen,
                                   X509 *x, size_t chainidx, int *al,
                                   void *add_arg)
{
    QUIC_TLS *qtls = add_arg;

    *out = qtls->local_transport_params;
    *outlen = qtls->local_transport_params_len;
    qtls->local_transport_params_consumed = 1;
    return 1;
}

static void free_transport_params_cb(SSL *s, unsigned int ext_type,
                                     unsigned int context,
                                     const unsigned char *out,
                                     void *add_arg)
{
}

static int parse_transport_params_cb(SSL *s, unsigned int ext_type,
                                     unsigned int context,
                                     const unsigned char *in,
                                     size_t inlen, X509 *x,
                                     size_t chainidx,
                                     int *al, void *parse_arg)
{
    QUIC_TLS *qtls = parse_arg;

    return qtls->args.got_transport_params_cb(in, inlen,
                                              qtls->args.got_transport_params_cb_arg);
}

QUIC_TLS *ossl_quic_tls_new(const QUIC_TLS_ARGS *args)
{
    QUIC_TLS *qtls;

    if (args->crypto_send_cb == NULL
        || args->crypto_recv_rcd_cb == NULL
        || args->crypto_release_rcd_cb == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    qtls = OPENSSL_zalloc(sizeof(*qtls));
    if (qtls == NULL)
        return NULL;

    if (args->ossl_quic && (qtls->error_state = OSSL_ERR_STATE_new()) == NULL) {
        OPENSSL_free(qtls);
        return NULL;
    }

    qtls->args = *args;
    return qtls;
}

void ossl_quic_tls_free(QUIC_TLS *qtls)
{
    if (qtls == NULL)
        return;
    OSSL_ERR_STATE_free(qtls->error_state);
    OPENSSL_free(qtls);
}

static int raise_error(QUIC_TLS *qtls, uint64_t error_code,
                       const char *error_msg,
                       const char *src_file,
                       int src_line,
                       const char *src_func)
{
    /*
     * When QTLS fails, add a "cover letter" error with information, potentially
     * with any underlying libssl errors underneath it (but our cover error may
     * be the only error in some cases). Then capture this into an ERR_STATE so
     * we can report it later if need be when the QUIC_CHANNEL asks for it.
     * For external QUIC TLS we just raise the error.
     */
    ERR_new();
    ERR_set_debug(src_file, src_line, src_func);
    ERR_set_error(ERR_LIB_SSL, SSL_R_QUIC_HANDSHAKE_LAYER_ERROR,
                  "handshake layer error, error code %llu (0x%llx) (\"%s\")",
                  error_code, error_code, error_msg);

    if (qtls->args.ossl_quic) {
        OSSL_ERR_STATE_save_to_mark(qtls->error_state);

        /*
         * We record the error information reported via the QUIC protocol
         * separately.
         */
        qtls->error_code        = error_code;
        qtls->error_msg         = error_msg;
        qtls->inerror           = 1;

        ERR_pop_to_mark();
    }
    return 0;
}

#define RAISE_ERROR(qtls, error_code, error_msg) \
    raise_error((qtls), (error_code), (error_msg), \
                OPENSSL_FILE, OPENSSL_LINE, OPENSSL_FUNC)

#ifndef OPENSSL_NO_QUIC
# define RAISE_INTERNAL_ERROR(qtls) \
    RAISE_ERROR((qtls), OSSL_QUIC_ERR_INTERNAL_ERROR, "internal error")
#else
# define RAISE_INTERNAL_ERROR(qtls) \
    RAISE_ERROR((qtls), 0x01, "internal error")
#endif

int ossl_quic_tls_configure(QUIC_TLS *qtls)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(qtls->args.s);
    BIO *nullbio;

    if (sc == NULL || !SSL_set_min_proto_version(qtls->args.s, TLS1_3_VERSION))
        return RAISE_INTERNAL_ERROR(qtls);

    nullbio = BIO_new(BIO_s_null());
    if (nullbio == NULL)
        return RAISE_INTERNAL_ERROR(qtls);

    /*
     * Our custom record layer doesn't use the BIO - but libssl generally
     * expects one to be present.
     */
    SSL_set_bio(qtls->args.s, nullbio, nullbio);

    SSL_clear_options(qtls->args.s, SSL_OP_ENABLE_MIDDLEBOX_COMPAT);
    ossl_ssl_set_custom_record_layer(sc, &quic_tls_record_method, qtls);

    if (!ossl_tls_add_custom_ext_intern(NULL, &sc->cert->custext,
                                        qtls->args.is_server ? ENDPOINT_SERVER
                                                             : ENDPOINT_CLIENT,
                                        TLSEXT_TYPE_quic_transport_parameters,
                                        SSL_EXT_TLS1_3_ONLY
                                        | SSL_EXT_CLIENT_HELLO
                                        | SSL_EXT_TLS1_3_ENCRYPTED_EXTENSIONS,
                                        add_transport_params_cb,
                                        free_transport_params_cb, qtls,
                                        parse_transport_params_cb, qtls))
        return 0;

    sc->s3.flags |= TLS1_FLAGS_QUIC;

    return 1;
}

#ifndef OPENSSL_NO_QUIC
int ossl_quic_tls_tick(QUIC_TLS *qtls)
{
    int ret, err;
    const unsigned char *alpn;
    unsigned int alpnlen;

    if (qtls->inerror)
        return 0;

    /*
     * SSL_get_error does not truly know what the cause of an SSL_read failure
     * is and to some extent guesses based on contextual information. In
     * particular, if there is _any_ ERR on the error stack, SSL_ERROR_SSL or
     * SSL_ERROR_SYSCALL will be returned no matter what and there is no
     * possibility of SSL_ERROR_WANT_READ/WRITE being returned, even if that was
     * the actual cause of the SSL_read() failure.
     *
     * This means that ordinarily, the below code might not work right if the
     * application has any ERR on the error stack. In order to make this code
     * perform correctly regardless of prior ERR state, we use a variant of
     * SSL_get_error() which ignores the error stack. However, some ERRs are
     * raised by SSL_read() and actually indicate that something has gone wrong
     * during the call to SSL_read(). We therefore adopt a strategy of marking
     * the ERR stack and seeing if any errors get appended during the call to
     * SSL_read(). If they are, we assume SSL_read() has raised an error and
     * that we should use normal SSL_get_error() handling.
     *
     * NOTE: Ensure all escape paths from this function call
     * ERR_clear_to_mark(). The RAISE macros handle this in failure cases.
     */
    ERR_set_mark();

    if (!qtls->configured) {
        SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(qtls->args.s);
        SSL_CTX *sctx;

        if (sc == NULL)
            return RAISE_INTERNAL_ERROR(qtls);
        sctx = SSL_CONNECTION_GET_CTX(sc);

        /*
         * No matter how the user has configured us, there are certain
         * requirements for QUIC-TLS that we enforce
         */

        /* ALPN is a requirement for QUIC and must be set */
        if (qtls->args.is_server) {
            if (sctx->ext.alpn_select_cb == NULL)
                return RAISE_INTERNAL_ERROR(qtls);
        } else {
            if (sc->ext.alpn == NULL || sc->ext.alpn_len == 0)
                return RAISE_ERROR(qtls, OSSL_QUIC_ERR_CRYPTO_NO_APP_PROTO,
                                   "ALPN must be configured when using QUIC");
        }

        if (!ossl_quic_tls_configure(qtls))
            return RAISE_INTERNAL_ERROR(qtls);

        sc->s3.flags |= TLS1_FLAGS_QUIC_INTERNAL;

        if (qtls->args.is_server)
            SSL_set_accept_state(qtls->args.s);
        else
            SSL_set_connect_state(qtls->args.s);

        qtls->configured = 1;
    }

    if (qtls->complete)
        /*
         * There should never be app data to read, but calling SSL_read() will
         * ensure any post-handshake messages are processed.
         */
        ret = SSL_read(qtls->args.s, NULL, 0);
    else
        ret = SSL_do_handshake(qtls->args.s);

    if (ret <= 0) {
        err = ossl_ssl_get_error(qtls->args.s, ret,
                                 /*check_err=*/ERR_count_to_mark() > 0);

        switch (err) {
        case SSL_ERROR_WANT_READ:
        case SSL_ERROR_WANT_WRITE:
        case SSL_ERROR_WANT_CLIENT_HELLO_CB:
        case SSL_ERROR_WANT_X509_LOOKUP:
        case SSL_ERROR_WANT_RETRY_VERIFY:
            ERR_pop_to_mark();
            return 1;

        default:
            return RAISE_INTERNAL_ERROR(qtls);
        }
    }

    if (!qtls->complete) {
        /* Validate that we have ALPN */
        SSL_get0_alpn_selected(qtls->args.s, &alpn, &alpnlen);
        if (alpn == NULL || alpnlen == 0)
            return RAISE_ERROR(qtls, OSSL_QUIC_ERR_CRYPTO_NO_APP_PROTO,
                               "no application protocol negotiated");

        qtls->complete = 1;
        ERR_pop_to_mark();
        return qtls->args.handshake_complete_cb(qtls->args.handshake_complete_cb_arg);
    }

    ERR_pop_to_mark();
    return 1;
}
#endif

void ossl_quic_tls_clear(QUIC_TLS *qtls)
{
    if (qtls == NULL)
        return;
    qtls->local_transport_params_consumed = 0;
}

int ossl_quic_tls_set_transport_params(QUIC_TLS *qtls,
                                       const unsigned char *transport_params,
                                       size_t transport_params_len)
{
    if (qtls->local_transport_params_consumed)
        return 0;

    qtls->local_transport_params       = transport_params;
    qtls->local_transport_params_len   = transport_params_len;
    return 1;
}

int ossl_quic_tls_get_error(QUIC_TLS *qtls,
                            uint64_t *error_code,
                            const char **error_msg,
                            ERR_STATE **error_state)
{
    if (qtls->inerror) {
        *error_code     = qtls->error_code;
        *error_msg      = qtls->error_msg;
        *error_state    = qtls->error_state;
    }

    return qtls->inerror;
}

/*
 * Returns true if the last handshake record message we processed was a
 * CertificateRequest
 */
int ossl_quic_tls_is_cert_request(QUIC_TLS *qtls)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(qtls->args.s);

    if (sc == NULL)
        return 0;

    return sc->s3.tmp.message_type == SSL3_MT_CERTIFICATE_REQUEST;
}

/*
 * Returns true if the last session associated with the connection has an
 * invalid max_early_data value for QUIC.
 */
int ossl_quic_tls_has_bad_max_early_data(QUIC_TLS *qtls)
{
    uint32_t max_early_data = SSL_get0_session(qtls->args.s)->ext.max_early_data;

    /*
     * If max_early_data was present we always ensure a non-zero value is
     * stored in the session for QUIC. Therefore if max_early_data == 0 here
     * we can be confident that it was not present in the NewSessionTicket
     */
    return max_early_data != 0xffffffff && max_early_data != 0;
}

int ossl_quic_tls_set_early_data_enabled(QUIC_TLS *qtls, int enabled)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(qtls->args.s);

    if (sc == NULL || !SSL_IS_QUIC_HANDSHAKE(sc) || !SSL_in_before(qtls->args.s))
        return 0;

    if (!enabled) {
        sc->max_early_data = 0;
        sc->early_data_state = SSL_EARLY_DATA_NONE;
        return 1;
    }

    if (sc->server) {
        sc->max_early_data = 0xffffffff;
        sc->early_data_state = SSL_EARLY_DATA_ACCEPTING;
        return 1;
    }

    if ((sc->session == NULL || sc->session->ext.max_early_data != 0xffffffff)
        && sc->psk_use_session_cb == NULL)
        return 0;

    sc->early_data_state = SSL_EARLY_DATA_CONNECTING;
    return 1;
}
