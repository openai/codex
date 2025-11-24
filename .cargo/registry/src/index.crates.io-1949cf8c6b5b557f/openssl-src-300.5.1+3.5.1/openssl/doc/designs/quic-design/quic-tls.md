QUIC-TLS Handshake Integration
==============================

QUIC reuses the TLS handshake for the establishment of keys. It does not use
the standard TLS record layer and instead assumes responsibility for the
confidentiality and integrity of QUIC packets itself. Only the TLS handshake is
used. Application data is entirely protected by QUIC.

QUIC_TLS Object
---------------

A QUIC-TLS handshake is managed by a QUIC_TLS object. This object provides
3 core functions to the rest of the QUIC implementation:

```c
QUIC_TLS *ossl_quic_tls_new(const QUIC_TLS_ARGS *args);
```

The `ossl_quic_tls_new` function instantiates a new `QUIC_TLS` object associated
with the QUIC Connection and initialises it with a set of callbacks and other
arguments provided in the `args` parameter. These callbacks are called at
various key points during the handshake lifecycle such as when new keys are
established, crypto frame data is ready to be sent or consumed, or when the
handshake is complete.

A key field of the `args` structure is the `SSL` object (`s`). This "inner"
`SSL` object is initialised with an `SSL_CONNECTION` to represent the TLS
handshake state. This is a different `SSL` object to the "user" visible `SSL`
object which contains a `QUIC_CONNECTION`, i.e. the user visible `SSL` object
contains a `QUIC_CONNECTION` which contains the inner `SSL` object which
contains an `SSL_CONNECTION`.

```c
void ossl_quic_tls_free(QUIC_TLS *qtls);
```

When the QUIC Connection no longer needs the handshake object it can be freed
via the `ossl_quic_tls_free` function.

```c
int ossl_quic_tls_tick(QUIC_TLS *qtls);
```

Finally the `ossl_quic_tls_tick` function is responsible for advancing the
state of the QUIC-TLS handshake. On each call to `ossl_quic_tls_tick` newly
received crypto frame data may be consumed, or new crypto frame data may be
queued for sending, or one or more of the various callbacks may be invoked.

QUIC_TLS_ARGS
-------------

A `QUIC_TLS_ARGS` object is passed to the `ossl_quic_tls_new` function by the
OpenSSL QUIC implementation to supply a set of callbacks and other essential
parameters. The `QUIC_TLS_ARGS` structure is as follows:

```c
typedef struct quic_tls_args_st {
    /*
     * The "inner" SSL object for the QUIC Connection. Contains an
     * SSL_CONNECTION
     */
    SSL *s;

    /*
     * Called to send data on the crypto stream. We use a callback rather than
     * passing the crypto stream QUIC_SSTREAM directly because this lets the CSM
     * dynamically select the correct outgoing crypto stream based on the
     * current EL.
     */
    int (*crypto_send_cb)(const unsigned char *buf, size_t buf_len,
                          size_t *consumed, void *arg);
    void *crypto_send_cb_arg;
    int (*crypto_recv_cb)(unsigned char *buf, size_t buf_len,
                          size_t *bytes_read, void *arg);
    void *crypto_recv_cb_arg;

    /* Called when a traffic secret is available for a given TLS protection level. */
    int (*yield_secret_cb)(uint32_t prot_level, int direction /* 0=RX, 1=TX */,
                           uint32_t suite_id, EVP_MD *md,
                           const unsigned char *secret, size_t secret_len,
                           void *arg);
    void *yield_secret_cb_arg;

    /*
     * Called when we receive transport parameters from the peer.
     *
     * Note: These parameters are not authenticated until the handshake is
     * marked as completed.
     */
    int (*got_transport_params_cb)(const unsigned char *params,
                                   size_t params_len,
                                   void *arg);
    void *got_transport_params_cb_arg;

    /*
     * Called when the handshake has been completed as far as the handshake
     * protocol is concerned, meaning that the connection has been
     * authenticated.
     */
    int (*handshake_complete_cb)(void *arg);
    void *handshake_complete_cb_arg;

    /*
     * Called when something has gone wrong with the connection as far as the
     * handshake layer is concerned, meaning that it should be immediately torn
     * down. Note that this may happen at any time, including after a connection
     * has been fully established.
     */
    int (*alert_cb)(void *arg, unsigned char alert_code);
    void *alert_cb_arg;

    /*
     * Transport parameters which client should send. Buffer lifetime must
     * exceed the lifetime of the QUIC_TLS object.
     */
    const unsigned char *transport_params;
    size_t transport_params_len;
} QUIC_TLS_ARGS;
```

The `crypto_send_cb` and `crypto_recv_cb` callbacks will be called by the
QUIC-TLS handshake when there is new CRYPTO frame data to be sent, or when it
wants to consume queued CRYPTO frame data from the peer.

When the TLS handshake generates secrets they will be communicated to the
OpenSSL QUIC implementation via the `yield_secret_cb`, and when the handshake
has successfully completed this will be communicated via `handshake_complete_cb`.

In the event that an error occurs a normal TLS handshake would send a TLS alert
record. QUIC handles this differently and so the QUIC_TLS object will intercept
attempts to send an alert and will communicate this via the `alert_cb` callback.

QUIC requires the use of a TLS extension in order to send and receive "transport
parameters". These transport parameters are opaque to the `QUIC_TLS` object. It
does not need to use them directly but instead simply includes them in an
extension to be sent in the ClientHello and receives them back from the peer in
the EncryptedExtensions message. The data to be sent is provided in the
`transport_params` argument. When the peer's parameters are received the
`got_transport_params_cb` callback is invoked.

