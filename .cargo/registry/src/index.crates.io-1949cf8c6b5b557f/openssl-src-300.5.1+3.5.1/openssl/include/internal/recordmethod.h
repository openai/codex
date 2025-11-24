/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_RECORDMETHOD_H
# define OSSL_INTERNAL_RECORDMETHOD_H
# pragma once

# include <openssl/ssl.h>

/*
 * We use the term "record" here to refer to a packet of data. Records are
 * typically protected via a cipher and MAC, or an AEAD cipher (although not
 * always). This usage of the term record is consistent with the TLS concept.
 * In QUIC the term "record" is not used but it is analogous to the QUIC term
 * "packet". The interface in this file applies to all protocols that protect
 * records/packets of data, i.e. (D)TLS and QUIC. The term record is used to
 * refer to both contexts.
 */

/*
 * An OSSL_RECORD_METHOD is a protocol specific method which provides the
 * functions for reading and writing records for that protocol. Which
 * OSSL_RECORD_METHOD to use for a given protocol is defined by the SSL_METHOD.
 */
typedef struct ossl_record_method_st OSSL_RECORD_METHOD;

/*
 * An OSSL_RECORD_LAYER is just an externally defined opaque pointer created by
 * the method
 */
typedef struct ossl_record_layer_st OSSL_RECORD_LAYER;


# define OSSL_RECORD_ROLE_CLIENT 0
# define OSSL_RECORD_ROLE_SERVER 1

# define OSSL_RECORD_DIRECTION_READ  0
# define OSSL_RECORD_DIRECTION_WRITE 1

# define OSSL_RECORD_RETURN_SUCCESS           1
# define OSSL_RECORD_RETURN_RETRY             0
# define OSSL_RECORD_RETURN_NON_FATAL_ERR    -1
# define OSSL_RECORD_RETURN_FATAL            -2
# define OSSL_RECORD_RETURN_EOF              -3

/*
 * Template for creating a record. A record consists of the |type| of data it
 * will contain (e.g. alert, handshake, application data, etc) along with a
 * buffer of payload data in |buf| of length |buflen|.
 */
struct ossl_record_template_st {
    unsigned char type;
    unsigned int version;
    const unsigned char *buf;
    size_t buflen;
};

typedef struct ossl_record_template_st OSSL_RECORD_TEMPLATE;

/*
 * Rather than a "method" approach, we could make this fetchable - Should we?
 * There could be some complexity in finding suitable record layer implementations
 * e.g. we need to find one that matches the negotiated protocol, cipher,
 * extensions, etc. The selection_cb approach given above doesn't work so well
 * if unknown third party providers with OSSL_RECORD_METHOD implementations are
 * loaded.
 */

/*
 * If this becomes public API then we will need functions to create and
 * free an OSSL_RECORD_METHOD, as well as functions to get/set the various
 * function pointers....unless we make it fetchable.
 */
struct ossl_record_method_st {
    /*
     * Create a new OSSL_RECORD_LAYER object for handling the protocol version
     * set by |vers|. |role| is 0 for client and 1 for server. |direction|
     * indicates either read or write. |level| is the protection level as
     * described above. |settings| are mandatory settings that will cause the
     * new() call to fail if they are not understood (for example to require
     * Encrypt-Then-Mac support). |options| are optional settings that will not
     * cause the new() call to fail if they are not understood (for example
     * whether to use "read ahead" or not).
     *
     * The BIO in |transport| is the BIO for the underlying transport layer.
     * Where the direction is "read", then this BIO will only ever be used for
     * reading data. Where the direction is "write", then this BIO will only
     * every be used for writing data.
     *
     * An SSL object will always have at least 2 OSSL_RECORD_LAYER objects in
     * force at any one time (one for reading and one for writing). In some
     * protocols more than 2 might be used (e.g. in DTLS for retransmitting
     * messages from an earlier epoch).
     *
     * The created OSSL_RECORD_LAYER object is stored in *ret on success (or
     * NULL otherwise). The return value will be one of
     * OSSL_RECORD_RETURN_SUCCESS, OSSL_RECORD_RETURN_FATAL or
     * OSSL_RECORD_RETURN_NON_FATAL. A non-fatal return means that creation of
     * the record layer has failed because it is unsuitable, but an alternative
     * record layer can be tried instead.
     */

