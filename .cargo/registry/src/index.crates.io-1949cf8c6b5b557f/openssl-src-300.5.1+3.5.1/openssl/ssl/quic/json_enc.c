/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/json_enc.h"
#include "internal/nelem.h"
#include "internal/numbers.h"
#include <string.h>

/*
 * wbuf
 * ====
 */
static int wbuf_flush(struct json_write_buf *wbuf, int full);

static int wbuf_init(struct json_write_buf *wbuf, BIO *bio, size_t alloc)
{
    wbuf->buf = OPENSSL_malloc(alloc);
    if (wbuf->buf == NULL)
        return 0;

    wbuf->cur   = 0;
    wbuf->alloc = alloc;
    wbuf->bio   = bio;
    return 1;
}

static void wbuf_cleanup(struct json_write_buf *wbuf)
{
    OPENSSL_free(wbuf->buf);
    wbuf->buf   = NULL;
    wbuf->alloc = 0;
}

static void wbuf_set0_bio(struct json_write_buf *wbuf, BIO *bio)
{
    wbuf->bio = bio;
}

/* Empty write buffer. */
static ossl_inline void wbuf_clean(struct json_write_buf *wbuf)
{
    wbuf->cur = 0;
}

/* Available data remaining in buffer. */
static ossl_inline size_t wbuf_avail(struct json_write_buf *wbuf)
{
    return wbuf->alloc - wbuf->cur;
}

/* Add character to write buffer, returning 0 on flush failure. */
static ossl_inline int wbuf_write_char(struct json_write_buf *wbuf, char c)
{
    if (wbuf_avail(wbuf) == 0) {
        if (!wbuf_flush(wbuf, /*full=*/0))
            return 0;
    }

    wbuf->buf[wbuf->cur++] = c;
    return 1;
}

/*
 * Write zero-terminated string to write buffer, returning 0 on flush failure.
 */
static int wbuf_write_str(struct json_write_buf *wbuf, const char *s)
{
    char c;

    while ((c = *s++) != 0)
        if (!wbuf_write_char(wbuf, c))
            return 0;

    return 1;
}

/* Flush write buffer, returning 0 on I/O failure. */
static int wbuf_flush(struct json_write_buf *wbuf, int full)
{
    size_t written = 0, total_written = 0;

    while (total_written < wbuf->cur) {
        if (!BIO_write_ex(wbuf->bio,
                          wbuf->buf + total_written,
                          wbuf->cur - total_written,
                          &written)) {
            memmove(wbuf->buf,
                    wbuf->buf + total_written,
                    wbuf->cur - total_written);
            wbuf->cur = 0;
            return 0;
        }

        total_written += written;
    }

    wbuf->cur = 0;

    if (full)
        (void)BIO_flush(wbuf->bio); /* best effort */

    return 1;
}

/*
 * OSSL_JSON_ENC: Stack Management
 * ===============================
 */

static int json_ensure_stack_size(OSSL_JSON_ENC *json, size_t num_bytes)
{
    unsigned char *stack;

    if (json->stack_bytes >= num_bytes)
        return 1;

    if (num_bytes <= OSSL_NELEM(json->stack_small)) {
        stack = json->stack_small;
    } else {
        if (json->stack == json->stack_small)
            json->stack = NULL;

        stack = OPENSSL_realloc(json->stack, num_bytes);
        if (stack == NULL)
            return 0;
    }

    json->stack         = stack;
    json->stack_bytes   = num_bytes;
    return 1;
}

/* Push one bit onto the stack. Returns 0 on allocation failure. */
static int json_push(OSSL_JSON_ENC *json, unsigned int v)
{
    if (v > 1)
        return 0;

    if (json->stack_end_byte >= json->stack_bytes) {
        size_t new_size
            = (json->stack_bytes == 0)
            ? OSSL_NELEM(json->stack_small)
            : (json->stack_bytes * 2);

        if (!json_ensure_stack_size(json, new_size))
            return 0;

        json->stack_bytes = new_size;
    }

    if (v > 0)
        json->stack[json->stack_end_byte] |= (v << json->stack_end_bit);
    else
        json->stack[json->stack_end_byte] &= ~(1U << json->stack_end_bit);

    json->stack_end_bit = (json->stack_end_bit + 1) % CHAR_BIT;
    if (json->stack_end_bit == 0)
        ++json->stack_end_byte;

    return 1;
}

/*
 * Pop a bit from the stack. Returns 0 if stack is empty. Use json_peek() to get
 * the value before calling this.
 */
static int json_pop(OSSL_JSON_ENC *json)
{
    if (json->stack_end_byte == 0 && json->stack_end_bit == 0)
        return 0;

    if (json->stack_end_bit == 0) {
        --json->stack_end_byte;
        json->stack_end_bit = CHAR_BIT - 1;
    } else {
        --json->stack_end_bit;
    }

    return 1;
}

