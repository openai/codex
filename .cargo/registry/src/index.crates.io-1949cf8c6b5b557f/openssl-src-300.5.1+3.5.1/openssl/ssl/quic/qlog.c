/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/qlog.h"
#include "internal/json_enc.h"
#include "internal/common.h"
#include "internal/cryptlib.h"
#include "crypto/ctype.h"

#define BITS_PER_WORD (sizeof(size_t) * 8)
#define NUM_ENABLED_W ((QLOG_EVENT_TYPE_NUM + BITS_PER_WORD - 1) / BITS_PER_WORD)

static ossl_unused ossl_inline int bit_get(const size_t *p, uint32_t bit_no)
{
    return p[bit_no / BITS_PER_WORD] & (((size_t)1) << (bit_no % BITS_PER_WORD));
}

static ossl_unused ossl_inline void bit_set(size_t *p, uint32_t bit_no, int enable)
{
    size_t mask = (((size_t)1) << (bit_no % BITS_PER_WORD));

    if (enable)
        p[bit_no / BITS_PER_WORD] |= mask;
    else
        p[bit_no / BITS_PER_WORD] &= ~mask;
}

struct qlog_st {
    QLOG_TRACE_INFO info;

    BIO             *bio;
    size_t          enabled[NUM_ENABLED_W];
    uint32_t        event_type;
    const char      *event_cat, *event_name, *event_combined_name;
    OSSL_TIME       event_time, prev_event_time;
    OSSL_JSON_ENC   json;
    int             header_done, first_event_done;
};

static OSSL_TIME default_now(void *arg)
{
    return ossl_time_now();
}

/*
 * Construction
 * ============
 */
QLOG *ossl_qlog_new(const QLOG_TRACE_INFO *info)
{
    QLOG *qlog = OPENSSL_zalloc(sizeof(QLOG));

    if (qlog == NULL)
        return NULL;

    qlog->info.odcid                = info->odcid;
    qlog->info.is_server            = info->is_server;
    qlog->info.now_cb               = info->now_cb;
    qlog->info.now_cb_arg           = info->now_cb_arg;
    qlog->info.override_process_id  = info->override_process_id;

    if (info->title != NULL
        && (qlog->info.title = OPENSSL_strdup(info->title)) == NULL)
        goto err;

    if (info->description != NULL
        && (qlog->info.description = OPENSSL_strdup(info->description)) == NULL)
        goto err;

    if (info->group_id != NULL
        && (qlog->info.group_id = OPENSSL_strdup(info->group_id)) == NULL)
        goto err;

    if (info->override_impl_name != NULL
        && (qlog->info.override_impl_name
                = OPENSSL_strdup(info->override_impl_name)) == NULL)
        goto err;

    if (!ossl_json_init(&qlog->json, NULL,
                        OSSL_JSON_FLAG_IJSON | OSSL_JSON_FLAG_SEQ))
        goto err;

    if (qlog->info.now_cb == NULL)
        qlog->info.now_cb = default_now;

    return qlog;

err:
    if (qlog != NULL) {
        OPENSSL_free((char *)qlog->info.title);
        OPENSSL_free((char *)qlog->info.description);
        OPENSSL_free((char *)qlog->info.group_id);
        OPENSSL_free((char *)qlog->info.override_impl_name);
        OPENSSL_free(qlog);
    }
    return NULL;
}

QLOG *ossl_qlog_new_from_env(const QLOG_TRACE_INFO *info)
{
    QLOG *qlog = NULL;
    const char *qlogdir = ossl_safe_getenv("QLOGDIR");
    const char *qfilter = ossl_safe_getenv("OSSL_QFILTER");
    char qlogdir_sep, *filename = NULL;
    size_t i, l, strl;

    if (info == NULL || qlogdir == NULL)
        return NULL;

    l = strlen(qlogdir);
    if (l == 0)
        return NULL;

    qlogdir_sep = ossl_determine_dirsep(qlogdir);

    /* dir; [sep]; ODCID; _; strlen("client" / "server"); strlen(".sqlog"); NUL */
    strl = l + 1 + info->odcid.id_len * 2 + 1 + 6 + 6 + 1;
    filename = OPENSSL_malloc(strl);
    if (filename == NULL)
        return NULL;

    memcpy(filename, qlogdir, l);
    if (qlogdir_sep != '\0')
        filename[l++] = qlogdir_sep;

    for (i = 0; i < info->odcid.id_len; ++i)
        l += BIO_snprintf(filename + l, strl - l, "%02x", info->odcid.id[i]);

    l += BIO_snprintf(filename + l, strl - l, "_%s.sqlog",
                      info->is_server ? "server" : "client");

    qlog = ossl_qlog_new(info);
    if (qlog == NULL)
        goto err;

    if (!ossl_qlog_set_sink_filename(qlog, filename))
        goto err;

    if (qfilter == NULL || qfilter[0] == '\0')
        qfilter = "*";

    if (!ossl_qlog_set_filter(qlog, qfilter))
        goto err;

    OPENSSL_free(filename);
    return qlog;

err:
    OPENSSL_free(filename);
    ossl_qlog_free(qlog);
    return NULL;
}

