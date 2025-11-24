Functions for explicitly fetched PKEY algorithms
================================================

Quick background
----------------

There are several proposed designs that end up revolving around the same
basic need, explicitly fetched signature algorithms.  The following method
type is affected by this document:

- `EVP_SIGNATURE`

Public API - Add variants of `EVP_PKEY_CTX` functionality
---------------------------------------------------------

Through OTC discussions, it's been determined that the most suitable APIs to
touch are the of `EVP_PKEY_` functions.
Specifically, `EVP_PKEY_sign()`, `EVP_PKEY_verify()`, `EVP_PKEY_verify_recover()`
and related functions.
They can be extended to accept an explicitly fetched algorithm of the right
type, and to be able to incrementally process indefinite length data streams
when the fetched algorithm permits it (for example, RSA-SHA256).

It must be made clear that the added functionality cannot be used to compose
an algorithm from different parts.  For example, it's not possible to specify
a `EVP_SIGNATURE` "RSA" and combine it with a parameter that specifies the
hash "SHA256" to get the "RSA-SHA256" functionality.  For an `EVP_SIGNATURE`
"RSA", the input is still expected to be a digest, or some other input that's
limited to the modulus size of the RSA pkey.

### Making things less confusing with distinct function names

Until now, `EVP_PKEY_sign()` and friends were only expected to act on the
pre-computed digest of a message (under the condition that proper flags
and signature md are specified using functions like
`EVP_PKEY_CTX_set_rsa_padding()` and `EVP_PKEY_CTX_set_signature_md()`),
or to act as "primitive" [^1] functions (under the condition that proper
flags are specified, like `RSA_NO_PADDING` for RSA signatures).

This design proposes an extension to also allow full (not pre-hashed)
messages to be passed, in a streaming style through an *update* and a
*final* function.

Discussions have revealed that it is potentially confusing to conflate the
current functionality with streaming style functionality into the same name,
so this design separates those out with specific init / update / final
functions for that purpose.  For oneshot functionality, `EVP_PKEY_sign()`
and `EVP_PKEY_verify()` remain supported.

