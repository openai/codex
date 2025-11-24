SLH-DSA Design
==============

This document covers OpenSSL specific SLH-DSA implementation details.
FIPS 205 clearly states most of the requirements of SLH-DSA and has comprehensive
pseudo code for all its algorithms.

SLH_DSA Parameters & Functions
------------------------------

There are 12 different parameter sets in FIPS 205. (See Section 11)
There are constants related to these, as well as there being a group of functions
associated with each set.

The constants include things like hash sizes and tree heights.

OpenSSL will have 12 different key managers and 12 corresponding signature functions.
The names used are of the form "SLH-DSA-SHA2-128s" and "SLH-DSA-SHAKE-128f".

There are 7 hash functions used. The algorithms using SHAKE have a much simpler
set of 7 functions as they just use SHAKE-256 XOF (Even for the SHAKE-128 names).
The SHA2 algorithms are much more complex and require HMAC, MGF1, as well as digests.
There are 2 sets of functions for the SHA2 case.

Some of the hash functions use an ADRS object. This is 32 bytes for SHAKE algorithms
and 22 bytes for SHA2. Because SHA2 used a compressed format the ADRS functions are
different.

There are many functions required to implement the sign and verify paths, which include
Merkle trees and WOTS+. The different functions normally call one of 2 of the
7 hash functions, as well as calling ADRS functions to pass to the HASH functions.

Rather that duplicating this code 12 times for every function, the constants are
stored within the SLH_DSA_KEY. This contains the HASH functions,
the ADRS functions, and the parameter constants. It also contains pre fetched algorithms.
A SLH_DSA_HASH_CTX object is also created, that references the key, as well as
containing per operation hash context objects.
This SLH_DSA_HASH_CTX is then passed to all functions. This context is allocated in the
provider's SLH_DSA signature context.

SLH-DSA keys
------------

SLH-DSA keys have 2 elements of size `n` for both the public and private keys.
Since different algorithms have different key sizes, buffers of the maximum size
will be used to hold the keys (since the keys are only a maximum of 64 bytes each)

struct slh_dsa_key_st {
    /* The public key consists of a SEED and ROOT values each of size |n| */
    uint8_t pub[SLH_DSA_MAX_KEYLEN];
    /* The private key consists of a SEED and PRF values of size |n| */
    uint8_t priv[SLH_DSA_MAX_KEYLEN];
    size_t key_len; /* This value is set to 2 * n if there is a public key */
    /* contains the algorithm name and constants such as |n| */
    const SLH_DSA_PARAMS *params;
    int has_priv; /* Set to 1 if there is a private key component */
    ...
};

The fields `key_len` and `has_priv` are used to determine if a key has loaded
the public and private key elements.
The `params` field is the parameter set which is resolved via the algorithm name.

In FIPS 205 the SLH_DSA private key contains the public key.
In OpenSSL these components are stored separately, so there must always be a
public key in order for the key to be valid.

The key generation process creates a private key and half of the public key
using DRBG's. The public key root component is then computed based on these
values. For ACVP testing these values are supplied as an ENTROPY parameter.
It is assumed that from data will not deal with a partial public key, and if this
is required the user should use the key generation operation.

Pure vs Pre Hashed Signature Generation
----------------------------------------

The normal signing process (called Pure SLH-DSA Signature Generation)
encodes the message internally as 0x00 || len(ctx) || ctx || message.
where `ctx` is some optional value of size 0x00..0xFF.

ACVP Testing requires the ability for the message to not be encoded also. This
will be controlled by settable parameters.

Pre Hash SLH-DSA Signature Generation encodes the message as

```c
0x01 || len(ctx) || ctx || digest_OID || H(message).
```

The scenario that is stated that this is useful for is when this encoded message
is supplied from an external source.

Currently we do not support the Pre Hash variant as this does not sit well with the
OpenSSL API's.

Signing API
-------------

As only the one-shot implementation is required and the message is not digested
the API's used should be

EVP_PKEY_sign_message_init(), EVP_PKEY_sign(),
EVP_PKEY_verify_message_init(), EVP_PKEY_verify().

OpenSSL command line support
----------------------------

For backwards compatibility reasons EVP_DigestSignInit_ex(), EVP_DigestSign(),
EVP_DigestVerifyInit_ex() and EVP_DigestVerify() may also be used, but the digest
passed in `mdname` must be NULL (i.e. it effectively behaves the same as above).
Passing a non NULL digest results in an error.

OSSL_PKEY_PARAM_MANDATORY_DIGEST must return "" in the key manager getter and
OSSL_SIGNATURE_PARAM_ALGORITHM_ID in the signature context getter.

Buffers
-------

There are many functions pass buffers of size `n` Where n is one of 16,24,32
depending on the algorithm name. These are used for key elements and hashes, so
PACKETS are not used for these.

Where it makes sense to, WPACKET is used for output (such as signature generation)
and PACKET for reading signature data.

Constant Time Considerations
----------------------------

As the security of SLH-DSA depends only on hash functions, we do not foresee
there being any constant time issues. Some if statements have been added to
detect failures in hash operations, and these errors are propagated all the way
up the function call stack. These errors should not happen in general so should
not affect the security of the algorithms.