void ossl_qlog_free(QLOG *qlog)
{
    if (qlog == NULL)
        return;

    ossl_json_flush_cleanup(&qlog->json);
    BIO_free_all(qlog->bio);
    OPENSSL_free((char *)qlog->info.title);
    OPENSSL_free((char *)qlog->info.description);
    OPENSSL_free((char *)qlog->info.group_id);
    OPENSSL_free((char *)qlog->info.override_impl_name);
    OPENSSL_free(qlog);
}

/*
 * Configuration
 * =============
 */
int ossl_qlog_set_sink_bio(QLOG *qlog, BIO *bio)
{
    if (qlog == NULL)
        return 0;

    ossl_qlog_flush(qlog); /* best effort */
    BIO_free_all(qlog->bio);
    qlog->bio = bio;
    ossl_json_set0_sink(&qlog->json, bio);
    return 1;
}

#ifndef OPENSSL_NO_STDIO

int ossl_qlog_set_sink_file(QLOG *qlog, FILE *f, int close_flag)
{
    BIO *bio;

    if (qlog == NULL)
        return 0;

    bio = BIO_new_fp(f, BIO_CLOSE);
    if (bio == NULL)
        return 0;

    if (!ossl_qlog_set_sink_bio(qlog, bio)) {
        BIO_free_all(bio);
        return 0;
    }

    return 1;
}

#endif

int ossl_qlog_set_sink_filename(QLOG *qlog, const char *filename)
{
    BIO *bio;

    if (qlog == NULL)
        return 0;

    /*
     * We supply our own text encoding as JSON requires UTF-8, so disable any
     * OS-specific processing here.
     */
    bio = BIO_new_file(filename, "wb");
    if (bio == NULL)
        return 0;

    if (!ossl_qlog_set_sink_bio(qlog, bio)) {
        BIO_free_all(bio);
        return 0;
    }

    return 1;
}

int ossl_qlog_flush(QLOG *qlog)
{
    if (qlog == NULL)
        return 1;

    return ossl_json_flush(&qlog->json);
}

int ossl_qlog_set_event_type_enabled(QLOG *qlog, uint32_t event_type,
                                     int enabled)
{
    if (qlog == NULL || event_type >= QLOG_EVENT_TYPE_NUM)
        return 0;

    bit_set(qlog->enabled, event_type, enabled);
    return 1;
}

int ossl_qlog_enabled(QLOG *qlog, uint32_t event_type)
{
    if (qlog == NULL)
        return 0;

    return bit_get(qlog->enabled, event_type) != 0;
}

/*
 * Event Lifecycle
 * ===============
 */
static void write_str_once(QLOG *qlog, const char *key, char **p)
{
    if (*p == NULL)
        return;

    ossl_json_key(&qlog->json, key);
    ossl_json_str(&qlog->json, *p);

    OPENSSL_free(*p);
    *p = NULL;
}

