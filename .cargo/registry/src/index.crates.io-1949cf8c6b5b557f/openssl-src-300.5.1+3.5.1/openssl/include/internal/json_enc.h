/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_JSON_ENC_H
# define OSSL_JSON_ENC_H

# include <openssl/bio.h>

/*
 * JSON Encoder
 * ============
 *
 * This JSON encoder is used for qlog. It supports ordinary JSON (RFC 7159),
 * JSON-SEQ (RFC 7464) and I-JSON (RFC 7493). It supports only basic ASCII.
 */

struct json_write_buf {
    BIO     *bio;
    char    *buf;
    size_t  alloc, cur;
};

typedef struct ossl_json_enc_st {
    uint32_t                flags;
    /* error: 1 if an error has occurred. */
    /* state: current state. */
    /* stack stores a bitmap. 0=object, 1=array. */
    /* stack cur   size: stack_end_byte bytes, stack_end_bit bits. */
    /* stack alloc size: stack_bytes bytes. */
    unsigned char           error, stack_end_bit, state, *stack, defer_indent;
    unsigned char           stack_small[16];
    struct json_write_buf   wbuf;
    size_t                  stack_end_byte, stack_bytes;
} OSSL_JSON_ENC;

/*
 * ossl_json_init
 * --------------
 *
 * Initialises a JSON encoder.
 *
 * If the flag OSSL_JSON_FLAG_SEQ is passed, the output is in JSON-SEQ. The
 * caller should use the encoder as though it is encoding members of a JSON
 * array (but without calling ossl_json_array_begin() or ossl_json_array_end()).
 * Each top-level JSON item (e.g. JSON object) encoded will be separated
 * correctly as per the JSON-SEQ format.
 *
 * If the flag OSSL_JSON_FLAG_SEQ is not passed, the output is in JSON format.
 * Generally the caller should encode only a single output item (e.g. a JSON
 * object).
 *
 * By default, JSON output is maximally compact. If OSSL_JSON_FLAG_PRETTY is
 * set, JSON/JSON-SEQ output is spaced for optimal human readability.
 *
 * If OSSL_JSON_FLAG_IJSON is set, integers outside the range `[-2**53 + 1,
 * 2**53 - 1]` are automatically converted to decimal strings before
 * serialization.
 */
#define OSSL_JSON_FLAG_NONE    0
#define OSSL_JSON_FLAG_SEQ     (1U << 0)
#define OSSL_JSON_FLAG_PRETTY  (1U << 1)
#define OSSL_JSON_FLAG_IJSON   (1U << 2)

int ossl_json_init(OSSL_JSON_ENC *json, BIO *bio, uint32_t flags);

/*
 * ossl_json_cleanup
 * -----------------
 *
 * Destroys a JSON encoder.
 */
void ossl_json_cleanup(OSSL_JSON_ENC *json);

/*
 * ossl_json_reset
 * ---------------
 *
 * Resets a JSON encoder, as though it has just been initialised, allowing it
 * to be used again for new output syntactically unrelated to any previous
 * output. This is similar to calling ossl_json_cleanup followed by
 * ossl_json_init but may allow internal buffers to be reused.
 *
 * If the JSON encoder has entered an error state, this function MAY allow
 * recovery from this error state, in which case it will return 1. If this
 * function returns 0, the JSON encoder is unrecoverable and
 * ossl_json_cleanup() must be called.
 *
 * Automatically calls ossl_json_flush().
 */
int ossl_json_reset(OSSL_JSON_ENC *json);

/*
 * ossl_json_flush
 * ---------------
 *
 * Flushes the JSON encoder, ensuring that any residual bytes in internal
 * buffers are written to the provided sink BIO. Flushing may also happen
 * autonomously as buffers are filled, but the caller must use this function
 * to guarantee all data has been flushed.
 */
int ossl_json_flush(OSSL_JSON_ENC *json);

/*
 * ossl_json_flush_cleanup
 * -----------------------
 *
 * Tries to flush as in a call to ossl_json_flush, and then calls
 * ossl_json_cleanup regardless of the result. The result of the flush call is
 * returned.
 */
