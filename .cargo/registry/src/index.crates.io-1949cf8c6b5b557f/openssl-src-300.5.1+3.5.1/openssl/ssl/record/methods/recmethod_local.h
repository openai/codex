/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/bio.h>
#include <openssl/ssl.h>
#include <openssl/err.h>
#include "../../ssl_local.h"
#include "../record_local.h"

typedef struct dtls_bitmap_st {
    /* Track 64 packets */
    uint64_t map;
    /* Max record number seen so far, 64-bit value in big-endian encoding */
    unsigned char max_seq_num[SEQ_NUM_SIZE];
} DTLS_BITMAP;

typedef struct ssl_mac_buf_st {
    unsigned char *mac;
    int alloced;
} SSL_MAC_BUF;

typedef struct tls_buffer_st {
    /* at least SSL3_RT_MAX_PACKET_SIZE bytes */
    unsigned char *buf;
    /* default buffer size (or 0 if no default set) */
    size_t default_len;
    /* buffer size */
    size_t len;
    /* where to 'copy from' */
    size_t offset;
    /* how many bytes left */
    size_t left;
    /* 'buf' is from application for KTLS */
    int app_buffer;
    /* The type of data stored in this buffer. Only used for writing */
    int type;
} TLS_BUFFER;

typedef struct tls_rl_record_st {
    /* Record layer version */
    /* r */
    int rec_version;
    /* type of record */
    /* r */
    int type;
    /* How many bytes available */
    /* rw */
    size_t length;
    /*
     * How many bytes were available before padding was removed? This is used
     * to implement the MAC check in constant time for CBC records.
     */
    /* rw */
    size_t orig_len;
    /* read/write offset into 'buf' */
    /* r */
    size_t off;
    /* pointer to the record data */
    /* rw */
    unsigned char *data;
    /* where the decode bytes are */
    /* rw */
    unsigned char *input;
    /* only used with decompression - malloc()ed */
    /* r */
    unsigned char *comp;
    /* epoch number, needed by DTLS1 */
    /* r */
    uint16_t epoch;
    /* sequence number, needed by DTLS1 */
    /* r */
    unsigned char seq_num[SEQ_NUM_SIZE];
} TLS_RL_RECORD;

/* Macros/functions provided by the TLS_RL_RECORD component */

#define TLS_RL_RECORD_set_type(r, t)              ((r)->type = (t))
#define TLS_RL_RECORD_set_rec_version(r, v)       ((r)->rec_version = (v))
#define TLS_RL_RECORD_get_length(r)               ((r)->length)
#define TLS_RL_RECORD_set_length(r, l)            ((r)->length = (l))
#define TLS_RL_RECORD_add_length(r, l)            ((r)->length += (l))
#define TLS_RL_RECORD_set_data(r, d)              ((r)->data = (d))
#define TLS_RL_RECORD_set_input(r, i)             ((r)->input = (i))
#define TLS_RL_RECORD_reset_input(r)              ((r)->input = (r)->data)


/* Protocol version specific function pointers */
struct record_functions_st {
    /*
     * Returns either OSSL_RECORD_RETURN_SUCCESS, OSSL_RECORD_RETURN_FATAL or
     * OSSL_RECORD_RETURN_NON_FATAL_ERR if we can keep trying to find an
     * alternative record layer.
     */
    int (*set_crypto_state)(OSSL_RECORD_LAYER *rl, int level,
                            unsigned char *key, size_t keylen,
                            unsigned char *iv, size_t ivlen,
                            unsigned char *mackey, size_t mackeylen,
                            const EVP_CIPHER *ciph,
                            size_t taglen,
                            int mactype,
                            const EVP_MD *md,
                            COMP_METHOD *comp);

    /*
     * Returns:
     *    0: if the record is publicly invalid, or an internal error, or AEAD
     *       decryption failed, or EtM decryption failed.
     *    1: Success or MtE decryption failed (MAC will be randomised)
     */
    int (*cipher)(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *recs, size_t n_recs,
                  int sending, SSL_MAC_BUF *macs, size_t macsize);
    /* Returns 1 for success or 0 for error */
    int (*mac)(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec, unsigned char *md,
               int sending);

    /* Return 1 for success or 0 for error */
    int (*set_protocol_version)(OSSL_RECORD_LAYER *rl, int version);

    /* Read related functions */

