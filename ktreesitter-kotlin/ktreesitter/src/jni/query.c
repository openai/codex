#include <ctype.h>
#include <string.h>

#include "utils.h"

static inline bool is_valid_identifier_char(char ch) { return isalnum(ch) || ch == '_'; }

static inline bool is_valid_predicate_char(char ch) {
    return isalnum(ch) || ch == '-' || ch == '_' || ch == '?' || ch == '.' || ch == '!';
}

jlong query_init(JNIEnv *env, jclass _class, jlong language, jstring source) {
    TSQueryError error_type;
    const char *source_chars = (*env)->GetStringUTFChars(env, source, NULL);
    uint32_t error_offset, source_len = (*env)->GetStringUTFLength(env, source);
    TSQuery *self =
        ts_query_new((TSLanguage *)language, source_chars, source_len, &error_offset, &error_type);
    if (self != NULL) {
        (*env)->ReleaseStringUTFChars(env, source, source_chars);
        return (jlong)self;
    }

    uint32_t start = 0, end = 0, row = 0, column;
#ifndef _MSC_VER
    char *line = strtok((char *)source_chars, "\n");
#else
    char *next_token = NULL;
    char *line = strtok_s((char *)source_chars, "\n", &next_token);
#endif
    while (line != NULL) {
        end = start + (uint32_t)strlen(line) + 1;
        if (end > error_offset)
            break;
        start = end;
        row += 1;
#ifndef _MSC_VER
        line = strtok(NULL, "\n");
#else
        line = strtok_s(NULL, "\n", &next_token);
#endif
    }
    column = error_offset - start, end = 0;

    switch (error_type) {
        case TSQueryErrorSyntax: {
            jobject exception;
            if (error_offset < source_len) {
                exception = NEW_OBJECT(QueryError$Syntax, (jlong)row, (jlong)column);
            } else {
                exception = NEW_OBJECT(QueryError$Syntax, (jlong)-1, (jlong)-1);
            }
            (*env)->Throw(env, (jthrowable)exception);
            break;
        }
        case TSQueryErrorCapture: {
            while (is_valid_predicate_char(source_chars[error_offset + end])) {
                end += 1;
            }

            char *capture_chars = calloc(end + 1, sizeof(char));
            memcpy(capture_chars, &source_chars[error_offset], end);
            jstring capture = (*env)->NewStringUTF(env, capture_chars);
            jobject exception = NEW_OBJECT(QueryError$Capture, (jint)row, (jint)column, capture);
            (*env)->Throw(env, (jthrowable)exception);
            free(capture_chars);
            break;
        }
        case TSQueryErrorNodeType: {
            while (is_valid_identifier_char(source_chars[error_offset + end])) {
                end += 1;
            }

            char *node_chars = calloc(end + 1, sizeof(char));
            memcpy(node_chars, &source_chars[error_offset], end);
            jstring node = (*env)->NewStringUTF(env, node_chars);
            jobject exception = NEW_OBJECT(QueryError$NodeType, (jint)row, (jint)column, node);
            (*env)->Throw(env, (jthrowable)exception);
            free(node_chars);
            break;
        }
        case TSQueryErrorField: {
            while (is_valid_identifier_char(source_chars[error_offset + end])) {
                end += 1;
            }

            char *field_chars = calloc(end + 1, sizeof(char));
            memcpy(field_chars, &source_chars[error_offset], end);
            jstring field = (*env)->NewStringUTF(env, field_chars);
            jobject exception = NEW_OBJECT(QueryError$Field, (jint)row, (jint)column, field);
            (*env)->Throw(env, (jthrowable)exception);
            free(field_chars);
            break;
        }
        case TSQueryErrorStructure: {
            jobject exception = NEW_OBJECT(QueryError$Structure, (jint)row, (jint)column);
            (*env)->Throw(env, (jthrowable)exception);
            break;
        }
        default:
            UNREACHABLE();
    }

    (*env)->ReleaseStringUTFChars(env, source, source_chars);
    return -1;
}

void query_delete CRITICAL_ARGS(jlong query, jlong cursor) {
    ts_query_delete((TSQuery *)query);
    ts_query_cursor_delete((TSQueryCursor *)cursor);
}

jint query_native_pattern_count(JNIEnv *env, jobject this) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    return (jint)ts_query_pattern_count(self);
}

jint query_native_capture_count(JNIEnv *env, jobject this) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    return (jint)ts_query_capture_count(self);
}

void query_disable_pattern(JNIEnv *env, jobject this, jint index) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    if (ts_query_pattern_count(self) > (uint32_t)index) {
        ts_query_disable_pattern(self, (uint32_t)index);
    } else {
        const char *fmt = "Pattern index %u is out of bounds";
        char buffer[45] = {0};
        sprintf_s(buffer, 45, fmt, (uint32_t)index);
        THROW(IndexOutOfBoundsException, (const char *)buffer);
    }
}

void query_disable_capture(JNIEnv *env, jobject this, jstring capture) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    const char *capture_chars = (*env)->GetStringUTFChars(env, capture, NULL);
    uint32_t length = (uint32_t)(*env)->GetStringUTFLength(env, capture);
    ts_query_disable_capture(self, capture_chars, length);
    (*env)->ReleaseStringUTFChars(env, capture, capture_chars);
}