int ossl_json_flush_cleanup(OSSL_JSON_ENC *json);

/*
 * ossl_json_set0_sink
 * -------------------
 *
 * Changes the sink used by the JSON encoder.
 */
int ossl_json_set0_sink(OSSL_JSON_ENC *json, BIO *bio);

/*
 * ossl_json_in_error
 * ------------------
 *
 * To enhance the ergonomics of the JSON API, the JSON object uses an implicit
 * error tracking model. When a JSON API call fails (for example due to caller
 * error, such as trying to close an array which was not opened), the JSON
 * object enters an error state and all further calls are silently ignored.
 *
 * The caller can detect this condition after it is finished making builder
 * calls to the JSON object by calling this function. This function returns 1
 * if an error occurred. At this point the caller's only recourse is to call
 * ossl_json_reset() or ossl_json_cleanup().
 *
 * Note that partial (i.e., invalid) output may still have been sent to the BIO
 * in this case. Since the amount of output which can potentially be produced
 * by a JSON object is unbounded, it is impractical to buffer it all before
 * flushing. It is expected that errors will ordinarily be either caller errors
 * (programming errors) or BIO errors.
 */
int ossl_json_in_error(OSSL_JSON_ENC *json);

/*
 * JSON Builder Calls
 * ==================
 *
 * These functions are used to build JSON output. The functions which have
 * begin and end function pairs must be called in correctly nested sequence.
 * When writing an object, ossl_json_key() must be called exactly once before
 * each call to write a JSON item.
 *
 * The JSON library takes responsibility for enforcing correct usage patterns.
 * If a call is made that does not correspond to the JSON syntax, the JSON
 * object enters the error state and all subsequent calls are ignored.
 *
 * In JSON-SEQ mode, the caller should act as though the library implicitly
 * places all calls between an ossl_json_array_begin() and
 * ossl_json_array_end() pair; for example, the normal usage pattern would be
 * to call ossl_json_object_begin() followed by ossl_json_object_end(), in
 * repeated sequence.
 *
 * The library does not enforce non-generation of duplicate keys. Avoiding this
 * is the caller's responsibility. It is also the caller's responsibility to
 * pass valid UTF-8 strings. All other forms of invalid output will cause an
 * error. Note that due to the immediate nature of the API, partial output may
 * have already been generated in such a case.
 */

/* Begin a new JSON object. */
void ossl_json_object_begin(OSSL_JSON_ENC *json);

/* End a JSON object. Must be matched with a call to ossl_json_object_begin(). */
void ossl_json_object_end(OSSL_JSON_ENC *json);

/* Begin a new JSON array. */
void ossl_json_array_begin(OSSL_JSON_ENC *json);

/* End a JSON array. Must be matched with a call to ossl_json_array_end(). */
void ossl_json_array_end(OSSL_JSON_ENC *json);

/*
 * Encode a JSON key within an object. Pass a zero-terminated string, which can
 * be freed immediately following the call to this function.
 */
void ossl_json_key(OSSL_JSON_ENC *json, const char *key);

/* Encode a JSON 'null' value. */
void ossl_json_null(OSSL_JSON_ENC *json);

/* Encode a JSON boolean value. */
void ossl_json_bool(OSSL_JSON_ENC *json, int value);

/* Encode a JSON integer from a uint64_t. */
void ossl_json_u64(OSSL_JSON_ENC *json, uint64_t value);

/* Encode a JSON integer from an int64_t. */
void ossl_json_i64(OSSL_JSON_ENC *json, int64_t value);

/*
 * Encode a JSON UTF-8 string from a zero-terminated string. The string passed
 * can be freed immediately following the call to this function.
 */
void ossl_json_str(OSSL_JSON_ENC *json, const char *str);

/*
 * Encode a JSON UTF-8 string from a string with the given length. The string
 * passed can be freed immediately following the call to this function.
 */
void ossl_json_str_len(OSSL_JSON_ENC *json, const char *str, size_t str_len);

/*
 * Encode binary data as a lowercase hex string. data_len is the data length in
 * bytes.
 */
void ossl_json_str_hex(OSSL_JSON_ENC *json, const void *data, size_t data_len);

#endif