    int (*read_n)(OSSL_RECORD_LAYER *rl, size_t n, size_t max, int extend,
                  int clearold, size_t *readbytes);

    int (*get_more_records)(OSSL_RECORD_LAYER *rl);

    /* Return 1 for success or 0 for error */
    int (*validate_record_header)(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec);

    /* Return 1 for success or 0 for error */
    int (*post_process_record)(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec);

    /* Write related functions */

    size_t (*get_max_records)(OSSL_RECORD_LAYER *rl, uint8_t type, size_t len,
                              size_t maxfrag, size_t *preffrag);

    /* Return 1 for success or 0 for error */
    int (*write_records)(OSSL_RECORD_LAYER *rl, OSSL_RECORD_TEMPLATE *templates,
                         size_t numtempl);

    /* Allocate the rl->wbuf buffers. Return 1 for success or 0 for error */
    int (*allocate_write_buffers)(OSSL_RECORD_LAYER *rl,
                                  OSSL_RECORD_TEMPLATE *templates,
                                  size_t numtempl, size_t *prefix);

    /*
     * Initialise the packets in the |pkt| array using the buffers in |rl->wbuf|.
     * Some protocol versions may use the space in |prefixtempl| to add
     * an artificial template in front of the |templates| array and hence may
     * initialise 1 more WPACKET than there are templates. |*wpinited|
     * returns the number of WPACKETs in |pkt| that were successfully
     * initialised. This must be 0 on entry and will be filled in even on error.
     */
    int (*initialise_write_packets)(OSSL_RECORD_LAYER *rl,
                                    OSSL_RECORD_TEMPLATE *templates,
                                    size_t numtempl,
                                    OSSL_RECORD_TEMPLATE *prefixtempl,
                                    WPACKET *pkt,
                                    TLS_BUFFER *bufs,
                                    size_t *wpinited);

    /* Get the actual record type to be used for a given template */
    uint8_t (*get_record_type)(OSSL_RECORD_LAYER *rl,
                               OSSL_RECORD_TEMPLATE *template);

    /* Write the record header data to the WPACKET */
    int (*prepare_record_header)(OSSL_RECORD_LAYER *rl, WPACKET *thispkt,
                                 OSSL_RECORD_TEMPLATE *templ,
                                 uint8_t rectype,
                                 unsigned char **recdata);

    int (*add_record_padding)(OSSL_RECORD_LAYER *rl,
                              OSSL_RECORD_TEMPLATE *thistempl,
                              WPACKET *thispkt,
                              TLS_RL_RECORD *thiswr);

    /*
     * This applies any mac that might be necessary, ensures that we have enough
     * space in the WPACKET to perform the encryption and sets up the
     * TLS_RL_RECORD ready for that encryption.
     */
    int (*prepare_for_encryption)(OSSL_RECORD_LAYER *rl,
                                  size_t mac_size,
                                  WPACKET *thispkt,
                                  TLS_RL_RECORD *thiswr);

    /*
     * Any updates required to the record after encryption has been applied. For
     * example, adding a MAC if using encrypt-then-mac
     */
    int (*post_encryption_processing)(OSSL_RECORD_LAYER *rl,
                                      size_t mac_size,
                                      OSSL_RECORD_TEMPLATE *thistempl,
                                      WPACKET *thispkt,
                                      TLS_RL_RECORD *thiswr);

    /*
     * Some record layer implementations need to do some custom preparation of
     * the BIO before we write to it. KTLS does this to prevent coalescing of
     * control and data messages.
     */
    int (*prepare_write_bio)(OSSL_RECORD_LAYER *rl, int type);
};

struct ossl_record_layer_st {
    OSSL_LIB_CTX *libctx;
    const char *propq;
    int isdtls;
    int version;
    int role;
    int direction;
    int level;
    const EVP_MD *md;
    /* DTLS only */
    uint16_t epoch;

    /*
     * A BIO containing any data read in the previous epoch that was destined
     * for this epoch
     */
    BIO *prev;

    /* The transport BIO */
    BIO *bio;

    /*
     * A BIO where we will send any data read by us that is destined for the
     * next epoch.
     */
    BIO *next;

    /* Types match the equivalent fields in the SSL object */
    uint64_t options;
    uint32_t mode;

    /* write IO goes into here */
    TLS_BUFFER wbuf[SSL_MAX_PIPELINES + 1];

    /* Next wbuf with pending data still to write */
    size_t nextwbuf;

