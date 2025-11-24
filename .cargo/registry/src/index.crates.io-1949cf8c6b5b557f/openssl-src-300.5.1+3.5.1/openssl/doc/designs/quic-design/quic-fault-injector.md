QUIC Fault Injector
===================

The OpenSSL QUIC implementation receives QUIC packets from the network layer and
processes them accordingly. It will need to behave appropriately in the event of
a misbehaving peer, i.e. one which is sending protocol elements (e.g. datagrams,
packets, frames, etc) that are not in accordance with the specifications or
OpenSSL's expectations.

The QUIC Fault Injector is a component within the OpenSSL test framework that
can be used to simulate misbehaving peers and confirm that OpenSSL QUIC
implementation behaves in the expected manner in the event of such misbehaviour.

Typically an individual test will inject one particular misbehaviour (i.e. a
fault) into an otherwise normal QUIC connection. Therefore the fault injector
will have to be capable of creating fully normal QUIC protocol elements, but
also offer the flexibility for a test to modify those normal protocol elements
as required for the specific test circumstances. The OpenSSL QUIC implementation
in libssl does not offer the capability to send faults since it is designed to
be RFC compliant.

The QUIC Fault Injector will be external to libssl (it will be in the test
framework) but it will reuse the standards compliant QUIC implementation in
libssl and will make use of 3 integration points to inject faults. 2 of these
integration points will use new callbacks added to libssl. The final integration
point does not require any changes to libssl to work.

QUIC Integration Points
-----------------------

### TLS Handshake

Fault Injector based tests may need to inject faults directly into the TLS
handshake data (i.e. the contents of CRYPTO frames). However such faults may
need to be done in handshake messages that would normally be encrypted.
Additionally the contents of handshake messages are hashed and each peer
confirms that the other peer has the same calculated hash value as part of the
"Finished" message exchange - so any modifications would be rejected and the
handshake would fail.

An example test might be to confirm that an OpenSSL QUIC client behaves
correctly in the case that the server provides incorrectly formatted transport
parameters. These transport parameters are sent from the server in the
EncryptedExtensions message. That message is encrypted and so cannot be
modified by a "man-in-the-middle".

To support this integration point two new callbacks will be introduced to libssl
that enables modification of handshake data prior to it being encrypted and
hashed. These callbacks will be internal only (i.e. not part of the public API)
and so only usable by the Fault Injector.

The new libssl callbacks will be as follows:

```` C
typedef int (*ossl_statem_mutate_handshake_cb)(const unsigned char *msgin,
                                               size_t inlen,
                                               unsigned char **msgout,
                                               size_t *outlen,
                                               void *arg);

typedef void (*ossl_statem_finish_mutate_handshake_cb)(void *arg);

int ossl_statem_set_mutator(SSL *s,
                            ossl_statem_mutate_handshake_cb mutate_handshake_cb,
                            ossl_statem_finish_mutate_handshake_cb finish_mutate_handshake_cb,
                            void *mutatearg);
````

The two callbacks are set via a single internal function call
`ossl_statem_set_mutator`. The mutator callback `mutate_handshake_cb` will be
called after each handshake message has been constructed and is ready to send, but
before it has been passed through the handshake hashing code. It will be passed
a pointer to the constructed handshake message in `msgin` along with its
associated length in `inlen`. The mutator will construct a replacement handshake
message (typically by copying the input message and modifying it) and store it
in a newly allocated buffer. A pointer to the new buffer will be passed back
in `*msgout` and its length will be stored in `*outlen`. Optionally the mutator
can choose to not mutate by simply creating a new buffer with a copy of the data
in it. A return value of 1 indicates that the callback completed successfully. A
return value of 0 indicates a fatal error.

Once libssl has finished using the mutated buffer it will call the
`finish_mutate_handshake_cb` callback which can then release the buffer and
perform any other cleanup as required.

### QUIC Pre-Encryption Packets

QUIC Packets are the primary mechanism for exchanging protocol data within QUIC.
Multiple packets may be held within a single datagram, and each packet may
itself contain multiple frames. A packet gets protected via an AEAD encryption
algorithm prior to it being sent. Fault Injector based tests may need to inject
faults into these packets prior to them being encrypted.

