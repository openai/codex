/*!

## Rationale for defaults

### Why is AES-256 preferred over AES-128?

This is a trade-off between:

1. classical security level: searching a 2^128 key space is as implausible as 2^256.
2. post-quantum security level: the difference is more meaningful, and AES-256 seems like the conservative choice.
3. performance: AES-256 is around 40% slower than AES-128, though hardware acceleration typically narrows this gap.

The choice is frankly quite marginal.

### Why is AES-GCM preferred over chacha20-poly1305?

Hardware support for accelerating AES-GCM is widespread, and hardware-accelerated AES-GCM
is quicker than un-accelerated chacha20-poly1305.

However, if you know your application will run on a platform without that, you should
_definitely_ change the default order to prefer chacha20-poly1305: both the performance and
the implementation security will be improved.  We think this is an uncommon case.

### Why is x25519 preferred for key exchange over nistp256?

Both provide roughly the same classical security level, but x25519 has better performance and
it's _much_ more likely that both peers will have good quality implementations.

### About the post-quantum-secure key exchange `X25519MLKEM768`

[`X25519MLKEM768`] -- a hybrid[^1], post-quantum-secure[^2] key exchange
algorithm -- is available when using the aws-lc-rs provider.

The `prefer-post-quantum` crate feature makes `X25519MLKEM768` the
highest-priority key exchange algorithm.  Otherwise, it is available but
not highest-priority.

[X25519MLKEM768] is pre-standardization, but is now widely deployed,
for example, by [Chrome] and [Cloudflare].

You may see unexpected connection failures (such as [tldr.fail])
-- [please report these to us][interop-bug].

The two components of this key exchange are well regarded:
X25519 alone is already used by default by rustls, and tends to have
higher quality implementations than other elliptic curves.
ML-KEM-768 was standardized by NIST in [FIPS203].

[`MLKEM768`] is available separately, but is not currently enabled
by default out of conservatism.

[^1]: meaning: a construction that runs a classical and post-quantum
      key exchange, and uses the output of both together.  This is a hedge
      against the post-quantum half being broken.

[^2]: a "post-quantum-secure" algorithm is one posited to be invulnerable
      to attack using a cryptographically-relevant quantum computer.  In contrast,
      classical algorithms would be broken by such a computer.  Note that such computers
      do not currently exist, and may never exist, but current traffic could be captured
      now and attacked later.

[X25519MLKEM768]: <https://datatracker.ietf.org/doc/draft-ietf-tls-ecdhe-mlkem/>
[`X25519MLKEM768`]: crate::crypto::aws_lc_rs::kx_group::X25519MLKEM768
[`MLKEM768`]: crate::crypto::aws_lc_rs::kx_group::MLKEM768
[FIPS203]: <https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.203.pdf>
[Chrome]: <https://security.googleblog.com/2024/09/a-new-path-for-kyber-on-web.html>
[Cloudflare]: <https://blog.cloudflare.com/pq-2024/#ml-kem-768-and-x25519>
[interop-bug]: <https://github.com/rustls/rustls/issues/new?assignees=&labels=&projects=&template=bug_report.md&title=>
[tldr.fail]: <https://tldr.fail/>

*/