    /* How many pipelines can be used to write data */
    size_t numwpipes;

    /* read IO goes into here */
    TLS_BUFFER rbuf;
    /* each decoded record goes in here */
    TLS_RL_RECORD rrec[SSL_MAX_PIPELINES];

    /* How many records have we got available in the rrec buffer */
    size_t num_recs;

    /* The record number in the rrec buffer that can be read next */
    size_t curr_rec;

    /* The number of records that have been released via tls_release_record */
    size_t num_released;

    /* where we are when reading */
    int rstate;

    /* used internally to point at a raw packet */
    unsigned char *packet;
    size_t packet_length;

    /* Sequence number for the next record */
    unsigned char sequence[SEQ_NUM_SIZE];

    /* Alert code to be used if an error occurs */
    int alert;

    /*
     * Read as many input bytes as possible (for non-blocking reads)
     */
    int read_ahead;

    /* The number of consecutive empty records we have received */
    size_t empty_record_count;

    /*
     * Do we need to send a prefix empty record before application data as a
     * countermeasure against known-IV weakness (necessary for SSLv3 and
     * TLSv1.0)
     */
    int need_empty_fragments;

    /* cryptographic state */
    EVP_CIPHER_CTX *enc_ctx;

    /* TLSv1.3 MAC ctx, only used with integrity-only cipher */
    EVP_MAC_CTX *mac_ctx;

    /* Explicit IV length */
    size_t eivlen;

    /* used for mac generation */
    EVP_MD_CTX *md_ctx;

    /* compress/uncompress */
    COMP_CTX *compctx;

    /* Set to 1 if this is the first handshake. 0 otherwise */
    int is_first_handshake;

    /*
     * The smaller of the configured and negotiated maximum fragment length
     * or SSL3_RT_MAX_PLAIN_LENGTH if none
     */
    unsigned int max_frag_len;

    /* The maximum amount of early data we can receive/send */
    uint32_t max_early_data;

    /* The amount of early data that we have sent/received */
    size_t early_data_count;

    /* TLSv1.3 record padding */
    size_t block_padding;
    size_t hs_padding;

    /* Only used by SSLv3 */
    unsigned char mac_secret[EVP_MAX_MD_SIZE];

    /* TLSv1.0/TLSv1.1/TLSv1.2 */
    int use_etm;

    /* Flags for GOST ciphers */
    int stream_mac;
    int tlstree;

    /* TLSv1.3 fields */
    unsigned char *iv;     /* static IV */
    unsigned char *nonce;  /* part of static IV followed by sequence number */
    int allow_plain_alerts;

    /* TLS "any" fields */
    /* Set to true if this is the first record in a connection */
    unsigned int is_first_record;

    size_t taglen;

    /* DTLS received handshake records (processed and unprocessed) */
    struct pqueue_st *unprocessed_rcds;
    struct pqueue_st *processed_rcds;

    /* records being received in the current epoch */
    DTLS_BITMAP bitmap;
    /* renegotiation starts a new set of sequence numbers */
    DTLS_BITMAP next_bitmap;

    /*
     * Whether we are currently in a handshake or not. Only maintained for DTLS
     */
    int in_init;

    /* Callbacks */
    void *cbarg;
    OSSL_FUNC_rlayer_skip_early_data_fn *skip_early_data;
    OSSL_FUNC_rlayer_msg_callback_fn *msg_callback;
    OSSL_FUNC_rlayer_security_fn *security;
    OSSL_FUNC_rlayer_padding_fn *padding;

    size_t max_pipelines;

    /* Function pointers for version specific functions */
    const struct record_functions_st *funcs;
};

typedef struct dtls_rlayer_record_data_st {
    unsigned char *packet;
    size_t packet_length;
    TLS_BUFFER rbuf;
    TLS_RL_RECORD rrec;
} DTLS_RLAYER_RECORD_DATA;

extern const struct record_functions_st ssl_3_0_funcs;
extern const struct record_functions_st tls_1_funcs;
extern const struct record_functions_st tls_1_3_funcs;
extern const struct record_functions_st tls_any_funcs;
extern const struct record_functions_st dtls_1_funcs;
extern const struct record_functions_st dtls_any_funcs;

void ossl_rlayer_fatal(OSSL_RECORD_LAYER *rl, int al, int reason,
                       const char *fmt, ...);