/*
 * Returns the bit on the top of the stack, or -1 if the stack is empty.
 */
static int json_peek(OSSL_JSON_ENC *json)
{
    size_t obyte, obit;

    obyte = json->stack_end_byte;
    obit  = json->stack_end_bit;
    if (obit == 0) {
       if (obyte == 0)
           return -1;

        --obyte;
        obit = CHAR_BIT - 1;
    } else {
        --obit;
    }

    return (json->stack[obyte] & (1U << obit)) != 0;
}

/*
 * OSSL_JSON_ENC: Initialisation
 * =============================
 */

enum {
    STATE_PRE_KEY,
    STATE_PRE_ITEM,
    STATE_PRE_COMMA
};

static ossl_inline int in_ijson(const OSSL_JSON_ENC *json)
{
    return (json->flags & OSSL_JSON_FLAG_IJSON) != 0;
}

static ossl_inline int in_seq(const OSSL_JSON_ENC *json)
{
    return (json->flags & OSSL_JSON_FLAG_SEQ) != 0;
}

static ossl_inline int in_pretty(const OSSL_JSON_ENC *json)
{
    return (json->flags & OSSL_JSON_FLAG_PRETTY) != 0;
}

int ossl_json_init(OSSL_JSON_ENC *json, BIO *bio, uint32_t flags)
{
    memset(json, 0, sizeof(*json));
    json->flags     = flags;
    json->error     = 0;
    if (!wbuf_init(&json->wbuf, bio, 4096))
        return 0;

    json->state = STATE_PRE_COMMA;
    return 1;
}

void ossl_json_cleanup(OSSL_JSON_ENC *json)
{
    wbuf_cleanup(&json->wbuf);

    if (json->stack != json->stack_small)
        OPENSSL_free(json->stack);

    json->stack = NULL;
}

int ossl_json_flush_cleanup(OSSL_JSON_ENC *json)
{
    int ok = ossl_json_flush(json);

    ossl_json_cleanup(json);
    return ok;
}

int ossl_json_reset(OSSL_JSON_ENC *json)
{
    wbuf_clean(&json->wbuf);
    json->stack_end_byte    = 0;
    json->stack_end_bit     = 0;
    json->error             = 0;
    return 1;
}

int ossl_json_flush(OSSL_JSON_ENC *json)
{
    return wbuf_flush(&json->wbuf, /*full=*/1);
}

int ossl_json_set0_sink(OSSL_JSON_ENC *json, BIO *bio)
{
    wbuf_set0_bio(&json->wbuf, bio);
    return 1;
}

int ossl_json_in_error(OSSL_JSON_ENC *json)
{
    return json->error;
}

/*
 * JSON Builder Calls
 * ==================
 */

static void json_write_qstring(OSSL_JSON_ENC *json, const char *str);
static void json_indent(OSSL_JSON_ENC *json);

static void json_raise_error(OSSL_JSON_ENC *json)
{
    json->error = 1;
}

static void json_undefer(OSSL_JSON_ENC *json)
{
    if (!json->defer_indent)
        return;

    json_indent(json);
}

static void json_write_char(OSSL_JSON_ENC *json, char ch)
{
    if (ossl_json_in_error(json))
        return;

    json_undefer(json);
    if (!wbuf_write_char(&json->wbuf, ch))
        json_raise_error(json);
}

static void json_write_str(OSSL_JSON_ENC *json, const char *s)
{
    if (ossl_json_in_error(json))
        return;

    json_undefer(json);
    if (!wbuf_write_str(&json->wbuf, s))
        json_raise_error(json);
}

static void json_indent(OSSL_JSON_ENC *json)
{
    size_t i, depth;

    json->defer_indent = 0;

    if (!in_pretty(json))
        return;

    json_write_char(json, '\n');

    depth = json->stack_end_byte * 8 + json->stack_end_bit;
    for (i = 0; i < depth * 4; ++i)
        json_write_str(json, "    ");
}

static int json_pre_item(OSSL_JSON_ENC *json)
{
    int s;

    if (ossl_json_in_error(json))
        return 0;

    switch (json->state) {
    case STATE_PRE_COMMA:
        s = json_peek(json);

        if (s == 0) {
            json_raise_error(json);
            return 0;
        }

        if (s == 1) {
            json_write_char(json, ',');
            if (ossl_json_in_error(json))
                return 0;

            json_indent(json);
        }

        if (s < 0 && in_seq(json))
            json_write_char(json, '\x1E');

        json->state = STATE_PRE_ITEM;
        break;

    case STATE_PRE_ITEM:
        break;

    case STATE_PRE_KEY:
    default:
        json_raise_error(json);
        return 0;
    }

    return 1;
}

