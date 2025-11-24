/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include "internal/thread_once.h"
#include <openssl/bio.h>
#include <openssl/crypto.h>
#include <openssl/trace.h>
#include "internal/bio.h"
#include "internal/nelem.h"
#include "internal/refcount.h"
#include "crypto/cryptlib.h"
#include "crypto/ctype.h"

#ifndef OPENSSL_NO_TRACE

static CRYPTO_RWLOCK *trace_lock = NULL;

static const BIO  *current_channel = NULL;

/*-
 * INTERNAL TRACE CHANNEL IMPLEMENTATION
 *
 * For our own flexibility, all trace categories are associated with a
 * BIO sink object, also called the trace channel. Instead of a BIO object,
 * the application can also provide a callback function, in which case an
 * internal trace channel is attached, which simply calls the registered
 * callback function.
 */
static int trace_write(BIO *b, const char *buf,
                               size_t num, size_t *written);
static int trace_puts(BIO *b, const char *str);
static long trace_ctrl(BIO *channel, int cmd, long argl, void *argp);
static int trace_free(BIO *b);

static const BIO_METHOD trace_method = {
    BIO_TYPE_SOURCE_SINK,
    "trace",
    trace_write,
    NULL,                        /* old write */
    NULL,                        /* read_ex */
    NULL,                        /* read */
    trace_puts,
    NULL,                        /* gets */
    trace_ctrl,                  /* ctrl */
    NULL,                        /* create */
    trace_free,                  /* free */
    NULL,                        /* callback_ctrl */
};

struct trace_data_st {
    OSSL_trace_cb callback;
    int category;
    void *data;
};

static int trace_write(BIO *channel,
                       const char *buf, size_t num, size_t *written)
{
    struct trace_data_st *ctx = BIO_get_data(channel);
    size_t cnt = ctx->callback(buf, num, ctx->category, OSSL_TRACE_CTRL_WRITE,
                               ctx->data);

    *written = cnt;
    return cnt != 0;
}

static int trace_puts(BIO *channel, const char *str)
{
    size_t written;

    if (trace_write(channel, str, strlen(str), &written))
        return (int)written;

    return EOF;
}

static long trace_ctrl(BIO *channel, int cmd, long argl, void *argp)
{
    struct trace_data_st *ctx = BIO_get_data(channel);

    switch (cmd) {
    case OSSL_TRACE_CTRL_BEGIN:
    case OSSL_TRACE_CTRL_END:
        /* We know that the callback is likely to return 0 here */
        ctx->callback("", 0, ctx->category, cmd, ctx->data);
        return 1;
    default:
        break;
    }
    return -2;                   /* Unsupported */
}

static int trace_free(BIO *channel)
{
    if (channel == NULL)
        return 0;
    OPENSSL_free(BIO_get_data(channel));
    return 1;
}
#endif

/*-
 * TRACE
 */

/* Helper struct and macro to get name string to number mapping */
struct trace_category_st {
    const char * const name;
    const int num;
};
#define TRACE_CATEGORY_(name)       { #name, OSSL_TRACE_CATEGORY_##name }

static const struct trace_category_st
    trace_categories[OSSL_TRACE_CATEGORY_NUM] = {
    TRACE_CATEGORY_(ALL),
    TRACE_CATEGORY_(TRACE),
    TRACE_CATEGORY_(INIT),
    TRACE_CATEGORY_(TLS),
    TRACE_CATEGORY_(TLS_CIPHER),
    TRACE_CATEGORY_(CONF),
    TRACE_CATEGORY_(ENGINE_TABLE),
    TRACE_CATEGORY_(ENGINE_REF_COUNT),
    TRACE_CATEGORY_(PKCS5V2),
    TRACE_CATEGORY_(PKCS12_KEYGEN),
    TRACE_CATEGORY_(PKCS12_DECRYPT),
    TRACE_CATEGORY_(X509V3_POLICY),
    TRACE_CATEGORY_(BN_CTX),
    TRACE_CATEGORY_(CMP),
    TRACE_CATEGORY_(STORE),
    TRACE_CATEGORY_(DECODER),
    TRACE_CATEGORY_(ENCODER),
    TRACE_CATEGORY_(REF_COUNT),
    TRACE_CATEGORY_(HTTP),
    TRACE_CATEGORY_(PROVIDER),
    TRACE_CATEGORY_(QUERY),
}; /* KEEP THIS LIST IN SYNC with #define OSSL_TRACE_CATEGORY_... in trace.h */