QUIC_TLS Implementation
-----------------------

The `QUIC_TLS` object utilises two main mechanisms for fulfilling its functions:

* It registers itself as a custom TLS record layer
* It supplies callbacks to register a custom TLS extension

### Custom TLS Record Layer

A TLS record layer is defined via an `OSSL_RECORD_METHOD` object. This object
consists of a set of function pointers which need to be implemented by any
record layer. Existing record layers include one for TLS, one for DTLS and one
for KTLS.

`QUIC_TLS` registers itself as a custom TLS record layer. A new internal
function is used to provide the custom record method data and associate it with
an `SSL_CONNECTION`:

```C
void ossl_ssl_set_custom_record_layer(SSL_CONNECTION *s,
                                      const OSSL_RECORD_METHOD *meth,
                                      void *rlarg);
```

The internal function `ssl_select_next_record_layer` which is used in the TLS
implementation to work out which record method should be used next is modified
to first check whether a custom record method has been specified and always use
that one if so.

The TLS record layer code is further modified to provide the following
capabilities which are needed in order to support QUIC.

The custom record layer will need a record layer specific argument (`rlarg`
above). This is passed as part of a modified `new_record_layer` call.

Existing TLS record layers use TLS keys and IVs that are calculated using a
KDF from a higher level secret. Instead of this QUIC needs direct access to the
higher level secret as well as the digest to be used in the KDF - so these
values are now also passed through as part of the `new_record_layer` call.

The most important function pointers in the `OSSL_RECORD_METHOD` for the
`QUIC_TLS` object are:

* `new_record_layer`

Invoked every time a new record layer object is created by the TLS
implementation. This occurs every time new keys are provisioned (once for the
"read" side and once for the "write" side). This function is responsible for
invoking the `yield_secret_cb` callback.

* `write_records`

Invoked every time the TLS implementation wants to send TLS handshake data. This
is responsible for calling the `crypto_send_cb` callback. It also includes
special processing in the event that the TLS implementation wants to send an
alert. This manifests itself as a call to `write_records` indicating a type of
`SSL3_RT_ALERT`. The `QUIC_TLS` implementation of `write_records` must parse the
alert data supplied by the TLS implementation (always a 2 byte record payload)
and pull out the alert description (a one byte integer) and invoke the
`alert_cb` callback. Note that while the TLS RFC strictly allows the 2 byte
alert record to be fragmented across two 1 byte records this is never done in
practice by OpenSSL's TLS stack and the `write_records` implementation can make
the optimising assumption that both bytes of an alert are always sent together.

* `quic_read_record`

Invoked when the TLS implementation wants to read more handshake data. This
results in a call to `crypto_recv_cb`.

This design does introduce an extra "copy" in the process when `crypto_recv_cb`
is invoked. CRYPTO frame data will be queued within internal QUIC "Stream
Receive Buffers" when it is received by the peer. However the TLS implementation
expects to request data from the record layer, get a handle on that data, and
then inform the record layer when it has finished using that data. The current
design of the Stream Receive Buffers does not allow for this model. Therefore
when `crypto_recv_cb` is invoked the data is copied into a QUIC_TLS object
managed buffer. This is inefficient, so it is expected that a later phase of
development will resolve this problem.

### Custom TLS extension

Libssl already has the ability for an application to supply a custom extension
via the `SSL_CTX_add_custom_ext()` API. There is no equivalent
`SSL_add_custom_ext()` and therefore an internal API is used to do this. This
mechanism is used for supporting QUIC transport parameters. An extension
type `TLSEXT_TYPE_quic_transport_parameters` with value 57 is used for this
purpose.

The custom extension API enables the caller to supply `add`, `free` and `parse`
callbacks. The `add` callback simply adds the `transport_params` data from
`QUIC_TLS_ARGS`. The `parse` callback invokes the `got_transport_params_cb`
callback when the transport parameters have been received from the peer.

### ALPN

QUIC requires the use of ALPN (Application-Layer Protocol Negotiation). This is
normally optional in OpenSSL but is mandatory for QUIC connections. Therefore
a QUIC client must call one of `SSL_CTX_set_alpn_protos` or
`SSL_set_alpn_protos` prior to initiating the handshake. If the ALPN data has
not been set then the `QUIC_TLS` object immediately fails.

### Other Implementation Details

The `SSL_CONNECTION` used for the TLS handshake is held alongside the QUIC
related data in the `SSL` object. Public API functions that are only relevant to
TLS will modify this internal `SSL_CONNECTION` as appropriate. This enables the
end application to configure the TLS connection parameters as it sees fit (e.g.
setting ciphersuites, providing client certificates, etc). However there are
certain settings that may be optional in a normal TLS connection but are
mandatory for QUIC. Where possible these settings will be automatically
configured just before the handshake starts.

One of these settings is the minimum TLS protocol version. QUIC requires that
TLSv1.3 is used as a minimum. Therefore the `QUIC_TLS` object automatically
calls `SSL_set_min_proto_version()` and specifies `TLS1_3_VERSION` as the
minimum version.

Secondly, QUIC enforces that the TLS "middlebox" mode must not be used. For
normal TLS this is "on" by default. Therefore the `QUIC_TLS` object will
automatically clear the `SSL_OP_ENABLE_MIDDLEBOX_COMPAT` option if it is set.