static void json_post_item(OSSL_JSON_ENC *json)
{
    int s = json_peek(json);

    json->state = STATE_PRE_COMMA;

    if (s < 0 && in_seq(json))
        json_write_char(json, '\n');
}

/*
 * Begin a composite structure (object or array).
 *
 * type: 0=object, 1=array.
 */
static void composite_begin(OSSL_JSON_ENC *json, int type, char ch)
{
    if (!json_pre_item(json)
        || !json_push(json, type))
        json_raise_error(json);

    json_write_char(json, ch);
    json->defer_indent = 1;
}

/*
 * End a composite structure (object or array).
 *
 * type: 0=object, 1=array. Errors on mismatch.
 */
static void composite_end(OSSL_JSON_ENC *json, int type, char ch)
{
    int was_defer = json->defer_indent;

    if (ossl_json_in_error(json))
        return;

    json->defer_indent = 0;

    if (json_peek(json) != type) {
        json_raise_error(json);
        return;
    }

    if (type == 0 && json->state == STATE_PRE_ITEM) {
        json_raise_error(json);
        return;
    }

    if (!json_pop(json)) {
        json_raise_error(json);
        return;
    }

    if (!was_defer)
        json_indent(json);

    json_write_char(json, ch);
    json_post_item(json);
}

/* Begin a new JSON object. */
void ossl_json_object_begin(OSSL_JSON_ENC *json)
{
    composite_begin(json, 0, '{');
    json->state = STATE_PRE_KEY;
}

/* End a JSON object. Must be matched with a call to ossl_json_object_begin(). */
void ossl_json_object_end(OSSL_JSON_ENC *json)
{
    composite_end(json, 0, '}');
}

/* Begin a new JSON array. */
void ossl_json_array_begin(OSSL_JSON_ENC *json)
{
    composite_begin(json, 1, '[');
    json->state = STATE_PRE_ITEM;
}

/* End a JSON array. Must be matched with a call to ossl_json_array_begin(). */
void ossl_json_array_end(OSSL_JSON_ENC *json)
{
    composite_end(json, 1, ']');
}

/*
 * Encode a JSON key within an object. Pass a zero-terminated string, which can
 * be freed immediately following the call to this function.
 */
void ossl_json_key(OSSL_JSON_ENC *json, const char *key)
{
    if (ossl_json_in_error(json))
        return;

    if (json_peek(json) != 0) {
        /* Not in object */
        json_raise_error(json);
        return;
    }

    if (json->state == STATE_PRE_COMMA) {
        json_write_char(json, ',');
        json->state = STATE_PRE_KEY;
    }

    json_indent(json);
    if (json->state != STATE_PRE_KEY) {
        json_raise_error(json);
        return;
    }

    json_write_qstring(json, key);
    if (ossl_json_in_error(json))
        return;

    json_write_char(json, ':');
    if (in_pretty(json))
        json_write_char(json, ' ');

    json->state = STATE_PRE_ITEM;
}

/* Encode a JSON 'null' value. */
void ossl_json_null(OSSL_JSON_ENC *json)
{
    if (!json_pre_item(json))
        return;

    json_write_str(json, "null");
    json_post_item(json);
}

void ossl_json_bool(OSSL_JSON_ENC *json, int v)
{
    if (!json_pre_item(json))
        return;

    json_write_str(json, v > 0 ? "true" : "false");
    json_post_item(json);
}

#define POW_53 (((int64_t)1) << 53)

/* Encode a JSON integer from a uint64_t. */
static void json_u64(OSSL_JSON_ENC *json, uint64_t v, int noquote)
{
    char buf[22], *p = buf + sizeof(buf) - 1;
    int quote = !noquote && in_ijson(json) && v > (uint64_t)(POW_53 - 1);

    if (!json_pre_item(json))
        return;

    if (quote)
        json_write_char(json, '"');

    if (v == 0)
        p = "0";
    else
        for (*p = '\0'; v > 0; v /= 10)
            *--p = '0' + v % 10;

    json_write_str(json, p);

    if (quote)
        json_write_char(json, '"');

    json_post_item(json);
}

void ossl_json_u64(OSSL_JSON_ENC *json, uint64_t v)
{
    json_u64(json, v, 0);
}

/* Encode a JSON integer from an int64_t. */
void ossl_json_i64(OSSL_JSON_ENC *json, int64_t value)
{
    uint64_t uv;
    int quote;

    if (value >= 0) {
        ossl_json_u64(json, (uint64_t)value);
        return;
    }

    if (!json_pre_item(json))
        return;

    quote = in_ijson(json)
        && (value > POW_53 - 1 || value < -POW_53 + 1);

    if (quote)
        json_write_char(json, '"');

    json_write_char(json, '-');

    uv = (value == INT64_MIN)
        ? ((uint64_t)-(INT64_MIN + 1)) + 1
        : (uint64_t)-value;
    json_u64(json, uv, /*noquote=*/1);

    if (quote && !ossl_json_in_error(json))
        json_write_char(json, '"');
}

