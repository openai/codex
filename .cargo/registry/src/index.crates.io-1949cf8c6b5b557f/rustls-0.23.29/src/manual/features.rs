/*!

The below list reflects the support provided with the default crate features.
Items marked with an asterisk `*` can be extended or altered via public
APIs ([`CryptoProvider`] for example).

[`CryptoProvider`]: crate::crypto::CryptoProvider

## Current features

* TLS1.2 and TLS1.3
* ECDSA, Ed25519 or RSA server authentication by clients `*`
* ECDSA, Ed25519[^1] or RSA server authentication by servers `*`
* Forward secrecy using ECDHE; with curve25519, nistp256 or nistp384 curves `*`
* Post-quantum hybrid key exchange with [X25519MLKEM768](https://datatracker.ietf.org/doc/draft-ietf-tls-ecdhe-mlkem/) [^2] `*`
* AES128-GCM and AES256-GCM bulk encryption, with safe nonces `*`
* ChaCha20-Poly1305 bulk encryption ([RFC7905](https://tools.ietf.org/html/rfc7905)) `*`
* ALPN support
* SNI support
* Tunable fragment size to make TLS messages match size of underlying transport
* Optional use of vectored IO to minimise system calls
* TLS1.2 session resumption
* TLS1.2 resumption via tickets ([RFC5077](https://tools.ietf.org/html/rfc5077))
* TLS1.3 resumption via tickets or session storage
* TLS1.3 0-RTT data
* Server and optional client authentication
* Extended master secret support ([RFC7627](https://tools.ietf.org/html/rfc7627))
* Exporters ([RFC5705](https://tools.ietf.org/html/rfc5705))
* OCSP stapling by servers
* [RFC7250](https://tools.ietf.org/html/rfc7250) raw public keys for TLS1.3
* [RFC8879](https://tools.ietf.org/html/rfc8879) certificate compression by clients
  and servers `*`
* Client-side Encrypted client hello (ECH)
   ([draft-ietf-tls-esni](https://datatracker.ietf.org/doc/draft-ietf-tls-esni/)).

[^1]: Note that, at the time of writing, Ed25519 does not have wide support
      in browsers.  It is also not supported by the WebPKI, because the
      CA/Browser Forum Baseline Requirements do not support it for publicly
      trusted certificates.
[^2]: See [the documentation][crate::manual::_05_defaults#about-the-post-quantum-secure-key-exchange-x25519mlkem768]

## Non-features

For reasons explained in the other sections of this manual, rustls does not
and will not support:

* SSL1, SSL2, SSL3, TLS1 or TLS1.1
* RC4
* DES or triple DES
* EXPORT ciphersuites
* MAC-then-encrypt ciphersuites
* Ciphersuites without forward secrecy
* Renegotiation
* Kerberos
* TLS 1.2 protocol compression
* Discrete-log Diffie-Hellman `*`
* Automatic protocol version downgrade
* Using CA certificates directly to authenticate a server/client (often called "self-signed
  certificates"). _Rustls' default certificate verifier does not support using a trust anchor as
  both a CA certificate and an end-entity certificate in order to limit complexity and risk in
  path building. While dangerous, all authentication can be turned off if required --
  see the [example code](https://github.com/rustls/rustls/blob/v/0.23.23/examples/src/bin/tlsclient-mio.rs#L338)_ `*`

### About "custom extensions"

OpenSSL allows an application to add arbitrary TLS extensions (via
the `SSL_CTX_add_custom_ext` function and associated APIs).  We don't
support this, with the following rationale:

Such an API is limited to extensions that are quite narrow in scope:
they cannot change the meaning of standard messages, or introduce new
messages, or make any changes to the connection's cryptography.

However, there is no reasonable way to technically limit that API to
that set of extensions.  That makes the API pretty unsafe (in the
TLS and cryptography sense, not memory safety sense).  This could
cause security or interop failures.

Instead, we suggest that potential users of that API consider:

- whether their use can fit in standard extensions such as ALPN,
  or [ALPS][alps][^3].
- if not, whether they can fit in a more general extension, and define
  and standardize that in the [IETF TLSWG][tlswg].

Note the above is not a guarantee or offer that rustls will implement
any specific extensions that are standardized by the IETF TLSWG.
It is a non-goal of this project to implement absolutely everything.

For experimentation and pre-standardization testing, we suggest
forking rustls.

See also: [Go's position on such an API][golang].

[alps]: https://datatracker.ietf.org/doc/html/draft-vvv-tls-alps
[golang]: https://github.com/golang/go/issues/51497
[tlswg]: https://datatracker.ietf.org/wg/tls/charter/
[^3]: rustls does not currently implement ALPS, but it is something we
  would consider once standardised and deployed.
*/
