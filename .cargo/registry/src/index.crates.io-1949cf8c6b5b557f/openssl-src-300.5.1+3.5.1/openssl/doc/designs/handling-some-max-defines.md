Handling Some MAX Defines in Future
===================================

Problem Definition
------------------

The public headers contain multiple `#define` macros that limit sizes or
numbers of various kinds. In some cases they are uncontroversial so they
do not require any changes or workarounds for these limits. Such values
are not discussed further in this document. This document discusses only
some particularly problematic values and proposes some ways how to
change or overcome these particular limits.

Individual Values
-----------------

### HMAC_MAX_MD_CBLOCK

**Current value:** 200

This is a deprecated define which is useless. It is not used anywhere.

#### Proposed solution:

It should be just removed with 4.0.

### EVP_MAX_MD_SIZE

**Current value:** 64

It is unlikely we will see longer than 512 bit hashes any time soon.
XOF functions do not count and the XOF output length is not and should
not be limited by this value.

It is widely used throughout the codebase and by 3rd party applications.

#### API calls depending on this:

HMAC() - no way to specify the length of the output buffer

X509_pubkey_digest() - no way to specify the length of the output buffer

EVP_Q_digest() - no way to specify the length of the output buffer

EVP_Digest() - no way to specify the length of the output buffer

EVP_DigestFinal_ex() - this is actually documented to allow larger output
if set explicitly by some application call that sets the output size

#### Proposed solution:

Keep the value as is, do not deprecate. Review the codebase if it isn't
used in places where XOF might be used with arbitrary output length.

Perhaps introduce API calls replacing the calls above that would have
an input parameter indicating the size of the output buffer.

### EVP_MAX_KEY_LENGTH

**Current value:** 64

This is used throughout the code and depended on in a subtle way. It can
be assumed that 3rd party applications use this value to allocate fixed
buffers for keys. It is unlikely that symmetric ciphers with keys longer
than 512 bits will be used any time soon.

#### API calls depending on this:

EVP_KDF_CTX_get_kdf_size() returns EVP_MAX_KEY_LENGTH for KRB5KDF until
the cipher is set.

EVP_CIPHER_CTX_rand_key() - no way to specify the length of the output
buffer.

#### Proposed solution:

Keep the value as is, do not deprecate. Possibly review the codebase
to not depend on this value but there are many such cases. Avoid adding
further APIs depending on this value.

### EVP_MAX_IV_LENGTH

**Current value:** 16

This value is the most problematic one as in case there are ciphers with
longer block size than 128 bits it could be potentially useful to have
longer IVs than just 16 bytes. There are many cases throughout the code
where fixed size arrays of EVP_MAX_IV_LENGTH are used.

#### API calls depending on this:

SSL_CTX_set_tlsext_ticket_key_evp_cb() explicitly uses EVP_MAX_IV_LENGTH
in the callback function signature.

SSL_CTX_set_tlsext_ticket_key_cb() is a deprecated version of the same
and has the same problem.

#### Proposed solution:

Deprecate the above API call and add a replacement which explicitly
passes the length of the _iv_ parameter.

Review and modify the codebase to not depend on and use EVP_MAX_IV_LENGTH.

Deprecate the EVP_MAX_IV_LENGTH macro. Avoid adding further APIs depending
on this value.

### EVP_MAX_BLOCK_LENGTH

**Current value:** 32

This is used in a few places in the code. It is possible that this is
used by 3rd party applications to allocate some fixed buffers for single
or multiple blocks. It is unlikely that symmetric ciphers with block sizes
 longer than 256 bits will be used any time soon.

#### API calls depending on this:

None

#### Proposed solution:

Keep the value as is, do not deprecate. Possibly review the codebase
to not depend on this value but there are many such cases. Avoid adding
APIs depending on this value.

### EVP_MAX_AEAD_TAG_LENGTH

**Current value:** 16

This macro is used in a single place in hpke to allocate a fixed buffer.
The EVP_EncryptInit(3) manual page mentions the tag size being at most
16 bytes for EVP_CIPHER_CTX_ctrl(EVP_CTRL_AEAD_SET_TAG). The value is
problematic as for HMAC/KMAC based AEAD ciphers the tag length can be
larger than block size. Even in case we would have block ciphers with
256 block size the maximum tag length value of 16 is limiting.

#### API calls depending on this:

None (except the documentation in
EVP_CIPHER_CTX_ctrl(EVP_CTRL_AEAD_SET/GET_TAG))

#### Proposed solution:

Review and modify the codebase to not depend on and use
EVP_MAX_AEAD_TAG_LENGTH.

Deprecate the EVP_MAX_AEAD_TAG_LENGTH macro. Avoid adding APIs depending
on this value.
