/*
 * Copyright 2015-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*****************************************************************************
 *                                                                           *
 * The following definitions are PRIVATE to the state machine. They should   *
 * NOT be used outside of the state machine.                                 *
 *                                                                           *
 *****************************************************************************/

/* Max message length definitions */

/* The spec allows for a longer length than this, but we limit it */
#define HELLO_VERIFY_REQUEST_MAX_LENGTH 258
#define END_OF_EARLY_DATA_MAX_LENGTH    0
#define HELLO_RETRY_REQUEST_MAX_LENGTH  20000
#define ENCRYPTED_EXTENSIONS_MAX_LENGTH 20000
#define SESSION_TICKET_MAX_LENGTH_TLS13 131338
#define SESSION_TICKET_MAX_LENGTH_TLS12 65541
#define SERVER_KEY_EXCH_MAX_LENGTH      102400
#define SERVER_HELLO_DONE_MAX_LENGTH    0
#define KEY_UPDATE_MAX_LENGTH           1
#define CCS_MAX_LENGTH                  1

/* Max ServerHello size permitted by RFC 8446 */
#define SERVER_HELLO_MAX_LENGTH         65607

/* Max CertificateVerify size permitted by RFC 8446 */
#define CERTIFICATE_VERIFY_MAX_LENGTH   65539

/* Max should actually be 36 but we are generous */
#define FINISHED_MAX_LENGTH             64

/* Dummy message type */
#define SSL3_MT_DUMMY   -1

/* Invalid extension ID for non-supported extensions */
#define TLSEXT_TYPE_invalid            0x10000
#define TLSEXT_TYPE_out_of_range       0x10001
unsigned int ossl_get_extension_type(size_t idx);

extern const unsigned char hrrrandom[];

/* Message processing return codes */
typedef enum {
    /* Something bad happened */
    MSG_PROCESS_ERROR,
    /* We've finished reading - swap to writing */
    MSG_PROCESS_FINISHED_READING,
    /*
     * We've completed the main processing of this message but there is some
     * post processing to be done.
     */
    MSG_PROCESS_CONTINUE_PROCESSING,
    /* We've finished this message - read the next message */
    MSG_PROCESS_CONTINUE_READING
} MSG_PROCESS_RETURN;

typedef CON_FUNC_RETURN (*confunc_f) (SSL_CONNECTION *s, WPACKET *pkt);

int ssl3_take_mac(SSL_CONNECTION *s);
int check_in_list(SSL_CONNECTION *s, uint16_t group_id, const uint16_t *groups,
                  size_t num_groups, int checkallow, size_t *pos);
int create_synthetic_message_hash(SSL_CONNECTION *s,
                                  const unsigned char *hashval,
                                  size_t hashlen, const unsigned char *hrr,
                                  size_t hrrlen);
int parse_ca_names(SSL_CONNECTION *s, PACKET *pkt);
const STACK_OF(X509_NAME) *get_ca_names(SSL_CONNECTION *s);
int construct_ca_names(SSL_CONNECTION *s, const STACK_OF(X509_NAME) *ca_sk,
                       WPACKET *pkt);
size_t construct_key_exchange_tbs(SSL_CONNECTION *s, unsigned char **ptbs,
                                  const void *param, size_t paramlen);

/*
 * TLS/DTLS client state machine functions
 */
int ossl_statem_client_read_transition(SSL_CONNECTION *s, int mt);
WRITE_TRAN ossl_statem_client_write_transition(SSL_CONNECTION *s);
WORK_STATE ossl_statem_client_pre_work(SSL_CONNECTION *s, WORK_STATE wst);
WORK_STATE ossl_statem_client_post_work(SSL_CONNECTION *s, WORK_STATE wst);
int ossl_statem_client_construct_message(SSL_CONNECTION *s,
                                         confunc_f *confunc, int *mt);
size_t ossl_statem_client_max_message_size(SSL_CONNECTION *s);
MSG_PROCESS_RETURN ossl_statem_client_process_message(SSL_CONNECTION *s,
                                                      PACKET *pkt);
WORK_STATE ossl_statem_client_post_process_message(SSL_CONNECTION *s,
                                                   WORK_STATE wst);

/*
 * TLS/DTLS server state machine functions
 */
