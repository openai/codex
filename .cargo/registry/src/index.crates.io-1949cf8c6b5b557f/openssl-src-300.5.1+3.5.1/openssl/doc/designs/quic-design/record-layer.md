Design Problem: Abstract Record Layer
=====================================

This document covers the design of an abstract record layer for use in (D)TLS.
The QUIC record layer is handled separately.

A record within this document refers to a packet of data. It will typically
contain some header data and some payload data, and will often be
cryptographically protected. A record may or may not have a one-to-one
correspondence with network packets, depending on the implementation details of
an individual record layer.

The term record comes directly from the TLS and DTLS specifications.

Libssl supports a number of different types of record layer, and record layer
variants:

- Standard TLS record layer
- Standard DTLS record layer
- Kernel TLS record layer

Within the TLS record layer there are options to handle "multiblock" and
"pipelining" which are different approaches for supporting the reading or
writing of multiple records at the same time. All record layer variants also
have to be able to handle different protocol versions.

These different record layer implementations, variants and protocol versions
have each been added at different times and over many years. The result is that
each took slightly different approaches for achieving the goals that were
appropriate at the time and the integration points where they were added were
spread throughout the code.

The introduction of QUIC support will see the implementation of a new record
layer, i.e. the QUIC-TLS record layer. This refers to the "inner" TLS
implementation used by QUIC. Records here will be in the form of QUIC CRYPTO
frames.

Requirements
------------