jint query_start_byte_for_pattern(JNIEnv *env, jobject this, jint index) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    if (ts_query_pattern_count(self) > (uint32_t)index) {
        return (jint)ts_query_start_byte_for_pattern(self, (uint32_t)index);
    }

    const char *fmt = "Pattern index %u is out of bounds";
    char buffer[45] = {0};
    sprintf_s(buffer, 45, fmt, (uint32_t)index);
    THROW(IndexOutOfBoundsException, (const char *)buffer);
    return -1;
}

jint query_end_byte_for_pattern(JNIEnv *env, jobject this, jint index) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    if (ts_query_pattern_count(self) > (uint32_t)index) {
        return (jint)ts_query_end_byte_for_pattern(self, (uint32_t)index);
    }

    const char *fmt = "Pattern index %u is out of bounds";
    char buffer[45] = {0};
    sprintf_s(buffer, 45, fmt, (uint32_t)index);
    THROW(IndexOutOfBoundsException, (const char *)buffer);
    return -1;
}

jboolean query_is_pattern_rooted(JNIEnv *env, jobject this, jint index) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    if (ts_query_pattern_count(self) > (uint32_t)index) {
        return (jboolean)ts_query_is_pattern_rooted(self, (uint32_t)index);
    }

    const char *fmt = "Pattern index %u is out of bounds";
    char buffer[45] = {0};
    sprintf_s(buffer, 45, fmt, (uint32_t)index);
    THROW(IndexOutOfBoundsException, (const char *)buffer);
    return JNI_FALSE;
}

jboolean query_is_pattern_non_local(JNIEnv *env, jobject this, jint index) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    if (ts_query_pattern_count(self) > (uint32_t)index) {
        return (jboolean)ts_query_is_pattern_non_local(self, (uint32_t)index);
    }

    const char *fmt = "Pattern index %u is out of bounds";
    char buffer[45] = {0};
    sprintf_s(buffer, 45, fmt, (uint32_t)index);
    THROW(IndexOutOfBoundsException, (const char *)buffer);
    return JNI_FALSE;
}

jboolean query_native_is_pattern_guaranteed_at_step(JNIEnv *env, jobject this, jint offset) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    return (jboolean)ts_query_is_pattern_guaranteed_at_step(self, (uint32_t)offset);
}

jint query_string_count(JNIEnv *env, jobject this) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    return (jint)ts_query_string_count(self);
}

jstring query_capture_name_for_id(JNIEnv *env, jobject this, jint index) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    uint32_t length;
    const char *name = ts_query_capture_name_for_id(self, (uint32_t)index, &length);
    return name ? (*env)->NewStringUTF(env, name) : NULL;
}

jstring query_string_value_for_id(JNIEnv *env, jobject this, jint index) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    uint32_t length;
    const char *value = ts_query_string_value_for_id(self, (uint32_t)index, &length);
    return value ? (*env)->NewStringUTF(env, value) : NULL;
}

jobject query_predicates_for_pattern(JNIEnv *env, jobject this, jint index) {
    TSQuery *self = GET_POINTER(TSQuery, this, Query_self);
    uint32_t step_count;
    const TSQueryPredicateStep *steps =
        ts_query_predicates_for_pattern(self, (uint32_t)index, &step_count);
    if (step_count == 0)
        return NULL;

    jobject predicates = NEW_OBJECT(ArrayList, (jint)step_count);
    for (uint32_t i = 0; i < step_count; ++i) {
        const jint values[2] = {(jint)steps[i].value_id, (jint)steps[i].type};
        jintArray predicate = (jintArray)(*env)->NewIntArray(env, 2);
        (*env)->SetIntArrayRegion(env, predicate, 0, 2, values);
        CALL_METHOD(Boolean, predicates, ArrayList_add, predicate);
        (*env)->DeleteLocalRef(env, predicate);
    }
    return predicates;
}

const JNINativeMethod Query_methods[] = {
    {"init", "(JLjava/lang/String;)J", (void *)&query_init},
    {"delete", "(J)V", (void *)&query_delete},
    {"nativePatternCount", "()I", (void *)&query_native_pattern_count},
    {"nativeCaptureCount", "()I", (void *)&query_native_capture_count},
    {"disablePattern", "(I)V", (void *)&query_disable_pattern},
    {"disableCapture", "(Ljava/lang/String;)V", (void *)&query_disable_capture},
    {"startByteForPattern", "(I)I", (void *)&query_start_byte_for_pattern},
    {"endByteForPattern", "(I)I", (void *)&query_end_byte_for_pattern},
    {"isPatternRooted", "(I)Z", (void *)&query_is_pattern_rooted},
    {"isPatternNonLocal", "(I)Z", (void *)&query_is_pattern_non_local},
    {"stringCount", "()I", (void *)&query_string_count},
    {"captureNameForId", "(I)Ljava/lang/String;", (void *)&query_capture_name_for_id},
    {"stringValueForId", "(I)Ljava/lang/String;", (void *)&query_string_value_for_id},
    {"nativeIsPatternGuaranteedAtStep", "(I)Z",
     (void *)&query_native_is_pattern_guaranteed_at_step},
    {"predicatesForPattern", "(I)Ljava/util/List;", (void *)&query_predicates_for_pattern},
};

const size_t Query_methods_size = sizeof Query_methods / sizeof(JNINativeMethod);