const char *OSSL_trace_get_category_name(int num)
{
    if (num < 0 || (size_t)num >= OSSL_NELEM(trace_categories))
        return NULL;
    /*
     * Partial check that OSSL_TRACE_CATEGORY_... macros
     * are synced with trace_categories array
     */
    if (!ossl_assert(trace_categories[num].name != NULL)
        || !ossl_assert(trace_categories[num].num == num))
        return NULL;
    return trace_categories[num].name;
}

int OSSL_trace_get_category_num(const char *name)
{
    size_t i;

    if (name == NULL)
        return -1;

    for (i = 0; i < OSSL_NELEM(trace_categories); i++)
        if (OPENSSL_strcasecmp(name, trace_categories[i].name) == 0)
            return trace_categories[i].num;

    return -1; /* not found */
}

#ifndef OPENSSL_NO_TRACE

/* We use one trace channel for each trace category */
static struct {
    enum { SIMPLE_CHANNEL, CALLBACK_CHANNEL } type;
    BIO *bio;
    char *prefix;
    char *suffix;
} trace_channels[OSSL_TRACE_CATEGORY_NUM] = {
    { 0, NULL, NULL, NULL },
};

#endif

#ifndef OPENSSL_NO_TRACE

enum {
    CHANNEL,
    PREFIX,
    SUFFIX
};

static int trace_attach_cb(int category, int type, const void *data)
{
    switch (type) {
    case CHANNEL:
        OSSL_TRACE2(TRACE, "Attach channel %p to category '%s'\n",
                    data, trace_categories[category].name);
        break;
    case PREFIX:
        OSSL_TRACE2(TRACE, "Attach prefix \"%s\" to category '%s'\n",
                    (const char *)data, trace_categories[category].name);
        break;
    case SUFFIX:
        OSSL_TRACE2(TRACE, "Attach suffix \"%s\" to category '%s'\n",
                    (const char *)data, trace_categories[category].name);
        break;
    default:                     /* No clue */
        break;
    }
    return 1;
}

static int trace_detach_cb(int category, int type, const void *data)
{
    switch (type) {
    case CHANNEL:
        OSSL_TRACE2(TRACE, "Detach channel %p from category '%s'\n",
                    data, trace_categories[category].name);
        break;
    case PREFIX:
        OSSL_TRACE2(TRACE, "Detach prefix \"%s\" from category '%s'\n",
                    (const char *)data, trace_categories[category].name);
        break;
    case SUFFIX:
        OSSL_TRACE2(TRACE, "Detach suffix \"%s\" from category '%s'\n",
                    (const char *)data, trace_categories[category].name);
        break;
    default:                     /* No clue */
        break;
    }
    return 1;
}

static int do_ossl_trace_init(void);
static CRYPTO_ONCE trace_inited = CRYPTO_ONCE_STATIC_INIT;
DEFINE_RUN_ONCE_STATIC(ossl_trace_init)
{
    return do_ossl_trace_init();
}

