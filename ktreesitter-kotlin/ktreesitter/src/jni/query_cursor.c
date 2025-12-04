#include "utils.h"

static bool query_progress_callback(TSQueryCursorState *state) {
    ProgressPayload *progress_payload = (ProgressPayload *)state->payload;
    JNIEnv *env = progress_payload->env;
    jobject offset = (*env)->AllocObject(env, global_class_cache.UInt);
    (*env)->SetIntField(env, offset, global_field_cache.UInt_data,
                        (jint)state->current_byte_offset);
    jobject result = CALL_METHOD(Object, progress_payload->callback, Function1_invoke, offset);
    (*env)->DeleteLocalRef(env, offset);
    return (bool)(*env)->GetBooleanField(env, result, global_field_cache.Boolean_value);
}

jlong query_cursor_init CRITICAL_NO_ARGS() { return (jlong)ts_query_cursor_new(); }

void query_cursor_delete CRITICAL_ARGS(jlong query) { ts_query_delete((TSQuery *)query); }

jlong query_cursor_get_timeout_micros(JNIEnv *env, jobject this) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    return (jlong)ts_query_cursor_timeout_micros(cursor);
}

void query_cursor_set_timeout_micros(JNIEnv *env, jobject this, jlong value) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    ts_query_cursor_set_timeout_micros(cursor, (uint64_t)value);
    (*env)->SetLongField(env, this, global_field_cache.QueryCursor_timeoutMicros, value);
}

jint query_cursor_get_match_limit(JNIEnv *env, jobject this) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    return (jint)ts_query_cursor_match_limit(cursor);
}

void query_cursor_set_match_limit(JNIEnv *env, jobject this, jint value) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    ts_query_cursor_set_match_limit(cursor, (uint32_t)value);
    (*env)->SetIntField(env, this, global_field_cache.QueryCursor_matchLimit, value);
}

void query_cursor_set_max_start_depth(JNIEnv *env, jobject this, jint value) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    ts_query_cursor_set_max_start_depth(cursor, (uint32_t)value);
    (*env)->SetIntField(env, this, global_field_cache.QueryCursor_maxStartDepth, value);
}

jboolean query_cursor_did_exceed_match_limit(JNIEnv *env, jobject this) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    return (jboolean)ts_query_cursor_did_exceed_match_limit(cursor);
}

jboolean query_cursor_native_set_byte_range(JNIEnv *env, jobject this, jint start, jint end) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    return (jboolean)ts_query_cursor_set_byte_range(cursor, (uint32_t)start, (uint32_t)end);
}

jboolean query_cursor_native_set_point_range(JNIEnv *env, jobject this, jobject start,
                                             jobject end) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    TSPoint start_point = unmarshal_point(env, start), end_point = unmarshal_point(env, end);
    return (jboolean)ts_query_cursor_set_point_range(cursor, start_point, end_point);
}

void query_cursor_exec(JNIEnv *env, jobject this, jlong query, jobject node,
                       jobject progress_callback) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    TSNode ts_node = unmarshal_node(env, node);
    if (progress_callback == NULL) {
        ts_query_cursor_exec(cursor, (TSQuery *)query, ts_node);
    } else {
        ProgressPayload progress_payload = {.env = env, .callback = progress_callback};
        TSQueryCursorOptions options = {
            .payload = (void *)&progress_payload,
            .progress_callback = query_progress_callback,
        };
        ts_query_cursor_exec_with_options(cursor, (TSQuery *)query, ts_node, &options);
    }
}

