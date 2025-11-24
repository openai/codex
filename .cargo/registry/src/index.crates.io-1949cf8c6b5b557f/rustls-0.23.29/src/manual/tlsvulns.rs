/*! # A review of protocol vulnerabilities

## CBC MAC-then-encrypt ciphersuites

Back in 2000 [Bellare and Namprempre](https://eprint.iacr.org/2000/025) discussed how to make authenticated
encryption by composing separate encryption and authentication primitives.  That paper included this table:

| Composition Method | Privacy | | | Integrity | |
|--------------------|---------|-|-|-----------|-|
|| IND-CPA | IND-CCA | NM-CPA | INT-PTXT | INT-CTXT |
| Encrypt-and-MAC | insecure | insecure | insecure | secure | insecure |
| MAC-then-encrypt | secure | insecure | insecure | secure | insecure |
| Encrypt-then-MAC | secure | secure | secure | secure | secure |

One may assume from this fairly clear result that encrypt-and-MAC and MAC-then-encrypt compositions would be quickly abandoned
in favour of the remaining proven-secure option.  But that didn't happen, not in TLSv1.1 (2006) nor in TLSv1.2 (2008).  Worse,
both RFCs included incorrect advice on countermeasures for implementers, suggesting that the flaw was "not believed to be large
enough to be exploitable".

[Lucky 13](http://www.isg.rhul.ac.uk/tls/Lucky13.html) (2013) exploited this flaw and affected all implementations, including
those written [after discovery](https://aws.amazon.com/blogs/security/s2n-and-lucky-13/). OpenSSL even had a
[memory safety vulnerability in the fix for Lucky 13](https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2016-2107), which
gives a flavour of the kind of complexity required to remove the side channel.

rustls does not implement CBC MAC-then-encrypt ciphersuites for these reasons.  TLSv1.3 removed support for these
ciphersuites in 2018.

There are some further rejected options worth mentioning: [RFC7366](https://tools.ietf.org/html/rfc7366) defines
Encrypt-then-MAC for TLS, but unfortunately cannot be negotiated without also supporting MAC-then-encrypt
(clients cannot express "I offer CBC, but only EtM and not MtE").

## RSA PKCS#1 encryption

"RSA key exchange" in TLS involves the client choosing a large random value and encrypting it using the server's
public key.  This has two overall problems:

1. It provides no _forward secrecy_: later compromise of the server's private key breaks confidentiality of
   *all* past sessions using that key.  This is a crucial property in the presence of software that is often
   [poor at keeping a secret](http://heartbleed.com/).
2. The padding used in practice in TLS ("PKCS#1", or fully "RSAES-PKCS1-v1_5") has been known to be broken since
   [1998](http://archiv.infsec.ethz.ch/education/fs08/secsem/bleichenbacher98.pdf).

In a similar pattern to the MAC-then-encrypt problem discussed above, TLSv1.0 (1999), TLSv1.1 (2006) and TLSv1.2 (2008)
continued to specify use of PKCS#1 encryption, again with incrementally more complex and incorrect advice on countermeasures.

[ROBOT](https://robotattack.org/) (2018) showed that implementations were still vulnerable to these attacks twenty years later.
[The Marvin Attack](https://people.redhat.com/~hkario/marvin/) (2023) demonstrated the same a further five years later.

rustls does not support RSA key exchange.  TLSv1.3 also removed support.

## BEAST

[BEAST](https://vnhacker.blogspot.com/2011/09/beast.html) ([CVE-2011-3389](https://nvd.nist.gov/vuln/detail/CVE-2011-3389))
was demonstrated in 2011 by Thai Duong and Juliano Rizzo,
and was another vulnerability in CBC-based ciphersuites in SSLv3.0 and TLSv1.0.  CBC mode is vulnerable to adaptive
chosen-plaintext attacks if the IV is predictable.  In the case of these protocol versions, the IV was the previous
block of ciphertext (as if the entire TLS session was one CBC ciphertext, albeit revealed incrementally).  This was
obviously predictable, since it was published on the wire.

OpenSSL contained a countermeasure for this problem from 2002 onwards: it encrypts an empty message before each real
one, so that the IV used in the real message is unpredictable.  This was turned off by default due to bugs in IE6.

TLSv1.1 fix this vulnerability, but not any of the other deficiencies of CBC mode (see above).

rustls does not support these ciphersuites.

## CRIME

In 2002 [John Kelsey](https://www.iacr.org/cryptodb/archive/2002/FSE/3091/3091.pdf) discussed the length side channel
as applied to compression of combined secret and attacker-chosen strings.

Compression continued to be an option in TLSv1.1 (2006) and in TLSv1.2 (2008).  Support in libraries was widespread.

[CRIME](https://en.wikipedia.org/wiki/CRIME) ([CVE-2012-4929](https://nvd.nist.gov/vuln/detail/CVE-2012-4929))
was demonstrated in 2012, again by Thai Duong and Juliano Rizzo.  It attacked several protocols offering transparent
compression of application data, allowing quick adaptive chosen-plaintext attacks against secret values like cookies.

rustls does not implement compression.  TLSv1.3 also removed support.

## Logjam / FREAK

Way back when SSL was first being born, circa 1995, the US government considered cryptography a munition requiring
export control.  SSL contained specific ciphersuites with dramatically small key sizes that were not subject
to export control.  These controls were dropped in 2000.

Since the "export-grade" ciphersuites no longer fulfilled any purpose, and because they were actively harmful to users,
one may have expected software support to disappear quickly. This did not happen.

In 2015 [the FREAK attack](https://mitls.org/pages/attacks/SMACK#freak) ([CVE-2015-0204](https://nvd.nist.gov/vuln/detail/CVE-2015-0204))
and [the Logjam attack](https://weakdh.org/) ([CVE-2015-4000](https://nvd.nist.gov/vuln/detail/CVE-2015-4000)) both
demonstrated total breaks of security in the presence of servers that accepted export ciphersuites.  FREAK factored
512-bit RSA keys, while Logjam optimised solving discrete logs in the 512-bit group used by many different servers.

Naturally, rustls does not implement any of these ciphersuites.

## SWEET32

Block ciphers are vulnerable to birthday attacks, where the probability of repeating a block increases dramatically
once a particular key has been used for many blocks.  For block ciphers with 64-bit blocks, this becomes probable
once a given key encrypts the order of 32GB of data.

[Sweet32](https://sweet32.info/) ([CVE-2016-2183](https://nvd.nist.gov/vuln/detail/CVE-2016-2183)) attacked this fact
in the context of TLS support for 3DES, breaking confidentiality by analysing a large amount of attacker-induced traffic
in one session.

rustls does not support any 64-bit block ciphers.

## DROWN

[DROWN](https://drownattack.com/) ([CVE-2016-0800](https://nvd.nist.gov/vuln/detail/CVE-2016-0800)) is a cross-protocol
attack that breaks the security of TLSv1.2 and earlier (when used with RSA key exchange) by using SSLv2.  It is required
that the server uses the same key for both protocol versions.

rustls naturally does not support SSLv2, but most importantly does not support RSA key exchange for TLSv1.2.

## Poodle

[POODLE](https://cdn1.vox-cdn.com/uploads/chorus_asset/file/2354994/ssl-poodle.0.pdf) ([CVE-2014-3566](https://nvd.nist.gov/vuln/detail/CVE-2014-3566))
is an attack against CBC mode ciphersuites in SSLv3.  This was possible in most cases because some clients willingly
downgraded to SSLv3 after failed handshakes for later versions.

rustls does not support CBC mode ciphersuites, or SSLv3.  Note that rustls does not need to implement `TLS_FALLBACK_SCSV`
introduced as a countermeasure because it contains no ability to downgrade from TLS 1.2 to earlier protocol versions,
and TLS 1.3 has protocol-level downgrade protection based on the [ServerHello server random value](https://www.rfc-editor.org/rfc/rfc8446#section-4.1.3).

## GCM nonces

[RFC5288](https://tools.ietf.org/html/rfc5288) introduced GCM-based ciphersuites for use in TLS.  Unfortunately
the design was poor; it reused design for an unrelated security setting proposed in RFC5116.

GCM is a typical nonce-based AEAD: it requires a unique (but not necessarily unpredictable) 96-bit nonce for each encryption
with a given key.  The design specified by RFC5288 left two-thirds of the nonce construction up to implementations:

- wasting 8 bytes per TLS ciphertext,
- meaning correct operation cannot be tested for (e.g., in protocol-level test vectors).

There were no trade-offs here: TLS has a 64-bit sequence number that is not allowed to wrap and would make an ideal nonce.

As a result, a [2016 study](https://eprint.iacr.org/2016/475.pdf) found:

- implementations from IBM, A10 and Citrix used randomly-chosen nonces, which are unlikely to be unique over long connections,
- an implementation from Radware used the same nonce for the first two messages.

rustls uses a counter from a random starting point for GCM nonces.  TLSv1.3 and the Chacha20-Poly1305 TLSv1.2 ciphersuite
standardise this method.

## Renegotiation

In 2009 Marsh Ray and Steve Dispensa [discovered](https://kryptera.se/Renegotiating%20TLS.pdf) that the renegotiation
feature of all versions of TLS allows a MitM to splice a request of their choice onto the front of the client's real HTTP
request.  A countermeasure was proposed and widely implemented to bind renegotiations to their previous negotiations;
unfortunately this was insufficient.

rustls does not support renegotiation in TLSv1.2.  TLSv1.3 also no longer supports renegotiation.

## 3SHAKE

[3SHAKE](https://www.mitls.org/pages/attacks/3SHAKE) (2014) described a complex attack that broke the "Secure Renegotiation" extension
introduced as a countermeasure to the previous protocol flaw.

rustls does not support renegotiation for TLSv1.2 connections, or RSA key exchange, and both are required for this attack
to work.  rustls implements the "Extended Master Secret" (RFC7627) extension for TLSv1.2 which was standardised as a countermeasure.

TLSv1.3 no longer supports renegotiation and RSA key exchange.  It also effectively incorporates the improvements made in RFC7627.

## KCI

[This vulnerability](https://kcitls.org/) makes use of TLS ciphersuites (those offering static DH) which were standardised
yet not widely used. However, they were implemented by libraries, and as a result enabled for various clients.  It coupled
this with misconfigured certificates (on services including facebook.com) which allowed their misuse to MitM connections.

rustls does not support static DH/EC-DH ciphersuites.  We assert that it is misissuance to sign an EC certificate
with the keyUsage extension allowing both signatures and key exchange.  That it isn't is probably a failure
of CAB Forum baseline requirements.
*/
