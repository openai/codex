#include <string.h>

#include "utils.h"

typedef struct {
    JNIEnv *env;
    jobject callback;
    struct {
        jstring string;
        const char *chars;
    } last_result;
} ReadPayload;

static inline TSInputEncoding get_encoding(JNIEnv *env, jobject encoding) {
    jobject UTF_8 = GET_STATIC_FIELD(Object, InputEncoding, InputEncoding_UTF_8);
    if (encoding == NULL || (*env)->IsSameObject(env, encoding, UTF_8)) {
        return TSInputEncodingUTF8;
    }
    jobject UTF_16LE = GET_STATIC_FIELD(Object, InputEncoding, InputEncoding_UTF_16LE);
    if ((*env)->IsSameObject(env, encoding, UTF_16LE)) {
        return TSInputEncodingUTF16BE;
    }
    jobject UTF_16BE = GET_STATIC_FIELD(Object, InputEncoding, InputEncoding_UTF_16BE);
    if ((*env)->IsSameObject(env, encoding, UTF_16BE)) {
        return TSInputEncodingUTF16LE;
    }
    UNREACHABLE();
}

static void log_function(void *payload, TSLogType log_type, const char *buffer) {
    JNIEnv *env;
    int rc = (*java_vm)->GetEnv(java_vm, (void **)&env, JNI_VERSION_1_6);
    if (rc == JNI_EDETACHED)
#ifndef __ANDROID__
        rc = (*java_vm)->AttachCurrentThread(java_vm, (void **)&env, NULL);
#else
        rc = (*java_vm)->AttachCurrentThread(java_vm, &env, NULL);
#endif
    if (rc != JNI_OK)
        abort();

    jobject log_type_value;
    switch (log_type) {
        case TSLogTypeParse:
            log_type_value = GET_STATIC_FIELD(Object, Parser$LogType, Parser$LogType_PARSE);
            break;
        case TSLogTypeLex:
            log_type_value = GET_STATIC_FIELD(Object, Parser$LogType, Parser$LogType_LEX);
            break;
        default:
            UNREACHABLE();
    }
    jstring message = (*env)->NewStringUTF(env, buffer);
    CALL_METHOD(Object, (jobject)payload, Function2_invoke, log_type_value, message);
}

static const char *parse_read_callback(void *payload, uint32_t byte_index, TSPoint position,
                                       uint32_t *bytes_read) {
    ReadPayload *read_payload = (ReadPayload *)payload;
    JNIEnv *env = read_payload->env;
    jstring last_string = read_payload->last_result.string;
    const char *last_chars = read_payload->last_result.chars;
    if (last_string) {
        (*env)->ReleaseStringUTFChars(env, last_string, last_chars);
    }

    jobject point = marshal_point(env, position);
    jobject byte = (*env)->AllocObject(env, global_class_cache.UInt);
    (*env)->SetIntField(env, byte, global_field_cache.UInt_data, (jint)byte_index);
    jobject char_sequence =
        CALL_METHOD(Object, read_payload->callback, Function2_invoke, byte, point);
    (*env)->DeleteLocalRef(env, byte);
    (*env)->DeleteLocalRef(env, point);
    if ((*env)->ExceptionCheck(env))
        return NULL;
    if (char_sequence == NULL)
        return NULL;

    jstring string = (jstring)CALL_METHOD_NO_ARGS(Object, char_sequence, CharSequence_toString);
    (*env)->DeleteLocalRef(env, char_sequence);
    if ((*env)->ExceptionCheck(env))
        return NULL;

    const char *result = (*env)->GetStringUTFChars(env, string, NULL);
    *bytes_read = (uint32_t)(*env)->GetStringUTFLength(env, string);
    read_payload->last_result.string = string;
    read_payload->last_result.chars = result;
    return result;
}

