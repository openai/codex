#pragma once

#include <jni.h>
#include <tree_sitter/api.h>

#define _xcat(a, b, c) a##b##c
#define _cat3(a, b, c) _xcat(a, b, c)
#define _cat2(a, b) _xcat(a, _, b)

#define PACKAGE "io/github/treesitter/ktreesitter/"

#define GET_FIELD(jtype, object, field)                                                            \
    (*env)->_cat3(Get, jtype, Field)(env, (object), global_field_cache.field)

#define GET_STATIC_FIELD(jtype, class, field)                                                      \
    (*env)->_cat3(GetStatic, jtype, Field)(env, global_class_cache.class, global_field_cache.field)

#define GET_POINTER(ctype, object, field) (ctype *)GET_FIELD(Long, object, field)

#define CALL_METHOD(jtype, object, method, ...)                                                    \
    (*env)->_cat3(Call, jtype, Method)(env, (object), global_method_cache.method, __VA_ARGS__)

#define CALL_STATIC_METHOD(jtype, class, method, ...)                                              \
    (*env)->_cat3(CallStatic, jtype, Method)(env, global_class_cache.class,                        \
                                             global_method_cache.method, __VA_ARGS__)

#define CALL_METHOD_NO_ARGS(jtype, object, method)                                                 \
    (*env)->_cat3(Call, jtype, Method)(env, (object), global_method_cache.method)

#define NEW_OBJECT(class, ...)                                                                     \
    (*env)->NewObject(env, global_class_cache.class, global_method_cache._cat2(class, init),       \
                      __VA_ARGS__)

#define THROW(class, message) (*env)->ThrowNew(env, global_class_cache.class, message)

#ifndef __ANDROID__
#define CRITICAL_ARGS(...) (JNIEnv * _env, jclass _class, __VA_ARGS__)
#define CRITICAL_NO_ARGS() (JNIEnv * _env, jclass _class)
#else
#define CRITICAL_ARGS(...) (__VA_ARGS__)
#define CRITICAL_NO_ARGS() ()
#endif

#if defined(__GNUC__) || defined(__clang__)
#define UNREACHABLE() __builtin_unreachable()
#elif defined(_MSC_VER)
#define UNREACHABLE() __assume(0)
#else
#define UNREACHABLE() abort()
#endif

#ifndef _WIN32
#define sprintf_s(buffer, size, format, ...) snprintf((buffer), (size), (format), __VA_ARGS__)
#endif

typedef struct {
    JNIEnv *env;
    jobject callback;
} ProgressPayload;

typedef struct {
    jfieldID Boolean_value;
    jfieldID InputEdit_newEndByte;
    jfieldID InputEdit_newEndPoint;
    jfieldID InputEdit_oldEndByte;
    jfieldID InputEdit_oldEndPoint;
    jfieldID InputEdit_startByte;
    jfieldID InputEdit_startPoint;
    jfieldID InputEncoding_UTF_8;
    jfieldID InputEncoding_UTF_16LE;
    jfieldID InputEncoding_UTF_16BE;
    jfieldID Language_self;
    jfieldID LookaheadIterator_self;
    jfieldID Node_context;
    jfieldID Node_id;
    jfieldID Node_internalChildren;
    jfieldID Node_tree;
    jfieldID Parser$LogType_LEX;
    jfieldID Parser$LogType_PARSE;
    jfieldID Parser_includedRanges;
    jfieldID Parser_language;
    jfieldID Parser_logger;
    jfieldID Parser_self;
    jfieldID Parser_timeoutMicros;
    jfieldID Point_column;
    jfieldID Point_row;
    jfieldID QueryCursor_matchLimit;
    jfieldID QueryCursor_maxStartDepth;
    jfieldID QueryCursor_self;
    jfieldID QueryCursor_timeoutMicros;
    jfieldID Query_language;
    jfieldID Query_self;
    jfieldID Query_source;
    jfieldID Range_endByte;
    jfieldID Range_endPoint;
    jfieldID Range_startByte;
    jfieldID Range_startPoint;
    jfieldID TreeCursor_internalNode;
    jfieldID TreeCursor_self;
    jfieldID TreeCursor_tree;
    jfieldID Tree_self;
    jfieldID Tree_source;
    jfieldID UInt_data;
    jfieldID UShort_data;
} FieldCache;

typedef struct {
    jmethodID ArrayList_add;
    jmethodID ArrayList_init;
    jmethodID Boolean_init;
    jmethodID CharSequence_toString;
    jmethodID Function1_invoke;
    jmethodID Function2_invoke;
    jmethodID Language_init;
    jmethodID Language$Metadata_init;
    jmethodID List_get;
    jmethodID List_size;
    jmethodID Node_init;
    jmethodID Pair_init;
    jmethodID Point_init;
    jmethodID QueryCapture_init;
    jmethodID QueryCursor_init;
    jmethodID QueryError$Capture_init;
    jmethodID QueryError$Field_init;
    jmethodID QueryError$NodeType_init;
    jmethodID QueryError$Structure_init;
    jmethodID QueryError$Syntax_init;
    jmethodID QueryMatch_init;
    jmethodID Range_init;
    jmethodID Tree_init;
    jmethodID Triple_init;
} MethodCache;

