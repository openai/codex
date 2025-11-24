EVP_SKEY Design
===============

The current API assumes that a byte buffer is always available and sufficient
to represent a symmetric key. But for hardware-backed providers, such as
PKCS#11 or TPM, keys generally cannot be exported and need to be represented
via some kind of handle or id.

This design proposes the use of an opaque type to represent symmetric keys. It
doesn't cover any extra parameters (e.g. AEAD-related nonces).

When a provider allows key exporting, the internal representation of the
EVP_SKEY may be turned back into an array of bytes, that can be cached and
become usable by any current cipher supporting symmetric keys in byte form.
In other cases it may happen to be a set of provider-specific parameters,
compatible only with the particular provider.

For clarity, when cipher algorithms are mentioned later, the implicit
assumption is that *any* algorithm accepting a symmetric key should be able
to deal with an EVP_SKEY object from the same provider.

Libcrypto perspective
---------------------

EVP_SKEY objects can be a wrapper around a raw key stored in an internal buffer.

A wrapper EVP_SKEY object stores the key and the key length in an internal
structure. The key management-backed EVP_SKEY object contains 2 pointers: a
pointer to an EVP_SKEYMGMT object and an opaque pointer to a provider specific
object that represents the key.
EVP_SKEYMGMT specifies what parameters are acceptable by the chosen provider and
may be used by specific algorithms of this provider. These parameters are
represented as an array of OSSL_PARAM. We create an API to manage the lifecycle
of EVP_SKEY objects (see below).

Once an EVP_SKEY object is created it can be used in place of a raw key.
The new APIs for this purpose are designed to match the existing ones but use
an EVP_SKEY object as an argument. For usability reasons we keep IV and IV length
as separate arguments in the new API even though they could have been provided as
an element of a OSSL_PARAM array argument.

There are other parameters that some but not all ciphers take. You'll find a
substantial list in the manual for EVP_CipherInit(), and they are suitable for
passing via OSSL_PARAM. The difference here is that *those* params are not
provider-specific but algorithm-specific or operation-specific.

Things that were previously passed to backend implementations with ctrl
functions have become EVP_CIPHER_CTX level OSSL_PARAM items with providers.

If an operation is available only from a different provider than the one that
generated the EVP_SKEY object, then the management code will attempt an export
(and possibly import) operation of the key, including when the target provider
offers only legacy APIs that accept a raw key only. If export fails the
operation fails.

Provider perspective
--------------------

For dealing with opaque keys, we provide a new structure - EVP_SKEYMGMT,
similar to EVP_KEYMGMT.

On the provider end, encryption and decryption implementations will receive an
opaque pointer similar to the provkey pointer used with asymmetric operations.
This pointer is a result of processing an array of OSSL_PARAM by a EVP_SKEYMGMT
operation (import, generate, etc..).

Once created, the EVP_SKEY structure can be exported to the extent the provider
and EVP_SKEYMGMT supports it. There is no guarantee that the result
of export-import operation is the same as the initial set of the parameters.

The EVP_SKEY object generally has a type, however there is a generic catchall
secret type for key that are not tied to a specific operation. Whether a key
is typed depends on how it was created.

Relationships between Objects
-----------------------------

We have an EVP_CIPHER object associated with a particular provider. An EVP_SKEY
object should be compatible with the particular EVP_CIPHER object (the check is
done via comparing the providers). After that it can be used within a
EVP_CIPHER_CTX object associated with the same EVP_CIPHER object.

The way to reuse the key with another provider is through an export/import
process. As mentioned before, it may fail. The way to reuse it with a different
cipher context is `EVP_SKEY_up_ref`.

Key management
--------------

The provider operating EVP_SKEY objects has to implement a separate symmetric
key management to deal with the opaque object.

Allocation and freeing
----------------------

EVP_SKEY object doesn't reuse the pattern 'allocate - assign', these operations
are combined.

```C
EVP_SKEY *EVP_SKEY_generate(OSSL_LIB_CTX *libctx, const char *skeymgmtname,
                            const char *propquery, const OSSL_PARAM *params);
EVP_SKEY *EVP_SKEY_import(OSSL_LIB_CTX *libctx, const char *skeymgmtname,
                          const char *propquery,
                          int selection, const OSSL_PARAM *params);
EVP_SKEY *EVP_SKEY_import_raw(OSSL_LIB_CTX *libctx, const char *skeymgmtname,
                              const char *key, size_t keylen,
                              const char *propquery);
int EVP_SKEY_up_ref(EVP_SKEY *skey);
void EVP_SKEY_free(EVP_SKEY *skey);
```

Exporting the key object
------------------------

```C
int EVP_SKEY_export(const EVP_SKEY *skey, int selection,
                    OSSL_CALLBACK *export_cb, void *export_cbarg);
```

Using EVP_SKEY in cipher operations
-----------------------------------

We provide a function

```C
int EVP_CipherInit_SKEY(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                        EVP_SKEY *skey, const unsigned char *iv, size_t iv_len,
                        int enc, const OSSL_PARAM params[]);
```

similar to `EVP_CipherInit_ex2`. After call to this function, the normal
EVP_CipherUpdate/EVP_CipherFinal API can be used.

Using EVP_SKEY with EVP_MAC
---------------------------

```C
int EVP_MAC_init_SKEY(EVP_MAC_CTX *ctx, const EVP_SKEY *skey,
                      const OSSL_PARAM params[]);
```

similar to `EVP_MAC_init`

API to derive an EVP_SKEY object
--------------------------------

This part is delayed for a while because the proposed API doesn't fit well with
TLS KDFs deriving multiple keys simultaneously.