[^1]: the term "primitive" is borrowed from [PKCS#1](https://www.rfc-editor.org/rfc/rfc8017#section-5)

### Making it possible to verify with an early signature

Some more recent verification algorithms need to obtain the signature
before processing the data.
This is particularly important for streaming modes of operation.
This design proposes a mechanism to accomodate these algorithms
and modes of operation.

New public API - API Reference
------------------------------

### For limited input size / oneshot signing with `EVP_SIGNATURE`

``` C
int EVP_PKEY_sign_init_ex2(EVP_PKEY_CTX *pctx,
                           EVP_SIGNATURE *algo,
                           const OSSL_PARAM params[]);
```

### For signing a stream with `EVP_SIGNATURE`

``` C
int EVP_PKEY_sign_message_init(EVP_PKEY_CTX *pctx,
                               EVP_SIGNATURE *algo,
                               const OSSL_PARAM params[]);
int EVP_PKEY_sign_message_update(EVP_PKEY_CTX *ctx,
                                 const unsigned char *in,
                                 size_t inlen);
int EVP_PKEY_sign_message_final(EVP_PKEY_CTX *ctx,
                                unsigned char *sig,
                                size_t *siglen);
#define EVP_PKEY_sign_message(ctx,sig,siglen,tbs,tbslen) \
    EVP_PKEY_sign(ctx,sig,siglen,tbs,tbslen)
```

### For limited input size / oneshot verification with `EVP_SIGNATURE`

``` C
int EVP_PKEY_verify_init_ex2(EVP_PKEY_CTX *pctx,
                             EVP_SIGNATURE *algo,
                             const OSSL_PARAM params[]);
```

### For verifying a stream with `EVP_SIGNATURE`

``` C
/* Initializers */
int EVP_PKEY_verify_message_init(EVP_PKEY_CTX *pctx,
                                 EVP_SIGNATURE *algo,
                                 const OSSL_PARAM params[]);
/* Signature setter */
int EVP_PKEY_CTX_set_signature(EVP_PKEY_CTX *pctx,
                               unsigned char *sig, size_t siglen,
                               size_t sigsize);
/* Update and final */
int EVP_PKEY_verify_message_update(EVP_PKEY_CTX *ctx,
                                   const unsigned char *in,
                                   size_t inlen);
int EVP_PKEY_verify_message_final(EVP_PKEY_CTX *ctx);

#define EVP_PKEY_verify_message(ctx,sig,siglen,tbs,tbslen) \
    EVP_PKEY_verify(ctx,sig,siglen,tbs,tbslen)
```

### For verify_recover with `EVP_SIGNATURE`

Preliminary feedback suggests that a streaming interface is uninteresting for
verify_recover, so we only specify a new init function.

``` C
/* Initializers */
int EVP_PKEY_verify_recover_init_ex2(EVP_PKEY_CTX *pctx,
                                     EVP_SIGNATURE *algo,
                                     const OSSL_PARAM params[]);
```

Requirements on the providers
-----------------------------

Because it's not immediately obvious from a composite algorithm name what
key type ("RSA", "EC", ...) it requires / supports, at least in code, allowing
the use of an explicitly fetched implementation of a composite algorithm
requires that providers cooperate by declaring what key type is required /
supported by each algorithm.

For non-composite operation algorithms (like "RSA"), this is not necessary,
see the fallback strategies below.

This is to be implemented through an added provider function that would work
like keymgmt's `query_operation_name` function, but would return a NULL
terminated array of key type name instead:

``` C
# define OSSL_FUNC_SIGNATURE_QUERY_KEY_TYPE         26
OSSL_CORE_MAKE_FUNC(const char **, signature_query_key_type, (void))
```

Furthermore, the distinction of intent, i.e. whether the input is expected
to be a pre-hashed digest or the original message, must be passed on to the
provider.  Because we already distinguish that with function names in the
public API, we use the same mapping in the provider interface.

The already existing `signature_sign` and `signature_verify` remain as they
are, and can be combined with message init calls.

``` C
# define OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_INIT      27
# define OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_UPDATE    28
# define OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_FINAL     29
OSSL_CORE_MAKE_FUNC(int, signature_sign_message_init,
                    (void *ctx, void *provkey, const OSSL_PARAM params[]))
OSSL_CORE_MAKE_FUNC(int, signature_sign_message_update,
                    (void *ctx, const unsigned char *in, size_t inlen))
OSSL_CORE_MAKE_FUNC(int, signature_sign_message_final,
                    (void *ctx, unsigned char *sig, size_t *siglen, size_t sigsize))

# define OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_INIT    30
# define OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_UPDATE  31
# define OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_FINAL   32
OSSL_CORE_MAKE_FUNC(int, signature_verify_message_init,
                    (void *ctx, void *provkey, const OSSL_PARAM params[]))
OSSL_CORE_MAKE_FUNC(int, signature_verify_message_update,
                    (void *ctx, const unsigned char *in, size_t inlen))
/*
 * signature_verify_message_final requires that the signature to be verified
 * against is specified via an OSSL_PARAM.
 */
OSSL_CORE_MAKE_FUNC(int, signature_verify_message_final, (void *ctx))
```

Fallback strategies
-------------------

Because existing providers haven't been updated to respond to the key type
query, some fallback strategies will be needed for the init calls that take
an explicitly fetched `EVP_SIGNATURE` argument (they can at least be used
for pre-hashed digest operations).  To find out if the `EVP_PKEY` key type
is possible to use with the explicitly fetched algorithm, the following
fallback strategies may be used.

-   Check if the fetched operation name matches the key type (keymgmt name)
    of the `EVP_PKEY` that's involved in the operation.  For example, this
    is useful when someone fetched the `EVP_SIGNATURE` "RSA".  This requires
    very little modification, as this is already done with the initializer
    functions that fetch the algorithm implicitly.
-   Check if the fetched algorithm name matches the name returned by the
    keymgmt's `query_operation_name` function.  For example, this is useful
    when someone fetched the `EVP_SIGNATURE` "ECDSA", for which the key type
    to use is "EC".  This requires very little modification, as this is
    already done with the initializer functions that fetch the algorithm
    implicitly.

If none of these strategies work out, the operation initialization should
fail.