static int set_trace_data(int category, int type, BIO **channel,
                          const char **prefix, const char **suffix,
                          int (*attach_cb)(int, int, const void *),
                          int (*detach_cb)(int, int, const void *))
{
    BIO *curr_channel = NULL;
    char *curr_prefix = NULL;
    char *curr_suffix = NULL;

    /* Ensure do_ossl_trace_init() is called once */
    if (!RUN_ONCE(&trace_inited, ossl_trace_init))
        return 0;

    curr_channel = trace_channels[category].bio;
    curr_prefix = trace_channels[category].prefix;
    curr_suffix = trace_channels[category].suffix;

    /* Make sure to run the detach callback first on all data */
    if (prefix != NULL && curr_prefix != NULL) {
        detach_cb(category, PREFIX, curr_prefix);
    }

    if (suffix != NULL && curr_suffix != NULL) {
        detach_cb(category, SUFFIX, curr_suffix);
    }

    if (channel != NULL && curr_channel != NULL) {
        detach_cb(category, CHANNEL, curr_channel);
    }

    /* After detach callbacks are done, clear data where appropriate */
    if (prefix != NULL && curr_prefix != NULL) {
        OPENSSL_free(curr_prefix);
        trace_channels[category].prefix = NULL;
    }

    if (suffix != NULL && curr_suffix != NULL) {
        OPENSSL_free(curr_suffix);
        trace_channels[category].suffix = NULL;
    }

    if (channel != NULL && curr_channel != NULL) {
        BIO_free(curr_channel);
        trace_channels[category].type = 0;
        trace_channels[category].bio = NULL;
    }

    /* Before running callbacks are done, set new data where appropriate */
    if (prefix != NULL && *prefix != NULL) {
        if ((curr_prefix = OPENSSL_strdup(*prefix)) == NULL)
            return 0;
        trace_channels[category].prefix = curr_prefix;
    }

    if (suffix != NULL && *suffix != NULL) {
        if ((curr_suffix = OPENSSL_strdup(*suffix)) == NULL)
            return 0;
        trace_channels[category].suffix = curr_suffix;
    }

    if (channel != NULL && *channel != NULL) {
        trace_channels[category].type = type;
        trace_channels[category].bio = *channel;
        /*
         * This must not be done before setting prefix/suffix,
         * as those may fail, and then the caller is mislead to free *channel.
         */
    }

    /* Finally, run the attach callback on the new data */
    if (channel != NULL && *channel != NULL) {
        attach_cb(category, CHANNEL, *channel);
    }

    if (prefix != NULL && *prefix != NULL) {
        attach_cb(category, PREFIX, *prefix);
    }

    if (suffix != NULL && *suffix != NULL) {
        attach_cb(category, SUFFIX, *suffix);
    }

    return 1;
}

static int do_ossl_trace_init(void)
{
    trace_lock = CRYPTO_THREAD_lock_new();
    return trace_lock != NULL;
}

#endif

void ossl_trace_cleanup(void)
{
#ifndef OPENSSL_NO_TRACE
    int category;
    BIO *channel = NULL;
    const char *prefix = NULL;
    const char *suffix = NULL;

    for (category = 0; category < OSSL_TRACE_CATEGORY_NUM; category++) {
        /* We force the TRACE category to be treated last */
        if (category == OSSL_TRACE_CATEGORY_TRACE)
            continue;
        set_trace_data(category, 0, &channel, &prefix, &suffix,
                       trace_attach_cb, trace_detach_cb);
    }
    set_trace_data(OSSL_TRACE_CATEGORY_TRACE, 0, &channel,
                   &prefix, &suffix,
                   trace_attach_cb, trace_detach_cb);
    CRYPTO_THREAD_lock_free(trace_lock);
#endif
}

int OSSL_trace_set_channel(int category, BIO *channel)
{
#ifndef OPENSSL_NO_TRACE
    if (category >= 0 && category < OSSL_TRACE_CATEGORY_NUM)
        return set_trace_data(category, SIMPLE_CHANNEL, &channel, NULL, NULL,
                              trace_attach_cb, trace_detach_cb);
#endif
    return 0;
}

#ifndef OPENSSL_NO_TRACE
static int trace_attach_w_callback_cb(int category, int type, const void *data)
{
    switch (type) {
    case CHANNEL:
        OSSL_TRACE2(TRACE,
                    "Attach channel %p to category '%s' (with callback)\n",
                    data, trace_categories[category].name);
        break;
    case PREFIX:
        OSSL_TRACE2(TRACE, "Attach prefix \"%s\" to category '%s'\n",
                    (const char *)data, trace_categories[category].name);
        break;
    case SUFFIX:
        OSSL_TRACE2(TRACE, "Attach suffix \"%s\" to category '%s'\n",
                    (const char *)data, trace_categories[category].name);
        break;
    default:                     /* No clue */
        break;
    }
    return 1;
}
#endif

int OSSL_trace_set_callback(int category, OSSL_trace_cb callback, void *data)
{
#ifndef OPENSSL_NO_TRACE
    BIO *channel = NULL;
    struct trace_data_st *trace_data = NULL;

    if (category < 0 || category >= OSSL_TRACE_CATEGORY_NUM)
        return 0;

    if (callback != NULL) {
        if ((channel = BIO_new(&trace_method)) == NULL
            || (trace_data =
                OPENSSL_zalloc(sizeof(struct trace_data_st))) == NULL)
            goto err;

        trace_data->callback = callback;
        trace_data->category = category;
        trace_data->data = data;

        BIO_set_data(channel, trace_data);
    }

    if (!set_trace_data(category, CALLBACK_CHANNEL, &channel, NULL, NULL,
                        trace_attach_w_callback_cb, trace_detach_cb))
        goto err;

    return 1;

 err:
    BIO_free(channel);
    OPENSSL_free(trace_data);
#endif

    return 0;
}

