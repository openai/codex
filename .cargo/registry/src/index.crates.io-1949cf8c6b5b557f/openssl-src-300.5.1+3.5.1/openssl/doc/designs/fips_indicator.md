OpenSSL FIPS Indicators
=======================

The following document refers to behaviour required by the OpenSSL FIPS provider,
the changes should not affect the default provider.

References
----------

- [1] FIPS 140-3 Standards: <https://csrc.nist.gov/projects/cryptographic-module-validation-program/fips-140-3-standards>
- [2] Approved Security Functions: <https://csrc.nist.gov/projects/cryptographic-module-validation-program/sp-800-140-series-supplemental-information/sp800-140c>
- [3] Approved SSP generation and Establishment methods: <https://csrc.nist.gov/projects/cryptographic-module-validation-program/sp-800-140-series-supplemental-information/sp800-140d>
- [4] Key transitions: <https://csrc.nist.gov/pubs/sp/800/131/a/r2/final>
- [5] FIPS 140-3 Implementation Guidance: <https://csrc.nist.gov/csrc/media/Projects/cryptographic-module-validation-program/documents/fips 140-3/FIPS 140-3 IG.pdf>

Requirements
------------

The following information was extracted from the FIPS 140-3 IG [5] “2.4.C Approved Security Service Indicator”

- A module must have an approved mode of operation that requires at least one service to use an approved security function (defined by [2] and [3]).
- A FIPS 140-3 compliant module requires a built-in service indicator capable of indicating the use of approved security services
- If a module only supports approved services in an approved manner an implicit indicator can be used (e.g. successful completion of a service is an indicator).
- An approved algorithm is not considered to be an approved implementation if it does not have a CAVP certificate or does not include its required self-tests. (i.e. My interpretation of this is that if the CAVP certificate lists an algorithm with only a subset of key sizes, digests, and/or ciphers compared to the implementation, the differences ARE NOT APPROVED. In many places we have no restrictions on the digest or cipher selected).
- Documentation is required to demonstrate how to use indicators for each approved cryptographic algorithm.
- Testing is required to execute all services and verify that the indicator provides an unambiguous indication of whether the service utilizes an approved cryptographic algorithm, security function or process in an approved manner or not.
- The Security Policy may require updates related to indicators. AWS/google have added a table in their security policy called ‘Non-Approved Algorithms not allowed in the approved mode of operation’. An example is RSA with a keysize of < 2048 bits (which has been enforced by [4]).

Since any new FIPS restrictions added could possibly break existing applications
the following additional OpenSSL requirements are also needed:

- The FIPS restrictions should be able to be disabled using Configuration file options (This results in unapproved mode and requires an indicator).
- A mechanism for logging the details of any unapproved mode operations that have been triggered (e.g. DSA Signing)
- The FIPS restrictions should be able to be enabled/disabled per algorithm context.
- If the per algorithm context value is not set, then the  Configuration file option is used.

Solution
--------

In OpenSSL most of the existing code in the FIPS provider is using
implicit indicators i.e. An error occurs if existing FIPS rules are violated.

The following rules will apply to any code that currently is not FIPS approved,
but needs to be.

- The fipsinstall application will have a configurable item added for each algorithm that requires a change. These options will be passed to the FIPS provider in a manner similar to existing code.

- A user defined callback similar to OSSL_SELF_TEST will be added. This callback will be triggered whenever an approved mode test fails.

It may be set up by the user using

```c
typedef int (OSSL_INDICATOR_CALLBACK)(const char *type,
                                      const char *desc,
                                      const OSSL_PARAM params[]);

void OSSL_INDICATOR_set_callback(OSSL_LIB_CTX *libctx,
                                 OSSL_INDICATOR_CALLBACK *cb);
```

The callback can be changed at any time.

- A getter is also supplied (which is also used internally)

```c
void OSSL_INDICATOR_get_callback(OSSL_LIB_CTX *libctx,
                                 OSSL_INDICATOR_CALLBACK **cb);
```

An application's indicator OSSL_INDICATOR_CALLBACK can be used to log that an
indicator was triggered. The callback should normally return non zero.
Returning 0 allows the algorithm to fail, in the same way that a strict check
would fail.

- To control an algorithm context's checks via code requires a setter for each individual check e.g OSSL_PKEY_PARAM_FIPS_KEY_CHECK.

```c
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FIPS_KEY_CHECK);
    if (p != NULL
        && !OSSL_PARAM_get_int(p, &ctx->key_check))
        return 0;
```

The setter is initially -1 (unknown) and can be set to 0 or 1 via a set_ctx call.
If the setter is needed it must be set BEFORE the FIPS related check is done.

If the FIPS related approved mode check fails and either the ctx setter is zero
OR the related FIPS configuration option is zero then the callback is triggered.
If neither the setter of config option are zero then the algorithm should fail.
If the callback is triggered a flag is set in the algorithm ctx that indicates
that this algorithm is unapproved. Once the context is unapproved it will
remain in this state.