jobject query_cursor_next_capture(JNIEnv *env, jobject this, jobject capture_names, jobject tree) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    uint32_t capture_index;
    TSQueryMatch match;
    if (!ts_query_cursor_next_capture(cursor, &match, &capture_index))
        return NULL;

    jobject captures = NEW_OBJECT(ArrayList, (jint)match.capture_count);
    for (uint16_t i = 0; i < match.capture_count; ++i) {
        TSQueryCapture capture = match.captures[i];
        jobject node = marshal_node(env, capture.node, tree);
        jobject name = CALL_METHOD(Object, capture_names, List_get, capture.index);
        if ((*env)->ExceptionCheck(env))
            return NULL;

        jobject capture_obj = NEW_OBJECT(QueryCapture, node, name);
        CALL_METHOD(Boolean, captures, ArrayList_add, capture_obj);
        (*env)->DeleteLocalRef(env, capture_obj);
        (*env)->DeleteLocalRef(env, node);
        (*env)->DeleteLocalRef(env, name);
        if ((*env)->ExceptionCheck(env))
            return NULL;
    }
    jobject match_obj = NEW_OBJECT(QueryMatch, (jint)match.pattern_index, captures);
    jobject index = (*env)->AllocObject(env, global_class_cache.UInt);
    (*env)->SetIntField(env, index, global_field_cache.UInt_data, (jint)capture_index);
    return NEW_OBJECT(Pair, index, match_obj);
}

jobject query_cursor_next_match(JNIEnv *env, jobject this, jobject capture_names, jobject tree) {
    TSQueryCursor *cursor = GET_POINTER(TSQueryCursor, this, QueryCursor_self);
    TSQueryMatch match;
    if (!ts_query_cursor_next_match(cursor, &match))
        return NULL;

    jobject captures = NEW_OBJECT(ArrayList, (jint)match.capture_count);
    for (uint16_t i = 0; i < match.capture_count; ++i) {
        TSQueryCapture capture = match.captures[i];
        jobject node = marshal_node(env, capture.node, tree);
        jobject name = CALL_METHOD(Object, capture_names, List_get, capture.index);
        if ((*env)->ExceptionCheck(env))
            return NULL;

        jobject capture_obj = NEW_OBJECT(QueryCapture, node, name);
        CALL_METHOD(Boolean, captures, ArrayList_add, capture_obj);
        (*env)->DeleteLocalRef(env, capture_obj);
        (*env)->DeleteLocalRef(env, node);
        (*env)->DeleteLocalRef(env, name);
        if ((*env)->ExceptionCheck(env))
            return NULL;
    }
    return NEW_OBJECT(QueryMatch, (jint)match.pattern_index, captures);
}

const JNINativeMethod QueryCursor_methods[] = {
    {"init", "()J", (void *)&query_cursor_init},
    {"delete", "(J)V", (void *)&query_cursor_delete},
    {"getTimeoutMicros", "()J", (void *)&query_cursor_get_timeout_micros},
    {"setTimeoutMicros", "(J)V", (void *)&query_cursor_set_timeout_micros},
    {"getMatchLimit", "()I", (void *)&query_cursor_get_match_limit},
    {"setMatchLimit", "(I)V", (void *)&query_cursor_set_match_limit},
    {"setMaxStartDepth", "(I)V", (void *)&query_cursor_set_max_start_depth},
    {"didExceedMatchLimit", "()Z", (void *)&query_cursor_did_exceed_match_limit},
    {"nativeSetByteRange", "(II)Z", (void *)&query_cursor_native_set_byte_range},
    {"nativeSetPointRange", "(L" PACKAGE "Point;L" PACKAGE "Point;)Z",
     (void *)&query_cursor_native_set_point_range},
    {"nextMatch", "(Ljava/util/List;L" PACKAGE "Tree;)L" PACKAGE "QueryMatch;",
     (void *)&query_cursor_next_match},
    {"nextCapture", "(Ljava/util/List;L" PACKAGE "Tree;)Lkotlin/Pair;",
     (void *)&query_cursor_next_capture},
    {"exec", "(JL" PACKAGE "Node;Lkotlin/jvm/functions/Function1;)V", (void *)&query_cursor_exec},
};

const size_t QueryCursor_methods_size = sizeof QueryCursor_methods / sizeof(JNINativeMethod);
