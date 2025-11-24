ML-DSA Design
==============

This document covers OpenSSL specific ML-DSA implementation details.
FIPS 204 clearly states most of the requirements of ML-DSA and has comprehensive
pseudo code for all its algorithms.

The base code for OpenSSL has been derived from the BoringSSL C++ code.
As OpenSSL is C code, templates can not be used. The OpenSSL code instead uses
parameters to pass algorithm specific constants, and also uses these constants
to run different conditional functions when required.

ML-DSA Parameters & Per algorithm Functions
-------------------------------------------

FIPS 204 contains 3 algorithms for different security strengths.
FIPS 204 Section 4 Table 1 & Table 2 specifies different constants that are
stored in a table of `ML_DSA_PARAM` objects.
The constants include key sizes and coefficient ranges.

OpenSSL uses 3 key managers and 3 signature functions corresponding to the algorithms
ML-DSA-44, ML-DSA-65 and ML-DSA-87.

ML-DSA only uses SHAKE-128 and SHAKE-256 for digest operations, so these digest
algorithms are pre-fetched and stored within a `ML_DSA` key.
Any functions that require these pre-fetched objects must pass either the key
or the pre-fetched object within the key as a parameter.
A temporary `EVP_MD_CTX` is created as needed and the shake object(s) are set
into this ctx.

Initially a separate object called `ML_DSA_CTX` object was passed around that
contained 2 `EVP_MD_CTXs` with the pre-fetched `EVP_MD` shake objects. It was
modified to match the ML-KEM code.

ML-DSA keys
------------

Once loaded an 'ML-DSA-KEY' object contains either a public key or a
public/private key pair.
When loading a private key, the public key is always generated. In the event
that the public key is also supplied, an error will occur if the generated public
key does not match the supplied public key.

An `ML_DSA` polynomial contains 256 32 bit values which is 1K of space.
Keys store vectors of size 'k' or 'l' plus a matrix of size 'k' * 'l',
where (k, l) correspond to (4,4), (6,5) or (8,7). The key data could be large
if buffers of the maximum size of (8,7) are used for the (4,4) use case.
To save space rather than use fixed size buffers, allocations are used instead,
for both the public and private elements. (i.e. The private allocations are not
done when only a public key is loaded).

Since keys consist of vectors and a matrix of polynomials, a single block
is used to allocate all polynomials, and then the polynomial blocks are
assigned to the individual vectors and matrix. This approach is also used when temporary
vectors, matrices or polynomials are required

Keys are not allowed to mutate, so checks are done during load to check that the
public and private key components are not changed once set.

`ossl_ml_dsa_key_get_pub()` and `ossl_ml_dsa_key_get_priv()` return the
encoded forms of the key components (which are stored within the key).
The hash of the encoded public key is also stored in the key.

The key generation process uses a seed to create a private key, and the public
key is then generated using this private key.

For ACVP testing the seed may be supplied.

Pure vs Pre Hashed Signature Generation
----------------------------------------

The normal signing process (called Pure ML-DSA Signature Generation)
encodes the message internally as 0x00 || len(ctx) || ctx || message.
where B<ctx> is some optional value of size 0x00..0xFF.

ACVP Testing requires the ability to process raw messages without the above encoding.
This will be controlled by settable parameters.

Pre Hash ML-DSA Signature Generation encode the message as

```text
0x01 || len(ctx) || ctx || digest_OID || H(message).
```

The scenario that is stated that this is useful for is when this encoded message
is supplied from an external source.
This ensures domain separation between signature variants

Currently OpenSSL does not support the Pre Hash variant as this does not sit
well with the OpenSSL API's.
The user could do the encoding themselves and then set the settable to not
encode the passed in message.

Signing API
-------------

As only the one-shot implementation is required and the message is not digested
the API's used should be

`EVP_PKEY_sign_message_init()`, `EVP_PKEY_sign()`,
`EVP_PKEY_verify_message_init()`, `EVP_PKEY_verify()`.

OpenSSL command line support
----------------------------

For backwards compatability reasons `EVP_DigestSignInit_ex()`,
`EVP_DigestSign()`, `EVP_DigestVerifyInit_ex()` and `EVP_DigestVerify()` may
also be used, but the digest passed in `mdname` must be NULL (i.e. it
effectively behaves the same as above).
Passing a non NULL digest results in an error.

`OSSL_PKEY_PARAM_MANDATORY_DIGEST` must return "" in the key manager getter and
`OSSL_SIGNATURE_PARAM_ALGORITHM_ID` in the signature context getter.

Encoding/Decoding
-----------------

Where it makes sense to, WPACKET is used for output (such as signature generation)
and PACKET for reading signature data.

Constant Time Considerations
----------------------------

Similar code to BoringSSL will be added that allows ctgrind to be used to
detect constant time issues.

There are many places that do hashing in the code, and these are capable (although
it is not likely) of returning errors. There is not attempt to deal with these cases.

Changes from BoringSSL
----------------------

At the time of writing, BoringSSL code only supported ML-DSA-65. Since there
is specialized code for encoding and decoding of different sizes of
polynomial coefficients, code was added to support these different sizes
(e.g hints have 1 bit coefficients so 8 coefficients can be packed into 1 byte)

Differences between BoringSSL and FIPS 204 pseudo code
------------------------------------------------------

The symmetric modulus operation normally gives a result in the range -a/2 ...
a/2.
BoringSSL chose to keep the result positive by adding q and reducing once as
required.

Montgomery multiplication is used to speed up multiplications (See FIPS 204
Appendix A).