- To access the indicator via code requires a getter

```c
    p = OSSL_PARAM_locate(params, OSSL_ALG_PARAM_FIPS_APPROVED_INDICATOR);
    if (p != NULL && !OSSL_PARAM_set_int(p, ctx->approved))
        return 0;
```

This initially has a value of 1, and may be set to 0 if the algorithm is
unapproved. The getter allows you to access the indicator value after the
operation has completed (e.g. Final or Derive related functions)

- Example Algorithm Check

```c
void alg_init(ALG_CTX *ctx)
{
    ctx->strict_checks = -1;
    ctx->approved = 1;
}

void alg_set_params(ALG_CTX *ctx, OSSL_PARAMS params[])
{
    p = OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_STRICT_CHECKS);
    if (p != NULL && !OSSL_PARAM_get_int(p, &ctx->strict_checks))
        return 0;
}

int alg_check_approved(ALG_CTX *ctx)
{
    int approved;

    approved = some_fips_test_passes(ctx->libctx); // Check FIPS restriction for alg
    if (!approved) {
        ctx->approved = 0;
        if (ctx->strict_checks == 0
                || fips_config_get(ctx->libctx, op) == 0) {
            if (!indicator_cb(ctx->libctx, "ALG NAME", "ALG DESC"))
                return 0;
        }
    }
    return 1;
}
```

- Existing security check changes

OpenSSL already uses FIPS configuration options to perform security_checks, but
the existing code needs to change to work with indicators.

e.g. existing code

```c
    if (ossl_securitycheck_enabled(ctx)) {
       pass = do_some_alg_test(ctx);
       if (!pass)
        return 0; /* Produce an error */
    }
```

In updated code for indicators the test always runs.. i.e.

```c
    pass = do_some_alg_test(ctx);
    // Do code similar to alg_check_approved() above
    // which will conditionally decide whether to return an error
    // or trigger the indicator callback.
```

Issues with per algorithm ctx setters
------------------------------------------------

Normally a user would set params (such as OSSL_PKEY_PARAM_FIPS_KEY_CHECK) using
set_ctx_params(), but some algorithms currently do checks in their init operation.
These init functions normally pass an OSSL_PARAM[] argument, but this still
requires the user to set the ctx params in their init.

e.g.

```c
int strict = 0;
params[0] = OSSL_PARAM_construct_int(OSSL_PKEY_PARAM_FIPS_KEY_CHECK, strict);
EVP_DigestSignInit_ex(ctx, &pctx, name, libctx, NULL, pkey, params);
// using EVP_PKEY_CTX_set_params() here would be too late
```

Delaying the check to after the init would be possible, but it would be a change
in existing behaviour. For example the keysize checks are done in the init since
this is when the key is setup.

Notes
-----

There was discussion related to also having a global config setting that could
turn off FIPS mode. This will not be added at this stage.

MACROS
------

A generic object is used internally to embed the variables required for
indicators into each algorithm ctx. The object contains the ctx settables and
the approved flag.

```c
typedef struct ossl_fips_ind_st {
    unsigned char approved;
    signed char settable[OSSL_FIPS_IND_SETTABLE_MAX];
} OSSL_FIPS_IND;
```

Since there are a lot of algorithms where indicators are needed it was decided
to use MACROS to simplify the process.
The following macros are only defined for the FIPS provider.

- OSSL_FIPS_IND_DECLARE

OSSL_FIPS_IND should be placed into the algorithm objects struct that is
returned by a new()

- OSSL_FIPS_IND_COPY(dst, src)

This is used to copy the OSSL_FIPS_IND when calling a dup(). If the dup() uses
*dst = *src then it is not required.

- OSSL_FIPS_IND_INIT(ctx)

Initializes the OSSL_FIPS_IND object. It should be called in the new().

- OSSL_FIPS_IND_ON_UNAPPROVED(ctx, id, libctx, algname, opname, configopt_fn)

This triggers the callback, id is the settable index and must also be used
by OSSL_FIPS_IND_SET_CTX_PARAM(), algname and opname are strings that are passed
to the indicator callback, configopt_fn is the FIPS configuration option.
Where this is triggered depends on the algorithm. In most cases this can be done
in the set_ctx().

- OSSL_FIPS_IND_SETTABLE_CTX_PARAM(name)

This must be put into the algorithms settable ctx_params table.
The name is the settable 'key' name such as OSSL_PKEY_PARAM_FIPS_KEY_CHECK.
There should be a matching name used by OSSL_FIPS_IND_SET_CTX_PARAM().
There may be multiple of these.

- OSSL_FIPS_IND_SET_CTX_PARAM(ctx, id, params, name)

This should be put at the top of the algorithms set_ctx_params().
There may be multiple of these. The name should match an
OSSL_FIPS_IND_SETTABLE_CTX_PARAM() entry.
The id should match an OSSL_FIPS_IND_ON_UNAPPROVED() entry.