/*
 * Encode a JSON UTF-8 string from a zero-terminated string. The string passed
 * can be freed immediately following the call to this function.
 */
static ossl_inline int hex_digit(int v)
{
    return v >= 10 ? 'a' + (v - 10) : '0' + v;
}

static ossl_inline void
json_write_qstring_inner(OSSL_JSON_ENC *json, const char *str, size_t str_len,
                         int nul_term)
{
    char c, *o, obuf[7];
    unsigned char *u_str;
    int i;
    size_t j;

    if (ossl_json_in_error(json))
        return;

    json_write_char(json, '"');

    for (j = nul_term ? strlen(str) : str_len; j > 0; str++, j--) {
        c = *str;
        u_str = (unsigned char*)str;
        switch (c) {
        case '\n': o = "\\n"; break;
        case '\r': o = "\\r"; break;
        case '\t': o = "\\t"; break;
        case '\b': o = "\\b"; break;
        case '\f': o = "\\f"; break;
        case '"': o = "\\\""; break;
        case '\\': o = "\\\\"; break;
        default:
            /* valid UTF-8 sequences according to RFC-3629 */
            if (u_str[0] >= 0xc2 && u_str[0] <= 0xdf && j >= 2
                    && u_str[1] >= 0x80 && u_str[1] <= 0xbf) {
                memcpy(obuf, str, 2);
                obuf[2] = '\0';
                str++, j--;
                o = obuf;
                break;
            }
            if (u_str[0] >= 0xe0 && u_str[0] <= 0xef && j >= 3
                    && u_str[1] >= 0x80 && u_str[1] <= 0xbf
                    && u_str[2] >= 0x80 && u_str[2] <= 0xbf
                    && !(u_str[0] == 0xe0 && u_str[1] <= 0x9f)
                    && !(u_str[0] == 0xed && u_str[1] >= 0xa0)) {
                memcpy(obuf, str, 3);
                obuf[3] = '\0';
                str += 2;
                j -= 2;
                o = obuf;
                break;
            }
            if (u_str[0] >= 0xf0 && u_str[0] <= 0xf4 && j >= 4
                    && u_str[1] >= 0x80 && u_str[1] <= 0xbf
                    && u_str[2] >= 0x80 && u_str[2] <= 0xbf
                    && u_str[3] >= 0x80 && u_str[3] <= 0xbf
                    && !(u_str[0] == 0xf0 && u_str[1] <= 0x8f)
                    && !(u_str[0] == 0xf4 && u_str[1] >= 0x90)) {
                memcpy(obuf, str, 4);
                obuf[4] = '\0';
                str += 3;
                j -= 3;
                o = obuf;
                break;
            }
            if (u_str[0] < 0x20 || u_str[0] >= 0x7f) {
                obuf[0] = '\\';
                obuf[1] = 'u';
                for (i = 0; i < 4; ++i)
                    obuf[2 + i] = hex_digit((u_str[0] >> ((3 - i) * 4)) & 0x0F);
                obuf[6] = '\0';
                o = obuf;
            } else {
                json_write_char(json, c);
                continue;
            }
            break;
        }

        json_write_str(json, o);
    }

    json_write_char(json, '"');
}

static void
json_write_qstring(OSSL_JSON_ENC *json, const char *str)
{
    json_write_qstring_inner(json, str, 0, 1);
}

static void
json_write_qstring_len(OSSL_JSON_ENC *json, const char *str, size_t str_len)
{
    json_write_qstring_inner(json, str, str_len, 0);
}

void ossl_json_str(OSSL_JSON_ENC *json, const char *str)
{
    if (!json_pre_item(json))
        return;

    json_write_qstring(json, str);
    json_post_item(json);
}

void ossl_json_str_len(OSSL_JSON_ENC *json, const char *str, size_t str_len)
{
    if (!json_pre_item(json))
        return;

    json_write_qstring_len(json, str, str_len);
    json_post_item(json);
}

/*
 * Encode binary data as a lowercase hex string. data_len is the data length in
 * bytes.
 */
void ossl_json_str_hex(OSSL_JSON_ENC *json, const void *data, size_t data_len)
{
    const unsigned char *b = data, *end = b + data_len;
    unsigned char c;

    if (!json_pre_item(json))
        return;

    json_write_char(json, '"');

    for (; b < end; ++b) {
        c = *b;
        json_write_char(json, hex_digit(c >> 4));
        json_write_char(json, hex_digit(c & 0x0F));
    }

    json_write_char(json, '"');
    json_post_item(json);
}
