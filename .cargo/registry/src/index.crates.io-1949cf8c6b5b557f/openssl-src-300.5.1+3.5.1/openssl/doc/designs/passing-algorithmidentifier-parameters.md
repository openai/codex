Handling AlgorithmIdentifier and its parameters with provider operations
========================================================================

Quick background
----------------

We currently only support passing the AlgorithmIdentifier (`X509_ALGOR`)
parameter field to symmetric cipher provider implementations.  We currently
only support getting full AlgorithmIdentifier (`X509_ALGOR`) from signature
provider implementations.

We do support passing them to legacy implementations of other types of
operation algorithms as well, but it's done in a way that can't be supported
with providers, because it involves sharing specific structures between
libcrypto and the backend implementation.

For a longer background and explanation, see
[Background / tl;dr](#background-tldr) at the end of this design.

Establish OSSL_PARAM keys that any algorithms may become aware of
-----------------------------------------------------------------

We already have known parameter keys:

- "algor_id_param", also known as the macro `OSSL_CIPHER_PARAM_ALGORITHM_ID_PARAMS`.

  This is currently only specified for `EVP_CIPHER`, in support of
  `EVP_CIPHER_param_to_asn1()` and `EVP_CIPHER_asn1_to_param()`

- "algorithm-id", also known as the macro `OSSL_SIGNATURE_PARAM_ALGORITHM_ID`.

This design proposes:

1. Adding a parameter key "algorithm-id-params", to replace "algor_id_param",
   and deprecate the latter.
2. Making both "algorithm-id" and "algorithm-id-params" generically available,
   rather than only tied to `EVP_SIGNATURE` ("algorithm-id") or `EVP_CIPHER`
   ("algor_id_param").

This way, these parameters can be used in the exact same manner with other
operations, with the value of the AlgorithmIdentifier as well as its
parameters as octet strings, to be used and interpreted by applications and
provider implementations alike in whatever way they see fit.

Applications can choose to add these in an `OSSL_PARAM` array, to be passed
with the multitude of initialization functions that take such an array, or
using specific operation `OSSL_PARAM` setters and getters (such as
`EVP_PKEY_CTX_set_params`), or using other available convenience functions
(see below).

These parameter will have to be documented in the following files:

- `doc/man7/provider-asym_cipher.pod`
- `doc/man7/provider-cipher.pod`
- `doc/man7/provider-digest.pod`
- `doc/man7/provider-kdf.pod`
- `doc/man7/provider-kem.pod`
- `doc/man7/provider-keyexch.pod`
- `doc/man7/provider-mac.pod`
- `doc/man7/provider-signature.pod`

That should cover all algorithms that are, or should be possible to fetch by
AlgorithmIdentifier.algorithm, and for which there's potentially a relevant
AlgorithmIdentifier.parameters field.

We may arguably want to consider `doc/man7/provider-keymgmt.pod` too, but
an AlgorithmIdentifier that's attached directly to a key is usually part of
a PrivKeyInfo or SubjectPublicKeyInfo structure, and those are handled by
encoders and decoders as those see fit, and there's no tangible reason why
that would have to change.

Public convenience API
----------------------

For convenience, the following set of functions would be added to pass the
AlgorithmIdentifier parameter data to diverse operations, or to retrieve
such parameter data from them.

``` C
/*
 * These two would essentially be aliases for EVP_CIPHER_param_to_asn1()
 * and EVP_CIPHER_asn1_to_param().
 */
EVP_CIPHER_CTX_set_algor_params(EVP_CIPHER_CTX *ctx, const X509_ALGOR *alg);
EVP_CIPHER_CTX_get_algor_params(EVP_CIPHER_CTX *ctx, X509_ALGOR *alg);
EVP_CIPHER_CTX_get_algor(EVP_CIPHER_CTX *ctx, X509_ALGOR **alg);

EVP_MD_CTX_set_algor_params(EVP_MD_CTX *ctx, const X509_ALGOR *alg);
EVP_MD_CTX_get_algor_params(EVP_MD_CTX *ctx, X509_ALGOR *alg);
EVP_MD_CTX_get_algor(EVP_MD_CTX *ctx, X509_ALGOR **alg);

EVP_MAC_CTX_set_algor_params(EVP_MAC_CTX *ctx, const X509_ALGOR *alg);
EVP_MAC_CTX_get_algor_params(EVP_MAC_CTX *ctx, X509_ALGOR *alg);
EVP_MAC_CTX_get_algor(EVP_MAC_CTX *ctx, X509_ALGOR **alg);

EVP_KDF_CTX_set_algor_params(EVP_KDF_CTX *ctx, const X509_ALGOR *alg);
EVP_KDF_CTX_get_algor_params(EVP_KDF_CTX *ctx, X509_ALGOR *alg);
EVP_KDF_CTX_get_algor(EVP_KDF_CTX *ctx, X509_ALGOR **alg);

EVP_PKEY_CTX_set_algor_params(EVP_PKEY_CTX *ctx, const X509_ALGOR *alg);
EVP_PKEY_CTX_get_algor_params(EVP_PKEY_CTX *ctx, X509_ALGOR *alg);
EVP_PKEY_CTX_get_algor(EVP_PKEY_CTX *ctx, X509_ALGOR **alg);
```

Note that all might not need to be added immediately, depending on if they
are considered useful or not.  For future proofing, however, they should
probably all be added.

Requirements on the providers
-----------------------------

Providers that implement ciphers or any operation that uses asymmetric keys
will have to implement support for passing AlgorithmIdentifier parameter
data, and will have to process that data in whatever manner that's necessary
to meet the standards for that operation.

Fallback strategies
-------------------

There are no possible fallback strategies, which is fine, considering that
current provider functionality doesn't support passing AlgorithmIdentifier
parameter data at all (except for `EVP_CIPHER`), and therefore do not work
at all when such parameter data needs to be passed.

-----

-----

Background / tl;dr
------------------

### AlgorithmIdenfier parameter and how it's used

OpenSSL has historically done a few tricks to not have to pass
AlgorithmIdenfier parameter data to the backend implementations of
cryptographic operations:

- In some cases, they were passed as part of the lower level key structure
  (for example, the `RSA` structure can also carry RSA-PSS parameters).
- In the `EVP_CIPHER` case, there is functionality to pass the parameter
  data specifically.
- For asymmetric key operations, PKCS#7 and CMS support was added as
  `EVP_PKEY` ctrls.

With providers, some of that support was retained, but not others.  Most
crucially, the `EVP_PKEY` ctrls for PKCS#7 and CMS were not retained,
because the way they were implemented violated the principle that provider
implementations *MUST NOT* share complex OpenSSL specific structures with
libcrypto.

### Usage examples

Quite a lot of the available examples today revolve around CMS, with a
number of RFCs that specify what parameters should be passed with certain
operations / algorithms.  This list is not exhaustive, the reader is
encouraged to research further usages.

- [DSA](https://www.rfc-editor.org/rfc/rfc3370#section-3.1) signatures
  typically have the domain parameters *p*, *q* and *g*.
- [RC2 key wrap](https://www.rfc-editor.org/rfc/rfc3370#section-4.3.2)
- [PBKDF2](https://www.rfc-editor.org/rfc/rfc3370#section-4.4.1)
- [3DES-CBC](https://www.rfc-editor.org/rfc/rfc3370#section-5.1)
- [RC2-CBC](https://www.rfc-editor.org/rfc/rfc3370#section-5.2)

- [GOST 28147-89](https://www.rfc-editor.org/rfc/rfc4490.html#section-5.1)

- [RSA-OAEP](https://www.rfc-editor.org/rfc/rfc8017#appendix-A.2.1)
- [RSA-PSS](https://www.rfc-editor.org/rfc/rfc8017#appendix-A.2.3)

- [XOR-MD5](https://www.rfc-editor.org/rfc/rfc6210.html) is experimental,
  but it does demonstrate the possibility of a parametrized hash algorithm.

Some of it can be claimed to already have support in OpenSSL.  However, this
is with old libcrypto code that has special knowledge of the algorithms that
are involved.