    /*
     * If we eventually make this fetchable then we will need to use something
     * other than EVP_CIPHER. Also mactype would not be a NID, but a string. For
     * now though, this works.
     */
    int (*new_record_layer)(OSSL_LIB_CTX *libctx,
                            const char *propq, int vers,
                            int role, int direction,
                            int level,
                            uint16_t epoch,
                            unsigned char *secret,
                            size_t secretlen,
                            unsigned char *key,
                            size_t keylen,
                            unsigned char *iv,
                            size_t ivlen,
                            unsigned char *mackey,
                            size_t mackeylen,
                            const EVP_CIPHER *ciph,
                            size_t taglen,
                            int mactype,
                            const EVP_MD *md,
                            COMP_METHOD *comp,
                            const EVP_MD *kdfdigest,
                            BIO *prev,
                            BIO *transport,
                            BIO *next,
                            BIO_ADDR *local,
                            BIO_ADDR *peer,
                            const OSSL_PARAM *settings,
                            const OSSL_PARAM *options,
                            const OSSL_DISPATCH *fns,
                            void *cbarg,
                            void *rlarg,
                            OSSL_RECORD_LAYER **ret);
    int (*free)(OSSL_RECORD_LAYER *rl);

    /* Returns 1 if we have unprocessed data buffered or 0 otherwise */
    int (*unprocessed_read_pending)(OSSL_RECORD_LAYER *rl);

    /*
     * Returns 1 if we have processed data buffered that can be read or 0 otherwise
     * - not necessarily app data
     */
    int (*processed_read_pending)(OSSL_RECORD_LAYER *rl);

    /*
     * The amount of processed app data that is internally buffered and
     * available to read
     */
    size_t (*app_data_pending)(OSSL_RECORD_LAYER *rl);

    /*
     * Find out the maximum number of records that the record layer is prepared
     * to process in a single call to write_records. It is the caller's
     * responsibility to ensure that no call to write_records exceeds this
     * number of records. |type| is the type of the records that the caller
     * wants to write, and |len| is the total amount of data that it wants
     * to send. |maxfrag| is the maximum allowed fragment size based on user
     * configuration, or TLS parameter negotiation. |*preffrag| contains on
     * entry the default fragment size that will actually be used based on user
     * configuration. This will always be less than or equal to |maxfrag|. On
     * exit the record layer may update this to an alternative fragment size to
     * be used. This must always be less than or equal to |maxfrag|.
     */
    size_t (*get_max_records)(OSSL_RECORD_LAYER *rl, uint8_t type, size_t len,
                              size_t maxfrag, size_t *preffrag);

    /*
     * Write |numtempl| records from the array of record templates pointed to
     * by |templates|. Each record should be no longer than the value returned
     * by get_max_record_len(), and there should be no more records than the
     * value returned by get_max_records().
     * Where possible the caller will attempt to ensure that all records are the
     * same length, except the last record. This may not always be possible so
     * the record method implementation should not rely on this being the case.
     * In the event of a retry the caller should call retry_write_records()
     * to try again. No more calls to write_records() should be attempted until
     * retry_write_records() returns success.
     * Buffers allocated for the record templates can be freed immediately after
     * write_records() returns - even in the case a retry.
     * The record templates represent the plaintext payload. The encrypted
     * output is written to the |transport| BIO.
     * Returns:
     *  1 on success
     *  0 on retry
     * -1 on failure
     */
    int (*write_records)(OSSL_RECORD_LAYER *rl, OSSL_RECORD_TEMPLATE *templates,
                         size_t numtempl);

    /*
     * Retry a previous call to write_records. The caller should continue to
     * call this until the function returns with success or failure. After
     * each retry more of the data may have been incrementally sent.
     * Returns:
     *  1 on success
     *  0 on retry
     * -1 on failure
     */
    int (*retry_write_records)(OSSL_RECORD_LAYER *rl);