An example test might insert an unrecognised frame type into a QUIC packet to
confirm that an OpenSSL QUIC client handles it appropriately (e.g. by raising a
protocol error).

The above functionality will be supported by the following two new callbacks
which will provide the ability to mutate packets before they are encrypted and
sent. As for the TLS callbacks these will be internal only and not part of the
public API.

```` C
typedef int (*ossl_mutate_packet_cb)(const QUIC_PKT_HDR *hdrin,
                                     const OSSL_QTX_IOVEC *iovecin, size_t numin,
                                     QUIC_PKT_HDR **hdrout,
                                     const OSSL_QTX_IOVEC **iovecout,
                                     size_t *numout,
                                     void *arg);

typedef void (*ossl_finish_mutate_cb)(void *arg);

void ossl_qtx_set_mutator(OSSL_QTX *qtx, ossl_mutate_packet_cb mutatecb,
                          ossl_finish_mutate_cb finishmutatecb, void *mutatearg);
````

A single new function call will set both callbacks. The `mutatecb` callback will
be invoked after each packet has been constructed but before protection has
been applied to it. The header for the packet will be pointed to by `hdrin` and
the payload will be in an iovec array pointed to by `iovecin` and containing
`numin` iovecs. The `mutatecb` callback is expected to allocate a new header
structure and return it in `*hdrout` and a new set of iovecs to be stored in
`*iovecout`. The number of iovecs need not be the same as the input. The number
of iovecs in the output array is stored in `*numout`. Optionally the callback
can choose to not mutate by simply creating new iovecs/headers with a copy of the
data in it. A return value of 1 indicates that the callback completed
successfully. A return value of 0 indicates a fatal error.

Once the OpenSSL QUIC implementation has finished using the mutated buffers the
`finishmutatecb` callback is called. This is expected to free any resources and
buffers that were allocated as part of the `mutatecb` call.

### QUIC Datagrams

Encrypted QUIC packets are sent in datagrams. There may be more than one QUIC
packet in a single datagram. Fault Injector based tests may need to inject
faults directly into these datagrams.

An example test might modify an encrypted packet to confirm that the AEAD
decryption process rejects it.

In order to provide this functionality the QUIC Fault Injector will insert
itself as a man-in-the-middle between the client and server. A BIO_s_dgram_pair()
will be used with one of the pair being used on the client end and the other
being associated with the Fault Injector. Similarly a second BIO_s_dgram_pair()
will be created with one used on the server and other used with the Fault
Injector.

With this setup the Fault Injector will act as a proxy and simply pass
datagrams sent from the client on to the server, and vice versa. Where a test
requires a modification to be made, that will occur prior to the datagram being
sent on.

This will all be implemented using public BIO APIs without requiring any
additional internal libssl callbacks.

Fault Injector API
------------------

The Fault Injector will utilise the callbacks described above in order to supply
a more test friendly API to test authors.

This API will primarily take the form of a set of event listener callbacks. A
test will be able to "listen" for a specific event occurring and be informed about
it when it does. Examples of events might include:

- An EncryptedExtensions handshake message being sent
- An ACK frame being sent
- A Datagram being sent

Each listener will be provided with additional data about the specific event.
For example a listener that is listening for an EncryptedExtensions message will
be provided with the parsed contents of that message in an easy to use
structure. Additional helper functions will be provided to make changes to the
message (such as to resize it).

Initially listeners will only be able to listen for events on the server side.
This is because, in MVP, it will be the client side that is under test - so the
faults need to be injected into protocol elements sent from the server. Post
MVP this will be extended in order to be able to test the server. It may be that
we need to do this during MVP in order to be able to observe protocol elements
sent from the client without modifying them (i.e. in order to confirm that the
client is behaving as we expect). This will be added if required as we develop
the tests.

It is expected that the Fault Injector API will expand over time as new
listeners and helper functions are added to support specific test scenarios. The
initial API will provide a basic set of listeners and helper functions in order
to provide the basis for future work.