#define RLAYERfatal(rl, al, r) RLAYERfatal_data((rl), (al), (r), NULL)
#define RLAYERfatal_data                                           \
    (ERR_new(),                                                    \
     ERR_set_debug(OPENSSL_FILE, OPENSSL_LINE, OPENSSL_FUNC),      \
     ossl_rlayer_fatal)

#define RLAYER_USE_EXPLICIT_IV(rl) ((rl)->version == TLS1_1_VERSION \
                                    || (rl)->version == TLS1_2_VERSION \
                                    || (rl)->version == DTLS1_BAD_VER \
                                    || (rl)->version == DTLS1_VERSION \
                                    || (rl)->version == DTLS1_2_VERSION)

void ossl_tls_rl_record_set_seq_num(TLS_RL_RECORD *r,
                                    const unsigned char *seq_num);

int ossl_set_tls_provider_parameters(OSSL_RECORD_LAYER *rl,
                                     EVP_CIPHER_CTX *ctx,
                                     const EVP_CIPHER *ciph,
                                     const EVP_MD *md);

int tls_increment_sequence_ctr(OSSL_RECORD_LAYER *rl);
int tls_alloc_buffers(OSSL_RECORD_LAYER *rl);
int tls_free_buffers(OSSL_RECORD_LAYER *rl);

int tls_default_read_n(OSSL_RECORD_LAYER *rl, size_t n, size_t max, int extend,
                       int clearold, size_t *readbytes);
int tls_get_more_records(OSSL_RECORD_LAYER *rl);
int dtls_get_more_records(OSSL_RECORD_LAYER *rl);

int dtls_prepare_record_header(OSSL_RECORD_LAYER *rl,
                               WPACKET *thispkt,
                               OSSL_RECORD_TEMPLATE *templ,
                               uint8_t rectype,
                               unsigned char **recdata);
int dtls_post_encryption_processing(OSSL_RECORD_LAYER *rl,
                                    size_t mac_size,
                                    OSSL_RECORD_TEMPLATE *thistempl,
                                    WPACKET *thispkt,
                                    TLS_RL_RECORD *thiswr);

int tls_default_set_protocol_version(OSSL_RECORD_LAYER *rl, int version);
int tls_default_validate_record_header(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *re);
int tls_do_compress(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *wr);
int tls_do_uncompress(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec);
int tls_default_post_process_record(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec);
int tls13_common_post_process_record(OSSL_RECORD_LAYER *rl, TLS_RL_RECORD *rec);

int
tls_int_new_record_layer(OSSL_LIB_CTX *libctx, const char *propq, int vers,
                         int role, int direction, int level,
                         const EVP_CIPHER *ciph, size_t taglen,
                         const EVP_MD *md, COMP_METHOD *comp, BIO *prev,
                         BIO *transport, BIO *next,
                         const OSSL_PARAM *settings, const OSSL_PARAM *options,
                         const OSSL_DISPATCH *fns, void *cbarg,
                         OSSL_RECORD_LAYER **retrl);
int tls_free(OSSL_RECORD_LAYER *rl);
int tls_unprocessed_read_pending(OSSL_RECORD_LAYER *rl);
int tls_processed_read_pending(OSSL_RECORD_LAYER *rl);
size_t tls_app_data_pending(OSSL_RECORD_LAYER *rl);
size_t tls_get_max_records(OSSL_RECORD_LAYER *rl, uint8_t type, size_t len,
                           size_t maxfrag, size_t *preffrag);
int tls_write_records(OSSL_RECORD_LAYER *rl, OSSL_RECORD_TEMPLATE *templates,
                      size_t numtempl);
int tls_retry_write_records(OSSL_RECORD_LAYER *rl);
int tls_get_alert_code(OSSL_RECORD_LAYER *rl);
int tls_set1_bio(OSSL_RECORD_LAYER *rl, BIO *bio);
int tls_read_record(OSSL_RECORD_LAYER *rl, void **rechandle, int *rversion,
                    uint8_t *type, const unsigned char **data, size_t *datalen,
                    uint16_t *epoch, unsigned char *seq_num);
int tls_release_record(OSSL_RECORD_LAYER *rl, void *rechandle, size_t length);
int tls_default_set_protocol_version(OSSL_RECORD_LAYER *rl, int version);
int tls_set_protocol_version(OSSL_RECORD_LAYER *rl, int version);
void tls_set_plain_alerts(OSSL_RECORD_LAYER *rl, int allow);
void tls_set_first_handshake(OSSL_RECORD_LAYER *rl, int first);
void tls_set_max_pipelines(OSSL_RECORD_LAYER *rl, size_t max_pipelines);
void tls_get_state(OSSL_RECORD_LAYER *rl, const char **shortstr,
                   const char **longstr);