    /*
     * Read a record and return the record layer version and record type in
     * the |rversion| and |type| parameters. |*data| is set to point to a
     * record layer buffer containing the record payload data and |*datalen|
     * is filled in with the length of that data. The |epoch| and |seq_num|
     * values are only used if DTLS has been negotiated. In that case they are
     * filled in with the epoch and sequence number from the record.
     * An opaque record layer handle for the record is returned in |*rechandle|
     * which is used in a subsequent call to |release_record|. The buffer must
     * remain available until all the bytes from record are released via one or
     * more release_record calls.
     *
     * Internally the OSSL_RECORD_METHOD implementation may read/process
     * multiple records in one go and buffer them.
     */
    int (*read_record)(OSSL_RECORD_LAYER *rl, void **rechandle, int *rversion,
                      uint8_t *type, const unsigned char **data, size_t *datalen,
                      uint16_t *epoch, unsigned char *seq_num);
    /*
     * Release length bytes from a buffer associated with a record previously
     * read with read_record. Once all the bytes from a record are released, the
     * whole record and its associated buffer is released. Records are
     * guaranteed to be released in the order that they are read.
     */
    int (*release_record)(OSSL_RECORD_LAYER *rl, void *rechandle, size_t length);

    /*
     * In the event that a fatal error is returned from the functions above then
     * get_alert_code() can be called to obtain a more details identifier for
     * the error. In (D)TLS this is the alert description code.
     */
    int (*get_alert_code)(OSSL_RECORD_LAYER *rl);

    /*
     * Update the transport BIO from the one originally set in the
     * new_record_layer call
     */
    int (*set1_bio)(OSSL_RECORD_LAYER *rl, BIO *bio);

    /* Called when protocol negotiation selects a protocol version to use */
    int (*set_protocol_version)(OSSL_RECORD_LAYER *rl, int version);

    /*
     * Whether we are allowed to receive unencrypted alerts, even if we might
     * otherwise expect encrypted records. Ignored by protocol versions where
     * this isn't relevant
     */
    void (*set_plain_alerts)(OSSL_RECORD_LAYER *rl, int allow);

    /*
     * Called immediately after creation of the record layer if we are in a
     * first handshake. Also called at the end of the first handshake
     */
    void (*set_first_handshake)(OSSL_RECORD_LAYER *rl, int first);

    /*
     * Set the maximum number of pipelines that the record layer should process.
     * The default is 1.
     */
    void (*set_max_pipelines)(OSSL_RECORD_LAYER *rl, size_t max_pipelines);

    /*
     * Called to tell the record layer whether we are currently "in init" or
     * not. Default at creation of the record layer is "yes".
     */
    void (*set_in_init)(OSSL_RECORD_LAYER *rl, int in_init);

    /*
     * Get a short or long human readable description of the record layer state
     */
    void (*get_state)(OSSL_RECORD_LAYER *rl, const char **shortstr,
                      const char **longstr);

    /*
     * Set new options or modify ones that were originally specified in the
     * new_record_layer call.
     */
    int (*set_options)(OSSL_RECORD_LAYER *rl, const OSSL_PARAM *options);

    const COMP_METHOD *(*get_compression)(OSSL_RECORD_LAYER *rl);

    /*
     * Set the maximum fragment length to be used for the record layer. This
     * will override any previous value supplied for the "max_frag_len"
     * setting during construction of the record layer.
     */
    void (*set_max_frag_len)(OSSL_RECORD_LAYER *rl, size_t max_frag_len);

    /*
     * The maximum expansion in bytes that the record layer might add while
     * writing a record
     */
    size_t (*get_max_record_overhead)(OSSL_RECORD_LAYER *rl);

    /*
     * Increment the record sequence number
     */
    int (*increment_sequence_ctr)(OSSL_RECORD_LAYER *rl);

    /*
     * Allocate read or write buffers. Does nothing if already allocated.
     * Assumes default buffer length and 1 pipeline.
     */
    int (*alloc_buffers)(OSSL_RECORD_LAYER *rl);

    /*
     * Free read or write buffers. Fails if there is pending read or write
     * data. Buffers are automatically reallocated on next read/write.
     */
    int (*free_buffers)(OSSL_RECORD_LAYER *rl);
};


/* Standard built-in record methods */
extern const OSSL_RECORD_METHOD ossl_tls_record_method;
# ifndef OPENSSL_NO_KTLS
extern const OSSL_RECORD_METHOD ossl_ktls_record_method;
# endif
extern const OSSL_RECORD_METHOD ossl_dtls_record_method;

#endif /* !defined(OSSL_INTERNAL_RECORDMETHOD_H) */