The following outlines an illustrative set of functions that will initially be
provided. A number of `TODO(QUIC TESTING)` comments are inserted to explain how
we might expand the API over time:

```` C
/* Type to represent the Fault Injector */
typedef struct ossl_quic_fault OSSL_QUIC_FAULT;

/*
 * Structure representing a parsed EncryptedExtension message. Listeners can
 * make changes to the contents of structure objects as required and the fault
 * injector will reconstruct the message to be sent on
 */
typedef struct ossl_qf_encrypted_extensions {
    /* EncryptedExtension messages just have an extensions block */
    unsigned char *extensions;
    size_t extensionslen;
} OSSL_QF_ENCRYPTED_EXTENSIONS;

/*
 * Given an SSL_CTX for the client and filenames for the server certificate and
 * keyfile, create a server and client instances as well as a fault injector
 * instance. |block| indicates whether we are using blocking mode or not.
 */
int qtest_create_quic_objects(OSSL_LIB_CTX *libctx, SSL_CTX *clientctx,
                              SSL_CTX *serverctx, char *certfile, char *keyfile,
                              int block, QUIC_TSERVER **qtserv, SSL **cssl,
                              OSSL_QUIC_FAULT **fault, BIO **tracebio);

/*
 * Free up a Fault Injector instance
 */
void ossl_quic_fault_free(OSSL_QUIC_FAULT *fault);

/*
 * Run the TLS handshake to create a QUIC connection between the client and
 * server.
 */
int qtest_create_quic_connection(QUIC_TSERVER *qtserv, SSL *clientssl);

/*
 * Same as qtest_create_quic_connection but will stop (successfully) if the
 * clientssl indicates SSL_ERROR_WANT_XXX as specified by |wanterr|
 */
int qtest_create_quic_connection_ex(QUIC_TSERVER *qtserv, SSL *clientssl,
                                    int wanterr);

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
 * Enable tests to listen for pre-encryption QUIC packets being sent
 */
typedef int (*ossl_quic_fault_on_packet_plain_cb)(OSSL_QUIC_FAULT *fault,
                                                  QUIC_PKT_HDR *hdr,
                                                  unsigned char *buf,
                                                  size_t len,
                                                  void *cbarg);

int ossl_quic_fault_set_packet_plain_listener(OSSL_QUIC_FAULT *fault,
                                    ossl_quic_fault_on_packet_plain_cb pplaincb,
                                    void *pplaincbarg);


/*
 * Helper function to be called from a packet_plain_listener callback if it
 * wants to resize the packet (either to add new data to it, or to truncate it).
 * The buf provided to packet_plain_listener is over allocated, so this just
 * changes the logical size and never changes the actual address of the buf.
 * This will fail if a large resize is attempted that exceeds the over
 * allocation.
 */
int ossl_quic_fault_resize_plain_packet(OSSL_QUIC_FAULT *fault, size_t newlen);

/*
 * Prepend frame data into a packet. To be called from a packet_plain_listener
 * callback
 */
int ossl_quic_fault_prepend_frame(OSSL_QUIC_FAULT *fault, unsigned char *frame,
                                  size_t frame_len);

/*
 * The general handshake message listener is sent the entire handshake message
 * data block, including the handshake header itself
 */
typedef int (*ossl_quic_fault_on_handshake_cb)(OSSL_QUIC_FAULT *fault,
                                               unsigned char *msg,
                                               size_t msglen,
                                               void *handshakecbarg);