int tls_set_options(OSSL_RECORD_LAYER *rl, const OSSL_PARAM *options);
const COMP_METHOD *tls_get_compression(OSSL_RECORD_LAYER *rl);
void tls_set_max_frag_len(OSSL_RECORD_LAYER *rl, size_t max_frag_len);
int tls_setup_read_buffer(OSSL_RECORD_LAYER *rl);
int tls_setup_write_buffer(OSSL_RECORD_LAYER *rl, size_t numwpipes,
                           size_t firstlen, size_t nextlen);

int tls_write_records_multiblock(OSSL_RECORD_LAYER *rl,
                                 OSSL_RECORD_TEMPLATE *templates,
                                 size_t numtempl);

size_t tls_get_max_records_default(OSSL_RECORD_LAYER *rl, uint8_t type,
                                   size_t len,
                                   size_t maxfrag, size_t *preffrag);
size_t tls_get_max_records_multiblock(OSSL_RECORD_LAYER *rl, uint8_t type,
                                      size_t len, size_t maxfrag,
                                      size_t *preffrag);
int tls_allocate_write_buffers_default(OSSL_RECORD_LAYER *rl,
                                       OSSL_RECORD_TEMPLATE *templates,
                                       size_t numtempl, size_t *prefix);
int tls_initialise_write_packets_default(OSSL_RECORD_LAYER *rl,
                                         OSSL_RECORD_TEMPLATE *templates,
                                         size_t numtempl,
                                         OSSL_RECORD_TEMPLATE *prefixtempl,
                                         WPACKET *pkt,
                                         TLS_BUFFER *bufs,
                                         size_t *wpinited);
int tls1_allocate_write_buffers(OSSL_RECORD_LAYER *rl,
                                OSSL_RECORD_TEMPLATE *templates,
                                size_t numtempl, size_t *prefix);
int tls1_initialise_write_packets(OSSL_RECORD_LAYER *rl,
                                  OSSL_RECORD_TEMPLATE *templates,
                                  size_t numtempl,
                                  OSSL_RECORD_TEMPLATE *prefixtempl,
                                  WPACKET *pkt,
                                  TLS_BUFFER *bufs,
                                  size_t *wpinited);
int tls_prepare_record_header_default(OSSL_RECORD_LAYER *rl,
                                      WPACKET *thispkt,
                                      OSSL_RECORD_TEMPLATE *templ,
                                      uint8_t rectype,
                                      unsigned char **recdata);
int tls_prepare_for_encryption_default(OSSL_RECORD_LAYER *rl,
                                       size_t mac_size,
                                       WPACKET *thispkt,
                                       TLS_RL_RECORD *thiswr);
int tls_post_encryption_processing_default(OSSL_RECORD_LAYER *rl,
                                           size_t mac_size,
                                           OSSL_RECORD_TEMPLATE *thistempl,
                                           WPACKET *thispkt,
                                           TLS_RL_RECORD *thiswr);
int tls_write_records_default(OSSL_RECORD_LAYER *rl,
                              OSSL_RECORD_TEMPLATE *templates,
                              size_t numtempl);

/* Macros/functions provided by the TLS_BUFFER component */

#define TLS_BUFFER_get_buf(b)              ((b)->buf)
#define TLS_BUFFER_set_buf(b, n)           ((b)->buf = (n))
#define TLS_BUFFER_get_len(b)              ((b)->len)
#define TLS_BUFFER_get_left(b)             ((b)->left)
#define TLS_BUFFER_set_left(b, l)          ((b)->left = (l))
#define TLS_BUFFER_sub_left(b, l)          ((b)->left -= (l))
#define TLS_BUFFER_get_offset(b)           ((b)->offset)
#define TLS_BUFFER_set_offset(b, o)        ((b)->offset = (o))
#define TLS_BUFFER_add_offset(b, o)        ((b)->offset += (o))
#define TLS_BUFFER_set_app_buffer(b, l)    ((b)->app_buffer = (l))
#define TLS_BUFFER_is_app_buffer(b)        ((b)->app_buffer)

void ossl_tls_buffer_release(TLS_BUFFER *b);