int ossl_statem_server_read_transition(SSL_CONNECTION *s, int mt);
WRITE_TRAN ossl_statem_server_write_transition(SSL_CONNECTION *s);
WORK_STATE ossl_statem_server_pre_work(SSL_CONNECTION *s, WORK_STATE wst);
WORK_STATE ossl_statem_server_post_work(SSL_CONNECTION *s, WORK_STATE wst);
int ossl_statem_server_construct_message(SSL_CONNECTION *s,
                                         confunc_f *confunc,int *mt);
size_t ossl_statem_server_max_message_size(SSL_CONNECTION *s);
MSG_PROCESS_RETURN ossl_statem_server_process_message(SSL_CONNECTION *s,
                                                      PACKET *pkt);
WORK_STATE ossl_statem_server_post_process_message(SSL_CONNECTION *s,
                                                   WORK_STATE wst);

/* Functions for getting new message data */
__owur int tls_get_message_header(SSL_CONNECTION *s, int *mt);
__owur int tls_get_message_body(SSL_CONNECTION *s, size_t *len);
__owur int dtls_get_message(SSL_CONNECTION *s, int *mt);
__owur int dtls_get_message_body(SSL_CONNECTION *s, size_t *len);

/* Message construction and processing functions */
__owur int tls_process_initial_server_flight(SSL_CONNECTION *s);
__owur MSG_PROCESS_RETURN tls_process_change_cipher_spec(SSL_CONNECTION *s,
                                                         PACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_finished(SSL_CONNECTION *s, PACKET *pkt);
__owur CON_FUNC_RETURN  tls_construct_change_cipher_spec(SSL_CONNECTION *s,
                                                         WPACKET *pkt);
__owur CON_FUNC_RETURN dtls_construct_change_cipher_spec(SSL_CONNECTION *s,
                                                         WPACKET *pkt);

__owur CON_FUNC_RETURN tls_construct_finished(SSL_CONNECTION *s, WPACKET *pkt);
__owur CON_FUNC_RETURN tls_construct_key_update(SSL_CONNECTION *s, WPACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_key_update(SSL_CONNECTION *s,
                                                 PACKET *pkt);
__owur WORK_STATE tls_finish_handshake(SSL_CONNECTION *s, WORK_STATE wst,
                                       int clearbufs, int stop);
__owur WORK_STATE dtls_wait_for_dry(SSL_CONNECTION *s);

#ifndef OPENSSL_NO_COMP_ALG
__owur MSG_PROCESS_RETURN tls13_process_compressed_certificate(SSL_CONNECTION *sc,
                                                               PACKET *pkt,
                                                               PACKET *tmppkt,
                                                               BUF_MEM *buf);
#endif

/* some client-only functions */
__owur CON_FUNC_RETURN tls_construct_client_hello(SSL_CONNECTION *s,
                                                  WPACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_server_hello(SSL_CONNECTION *s,
                                                   PACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_certificate_request(SSL_CONNECTION *s,
                                                          PACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_new_session_ticket(SSL_CONNECTION *s,
                                                         PACKET *pkt);
__owur int tls_process_cert_status_body(SSL_CONNECTION *s, PACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_cert_status(SSL_CONNECTION *s,
                                                  PACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_server_done(SSL_CONNECTION *s,
                                                  PACKET *pkt);
__owur CON_FUNC_RETURN tls_construct_cert_verify(SSL_CONNECTION *s,
                                                 WPACKET *pkt);
__owur WORK_STATE tls_prepare_client_certificate(SSL_CONNECTION *s,
                                                 WORK_STATE wst);
__owur CON_FUNC_RETURN tls_construct_client_certificate(SSL_CONNECTION *s,
                                                        WPACKET *pkt);
#ifndef OPENSSL_NO_COMP_ALG
__owur CON_FUNC_RETURN tls_construct_client_compressed_certificate(SSL_CONNECTION *sc,
                                                                   WPACKET *pkt);
#endif
__owur int ssl_do_client_cert_cb(SSL_CONNECTION *s, X509 **px509,
                                 EVP_PKEY **ppkey);
__owur CON_FUNC_RETURN tls_construct_client_key_exchange(SSL_CONNECTION *s,
                                                         WPACKET *pkt);
__owur int tls_client_key_exchange_post_work(SSL_CONNECTION *s);
__owur int tls_construct_cert_status_body(SSL_CONNECTION *s, WPACKET *pkt);
__owur CON_FUNC_RETURN tls_construct_cert_status(SSL_CONNECTION *s,
                                                 WPACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_key_exchange(SSL_CONNECTION *s,
                                                   PACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_server_rpk(SSL_CONNECTION *sc,
                                                 PACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_client_rpk(SSL_CONNECTION *sc,
                                                 PACKET *pkt);
__owur unsigned long tls_output_rpk(SSL_CONNECTION *sc, WPACKET *pkt,
                                    CERT_PKEY *cpk);
__owur int tls_process_rpk(SSL_CONNECTION *s, PACKET *pkt, EVP_PKEY **peer_rpk);
__owur MSG_PROCESS_RETURN tls_process_server_certificate(SSL_CONNECTION *s,
                                                         PACKET *pkt);
__owur WORK_STATE tls_post_process_server_certificate(SSL_CONNECTION *s,
                                                      WORK_STATE wst);
#ifndef OPENSSL_NO_COMP_ALG
__owur MSG_PROCESS_RETURN tls_process_server_compressed_certificate(SSL_CONNECTION *sc,
                                                                    PACKET *pkt);
#endif
__owur int ssl3_check_cert_and_algorithm(SSL_CONNECTION *s);
#ifndef OPENSSL_NO_NEXTPROTONEG
__owur CON_FUNC_RETURN tls_construct_next_proto(SSL_CONNECTION *s, WPACKET *pkt);
#endif
__owur MSG_PROCESS_RETURN tls_process_hello_req(SSL_CONNECTION *s, PACKET *pkt);
__owur MSG_PROCESS_RETURN dtls_process_hello_verify(SSL_CONNECTION *s, PACKET *pkt);
__owur CON_FUNC_RETURN tls_construct_end_of_early_data(SSL_CONNECTION *s,
                                                       WPACKET *pkt);

/* some server-only functions */
__owur MSG_PROCESS_RETURN tls_process_client_hello(SSL_CONNECTION *s,
                                                   PACKET *pkt);
__owur WORK_STATE tls_post_process_client_hello(SSL_CONNECTION *s,
                                                WORK_STATE wst);
__owur CON_FUNC_RETURN tls_construct_server_hello(SSL_CONNECTION *s,
                                                  WPACKET *pkt);
__owur CON_FUNC_RETURN dtls_construct_hello_verify_request(SSL_CONNECTION *s,
                                                           WPACKET *pkt);
__owur CON_FUNC_RETURN tls_construct_server_certificate(SSL_CONNECTION *s,
                                                        WPACKET *pkt);
#ifndef OPENSSL_NO_COMP_ALG
__owur CON_FUNC_RETURN tls_construct_server_compressed_certificate(SSL_CONNECTION *sc,
                                                                   WPACKET *pkt);
#endif
__owur CON_FUNC_RETURN tls_construct_server_key_exchange(SSL_CONNECTION *s,
                                                         WPACKET *pkt);
__owur CON_FUNC_RETURN tls_construct_certificate_request(SSL_CONNECTION *s,
                                                         WPACKET *pkt);
__owur CON_FUNC_RETURN tls_construct_server_done(SSL_CONNECTION *s,
                                                 WPACKET *pkt);
__owur MSG_PROCESS_RETURN tls_process_client_certificate(SSL_CONNECTION *s,
                                                         PACKET *pkt);
#ifndef OPENSSL_NO_COMP_ALG
__owur MSG_PROCESS_RETURN tls_process_client_compressed_certificate(SSL_CONNECTION *sc,
                                                                    PACKET *pkt);
#endif
__owur MSG_PROCESS_RETURN tls_process_client_key_exchange(SSL_CONNECTION *s,
                                                          PACKET *pkt);
__owur WORK_STATE tls_post_process_client_key_exchange(SSL_CONNECTION *s,
                                                       WORK_STATE wst);
__owur MSG_PROCESS_RETURN tls_process_cert_verify(SSL_CONNECTION *s,
                                                  PACKET *pkt);
#ifndef OPENSSL_NO_NEXTPROTONEG
__owur MSG_PROCESS_RETURN tls_process_next_proto(SSL_CONNECTION *s,
                                                 PACKET *pkt);
#endif
__owur CON_FUNC_RETURN tls_construct_new_session_ticket(SSL_CONNECTION *s,
                                                        WPACKET *pkt);
MSG_PROCESS_RETURN tls_process_end_of_early_data(SSL_CONNECTION *s,
                                                 PACKET *pkt);

#ifndef OPENSSL_NO_GOST
/* These functions are used in GOST18 CKE, both for client and server */
int ossl_gost18_cke_cipher_nid(const SSL_CONNECTION *s);
int ossl_gost_ukm(const SSL_CONNECTION *s, unsigned char *dgst_buf);
#endif

/* Extension processing */

typedef enum ext_return_en {
    EXT_RETURN_FAIL,
    EXT_RETURN_SENT,
    EXT_RETURN_NOT_SENT
} EXT_RETURN;

__owur int tls_validate_all_contexts(SSL_CONNECTION *s, unsigned int thisctx,
                                     RAW_EXTENSION *exts);
__owur int extension_is_relevant(SSL_CONNECTION *s, unsigned int extctx,
                                 unsigned int thisctx);
__owur int tls_collect_extensions(SSL_CONNECTION *s, PACKET *packet,
                                  unsigned int context,
                                  RAW_EXTENSION **res, size_t *len, int init);
__owur int tls_parse_extension(SSL_CONNECTION *s, TLSEXT_INDEX idx, int context,
                               RAW_EXTENSION *exts,  X509 *x, size_t chainidx);
__owur int tls_parse_all_extensions(SSL_CONNECTION *s, int context,
                                    RAW_EXTENSION *exts,
                                    X509 *x, size_t chainidx, int fin);
__owur int should_add_extension(SSL_CONNECTION *s, unsigned int extctx,
                                unsigned int thisctx, int max_version);
__owur int tls_construct_extensions(SSL_CONNECTION *s, WPACKET *pkt,
                                    unsigned int context,
                                    X509 *x, size_t chainidx);

__owur int tls_psk_do_binder(SSL_CONNECTION *s, const EVP_MD *md,
                             const unsigned char *msgstart,
                             size_t binderoffset, const unsigned char *binderin,
                             unsigned char *binderout,
                             SSL_SESSION *sess, int sign, int external);

/* Server Extension processing */
int tls_parse_ctos_renegotiate(SSL_CONNECTION *s, PACKET *pkt,
                               unsigned int context,
                               X509 *x, size_t chainidx);
int tls_parse_ctos_server_name(SSL_CONNECTION *s, PACKET *pkt,
                               unsigned int context,
                               X509 *x, size_t chainidx);
int tls_parse_ctos_maxfragmentlen(SSL_CONNECTION *s, PACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
#ifndef OPENSSL_NO_SRP
int tls_parse_ctos_srp(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);
#endif
int tls_parse_ctos_early_data(SSL_CONNECTION *s, PACKET *pkt,
                              unsigned int context,
                              X509 *x, size_t chainidx);
int tls_parse_ctos_ec_pt_formats(SSL_CONNECTION *s, PACKET *pkt,
                                 unsigned int context,
                                 X509 *x, size_t chainidx);
int tls_parse_ctos_supported_groups(SSL_CONNECTION *s, PACKET *pkt,
                                    unsigned int context,
                                    X509 *x, size_t chainidxl);
int tls_parse_ctos_session_ticket(SSL_CONNECTION *s, PACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
int tls_parse_ctos_sig_algs_cert(SSL_CONNECTION *s, PACKET *pkt,
                                 unsigned int context,
                                 X509 *x, size_t chainidx);
int tls_parse_ctos_sig_algs(SSL_CONNECTION *s, PACKET *pkt,
                            unsigned int context, X509 *x, size_t chainidx);
#ifndef OPENSSL_NO_OCSP
int tls_parse_ctos_status_request(SSL_CONNECTION *s, PACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
#endif
#ifndef OPENSSL_NO_NEXTPROTONEG
int tls_parse_ctos_npn(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);
#endif
int tls_parse_ctos_alpn(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                        X509 *x, size_t chainidx);
#ifndef OPENSSL_NO_SRTP
int tls_parse_ctos_use_srtp(SSL_CONNECTION *s, PACKET *pkt,
                            unsigned int context, X509 *x, size_t chainidx);
#endif
int tls_parse_ctos_etm(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);
int tls_parse_ctos_key_share(SSL_CONNECTION *s, PACKET *pkt,
                             unsigned int context, X509 *x, size_t chainidx);
int tls_parse_ctos_cookie(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                          X509 *x, size_t chainidx);
int tls_parse_ctos_ems(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);
int tls_parse_ctos_psk_kex_modes(SSL_CONNECTION *s, PACKET *pkt,
                                 unsigned int context,
                                 X509 *x, size_t chainidx);
int tls_parse_ctos_psk(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);
int tls_parse_ctos_post_handshake_auth(SSL_CONNECTION *, PACKET *pkt,
                                       unsigned int context,
                                       X509 *x, size_t chainidx);

EXT_RETURN tls_construct_stoc_renegotiate(SSL_CONNECTION *s, WPACKET *pkt,
                                          unsigned int context, X509 *x,
                                          size_t chainidx);
EXT_RETURN tls_construct_stoc_server_name(SSL_CONNECTION *s, WPACKET *pkt,
                                          unsigned int context, X509 *x,
                                          size_t chainidx);
EXT_RETURN tls_construct_stoc_early_data(SSL_CONNECTION *s, WPACKET *pkt,
                                         unsigned int context, X509 *x,
                                         size_t chainidx);
EXT_RETURN tls_construct_stoc_maxfragmentlen(SSL_CONNECTION *s, WPACKET *pkt,
                                             unsigned int context, X509 *x,
                                             size_t chainidx);
EXT_RETURN tls_construct_stoc_ec_pt_formats(SSL_CONNECTION *s, WPACKET *pkt,
                                            unsigned int context, X509 *x,
                                            size_t chainidx);
EXT_RETURN tls_construct_stoc_supported_groups(SSL_CONNECTION *s, WPACKET *pkt,
                                               unsigned int context, X509 *x,
                                               size_t chainidx);
EXT_RETURN tls_construct_stoc_session_ticket(SSL_CONNECTION *s, WPACKET *pkt,
                                             unsigned int context, X509 *x,
                                             size_t chainidx);
#ifndef OPENSSL_NO_OCSP
EXT_RETURN tls_construct_stoc_status_request(SSL_CONNECTION *s, WPACKET *pkt,
                                             unsigned int context, X509 *x,
                                             size_t chainidx);
#endif
#ifndef OPENSSL_NO_NEXTPROTONEG
EXT_RETURN tls_construct_stoc_next_proto_neg(SSL_CONNECTION *s, WPACKET *pkt,
                                             unsigned int context, X509 *x,
                                             size_t chainidx);
#endif
EXT_RETURN tls_construct_stoc_alpn(SSL_CONNECTION *s, WPACKET *pkt,
                                   unsigned int context,
                                   X509 *x, size_t chainidx);
#ifndef OPENSSL_NO_SRTP
EXT_RETURN tls_construct_stoc_use_srtp(SSL_CONNECTION *s, WPACKET *pkt,
                                       unsigned int context,
                                       X509 *x, size_t chainidx);
#endif
EXT_RETURN tls_construct_stoc_etm(SSL_CONNECTION *s, WPACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
EXT_RETURN tls_construct_stoc_ems(SSL_CONNECTION *s, WPACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
EXT_RETURN tls_construct_stoc_supported_versions(SSL_CONNECTION *s, WPACKET *pkt,
                                                 unsigned int context, X509 *x,
                                                 size_t chainidx);
EXT_RETURN tls_construct_stoc_key_share(SSL_CONNECTION *s, WPACKET *pkt,
                                        unsigned int context, X509 *x,
                                        size_t chainidx);
EXT_RETURN tls_construct_stoc_cookie(SSL_CONNECTION *s, WPACKET *pkt,
                                     unsigned int context,
                                     X509 *x, size_t chainidx);
/*
 * Not in public headers as this is not an official extension. Only used when
 * SSL_OP_CRYPTOPRO_TLSEXT_BUG is set.
 */
#define TLSEXT_TYPE_cryptopro_bug      0xfde8
EXT_RETURN tls_construct_stoc_cryptopro_bug(SSL_CONNECTION *s, WPACKET *pkt,
                                            unsigned int context, X509 *x,
                                            size_t chainidx);
EXT_RETURN tls_construct_stoc_psk(SSL_CONNECTION *s, WPACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);

/* Client Extension processing */
EXT_RETURN tls_construct_ctos_renegotiate(SSL_CONNECTION *s, WPACKET *pkt,
                                          unsigned int context,
                                          X509 *x, size_t chainidx);
EXT_RETURN tls_construct_ctos_server_name(SSL_CONNECTION *s, WPACKET *pkt,
                                          unsigned int context,
                                          X509 *x, size_t chainidx);
EXT_RETURN tls_construct_ctos_maxfragmentlen(SSL_CONNECTION *s, WPACKET *pkt,
                                             unsigned int context,
                                             X509 *x, size_t chainidx);
#ifndef OPENSSL_NO_SRP
EXT_RETURN tls_construct_ctos_srp(SSL_CONNECTION *s, WPACKET *pkt,
                                  unsigned int context, X509 *x,
                                  size_t chainidx);
#endif
EXT_RETURN tls_construct_ctos_ec_pt_formats(SSL_CONNECTION *s, WPACKET *pkt,
                                            unsigned int context, X509 *x,
                                            size_t chainidx);
EXT_RETURN tls_construct_ctos_supported_groups(SSL_CONNECTION *s, WPACKET *pkt,
                                               unsigned int context, X509 *x,
                                               size_t chainidx);

EXT_RETURN tls_construct_ctos_early_data(SSL_CONNECTION *s, WPACKET *pkt,
                                         unsigned int context, X509 *x,
                                         size_t chainidx);
EXT_RETURN tls_construct_ctos_session_ticket(SSL_CONNECTION *s, WPACKET *pkt,
                                             unsigned int context, X509 *x,
                                             size_t chainidx);
EXT_RETURN tls_construct_ctos_sig_algs(SSL_CONNECTION *s, WPACKET *pkt,
                                       unsigned int context, X509 *x,
                                       size_t chainidx);
#ifndef OPENSSL_NO_OCSP
EXT_RETURN tls_construct_ctos_status_request(SSL_CONNECTION *s, WPACKET *pkt,
                                             unsigned int context, X509 *x,
                                             size_t chainidx);
#endif
#ifndef OPENSSL_NO_NEXTPROTONEG
EXT_RETURN tls_construct_ctos_npn(SSL_CONNECTION *s, WPACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
#endif
EXT_RETURN tls_construct_ctos_alpn(SSL_CONNECTION *s, WPACKET *pkt,
                                   unsigned int context,
                                   X509 *x, size_t chainidx);
#ifndef OPENSSL_NO_SRTP
EXT_RETURN tls_construct_ctos_use_srtp(SSL_CONNECTION *s, WPACKET *pkt,
                                       unsigned int context,
                                       X509 *x, size_t chainidx);
#endif
EXT_RETURN tls_construct_ctos_etm(SSL_CONNECTION *s, WPACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
#ifndef OPENSSL_NO_CT
EXT_RETURN tls_construct_ctos_sct(SSL_CONNECTION *s, WPACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
#endif
EXT_RETURN tls_construct_ctos_ems(SSL_CONNECTION *s, WPACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
EXT_RETURN tls_construct_ctos_supported_versions(SSL_CONNECTION *s, WPACKET *pkt,
                                                 unsigned int context, X509 *x,
                                                 size_t chainidx);
EXT_RETURN tls_construct_ctos_key_share(SSL_CONNECTION *s, WPACKET *pkt,
                                        unsigned int context, X509 *x,
                                        size_t chainidx);
EXT_RETURN tls_construct_ctos_psk_kex_modes(SSL_CONNECTION *s, WPACKET *pkt,
                                            unsigned int context, X509 *x,
                                            size_t chainidx);
EXT_RETURN tls_construct_ctos_cookie(SSL_CONNECTION *s, WPACKET *pkt,
                                     unsigned int context,
                                     X509 *x, size_t chainidx);
EXT_RETURN tls_construct_ctos_padding(SSL_CONNECTION *s, WPACKET *pkt,
                                      unsigned int context, X509 *x,
                                      size_t chainidx);
EXT_RETURN tls_construct_ctos_psk(SSL_CONNECTION *s, WPACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
EXT_RETURN tls_construct_ctos_post_handshake_auth(SSL_CONNECTION *s, WPACKET *pkt,
                                                  unsigned int context,
                                                  X509 *x, size_t chainidx);

int tls_parse_stoc_renegotiate(SSL_CONNECTION *s, PACKET *pkt,
                               unsigned int context,
                               X509 *x, size_t chainidx);
int tls_parse_stoc_server_name(SSL_CONNECTION *s, PACKET *pkt,
                               unsigned int context,
                               X509 *x, size_t chainidx);
int tls_parse_stoc_early_data(SSL_CONNECTION *s, PACKET *pkt,
                              unsigned int context,
                              X509 *x, size_t chainidx);
int tls_parse_stoc_maxfragmentlen(SSL_CONNECTION *s, PACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
int tls_parse_stoc_ec_pt_formats(SSL_CONNECTION *s, PACKET *pkt,
                                 unsigned int context,
                                 X509 *x, size_t chainidx);
int tls_parse_stoc_session_ticket(SSL_CONNECTION *s, PACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
#ifndef OPENSSL_NO_OCSP
int tls_parse_stoc_status_request(SSL_CONNECTION *s, PACKET *pkt,
                                  unsigned int context,
                                  X509 *x, size_t chainidx);
#endif
#ifndef OPENSSL_NO_CT
int tls_parse_stoc_sct(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);
#endif
#ifndef OPENSSL_NO_NEXTPROTONEG
int tls_parse_stoc_npn(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);
#endif
int tls_parse_stoc_alpn(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                        X509 *x, size_t chainidx);
#ifndef OPENSSL_NO_SRTP
int tls_parse_stoc_use_srtp(SSL_CONNECTION *s, PACKET *pkt,
                            unsigned int context, X509 *x, size_t chainidx);
#endif
int tls_parse_stoc_etm(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);
int tls_parse_stoc_ems(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);
int tls_parse_stoc_supported_versions(SSL_CONNECTION *s, PACKET *pkt,
                                      unsigned int context,
                                      X509 *x, size_t chainidx);
int tls_parse_stoc_key_share(SSL_CONNECTION *s, PACKET *pkt,
                             unsigned int context, X509 *x, size_t chainidx);
int tls_parse_stoc_cookie(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                          X509 *x, size_t chainidx);
int tls_parse_stoc_psk(SSL_CONNECTION *s, PACKET *pkt, unsigned int context,
                       X509 *x, size_t chainidx);

int tls_handle_alpn(SSL_CONNECTION *s);

int tls13_save_handshake_digest_for_pha(SSL_CONNECTION *s);
int tls13_restore_handshake_digest_for_pha(SSL_CONNECTION *s);

__owur EVP_PKEY* tls_get_peer_pkey(const SSL_CONNECTION *sc);
/* RFC7250 */
EXT_RETURN tls_construct_ctos_client_cert_type(SSL_CONNECTION *sc, WPACKET *pkt,
                                               unsigned int context,
                                               X509 *x, size_t chainidx);
EXT_RETURN tls_construct_stoc_client_cert_type(SSL_CONNECTION *sc, WPACKET *pkt,
                                               unsigned int context,
                                               X509 *x, size_t chainidx);
int tls_parse_ctos_client_cert_type(SSL_CONNECTION *sc, PACKET *pkt,
                                    unsigned int context,
                                    X509 *x, size_t chainidx);
int tls_parse_stoc_client_cert_type(SSL_CONNECTION *sc, PACKET *pkt,
                                    unsigned int context,
                               X509 *x, size_t chainidx);
EXT_RETURN tls_construct_ctos_server_cert_type(SSL_CONNECTION *sc, WPACKET *pkt,
                                               unsigned int context,
                                               X509 *x, size_t chainidx);
EXT_RETURN tls_construct_stoc_server_cert_type(SSL_CONNECTION *sc, WPACKET *pkt,
                                               unsigned int context,
                                               X509 *x, size_t chainidx);
int tls_parse_ctos_server_cert_type(SSL_CONNECTION *sc, PACKET *pkt,
                                    unsigned int context,
                                    X509 *x, size_t chainidx);
int tls_parse_stoc_server_cert_type(SSL_CONNECTION *s, PACKET *pkt,
                                    unsigned int context,
                                    X509 *x, size_t chainidx);