int OSSL_trace_set_prefix(int category, const char *prefix)
{
#ifndef OPENSSL_NO_TRACE
    if (category >= 0 && category < OSSL_TRACE_CATEGORY_NUM)
        return set_trace_data(category, 0, NULL, &prefix, NULL,
                              trace_attach_cb, trace_detach_cb);
#endif
    return 0;
}

int OSSL_trace_set_suffix(int category, const char *suffix)
{
#ifndef OPENSSL_NO_TRACE
    if (category >= 0 && category < OSSL_TRACE_CATEGORY_NUM)
        return set_trace_data(category, 0, NULL, NULL, &suffix,
                              trace_attach_cb, trace_detach_cb);
#endif
    return 0;
}

#ifndef OPENSSL_NO_TRACE
static int ossl_trace_get_category(int category)
{
    if (category < 0 || category >= OSSL_TRACE_CATEGORY_NUM)
        return -1;
    if (trace_channels[category].bio != NULL)
        return category;
    return OSSL_TRACE_CATEGORY_ALL;
}
#endif

int OSSL_trace_enabled(int category)
{
    int ret = 0;
#ifndef OPENSSL_NO_TRACE
    category = ossl_trace_get_category(category);
    if (category >= 0)
        ret = trace_channels[category].bio != NULL;
#endif
    return ret;
}

BIO *OSSL_trace_begin(int category)
{
    BIO *channel = NULL;
#ifndef OPENSSL_NO_TRACE
    char *prefix = NULL;

    category = ossl_trace_get_category(category);
    if (category < 0 || !OSSL_trace_enabled(category))
        return NULL;

    channel = trace_channels[category].bio;
    prefix = trace_channels[category].prefix;

    if (channel != NULL) {
        if (!CRYPTO_THREAD_write_lock(trace_lock))
            return NULL;
        current_channel = channel;
        switch (trace_channels[category].type) {
        case SIMPLE_CHANNEL:
            if (prefix != NULL) {
                (void)BIO_puts(channel, prefix);
                (void)BIO_puts(channel, "\n");
            }
            break;
        case CALLBACK_CHANNEL:
            (void)BIO_ctrl(channel, OSSL_TRACE_CTRL_BEGIN,
                           prefix == NULL ? 0 : strlen(prefix), prefix);
            break;
        }
    }
#endif
    return channel;
}

void OSSL_trace_end(int category, BIO *channel)
{
#ifndef OPENSSL_NO_TRACE
    char *suffix = NULL;

    category = ossl_trace_get_category(category);
    if (category < 0)
        return;
    suffix = trace_channels[category].suffix;
    if (channel != NULL
        && ossl_assert(channel == current_channel)) {
        (void)BIO_flush(channel);
        switch (trace_channels[category].type) {
        case SIMPLE_CHANNEL:
            if (suffix != NULL) {
                (void)BIO_puts(channel, suffix);
                (void)BIO_puts(channel, "\n");
            }
            break;
        case CALLBACK_CHANNEL:
            (void)BIO_ctrl(channel, OSSL_TRACE_CTRL_END,
                           suffix == NULL ? 0 : strlen(suffix), suffix);
            break;
        }
        current_channel = NULL;
        CRYPTO_THREAD_unlock(trace_lock);
    }
#endif
}

int OSSL_trace_string(BIO *out, int text, int full,
                      const unsigned char *data, size_t size)
{
    unsigned char buf[OSSL_TRACE_STRING_MAX + 1];
    int len, i;

    if (!full && size > OSSL_TRACE_STRING_MAX) {
        BIO_printf(out, "[len %zu limited to %d]: ",
                   size, OSSL_TRACE_STRING_MAX);
        len = OSSL_TRACE_STRING_MAX;
    } else {
        len = (int)size;
    }
    if (!text) { /* mask control characters while preserving newlines */
        for (i = 0; i < len; i++, data++)
            buf[i] = (char)*data != '\n' && ossl_iscntrl((int)*data)
                ? ' ' : *data;
        if (len == 0 || data[-1] != '\n')
            buf[len++] = '\n';
        data = buf;
    }
    return BIO_printf(out, "%.*s", len, data);
}