typedef struct {
    jclass ArrayList;
    jclass Boolean;
    jclass CharSequence;
    jclass Function1;
    jclass Function2;
    jclass IllegalArgumentException;
    jclass IllegalStateException;
    jclass IndexOutOfBoundsException;
    jclass InputEdit;
    jclass InputEncoding;
    jclass Language;
    jclass Language$Metadata;
    jclass List;
    jclass LookaheadIterator;
    jclass Node;
    jclass Pair;
    jclass Parser$LogType;
    jclass Parser;
    jclass Point;
    jclass Query;
    jclass QueryCapture;
    jclass QueryCursor;
    jclass QueryError$Capture;
    jclass QueryError$Field;
    jclass QueryError$NodeType;
    jclass QueryError$Structure;
    jclass QueryError$Syntax;
    jclass QueryMatch;
    jclass Range;
    jclass Tree;
    jclass TreeCursor;
    jclass Triple;
    jclass UInt;
    jclass UShort;
} ClassCache;

extern FieldCache global_field_cache;
extern MethodCache global_method_cache;
extern ClassCache global_class_cache;
extern JavaVM *java_vm;

static inline jobject marshal_node(JNIEnv *env, TSNode ts_node, const jobject tree) {
    jintArray context = (*env)->NewIntArray(env, 4);
    (*env)->SetIntArrayRegion(env, context, 0, 4, (jint *)ts_node.context);
    return NEW_OBJECT(Node, (jlong)ts_node.id, context, tree);
}

static inline TSNode unmarshal_node(JNIEnv *env, const jobject node) {
    jobject tree = GET_FIELD(Object, node, Node_tree);
    jintArray context = (jintArray)GET_FIELD(Object, node, Node_context);
    jint *elements = (*env)->GetIntArrayElements(env, context, NULL);
    TSNode ts_node;
    ts_node.id = GET_POINTER(void, node, Node_id);
    ts_node.tree = GET_POINTER(TSTree, tree, Tree_self);
    ts_node.context[0] = elements[0];
    ts_node.context[1] = elements[1];
    ts_node.context[2] = elements[2];
    ts_node.context[3] = elements[3];
    (*env)->ReleaseIntArrayElements(env, context, elements, 0);
    return ts_node;
}

static inline jobject marshal_point(JNIEnv *env, TSPoint ts_point) {
    return NEW_OBJECT(Point, (jint)ts_point.row, (jint)ts_point.column);
}

static inline TSPoint unmarshal_point(JNIEnv *env, const jobject point) {
    uint32_t row = (uint32_t)GET_FIELD(Int, point, Point_row),
             column = (uint32_t)GET_FIELD(Int, point, Point_column);
    return (TSPoint){row, column};
}

static inline TSRange unmarshal_range(JNIEnv *env, const jobject range) {
    TSRange ts_range;
    ts_range.start_byte = (uint32_t)GET_FIELD(Int, range, Range_startByte);
    ts_range.end_byte = (uint32_t)GET_FIELD(Int, range, Range_endByte);
    ts_range.start_point = unmarshal_point(env, GET_FIELD(Object, range, Range_startPoint));
    ts_range.end_point = unmarshal_point(env, GET_FIELD(Object, range, Range_endPoint));
    return ts_range;
}

static inline jobject marshal_range(JNIEnv *env, const TSRange *ts_range) {
    jint start_byte = (jint)ts_range->start_byte, end_byte = (jint)ts_range->end_byte;
    jobject start_point = marshal_point(env, ts_range->start_point),
            end_point = marshal_point(env, ts_range->end_point);
    return NEW_OBJECT(Range, start_point, end_point, start_byte, end_byte);
}

static inline TSInputEdit unmarshal_input_edit(JNIEnv *env, const jobject edit) {
    TSInputEdit ts_edit;
    jobject startPoint = GET_FIELD(Object, edit, InputEdit_startPoint),
            oldEndPoint = GET_FIELD(Object, edit, InputEdit_oldEndPoint),
            newEndPoint = GET_FIELD(Object, edit, InputEdit_newEndPoint);
    ts_edit.start_byte = (uint32_t)GET_FIELD(Int, edit, InputEdit_startByte);
    ts_edit.old_end_byte = (uint32_t)GET_FIELD(Int, edit, InputEdit_oldEndByte);
    ts_edit.new_end_byte = (uint32_t)GET_FIELD(Int, edit, InputEdit_newEndByte);
    ts_edit.start_point = unmarshal_point(env, startPoint);
    ts_edit.old_end_point = unmarshal_point(env, oldEndPoint);
    ts_edit.new_end_point = unmarshal_point(env, newEndPoint);
    return ts_edit;
}