static void qlog_event_seq_header(QLOG *qlog)
{
    if (qlog->header_done)
        return;

    ossl_json_object_begin(&qlog->json);
    {
        ossl_json_key(&qlog->json, "qlog_version");
        ossl_json_str(&qlog->json, "0.3");

        ossl_json_key(&qlog->json, "qlog_format");
        ossl_json_str(&qlog->json, "JSON-SEQ");

        write_str_once(qlog, "title", (char **)&qlog->info.title);
        write_str_once(qlog, "description", (char **)&qlog->info.description);

        ossl_json_key(&qlog->json, "trace");
        ossl_json_object_begin(&qlog->json);
        {
            ossl_json_key(&qlog->json, "common_fields");
            ossl_json_object_begin(&qlog->json);
            {
                ossl_json_key(&qlog->json, "time_format");
                ossl_json_str(&qlog->json, "delta");

                ossl_json_key(&qlog->json, "protocol_type");
                ossl_json_array_begin(&qlog->json);
                {
                    ossl_json_str(&qlog->json, "QUIC");
                } /* protocol_type */
                ossl_json_array_end(&qlog->json);

                write_str_once(qlog, "group_id", (char **)&qlog->info.group_id);

                ossl_json_key(&qlog->json, "system_info");
                ossl_json_object_begin(&qlog->json);
                {
                    if (qlog->info.override_process_id != 0) {
                        ossl_json_key(&qlog->json, "process_id");
                        ossl_json_u64(&qlog->json, qlog->info.override_process_id);
                    } else {
#if defined(OPENSSL_SYS_UNIX)
                        ossl_json_key(&qlog->json, "process_id");
                        ossl_json_u64(&qlog->json, (uint64_t)getpid());
#elif defined(OPENSSL_SYS_WINDOWS)
                        ossl_json_key(&qlog->json, "process_id");
                        ossl_json_u64(&qlog->json, (uint64_t)GetCurrentProcessId());
#endif
                    }
                } /* system_info */
                ossl_json_object_end(&qlog->json);
            } /* common_fields */
            ossl_json_object_end(&qlog->json);

            ossl_json_key(&qlog->json, "vantage_point");
            ossl_json_object_begin(&qlog->json);
            {
                char buf[128];
                const char *p = buf;

                if (qlog->info.override_impl_name != NULL) {
                    p = qlog->info.override_impl_name;
                } else {
                    BIO_snprintf(buf, sizeof(buf), "OpenSSL/%s (%s)",
                                 OpenSSL_version(OPENSSL_FULL_VERSION_STRING),
                                 OpenSSL_version(OPENSSL_PLATFORM) + 10);
                }

                ossl_json_key(&qlog->json, "type");
                ossl_json_str(&qlog->json,
                              qlog->info.is_server ? "server" : "client");

                ossl_json_key(&qlog->json, "name");
                ossl_json_str(&qlog->json, p);
            } /* vantage_point */
            ossl_json_object_end(&qlog->json);
        } /* trace */
        ossl_json_object_end(&qlog->json);
    }
    ossl_json_object_end(&qlog->json);

    qlog->header_done = 1;
}

static void qlog_event_prologue(QLOG *qlog)
{
    qlog_event_seq_header(qlog);

    ossl_json_object_begin(&qlog->json);

    ossl_json_key(&qlog->json, "name");
    ossl_json_str(&qlog->json, qlog->event_combined_name);

    ossl_json_key(&qlog->json, "data");
    ossl_json_object_begin(&qlog->json);
}

static void qlog_event_epilogue(QLOG *qlog)
{
    ossl_json_object_end(&qlog->json);

    ossl_json_key(&qlog->json, "time");
    if (!qlog->first_event_done) {
        ossl_json_u64(&qlog->json, ossl_time2ms(qlog->event_time));
        qlog->prev_event_time = qlog->event_time;
        qlog->first_event_done = 1;
    } else {
        OSSL_TIME delta = ossl_time_subtract(qlog->event_time,
                                             qlog->prev_event_time);

        ossl_json_u64(&qlog->json, ossl_time2ms(delta));
        qlog->prev_event_time = qlog->event_time;
    }

    ossl_json_object_end(&qlog->json);
}

int ossl_qlog_event_try_begin(QLOG *qlog,
                              uint32_t event_type,
                              const char *event_cat,
                              const char *event_name,
                              const char *event_combined_name)
{
    if (qlog == NULL)
        return 0;

    if (!ossl_assert(qlog->event_type == QLOG_EVENT_TYPE_NONE)
        || !ossl_qlog_enabled(qlog, event_type))
        return 0;

    qlog->event_type            = event_type;
    qlog->event_cat             = event_cat;
    qlog->event_name            = event_name;
    qlog->event_combined_name   = event_combined_name;
    qlog->event_time            = qlog->info.now_cb(qlog->info.now_cb_arg);

    qlog_event_prologue(qlog);
    return 1;
}

