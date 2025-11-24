/*
 * Copyright 2017-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/opensslconf.h>
#include <openssl/trace.h>
#include "apps.h"
#include "../testutil.h"

#ifndef OPENSSL_NO_TRACE
typedef struct tracedata_st {
    BIO *bio;
    unsigned int ingroup:1;
} tracedata;

static size_t internal_trace_cb(const char *buf, size_t cnt,
                                int category, int cmd, void *vdata)
{
    int ret = 0;
    tracedata *trace_data = vdata;
    char buffer[256], *hex;
    CRYPTO_THREAD_ID tid;

    switch (cmd) {
    case OSSL_TRACE_CTRL_BEGIN:
        trace_data->ingroup = 1;

        tid = CRYPTO_THREAD_get_current_id();
        hex = OPENSSL_buf2hexstr((const unsigned char *)&tid, sizeof(tid));
        BIO_snprintf(buffer, sizeof(buffer), "TRACE[%s]:%s: ",
                     hex, OSSL_trace_get_category_name(category));
        OPENSSL_free(hex);
        BIO_set_prefix(trace_data->bio, buffer);
        break;
    case OSSL_TRACE_CTRL_WRITE:
        ret = BIO_write(trace_data->bio, buf, cnt);
        break;
    case OSSL_TRACE_CTRL_END:
        trace_data->ingroup = 0;

        BIO_set_prefix(trace_data->bio, NULL);
        break;
    }

    return ret < 0 ? 0 : ret;
}

DEFINE_STACK_OF(tracedata)
static STACK_OF(tracedata) *trace_data_stack;

static void tracedata_free(tracedata *data)
{
    BIO_free_all(data->bio);
    OPENSSL_free(data);
}

static STACK_OF(tracedata) *trace_data_stack;

static void cleanup_trace(void)
{
    sk_tracedata_pop_free(trace_data_stack, tracedata_free);
}

static void setup_trace_category(int category)
{
    BIO *channel;
    tracedata *trace_data;
    BIO *bio = NULL;

    if (OSSL_trace_enabled(category))
        return;

    bio = BIO_new(BIO_f_prefix());
    channel = BIO_push(bio,
                       BIO_new_fp(stderr, BIO_NOCLOSE | BIO_FP_TEXT));
    trace_data = OPENSSL_zalloc(sizeof(*trace_data));

    if (trace_data == NULL
        || bio == NULL
        || (trace_data->bio = channel) == NULL
        || OSSL_trace_set_callback(category, internal_trace_cb,
                                   trace_data) == 0
        || sk_tracedata_push(trace_data_stack, trace_data) == 0) {

        fprintf(stderr,
                "warning: unable to setup trace callback for category '%s'.\n",
                OSSL_trace_get_category_name(category));

        OPENSSL_free(trace_data);
        OSSL_trace_set_callback(category, NULL, NULL);
        BIO_free_all(channel);
    }
}

static void setup_trace(const char *str)
{
    char *val;

    /*
     * We add this handler as early as possible to ensure it's executed
     * as late as possible, i.e. after the TRACE code has done its cleanup
     * (which happens last in OPENSSL_cleanup).
     */
    atexit(cleanup_trace);

    trace_data_stack = sk_tracedata_new_null();
    val = OPENSSL_strdup(str);

    if (val != NULL) {
        char *valp = val;
        char *item;

        for (valp = val; (item = strtok(valp, ",")) != NULL; valp = NULL) {
            int category = OSSL_trace_get_category_num(item);

            if (category == OSSL_TRACE_CATEGORY_ALL) {
                while (++category < OSSL_TRACE_CATEGORY_NUM)
                    setup_trace_category(category);
                break;
            } else if (category > 0) {
                setup_trace_category(category);
            } else {
                fprintf(stderr,
                        "warning: unknown trace category: '%s'.\n", item);
            }
        }
    }

    OPENSSL_free(val);
}
#endif /* OPENSSL_NO_TRACE */

int global_init(void)
{
#ifndef OPENSSL_NO_TRACE
    setup_trace(getenv("OPENSSL_TRACE"));
#endif

    return 1;
}