int ossl_quic_fault_set_handshake_listener(OSSL_QUIC_FAULT *fault,
                                           ossl_quic_fault_on_handshake_cb handshakecb,
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
int ossl_quic_fault_resize_handshake(OSSL_QUIC_FAULT *fault, size_t newlen);

/*
 * TODO(QUIC TESTING): Add listeners for specific types of frame here. E.g.
 * we might expect to see an "ACK" frame listener which will be passed
 * pre-parsed ack data that can be modified as required.
 */

/*
 * Handshake message specific listeners. Unlike the general handshake message
 * listener these messages are pre-parsed and supplied with message specific
 * data and exclude the handshake header.
 */
typedef int (*ossl_quic_fault_on_enc_ext_cb)(OSSL_QUIC_FAULT *fault,
                                             OSSL_QF_ENCRYPTED_EXTENSIONS *ee,
                                             size_t eelen,
                                             void *encextcbarg);

int ossl_quic_fault_set_hand_enc_ext_listener(OSSL_QUIC_FAULT *fault,
                                              ossl_quic_fault_on_enc_ext_cb encextcb,
                                              void *encextcbarg);

/* TODO(QUIC TESTING): Add listeners for other types of handshake message here */


/*
 * Helper function to be called from message specific listener callbacks. newlen
 * is the new length of the specific message excluding the handshake message
 * header.  The buffers provided to the message specific listeners are over
 * allocated, so this just changes the logical size and never changes the actual
 * address of the buffer. This will fail if a large resize is attempted that
 * exceeds the over allocation.
 */
int ossl_quic_fault_resize_message(OSSL_QUIC_FAULT *fault, size_t newlen);

/*
 * Helper function to delete an extension from an extension block. |exttype| is
 * the type of the extension to be deleted. |ext| points to the extension block.
 * On entry |*extlen| contains the length of the extension block. It is updated
 * with the new length on exit.
 */
int ossl_quic_fault_delete_extension(OSSL_QUIC_FAULT *fault,
                                     unsigned int exttype, unsigned char *ext,
                                     size_t *extlen);

/*
 * TODO(QUIC TESTING): Add additional helper functions for querying extensions
 * here (e.g. finding or adding them). We could also provide a "listener" API
 * for listening for specific extension types.
 */

/*
 * Enable tests to listen for post-encryption QUIC packets being sent
 */
typedef int (*ossl_quic_fault_on_packet_cipher_cb)(OSSL_QUIC_FAULT *fault,
                                                   /* The parsed packet header */
                                                   QUIC_PKT_HDR *hdr,
                                                   /* The packet payload data */
                                                   unsigned char *buf,
                                                   /* Length of the payload */
                                                   size_t len,
                                                   void *cbarg);

int ossl_quic_fault_set_packet_cipher_listener(OSSL_QUIC_FAULT *fault,
                                ossl_quic_fault_on_packet_cipher_cb pciphercb,
                                void *picphercbarg);

/*
 * Enable tests to listen for datagrams being sent
 */
typedef int (*ossl_quic_fault_on_datagram_cb)(OSSL_QUIC_FAULT *fault,
                                              BIO_MSG *m,
                                              size_t stride,
                                              void *cbarg);

int ossl_quic_fault_set_datagram_listener(OSSL_QUIC_FAULT *fault,
                                          ossl_quic_fault_on_datagram_cb datagramcb,
                                          void *datagramcbarg);

/*
 * To be called from a datagram_listener callback. The datagram buffer is over
 * allocated, so this just changes the logical size and never changes the actual
 * address of the buffer. This will fail if a large resize is attempted that
 * exceeds the over allocation.
 */
int ossl_quic_fault_resize_datagram(OSSL_QUIC_FAULT *fault, size_t newlen);

````

Example Tests
-------------

This section provides some example tests to illustrate how the Fault Injector
might be used to create tests.

### Unknown Frame Test

An example test showing a server sending a frame of an unknown type to the
client:

```` C
/*
 * Test that adding an unknown frame type is handled correctly
 */
static int add_unknown_frame_cb(OSSL_QUIC_FAULT *fault, QUIC_PKT_HDR *hdr,
                                unsigned char *buf, size_t len, void *cbarg)
{
    static size_t done = 0;
    /*
     * There are no "reserved" frame types which are definitately safe for us
     * to use for testing purposes - but we just use the highest possible
     * value (8 byte length integer) and with no payload bytes
     */
    unsigned char unknown_frame[] = {
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
    };

    /* We only ever add the unknown frame to one packet */
    if (done++)
        return 1;

    return ossl_quic_fault_prepend_frame(fault, unknown_frame,
                                         sizeof(unknown_frame));
}

static int test_unknown_frame(void)
{
    int testresult = 0, ret;
    SSL_CTX *cctx = SSL_CTX_new(OSSL_QUIC_client_method());
    QUIC_TSERVER *qtserv = NULL;
    SSL *cssl = NULL;
    char *msg = "Hello World!";
    size_t msglen = strlen(msg);
    unsigned char buf[80];
    size_t byteswritten;
    OSSL_QUIC_FAULT *fault = NULL;

    if (!TEST_ptr(cctx))
        goto err;

    if (!TEST_true(qtest_create_quic_objects(NULL, cctx, NULL, cert, privkey, 0,
                                             &qtserv, &cssl, &fault, NULL)))
        goto err;

    if (!TEST_true(qtest_create_quic_connection(qtserv, cssl)))
        goto err;

    /*
     * Write a message from the server to the client and add an unknown frame
     * type
     */
    if (!TEST_true(ossl_quic_fault_set_packet_plain_listener(fault,
                                                             add_unknown_frame_cb,
                                                             NULL)))
        goto err;

    if (!TEST_true(ossl_quic_tserver_write(qtserv, (unsigned char *)msg, msglen,
                                           &byteswritten)))
        goto err;

    if (!TEST_size_t_eq(msglen, byteswritten))
        goto err;

    ossl_quic_tserver_tick(qtserv);
    if (!TEST_true(SSL_tick(cssl)))
        goto err;

    if (!TEST_int_le(ret = SSL_read(cssl, buf, sizeof(buf)), 0))
        goto err;

    if (!TEST_int_eq(SSL_get_error(cssl, ret), SSL_ERROR_SSL))
        goto err;

    if (!TEST_int_eq(ERR_GET_REASON(ERR_peek_error()),
                     SSL_R_UNKNOWN_FRAME_TYPE_RECEIVED))
        goto err;

    if (!TEST_true(qtest_check_server_protocol_err(qtserv)))
        goto err;

    testresult = 1;
 err:
    ossl_quic_fault_free(fault);
    SSL_free(cssl);
    ossl_quic_tserver_free(qtserv);
    SSL_CTX_free(cctx);
    return testresult;
}
````

### No Transport Parameters test

An example test showing the case where a server does not supply any transport
parameters in the TLS handshake:

```` C
/*
 * Test that a server that fails to provide transport params cannot be
 * connected to.
 */
static int drop_transport_params_cb(OSSL_QUIC_FAULT *fault,
                                    OSSL_QF_ENCRYPTED_EXTENSIONS *ee,
                                    size_t eelen, void *encextcbarg)
{
    if (!ossl_quic_fault_delete_extension(fault,
                                          TLSEXT_TYPE_quic_transport_parameters,
                                          ee->extensions, &ee->extensionslen))
        return 0;

    return 1;
}

static int test_no_transport_params(void)
{
    int testresult = 0;
    SSL_CTX *cctx = SSL_CTX_new(OSSL_QUIC_client_method());
    QUIC_TSERVER *qtserv = NULL;
    SSL *cssl = NULL;
    OSSL_QUIC_FAULT *fault = NULL;

    if (!TEST_ptr(cctx))
        goto err;

    if (!TEST_true(qtest_create_quic_objects(NULL, cctx, NULL, cert, privkey, 0,
                                             &qtserv, &cssl, &fault, NULL)))
        goto err;

    if (!TEST_true(ossl_quic_fault_set_hand_enc_ext_listener(fault,
                                                             drop_transport_params_cb,
                                                             NULL)))
        goto err;

    /*
     * We expect the connection to fail because the server failed to provide
     * transport parameters
     */
    if (!TEST_false(qtest_create_quic_connection(qtserv, cssl)))
        goto err;

    if (!TEST_true(qtest_check_server_protocol_err(qtserv)))
        goto err;

    testresult = 1;
 err:
    ossl_quic_fault_free(fault);
    SSL_free(cssl);
    ossl_quic_tserver_free(qtserv);
    SSL_CTX_free(cctx);
    return testresult;

}
````