void ossl_qlog_event_end(QLOG *qlog)
{
    if (!ossl_assert(qlog != NULL && qlog->event_type != QLOG_EVENT_TYPE_NONE))
        return;

    qlog_event_epilogue(qlog);
    qlog->event_type = QLOG_EVENT_TYPE_NONE;
}

/*
 * Field Generators
 * ================
 */
void ossl_qlog_group_begin(QLOG *qlog, const char *name)
{
    if (name != NULL)
        ossl_json_key(&qlog->json, name);

    ossl_json_object_begin(&qlog->json);
}

void ossl_qlog_group_end(QLOG *qlog)
{
    ossl_json_object_end(&qlog->json);
}

void ossl_qlog_array_begin(QLOG *qlog, const char *name)
{
    if (name != NULL)
        ossl_json_key(&qlog->json, name);

    ossl_json_array_begin(&qlog->json);
}

void ossl_qlog_array_end(QLOG *qlog)
{
    ossl_json_array_end(&qlog->json);
}

void ossl_qlog_override_time(QLOG *qlog, OSSL_TIME event_time)
{
    qlog->event_time = event_time;
}

void ossl_qlog_str(QLOG *qlog, const char *name, const char *value)
{
    if (name != NULL)
        ossl_json_key(&qlog->json, name);

    ossl_json_str(&qlog->json, value);
}

void ossl_qlog_str_len(QLOG *qlog, const char *name,
                       const char *value, size_t value_len)
{
    if (name != NULL)
        ossl_json_key(&qlog->json, name);

    ossl_json_str_len(&qlog->json, value, value_len);
}

void ossl_qlog_u64(QLOG *qlog, const char *name, uint64_t value)
{
    if (name != NULL)
        ossl_json_key(&qlog->json, name);

    ossl_json_u64(&qlog->json, value);
}

void ossl_qlog_i64(QLOG *qlog, const char *name, int64_t value)
{
    if (name != NULL)
        ossl_json_key(&qlog->json, name);

    ossl_json_i64(&qlog->json, value);
}

void ossl_qlog_bool(QLOG *qlog, const char *name, int value)
{
    if (name != NULL)
        ossl_json_key(&qlog->json, name);

    ossl_json_bool(&qlog->json, value);
}

void ossl_qlog_bin(QLOG *qlog, const char *name,
                   const void *value, size_t value_len)
{
    if (name != NULL)
        ossl_json_key(&qlog->json, name);

    ossl_json_str_hex(&qlog->json, value, value_len);
}

/*
 * Filter Parsing
 * ==============
 */
struct lexer {
    const char *p, *term_end, *end;
};

static ossl_inline int is_term_sep_ws(char c)
{
    return c == ' ' || c == '\r' || c == '\n' || c == '\t';
}

static ossl_inline int is_name_char(char c)
{
    return ossl_isalpha(c) || ossl_isdigit(c) || c == '_' || c == '-';
}

static int lex_init(struct lexer *lex, const char *in, size_t in_len)
{
    if (in == NULL)
        return 0;

    lex->p          = in;
    lex->term_end   = in;
    lex->end        = in + in_len;
    return 1;
}

static int lex_do(struct lexer *lex)
{
    const char *p = lex->term_end, *end = lex->end, *term_end;

    for (; is_term_sep_ws(*p) && p < end; ++p);

    if (p == end) {
        lex->p          = end;
        lex->term_end   = end;
        return 0;
    }

    for (term_end = p; !is_term_sep_ws(*term_end) && term_end < end; ++term_end);

    lex->p          = p;
    lex->term_end   = term_end;
    return 1;
}

static int lex_eot(struct lexer *lex)
{
    return lex->p == lex->term_end;
}

static int lex_peek_char(struct lexer *lex)
{
    return lex_eot(lex) ? -1 : *lex->p;
}

static int lex_skip_char(struct lexer *lex)
{
    if (lex_eot(lex))
        return 0;

    ++lex->p;
    return 1;
}

static int lex_match(struct lexer *lex, const char *s, size_t s_len)
{
    if ((size_t)(lex->term_end - lex->p) != s_len)
        return 0;

    if (memcmp(lex->p, s, s_len))
        return 0;

    return 1;
}

