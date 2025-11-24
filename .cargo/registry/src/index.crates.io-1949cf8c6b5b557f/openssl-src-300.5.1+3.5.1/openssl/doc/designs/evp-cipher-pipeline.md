EVP APIs for supporting cipher pipelining in provided ciphers
=============================================================

OpenSSL previously supported "pipeline" ciphers via ENGINE implementations.
That support was lost when we moved to providers. This document discusses API
design to restore that capability and enable providers to implement such
ciphers.

Pipeline operation
-------------------

Certain ciphers, such as AES-GCM, can be optimized by computing blocks in
parallel. Cipher pipelining support allows application to submit multiple
chunks of data in one cipher update call, thereby allowing the provided
implementation to take advantage of parallel computing. This is very beneficial
for hardware accelerators as pipeline amortizes the latency over multiple
chunks. Our libssl makes use of pipeline as discussed in
[here](https://www.openssl.org/docs/manmaster/man3/SSL_CTX_set_max_pipelines.html).

Pipelining with ENGINE
-----------------------

Before discussing API design for providers, let's take a look at existing
pipeline API that works with engines.

**EVP Interface:**
Flag to denote pipeline support

```c
cipher->flags & EVP_CIPH_FLAG_PIPELINE
```

Input/output and aad buffers are set using `EVP_CIPHER_CTX_ctrl()`

```c
EVP_CIPHER_CTX_ctrl()
    - EVP_CTRL_AEAD_TLS1_AAD (loop: one aad at a time)
    - EVP_CTRL_SET_PIPELINE_OUTPUT_BUFS (array of buffer pointers)
    - EVP_CTRL_SET_PIPELINE_INPUT_BUFS (array of buffer pointers)
    - EVP_CTRL_SET_PIPELINE_INPUT_LENS
```

Single-call cipher invoked to perform encryption/decryption.

```c
EVP_Cipher()
```

Proposal for EVP pipeline APIs
-------------------------------------

Current API design is made similar to non-pipeline counterpart. The document
will be final once the changes are integrated.

**EVP Interface:**
API to check for pipeline support in provided cipher.

```c
/**
 * @brief checks if the provider has exported required pipeline functions
 * This function works only with explicitly fetched EVP_CIPHER instances. i.e.
 * fetched using `EVP_CIPHER_fetch()`. For non-fetched ciphers, it returns 0.
 *
 * @param enc   1 for encryption, 0 for decryption
 * @return 0 (pipeline not supported) or 1 (pipeline supported)
 */
int EVP_CIPHER_can_pipeline(const EVP_CIPHER *cipher, int enc);
```

Multi-call APIs for init, update and final. Associated data for AEAD ciphers
are set in `EVP_CipherPipelineUpdate`.

```c
/**
 * @param iv    array of pointers (array length must be numpipes)
 */
int EVP_CipherPipelineEncryptInit(EVP_CIPHER_CTX *ctx,
                                  const EVP_CIPHER *cipher,
                                  const unsigned char *key, size_t keylen,
                                  size_t numpipes,
                                  const unsigned char **iv, size_t ivlen);
int EVP_CipherPipelineDecryptInit(EVP_CIPHER_CTX *ctx,
                                  const EVP_CIPHER *cipher,
                                  const unsigned char *key, size_t keylen,
                                  size_t numpipes,
                                  const unsigned char **iv, size_t ivlen);

/*
 * @param out      array of pointers to output buffers (array length must be
 *                 numpipes)
 *                 when NULL, input buffers are treated as AAD data
 * @param outl     pointer to array of output length (array length must be
 *                 numpipes)
 * @param outsize  pointer to array of output buffer size (array length must be
 *                 numpipes)
 * @param in       array of pointers to input buffers (array length must be
 *                 numpipes)
 * @param inl      pointer to array of input length (array length must be numpipes)
 */
int EVP_CipherPipelineUpdate(EVP_CIPHER_CTX *ctx,
                             unsigned char **out, size_t *outl,
                             const size_t *outsize,
                             const unsigned char **in, const size_t *inl);

/*
 * @param outm     array of pointers to output buffers (array length must be
 *                 numpipes)
 * @param outl     pointer to array of output length (array length must be
 *                 numpipes)
 * @param outsize  pointer to array of output buffer size (array length must be
 *                 numpipes)
 */
int EVP_CipherPipelineFinal(EVP_CIPHER_CTX *ctx,
                            unsigned char **outm, size_t *outl,
                            const size_t *outsize);
```

API to get/set AEAD auth tag.

```c
/**
 * @param buf   array of pointers to aead buffers (array length must be
 *              numpipes)
 * @param bsize size of one buffer (all buffers must be of same size)
 *
 * AEAD tag len is set using OSSL_CIPHER_PARAM_AEAD_TAGLEN
 */
OSSL_CIPHER_PARAM_PIPELINE_AEAD_TAG (type OSSL_PARAM_OCTET_PTR)
```

**Alternative:** iovec style interface for input/output buffers.

```c
typedef struct {
    unsigned char *buf;
    size_t buf_len;
} EVP_CIPHER_buf;

/**
 * @param out       array of EVP_CIPHER_buf containing output buffers (array
 *                  length must be numpipes)
 *                  when this param is NULL, input buffers are treated as AAD
 *                  data (individual pointers within array being NULL will be
 *                  an error)
 * @param in        array of EVP_CIPHER_buf containing input buffers (array
 *                  length must be numpipes)
 * @param stride    The stride argument must be set to sizeof(EVP_CIPHER_buf)
 */
EVP_CipherPipelineUpdate(EVP_CIPHER_CTX *ctx, EVP_CIPHER_buf *out,
                          EVP_CIPHER_buf *in, size_t stride);

/**
 * @param outm      array of EVP_CIPHER_buf containing output buffers (array
 *                  length must be numpipes)
 * @param stride    The stride argument must be set to sizeof(EVP_CIPHER_buf)
 */
EVP_CipherPipelineFinal(EVP_CIPHER_CTX *ctx,
                          EVP_CIPHER_buf *out, size_t stride);

/**
 * @param buf       array of EVP_CIPHER_buf containing output buffers (array
 *                  length must be numpipes)
 * @param bsize     stride; sizeof(EVP_CIPHER_buf)
 */
OSSL_CIPHER_PARAM_PIPELINE_AEAD_TAG (type OSSL_PARAM_OCTET_PTR)
```

**Design Decisions:**

1. Denoting pipeline support
    - [ ] a. A cipher flag `EVP_CIPH_FLAG_PROVIDED_PIPELINE` (this has to be
      different than EVP_CIPH_FLAG_PIPELINE, so that it doesn't break legacy
      applications).
    - [x] b. A function `EVP_CIPHER_can_pipeline()` that checks if the provider
      exports pipeline functions.
    > **Justification:** flags variable is deprecated in EVP_CIPHER struct.
    > Moreover, EVP can check for presence of pipeline functions, rather than
    > requiring providers to set a flag.

With the introduction of this new API, there will be two APIs for
pipelining available until the legacy code is phased out:
    a. When an Engine that supports pipelining is loaded, it will set the
      `ctx->flags & EVP_CIPH_FLAG_PIPELINE`. If this flag is set, applications
      can continue to use the legacy API for pipelining.
    b. When a Provider that supports pipelining is fetched,
      `EVP_CIPHER_can_pipeline()` will return true, allowing applications to
      utilize the new API for pipelining.

2. `numpipes` argument
    - [x] a. `numpipes` received only in `EVP_CipherPipelineEncryptInit()` and
      saved in EVP_CIPHER_CTX for further use.
    - [ ] b. `numpipes` value is repeatedly received in each
      `EVP_CipherPipelineEncryptInit()`, `EVP_CipherPipelineUpdate()` and
      `EVP_CipherPipelineFinal()` call.
    > **Justification:** It is expected for numpipes to be same across init,
    > update and final operation.

3. Input/Output buffers
    - [x] a. A set of buffers is represented by an array of buffer pointers and
      an array of lengths. Example: `unsigned char **out, size_t *outl`.
    - [ ] b. iovec style: A new type that holds one buffer pointer along with
      its size. Example: `EVP_CIPHER_buf *out`
    > **Justification:** While iovec style is better buffer representation, the
    > EVP - provider interface in core_dispatch.h uses only primitive types.

4. AEAD tag
    - [x] a. A new OSSL_CIPHER_PARAM of type OSSL_PARAM_OCTET_PTR,
      `OSSL_CIPHER_PARAM_PIPELINE_AEAD_TAG`, that uses an array of buffer
      pointers. This can be used with `iovec_buf` if we decide with 3.b.
    - [ ] b. Reuse `OSSL_CIPHER_PARAM_AEAD_TAG` by using it in a loop,
      processing one tag at a time.
    > **Justification:** Reduces cipher get/set param operations.

Future Ideas
------------

1. It would be nice to have a mechanism for fetching provider with pipeline
   support over other providers that don't support pipeline. Maybe by using
   property query strings.