The technical requirements
[document](https://github.com/openssl/openssl/blob/master/doc/designs/quic-design/quic-requirements.md)
lists these requirements that are relevant to the record layer:

* The current libssl record layer includes support for TLS, DTLS and KTLS. QUIC
  will introduce another variant and there may be more over time. The OMC
  requires a pluggable record layer interface to be implemented to enable this
  to be less intrusive, more maintainable, and to harmonize the existing record
  layer interactions between TLS, DTLS, KTLS and the planned QUIC protocols. The
  pluggable record layer interface will be internal only for MVP and be public
  in a future release.

* The minimum viable product (MVP) for the next release is a pluggable record
  layer interface and a single stream QUIC client in the form of s_client that
  does not require significant API changes. In the MVP, interoperability should
  be prioritized over strict standards compliance.

* Once we have a fully functional QUIC implementation (in a subsequent release),
  it should be possible for external libraries to be able to use the pluggable
  record layer interface and it should offer a stable ABI (via a provider).

The MVP requirements are:

* a pluggable record layer (not public for MVP)

Candidate Solutions that were considered
----------------------------------------

This section outlines two different solution approaches that were considered for
the abstract record layer

### Use a METHOD based approach

A METHOD based approach is simply a structure containing function pointers. It
is a common pattern in the OpenSSL codebase. Different strategies for
implementing a METHOD can be employed, but these differences are hidden from
the caller of the METHOD.

In this solution we would seek to implement a different METHOD for each of the
types of record layer that we support, i.e. there would be one for the standard
TLS record layer, one for the standard DTLS record layer, one for kernel TLS and
one for QUIC-TLS.

In the MVP the METHOD approach would be private. However, once it has
stabilised, it would be straight forward to supply public functions to enable
end user applications to construct their own METHODs.

This option is simpler to implement than the alternative of having a provider
based approach. However it could be used as a "stepping stone" for that, i.e.
the MVP could implement a METHOD based approach, and subsequent releases could
convert the METHODs into fully fetchable algorithms.

Pros:

* Simple approach that has been used historically in OpenSSL
* Could be used as the basis for the final public solution
* Could also be used as the basis for a fetchable solution in a subsequent
  release
* If this option is later converted to a fetchable solution then much of the
  effort involved in making the record layer fetchable can be deferred to a
  later release

Cons:

* Not consistent with the provider based approach we used for extensibility in
  3.0
* If this option is implemented and later converted to a fetchable solution then
  some rework might be required

### Use a provider based approach

This approach is very similar to the alternative METHOD based approach. The
main difference is that the record layer implementations would be held in
providers and "fetched" in much the same way that cryptographic algorithms are
fetched in OpenSSL 3.0.

This approach is more consistent with the approach adopted for extensibility in
3.0. METHODS are being deprecated with providers being used extensively.

Complex objects (e.g. an `SSL` object) cannot be passed across the
libssl/provider boundary. This imposes some restrictions on the design of the
functions that can be implemented. Additionally implementing the infrastructure
for a new fetchable operation is more involved than a METHOD based approach.

Pros:

* Consistent with the extensibility solution used in 3.0
* If this option is implemented immediately in the MVP then it would avoid later
  rework if adopted in a subsequent release

Cons:

* More complicated to implement than the simple METHOD based approach
* Cannot pass complex objects across the provider boundary

### Selected solution

The METHOD based approach has been selected for MVP, with the expectation that
subsequent releases will convert it to a full provider based solution accessible
to third party applications.

Solution Description: The METHOD based approach
-----------------------------------------------

This section focuses on the selected approach of using METHODs and further
elaborates on how the design works.

A proposed internal record method API is given in
[Appendix A](#appendix-a-the-internal-record-method-api).

An `OSSL_RECORD_METHOD` represents the implementation of a particular type of
record layer. It contains a set of function pointers to represent the various
actions that can be performed by a record layer.

An `OSSL_RECORD_LAYER` object represents a specific instantiation of a
particular `OSSL_RECORD_METHOD`. It contains the state used by that
`OSSL_RECORD_METHOD` for a specific connection (i.e. `SSL` object). Any `SSL`
object will have at least 2 `OSSL_RECORD_LAYER` objects associated with it - one
for reading and one for writing. In some cases there may be more than 2 - for
example in DTLS it may be necessary to retransmit records from a previous epoch.
There will be different `OSSL_RECORD_LAYER` objects for different protection
levels or epochs. It may be that different `OSSL_RECORD_METHOD`s are used for
different protection levels. For example a connection might start using the
standard TLS record layer during the handshake, and later transition to using
the kernel TLS record layer once the handshake is complete.

A new `OSSL_RECORD_LAYER` is created by calling the `new` function of the
associated `OSSL_RECORD_METHOD`, and freed by calling the `free` function. The
parameters to the `new` function also supply all of the cryptographic state
(e.g. keys, ivs, symmetric encryption algorithms, hash algorithm etc) used by
the record layer. The internal structure details of an `OSSL_RECORD_LAYER` are
entirely hidden to the rest of libssl and can be specific to the given
`OSSL_RECORD_METHOD`. In practice the standard internal TLS, DTLS and KTLS
`OSSL_RECORD_METHOD`s all use a common `OSSL_RECORD_LAYER` structure. However
the QUIC-TLS implementation is likely to use a different structure layout.

All of the header and payload data for a single record will be represented by an
`OSSL_RECORD_TEMPLATE` structure when writing. Libssl will construct a set of
templates for records to be written out and pass them to the "write" record
layer. In most cases only a single record is ever written out at one time,
however there are some cases (such as when using the "pipelining" or
"multibuffer" optimisations) that multiple records can be written in one go.

It is the record layer's responsibility to know whether it can support multiple
records in one go or not. It is libssl's responsibility to split the payload
data into `OSSL_RECORD_TEMPLATE` objects. Libssl will call the record layer's
`get_max_records()` function to determine how many records a given payload
should be split into. If that value is more than one, then libssl will construct
(up to) that number of `OSSL_RECORD_TEMPLATE`s and pass the whole set to the
record layer's `write_records()` function.

The implementation of the `write_records` function must construct the
appropriate number of records, apply protection to them as required and then
write them out to the underlying transport layer BIO. In the event that not
all the data can be transmitted at the current time (e.g. because the underlying
transport has indicated a retry), then the `write_records` function will return
a "retry" response. It is permissible for the data to be partially sent, but
this is still considered a "retry" until all of the data is sent.

On a success or retry response libssl may free its buffers immediately. The
`OSSL_RECORD_LAYER` object will have to buffer any untransmitted data until it
is eventually sent.

If a "retry" occurs, then libssl will subsequently call `retry_write_records`
and continue to do so until a success return value is received. Libssl will
never call `write_records` a second time until a previous call to
`write_records` or `retry_write_records` has indicated success.

Libssl will read records by calling the `read_record` function. The
`OSSL_RECORD_LAYER` may read multiple records in one go and buffer them, but the
`read_record` function only ever returns one record at a time. The
`OSSL_RECORD_LAYER` object owns the buffers for the record that has been read
and supplies a pointer into that buffer back to libssl for the payload data, as
well as other information about the record such as its length and the type of
data contained in it. Each record has an associated opaque handle `rechandle`.
The record data must remain buffered by the `OSSL_RECORD_LAYER` until it has
been released via a call to `release_record()`.

A record layer implementation supplies various functions to enable libssl to
query the current state. In particular:

`unprocessed_read_pending()`: to query whether there is data buffered that has
already been read from the underlying BIO, but not yet processed.

`processed_read_pending()`: to query whether there is data buffered that has
been read from the underlying BIO and has been processed. The data is not
necessarily application data.

`app_data_pending()`: to query the amount of processed application data that is
buffered and available for immediate read.

`get_alert_code()`: to query the alert code that should be used in the event
that a previous attempt to read or write records failed.

`get_state()`: to obtain a printable string to describe the current state of the
record layer.

`get_compression()`: to obtain information about the compression method
currently being used by the record layer.

`get_max_record_overhead()`: to obtain the maximum amount of bytes the record
layer will add to the payload bytes before transmission. This does not include
any expansion that might occur during compression. Currently this is only
implemented for DTLS.

In addition, libssl will tell the record layer about various events that might
occur that are relevant to the record layer's operation:

`set1_bio()`: called if the underlying BIO being used by the record layer has
been changed.

`set_protocol_version()`: called during protocol version negotiation when a
specific protocol version has been selected.

`set_plain_alerts()`: to indicate that receiving unencrypted alerts is allowed
in the current context, even if normally we would expect to receive encrypted
data. This is only relevant for TLSv1.3.

`set_first_handshake()`: called at the beginning and end of the first handshake
for any given (D)TLS connection.

`set_max_pipelines()`: called to configure the maximum number of pipelines of
data that the record layer should process in one go. By default this is 1.

`set_in_init()`: called by libssl to tell the record layer whether we are
currently `in_init` or not. Defaults to "true".

`set_options()`: called by libssl in the event that the current set of options
to use has been updated.

`set_max_frag_len()`: called by libssl to set the maximum allowed fragment
length that is in force at the moment. This might be the result of user
configuration, or it may be negotiated during the handshake.

`increment_sequence_ctr()`: force the record layer to increment its sequence
counter. In most cases the record layer will entirely manage its own sequence
counters. However in the DTLSv1_listen() corner case, libssl needs to initialise
the record layer with an incremented sequence counter.

`alloc_buffers()`: called by libssl to request that the record layer allocate
its buffers. This is a hint only and the record layer is expected to manage its
own buffer allocation and freeing.

`free_buffers()`: called by libssl to request that the record layer free its
buffers. This is a hint only and the record layer is expected to manage its own
buffer allocation and freeing.

Appendix A: The internal record method API
------------------------------------------

The internal recordmethod.h header file for the record method API:

```` C
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

/*
 * Protection level. For <= TLSv1.2 only "NONE" and "APPLICATION" are used.
 */
# define OSSL_RECORD_PROTECTION_LEVEL_NONE        0
# define OSSL_RECORD_PROTECTION_LEVEL_EARLY       1
# define OSSL_RECORD_PROTECTION_LEVEL_HANDSHAKE   2
# define OSSL_RECORD_PROTECTION_LEVEL_APPLICATION 3

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
    int type;
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
                            BIO *prev,
                            BIO *transport,
                            BIO *next,
                            BIO_ADDR *local,
                            BIO_ADDR *peer,
                            const OSSL_PARAM *settings,
                            const OSSL_PARAM *options,
                            const OSSL_DISPATCH *fns,
                            void *cbarg,
                            OSSL_RECORD_LAYER **ret);
    int (*free)(OSSL_RECORD_LAYER *rl);

    int (*reset)(OSSL_RECORD_LAYER *rl); /* Is this needed? */

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
     * remain available until release_record is called.
     *
     * Internally the OSSL_RECORD_METHOD the implementation may read/process
     * multiple records in one go and buffer them.
     */
    int (*read_record)(OSSL_RECORD_LAYER *rl, void **rechandle, int *rversion,
                      uint8_t *type, unsigned char **data, size_t *datalen,
                      uint16_t *epoch, unsigned char *seq_num);
    /*
     * Release a buffer associated with a record previously read with
     * read_record. Records are guaranteed to be released in the order that they
     * are read.
     */
    int (*release_record)(OSSL_RECORD_LAYER *rl, void *rechandle);

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
````
