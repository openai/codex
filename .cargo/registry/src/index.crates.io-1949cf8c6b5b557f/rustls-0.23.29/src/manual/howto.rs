/*! # Customising private key usage

By default rustls supports PKCS#8-format[^1] RSA or ECDSA keys, plus PKCS#1-format RSA keys.

However, if your private key resides in a HSM, or in another process, or perhaps
another machine, rustls has some extension points to support this:

The main trait you must implement is [`sign::SigningKey`][signing_key]. The primary method here
is [`choose_scheme()`][choose_scheme] where you are given a set of [`SignatureScheme`s][sig_scheme] the client says
it supports: you must choose one (or return `None` -- this aborts the handshake). Having
done that, you return an implementation of the [`sign::Signer`][signer] trait.
The [`sign()`][sign_method] performs the signature and returns it.

(Unfortunately this is currently designed for keys with low latency access, like in a
PKCS#11 provider, Microsoft CryptoAPI, etc. so is blocking rather than asynchronous.
It's a TODO to make these and other extension points async.)

Once you have these two pieces, configuring a server to use them involves, briefly:

- packaging your [`sign::SigningKey`][signing_key] with the matching certificate chain into a [`sign::CertifiedKey`][certified_key]
- making a [`ResolvesServerCertUsingSni`][cert_using_sni] and feeding in your [`sign::CertifiedKey`][certified_key] for all SNI hostnames you want to use it for,
- setting that as your `ServerConfig`'s [`cert_resolver`][cert_resolver]

For a complete example of implementing a custom [`sign::SigningKey`][signing_key] and
[`sign::Signer`][signer] see the [`signer` module in the `rustls-cng` crate][rustls-cng-signer].

[signing_key]: crate::crypto::signer::SigningKey
[choose_scheme]: crate::crypto::signer::SigningKey::choose_scheme
[sig_scheme]: crate::SignatureScheme
[signer]: crate::crypto::signer::Signer
[sign_method]: crate::crypto::signer::Signer::sign
[certified_key]: crate::crypto::signer::CertifiedKey
[cert_using_sni]: crate::server::ResolvesServerCertUsingSni
[cert_resolver]: crate::ServerConfig::cert_resolver
[rustls-cng-signer]: https://github.com/rustls/rustls-cng/blob/dev/src/signer.rs

[^1]: For PKCS#8 it does not support password encryption -- there's not a meaningful threat
      model addressed by this, and the encryption supported is typically extremely poor.

# Unexpected EOF

TLS has a `close_notify` mechanism to prevent truncation attacks[^2].
According to the TLS RFCs, each party is required to send a `close_notify` message before
closing the write side of the connection. However, some implementations don't send it.
So long as the application layer protocol (for instance HTTP/2) has message length framing
and can reject truncated messages, this is not a security problem.

Rustls treats an EOF without `close_notify` as an error of type `std::io::Error` with
`ErrorKind::UnexpectedEof`. In some situations it's appropriate for the application to handle
this error the same way it would handle a normal EOF (a read returning `Ok(0)`). In particular
if `UnexpectedEof` occurs on an idle connection it is appropriate to treat it the same way as a
clean shutdown. And if an application always uses messages with length framing (in other words,
messages are never delimited by the close of the TCP connection), it can unconditionally
ignore `UnexpectedEof` errors from rustls.

[^2]: <https://datatracker.ietf.org/doc/html/rfc8446#section-6.1>

# Debugging

If you encounter a bug with Rustls it can be helpful to collect up as much diagnostic
information as possible.

## Collecting logs

If your bug reproduces with one of the [Rustls examples] you can use the
[`RUST_LOG`] environment variable to increase the log verbosity. If you're using
your own application, you may need to configure it with a logging backend
like `env_logger`.

Consider reproducing your bug with `RUST_LOG=rustls=trace` and sharing the result
in a [GitHub gist].

[Rustls examples]: https://github.com/rustls/rustls/tree/main/examples
[`RUST_LOG`]: https://docs.rs/env_logger/latest/env_logger/#enabling-logging
[`env_logger`]: https://docs.rs/env_logger/
[GitHub gist]: https://docs.github.com/en/get-started/writing-on-github/editing-and-sharing-content-with-gists/creating-gists

## Taking a packet capture

When logs aren't enough taking a packet capture ("pcap") is another helpful tool.
The details of how to accomplish this vary by operating system/context.

### tcpdump

As one example, on Linux using [`tcpdump`] is often easiest.

If you know the IP address of the remote server your bug demonstrates with you
could take a short packet capture with this command (after replacing
`XX.XX.XX.XX` with the correct IP address):

```bash
sudo tcpdump -i any tcp and dst host XX.XX.XX.XX -C5 -w rustls.pcap
```

The `-i any` captures on any network interface. The `tcp and dst host XX.XX.XX.XX`
portion target the capture to TCP traffic to the specified IP address. The `-C5`
argument limits the capture to at most 5MB. Lastly the `-w` argument writes the
capture to `rustls.pcap`.

Another approach is to use `tcp and port XXXX` instead of `tcp and dst host XX.XX.XX.XX`
to capture all traffic to a specific port instead of a specific host server.

[`tcpdump`]: https://www.redhat.com/en/blog/introduction-using-tcpdump-linux-command-line

### SSLKEYLOGFILE

If the bug you are reporting happens after data is encrypted you may also wish to
share the secret keys required to decrypt the post-handshake traffic.

If you're using one of the [Rustls examples] you can set the `SSLKEYLOGFILE` environment
variable to a path where secrets will be written. E.g. `SSLKEYLOGFILE=rustls.pcap.keys`.

If you're using your own application you may need to customize the Rustls `ClientConfig`
or `ServerConfig`'s `key_log` setting like the example applications do.

With the file from `SSLKEYLOGFILE` it is possible to use [Wireshark] or another tool to
decrypt the post-handshake messages, following [these instructions][curl-sslkeylogfile].

Remember this allows plaintext decryption and should only be done in testing contexts
where no sensitive data (API keys, etc) are being shared.

[Wireshark]: https://www.wireshark.org/download.html
[curl-sslkeylogfile]: https://everything.curl.dev/usingcurl/tls/sslkeylogfile.html
*/