static void lex_get_rest(struct lexer *lex, const char **str, size_t *str_l)
{
    *str    = lex->p;
    *str_l  = lex->term_end - lex->p;
}

static int lex_extract_to(struct lexer *lex, char c,
                          const char **str, size_t *str_l)
{
    const char *p = lex->p, *term_end = lex->term_end, *s;

    for (s = p; s < term_end && *s != c; ++s);
    if (s == term_end)
        return 0;

    *str    = p;
    *str_l  = s - p;
    lex->p  = ++s;
    return 1;
}

static int ossl_unused filter_match_event(const char *cat, size_t cat_l,
                                          const char *event, size_t event_l,
                                          const char *expect_cat,
                                          const char *expect_event)
{
    size_t expect_cat_l = strlen(expect_cat);
    size_t expect_event_l = strlen(expect_event);

    if ((cat != NULL && cat_l != expect_cat_l)
        || (event != NULL && event_l != expect_event_l)
        || (cat != NULL && memcmp(cat, expect_cat, expect_cat_l))
        || (event != NULL && memcmp(event, expect_event, expect_event_l)))
        return 0;

    return 1;
}

/*
 * enabled: event enablement bitmask Array of size NUM_ENABLED_W.
 * add: 1 to enable an event, 0 to disable.
 * cat: Category name/length. Not necessarily zero terminated.
 *      NULL to match any.
 * event: Event name/length. Not necessarily zero terminated.
 *        NULL to match any.
 */
static void filter_apply(size_t *enabled, int add,
                         const char *cat, size_t cat_l,
                         const char *event, size_t event_l)
{
    /* Find events which match the given filters. */
# define QLOG_EVENT(e_cat, e_name)                          \
    if (filter_match_event(cat, cat_l, event, event_l, \
                                #e_cat, #e_name))           \
        bit_set(enabled, QLOG_EVENT_TYPE_##e_cat##_##e_name, add);
# include "internal/qlog_events.h"
# undef QLOG_EVENT
}

static int lex_fail(struct lexer *lex, const char *msg)
{
    /*
     * TODO(QLOG FUTURE): Determine how to print log messages about bad filter
     * strings
     */
    lex->p = lex->term_end = lex->end;
    return 0;
}

static int validate_name(const char **p, size_t *l)
{
    const char *p_ = *p;
    size_t i, l_ = *l;

    if (l_ == 1 && *p_ == '*') {
        *p = NULL;
        *l = 0;
        return 1;
    }

    if (l_ == 0)
        return 0;

    for (i = 0; i < l_; ++i)
        if (!is_name_char(p_[i]))
            return 0;

    return 1;
}

int ossl_qlog_set_filter(QLOG *qlog, const char *filter)
{
    struct lexer lex = {0};
    char c;
    const char *cat, *event;
    size_t cat_l, event_l, enabled[NUM_ENABLED_W];
    int add;

    memcpy(enabled, qlog->enabled, sizeof(enabled));

    if (!lex_init(&lex, filter, strlen(filter)))
        return 0;

    while (lex_do(&lex)) {
        c = lex_peek_char(&lex);
        if (c == '+' || c == '-') {
            add = (c == '+');
            lex_skip_char(&lex);

            c = lex_peek_char(&lex);
            if (!is_name_char(c) && c != '*')
                return lex_fail(&lex, "expected alphanumeric name or '*'"
                                " after +/-");
        } else if (!is_name_char(c) && c != '*') {
            return lex_fail(&lex, "expected +/- or alphanumeric name or '*'");
        } else {
            add = 1;
        }

        if (lex_match(&lex, "*", 1)) {
            filter_apply(enabled, add, NULL, 0, NULL, 0);
            continue;
        }

        if (!lex_extract_to(&lex, ':', &cat, &cat_l))
            return lex_fail(&lex, "expected ':' after category name");

        lex_get_rest(&lex, &event, &event_l);
        if (!validate_name(&cat, &cat_l))
            return lex_fail(&lex, "expected alphanumeric category name or '*'");
        if (!validate_name(&event, &event_l))
            return lex_fail(&lex, "expected alphanumeric event name or '*'");

        filter_apply(enabled, add, cat, cat_l, event, event_l);
    }

    memcpy(qlog->enabled, enabled, sizeof(enabled));
    return 1;
}