static bool parse_progress_callback(TSParseState *state) {
    ProgressPayload *progress_payload = (ProgressPayload *)state->payload;
    JNIEnv *env = progress_payload->env;
    jobject offset = (*env)->AllocObject(env, global_class_cache.UInt);
    (*env)->SetIntField(env, offset, global_field_cache.UInt_data,
                        (jint)state->current_byte_offset);
    jobject error = NEW_OBJECT(Boolean, (jboolean)state->has_error);
    jobject result =
        CALL_METHOD(Object, progress_payload->callback, Function2_invoke, offset, error);
    (*env)->DeleteLocalRef(env, offset);
    (*env)->DeleteLocalRef(env, error);
    return (bool)(*env)->GetBooleanField(env, result, global_field_cache.Boolean_value);
}

jlong JNICALL parser_init CRITICAL_NO_ARGS() { return (jlong)ts_parser_new(); }

void JNICALL parser_delete(JNIEnv *env, jclass _class, jlong self) {
    TSLogger logger = ts_parser_logger((TSParser *)self);
    if (logger.payload != NULL)
        (*env)->DeleteGlobalRef(env, (jobject)logger.payload);
    ts_parser_delete((TSParser *)self);
}

jlong JNICALL parser_get_timeout_micros(JNIEnv *env, jobject this) {
    TSParser *self = GET_POINTER(TSParser, this, Parser_self);
    return ts_parser_timeout_micros(self);
}

void JNICALL parser_set_language(JNIEnv *env, jobject this, jobject value) {
    TSParser *self = GET_POINTER(TSParser, this, Parser_self);
    TSLanguage *language = GET_POINTER(TSLanguage, value, Language_self);
    ts_parser_set_language(self, language);
    (*env)->SetObjectField(env, this, global_field_cache.Parser_language, value);
}

void JNICALL parser_set_included_ranges(JNIEnv *env, jobject this, jobject value) {
    TSParser *self = GET_POINTER(TSParser, this, Parser_self);
    uint32_t size = (uint32_t)CALL_METHOD_NO_ARGS(Int, value, List_size);
    if ((*env)->ExceptionCheck(env))
        return;

    TSRange *ts_ranges = (TSRange *)calloc(size, sizeof(TSRange));
    for (uint32_t i = 0; i < size; ++i) {
        jobject range = CALL_METHOD(Object, value, List_get, (jint)i);
        if ((*env)->ExceptionCheck(env))
            return;
        *(ts_ranges + i) = unmarshal_range(env, range);
        (*env)->DeleteLocalRef(env, range);
    }
    if (ts_parser_set_included_ranges(self, ts_ranges, size)) {
        (*env)->SetObjectField(env, this, global_field_cache.Parser_includedRanges, value);
    } else {
        const char *error = "Included ranges must be in ascending order and must not overlap";
        THROW(IllegalArgumentException, error);
    }
    free(ts_ranges);
}

void JNICALL parser_set_timeout_micros(JNIEnv *env, jobject this, jlong value) {
    TSParser *self = GET_POINTER(TSParser, this, Parser_self);
    ts_parser_set_timeout_micros(self, (uint64_t)value);
    (*env)->SetLongField(env, this, global_field_cache.Parser_timeoutMicros, value);
}

void JNICALL parser_set_logger(JNIEnv *env, jobject this, jobject value) {
    TSParser *self = GET_POINTER(TSParser, this, Parser_self);
    TSLogger logger = ts_parser_logger(self);
    if (logger.payload != NULL) {
        (*env)->DeleteGlobalRef(env, (jobject)logger.payload);
    }
    if (value != NULL) {
        jobject payload = (*env)->NewGlobalRef(env, value);
        logger.payload = (void *)payload;
        logger.log = log_function;
    } else {
        logger.payload = NULL;
        logger.log = NULL;
    }
    ts_parser_set_logger(self, logger);
    (*env)->SetObjectField(env, this, global_field_cache.Parser_logger, value);
}