- OSSL_FIPS_IND_GETTABLE_CTX_PARAM()

This must be placed in the algorithms gettable_ctx_params table

- OSSL_FIPS_IND_GET_CTX_PARAM(ctx, params)

This must be placed in the algorithms get_ctx_params(),

Some existing algorithms will require set_ctx_params()/settable_ctx_params()
and/or get_ctx_params()/gettable_ctx_params() to be added if required.

New Changes Required
--------------------

The following changes are required for FIPS 140-3 and will require indicators.
On a cases by case basis we must decide what to do when unapproved mode is
detected.
The mechanism using FIPS configuration options and the indicator callback should
be used for most of these unapproved cases (rather than always returning an error).

### key size >= 112 bits

There are a few places where we do not enforce key size that need to be addressed.

- HMAC  Which applies to all algorithms that use HMAC also (e.g. HKDF, SSKDF, KBKDF)
- CMAC
- KMAC

### Algorithm Transitions

- DES_EDE3_ECB.  Disallowed for encryption, allowed for legacy decryption
- DSA.  Keygen and Signing are no longer approved, verify is still allowed.
- ECDSA B & K curves are deprecated, but still approved according to (IG C.K Resolution 4).\
  If we chose not to remove them , then we need to check that OSSL_PKEY_PARAM_USE_COFACTOR_ECDH is set for key agreement if the cofactor is not 1.
- ED25519 and ED448 are now approved.
- X25519 and X448 are not approved currently. keygen and keyexchange would also need an indicator if we allow them?
- RSA encryption(for key agreement/key transport) using PKCSV15 is no longer allowed. (Note that this breaks TLS 1.2 using RSA for KeyAgreement),
  Padding mode updates required. Check RSA KEM also.
- RSA signing using PKCS1 is still allowed (i.e. signature uses shaXXXWithRSAEncryption)
- RSA signing using X931 is no longer allowed. (Still allowed for verification). Check if PSS saltlen needs a indicator (Note FIPS 186-4 Section 5.5 bullet(e). Padding mode updates required in rsa_check_padding(). Check if sha1 is allowed?
- RSA (From SP800-131Ar2) RSA >= 2048 is approved for keygen, signatures and key transport. Verification allows 1024 also. Note also that according to the (IG section C.F) that fips 186-2 verification is also allowed (So this may need either testing OR an indicator, it also mentions the modulus size must be a multiple of 256). Check that rsa_keygen_pairwise_test() and RSA self tests are all compliant with the above RSA restrictions.
- TLS1_PRF  If we are only trying to support TLS1.2 here then we should remove the tls1.0/1.1 code from the FIPS MODULE.
- ECDSA Verify using prehashed message is not allowed.

### Digest Checks

Any algorithms that use a digest need to make sure that the CAVP certificate lists all supported FIPS digests otherwise an indicator is required.
This applies to the following algorithms:

- TLS_1_3_KDF (Only SHA256 and SHA384 Are allowed due to RFC 8446  Appendix B.4)
- TLS1_PRF (Only SHA256,SHA384,SHA512 are allowed)
- X963KDF (SHA1 is not allowed)
- X942KDF
- PBKDF2
- HKDF
- KBKDF
- SSKDF
- SSHKDF
- HMAC
- KMAC
- Any signature algorithms such as RSA, DSA, ECDSA.

The FIPS 140-3 IG Section C.B & C.C have notes related to Vendor affirmation.

Note many of these (such as KDF's will not support SHAKE).
See <https://gitlab.com/redhat/centos-stream/rpms/openssl/-/blob/c9s/0078-KDF-Add-FIPS-indicators.patch?ref_type=heads>
ECDSA and RSA-PSS Signatures allow use of SHAKE.

KECCAK-KMAC-128 and KECCAK-KMAC-256 should not be allowed for anything other than KMAC.
Do we need to check which algorithms allow SHA1 also?

Test that Deterministic ECDSA does not allow SHAKE (IG C.K Additional Comments 6)

### Cipher Checks

- CMAC
- KBKDF CMAC
- GMAC

We should only allow AES. We currently just check the mode.

### Configurable options

- PBKDF2 'lower_bound_checks' needs to be part of the indicator check

Other Changes
-------------

- AES-GCM Security Policy must list AES GCM IV generation scenarios
- TEST_RAND is not approved.
- SSKDF  The security policy needs to be specific about what it supports i.e. hash, kmac 128/256, hmac-hash. There are also currently no limitations on the digest for hash and hmac
- KBKDF  Security policy should list KMAC-128, KMAC-256 otherwise it should be removed.
- KMAC may need a lower bound check on the output size (SP800-185 Section 8.4.2)
- HMAC (FIPS 140-3 IG Section C.D has notes about the output length when using a Truncated HMAC)
