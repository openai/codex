/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QLOG_H
# define OSSL_QLOG_H

# include <openssl/ssl.h>
# include "internal/quic_types.h"
# include "internal/time.h"

typedef struct qlog_st QLOG;

# ifndef OPENSSL_NO_QLOG

enum {
    QLOG_EVENT_TYPE_NONE,

#  define QLOG_EVENT(cat, name) QLOG_EVENT_TYPE_##cat##_##name,
#  include "internal/qlog_events.h"
#  undef QLOG_EVENT

    QLOG_EVENT_TYPE_NUM
};

typedef struct qlog_trace_info_st {
    QUIC_CONN_ID    odcid;
    const char      *title, *description, *group_id;
    int             is_server;
    OSSL_TIME       (*now_cb)(void *arg);
    void            *now_cb_arg;
    uint64_t        override_process_id;
    const char      *override_impl_name;
} QLOG_TRACE_INFO;

QLOG *ossl_qlog_new(const QLOG_TRACE_INFO *info);
QLOG *ossl_qlog_new_from_env(const QLOG_TRACE_INFO *info);

void ossl_qlog_free(QLOG *qlog);

/* Configuration */
int ossl_qlog_set_event_type_enabled(QLOG *qlog, uint32_t event_type,
                                     int enable);
int ossl_qlog_set_filter(QLOG *qlog, const char *filter);

int ossl_qlog_set_sink_bio(QLOG *qlog, BIO *bio);
#  ifndef OPENSSL_NO_STDIO
int ossl_qlog_set_sink_file(QLOG *qlog, FILE *file, int close_flag);
#  endif
int ossl_qlog_set_sink_filename(QLOG *qlog, const char *filename);

/* Operations */
int ossl_qlog_flush(QLOG *qlog);

/* Queries */
int ossl_qlog_enabled(QLOG *qlog, uint32_t event_type);

/* Grouping Functions */
int ossl_qlog_event_try_begin(QLOG *qlog, uint32_t event_type,
                              const char *event_cat, const char *event_name,
                              const char *event_combined_name);
void ossl_qlog_event_end(QLOG *qlog);

void ossl_qlog_group_begin(QLOG *qlog, const char *name);
void ossl_qlog_group_end(QLOG *qlog);

void ossl_qlog_array_begin(QLOG *qlog, const char *name);
void ossl_qlog_array_end(QLOG *qlog);

void ossl_qlog_override_time(QLOG *qlog, OSSL_TIME event_time);

/* Grouping Macros */
#  define QLOG_EVENT_BEGIN(qlog, cat, name)                                 \
    {                                                                       \
        QLOG *qlog_instance = (qlog);                                       \
        uint32_t qlog_event_type = QLOG_EVENT_TYPE_##cat##_##name;          \
                                                                            \
        if (ossl_qlog_event_try_begin(qlog_instance, qlog_event_type,       \
                                      #cat, #name, #cat ":" #name)) {

#  define QLOG_EVENT_END()                                                  \
            ossl_qlog_event_end(qlog_instance);                             \
        }                                                                   \
    }

#  define QLOG_BEGIN(name)                                                  \
    {                                                                       \
        ossl_qlog_group_begin(qlog_instance, (name));

#  define QLOG_END()                                                        \
        ossl_qlog_group_end(qlog_instance);                                 \
    }

#  define QLOG_BEGIN_ARRAY(name)                                            \
    {                                                                       \
        ossl_qlog_array_begin(qlog_instance, (name));

#  define QLOG_END_ARRAY()                                                  \
        ossl_qlog_array_end(qlog_instance);                                 \
    }

/* Field Functions */
void ossl_qlog_str(QLOG *qlog, const char *name, const char *value);
void ossl_qlog_str_len(QLOG *qlog, const char *name,
                       const char *value, size_t value_len);
void ossl_qlog_u64(QLOG *qlog, const char *name, uint64_t value);
void ossl_qlog_i64(QLOG *qlog, const char *name, int64_t value);
void ossl_qlog_bool(QLOG *qlog, const char *name, int value);
void ossl_qlog_bin(QLOG *qlog, const char *name,
                   const void *value, size_t value_len);

/* Field Macros */
#  define QLOG_STR(name, value)   ossl_qlog_str(qlog_instance, (name), (value))
#  define QLOG_STR_LEN(name, value, value_len)                                \
    ossl_qlog_str_len(qlog_instance, (name), (value), (value_len))
#  define QLOG_I64(name, value)   ossl_qlog_i64(qlog_instance, (name), (value))
#  define QLOG_U64(name, value)   ossl_qlog_u64(qlog_instance, (name), (value))
#  define QLOG_F64(name, value)   ossl_qlog_f64(qlog_instance, (name), (value))
#  define QLOG_BOOL(name, value)  ossl_qlog_bool(qlog_instance, (name), (value))
#  define QLOG_BIN(name, value, value_len) \
    ossl_qlog_bin(qlog_instance, (name), (value), (value_len))
#  define QLOG_CID(name, value) QLOG_BIN((name), (value)->id, (value)->id_len)

# endif

#endif