jobject JNICALL parser_parse__string(JNIEnv *env, jobject this, jstring source, jobject encoding,
                                     jobject old_tree) {
    TSParser *self = GET_POINTER(TSParser, this, Parser_self);
    jobject language = GET_FIELD(Object, this, Parser_language);
    if (language == NULL) {
        const char *error = "The parser has no language assigned";
        (*env)->ThrowNew(env, global_class_cache.IllegalStateException, error);
        return NULL;
    }

    TSTree *old_ts_tree = old_tree ? GET_POINTER(TSTree, old_tree, Tree_self) : NULL;
    uint32_t length;
    const char *string = (*env)->GetStringUTFChars(env, source, NULL);
    length = (uint32_t)(*env)->GetStringUTFLength(env, source);
    TSInputEncoding input_encoding = get_encoding(env, encoding);
    TSTree *ts_tree =
        ts_parser_parse_string_encoding(self, old_ts_tree, string, length, input_encoding);
    (*env)->ReleaseStringUTFChars(env, source, string);

    if (ts_tree == NULL) {
        const char *error = "Parsing failed";
        (*env)->ThrowNew(env, global_class_cache.IllegalStateException, error);
        return NULL;
    }
    return NEW_OBJECT(Tree, (jlong)ts_tree, source, language);
}

jobject JNICALL parser_parse__function(JNIEnv *env, jobject this, jobject encoding,
                                       jobject old_tree, jobject progress_callback,
                                       jobject read_callback) {
    TSParser *self = GET_POINTER(TSParser, this, Parser_self);
    jobject language = GET_FIELD(Object, this, Parser_language);
    if (language == NULL) {
        const char *error = "The parser has no language assigned";
        (*env)->ThrowNew(env, global_class_cache.IllegalStateException, error);
        return NULL;
    }
    TSTree *old_ts_tree = old_tree ? GET_POINTER(TSTree, old_tree, Tree_self) : NULL;

    ReadPayload read_payload = {.env = env, .callback = read_callback};
    TSInputEncoding input_encoding = get_encoding(env, encoding);
    TSInput input = {
        .payload = (void *)&read_payload,
        .read = parse_read_callback,
        .encoding = input_encoding,
    };
    TSTree *ts_tree;
    if (progress_callback == NULL) {
        ts_tree = ts_parser_parse(self, old_ts_tree, input);
    } else {
        ProgressPayload progress_payload = {.env = env, .callback = progress_callback};
        TSParseOptions options = {
            .payload = (void *)&progress_payload,
            .progress_callback = parse_progress_callback,
        };
        ts_tree = ts_parser_parse_with_options(self, old_ts_tree, input, options);
    }

    if ((*env)->ExceptionCheck(env)) {
        (*env)->Throw(env, (*env)->ExceptionOccurred(env));
        return NULL;
    }
    if (ts_tree == NULL) {
        const char *error = "Parsing failed";
        (*env)->ThrowNew(env, global_class_cache.IllegalStateException, error);
        return NULL;
    }
    return NEW_OBJECT(Tree, (jlong)ts_tree, NULL, language);
}

void JNICALL parser_reset(JNIEnv *env, jobject this) {
    TSParser *self = GET_POINTER(TSParser, this, Parser_self);
    ts_parser_reset(self);
}

const JNINativeMethod Parser_methods[] = {
    {"init", "()J", (void *)&parser_init},
    {"delete", "(J)V", (void *)&parser_delete},
    {"setLanguage", "(L" PACKAGE "Language;)V", (void *)&parser_set_language},
    {"setIncludedRanges", "(Ljava/util/List;)V", (void *)&parser_set_included_ranges},
    {"getTimeoutMicros", "()J", (void *)&parser_get_timeout_micros},
    {"setTimeoutMicros", "(J)V", (void *)&parser_set_timeout_micros},
    {"setLogger", "(Lkotlin/jvm/functions/Function2;)V", (void *)&parser_set_logger},
    {"parse", "(Ljava/lang/String;L" PACKAGE "InputEncoding;L" PACKAGE "Tree;)L" PACKAGE "Tree;",
     (void *)&parser_parse__string},
    {"parse",
     "(L" PACKAGE "InputEncoding;L" PACKAGE "Tree;Lkotlin/jvm/functions/Function2;"
     "Lkotlin/jvm/functions/Function2;)L" PACKAGE "Tree;",
     (void *)&parser_parse__function},
    {"reset", "()V", (void *)&parser_reset},
};

const size_t Parser_methods_size = sizeof Parser_methods / sizeof(JNINativeMethod);
