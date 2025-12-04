#include "utils.h"

extern const JNINativeMethod Language_methods[];
extern const size_t Language_methods_size;

extern const JNINativeMethod LookaheadIterator_methods[];
extern const size_t LookaheadIterator_methods_size;

extern const JNINativeMethod Node_methods[];
extern const size_t Node_methods_size;

extern const JNINativeMethod Tree_methods[];
extern const size_t Tree_methods_size;

extern const JNINativeMethod TreeCursor_methods[];
extern const size_t TreeCursor_methods_size;

extern const JNINativeMethod Parser_methods[];
extern const size_t Parser_methods_size;

extern const JNINativeMethod Query_methods[];
extern const size_t Query_methods_size;

extern const JNINativeMethod QueryCursor_methods[];
extern const size_t QueryCursor_methods_size;

FieldCache global_field_cache = {0};
MethodCache global_method_cache = {0};
ClassCache global_class_cache = {0};
JavaVM *java_vm = NULL;

#define REGISTER_CLASS(name)                                                                       \
    do {                                                                                           \
        jclass _cat2(name, class) = (*env)->FindClass(env, PACKAGE #name);                         \
        if (_cat2(name, class) == NULL)                                                            \
            return JNI_ERR;                                                                        \
                                                                                                   \
        rc = (*env)->RegisterNatives(env, _cat2(name, class), _cat2(name, methods),                \
                                     (jint)_cat2(name, methods_size));                             \
        if (rc != JNI_OK)                                                                          \
            return rc;                                                                             \
                                                                                                   \
        global_class_cache.name = (*env)->NewGlobalRef(env, (jobject)_cat2(name, class));          \
    } while (0)

#define CACHE_CLASS(package, name)                                                                 \
    do {                                                                                           \
        jclass _cat2(name, class) = (*env)->FindClass(env, package #name);                         \
        if (_cat2(name, class) == NULL)                                                            \
            return JNI_ERR;                                                                        \
                                                                                                   \
        global_class_cache.name = (*env)->NewGlobalRef(env, (jobject)_cat2(name, class));          \
    } while (0)

#define CACHE_FIELD(class, field, type)                                                            \
    global_field_cache._cat2(class, field) =                                                       \
        (*env)->GetFieldID(env, global_class_cache.class, #field, type)

#define CACHE_STATIC_FIELD(class, field, type)                                                     \
    global_field_cache._cat2(class, field) =                                                       \
        (*env)->GetStaticFieldID(env, global_class_cache.class, #field, type)

#define CACHE_METHOD(class, method, name, type)                                                    \
    global_method_cache._cat2(class, method) =                                                     \
        (*env)->GetMethodID(env, global_class_cache.class, name, type)

#define CACHE_STATIC_METHOD(class, method, name, type)                                             \
    global_method_cache._cat2(class, method) =                                                     \
        (*env)->GetStaticMethodID(env, global_class_cache.class, name, type)

JNIEXPORT jint JNI_OnLoad(JavaVM *vm, void *_reserved) {
    java_vm = vm;

    JNIEnv *env;
    if ((*vm)->GetEnv(vm, (void **)&env, JNI_VERSION_1_6) != JNI_OK)
        return JNI_ERR;

    int rc;

    REGISTER_CLASS(Language);
    CACHE_FIELD(Language, self, "J");
    CACHE_METHOD(Language, init, "<init>", "(Ljava/lang/Object;)V");

    REGISTER_CLASS(LookaheadIterator);
    CACHE_FIELD(LookaheadIterator, self, "J");

    REGISTER_CLASS(Node);
    CACHE_FIELD(Node, id, "J");
    CACHE_FIELD(Node, context, "[I");
    CACHE_FIELD(Node, tree, "L" PACKAGE "Tree;");
    CACHE_FIELD(Node, internalChildren, "Ljava/util/List;");
    CACHE_METHOD(Node, init, "<init>", "(J[IL" PACKAGE "Tree;)V");

    REGISTER_CLASS(Tree);
    CACHE_FIELD(Tree, self, "J");
    CACHE_FIELD(Tree, source, "Ljava/lang/String;");
    CACHE_METHOD(Tree, init, "<init>", "(JLjava/lang/String;L" PACKAGE "Language;)V");

    REGISTER_CLASS(TreeCursor);
    CACHE_FIELD(TreeCursor, self, "J");
    CACHE_FIELD(TreeCursor, tree, "L" PACKAGE "Tree;");
    CACHE_FIELD(TreeCursor, internalNode, "L" PACKAGE "Node;");

    REGISTER_CLASS(Query);
    CACHE_FIELD(Query, self, "J");
    CACHE_FIELD(Query, language, "L" PACKAGE "Language;");
    CACHE_FIELD(Query, source, "Ljava/lang/String;");

    REGISTER_CLASS(QueryCursor);
    CACHE_FIELD(QueryCursor, self, "J");
    CACHE_FIELD(QueryCursor, matchLimit, "I");
    CACHE_FIELD(QueryCursor, maxStartDepth, "I");
    CACHE_FIELD(QueryCursor, timeoutMicros, "J");

    REGISTER_CLASS(Parser);
    CACHE_FIELD(Parser, self, "J");
    CACHE_FIELD(Parser, timeoutMicros, "J");
    CACHE_FIELD(Parser, language, "L" PACKAGE "Language;");
    CACHE_FIELD(Parser, includedRanges, "Ljava/util/List;");
    CACHE_FIELD(Parser, logger, "Lkotlin/jvm/functions/Function2;");

    CACHE_CLASS(PACKAGE, Point);
    CACHE_FIELD(Point, row, "I");
    CACHE_FIELD(Point, column, "I");
    CACHE_METHOD(Point, init, "<init>", "(II)V");

    CACHE_CLASS(PACKAGE, Range);
    CACHE_FIELD(Range, startByte, "I");
    CACHE_FIELD(Range, endByte, "I");
    CACHE_FIELD(Range, startPoint, "L" PACKAGE "Point;");
    CACHE_FIELD(Range, endPoint, "L" PACKAGE "Point;");
    CACHE_METHOD(Range, init, "<init>", "(L" PACKAGE "Point;L" PACKAGE "Point;II)V");

    CACHE_CLASS(PACKAGE, InputEdit);
    CACHE_FIELD(InputEdit, startByte, "I");
    CACHE_FIELD(InputEdit, oldEndByte, "I");
    CACHE_FIELD(InputEdit, newEndByte, "I");
    CACHE_FIELD(InputEdit, startPoint, "L" PACKAGE "Point;");
    CACHE_FIELD(InputEdit, oldEndPoint, "L" PACKAGE "Point;");
    CACHE_FIELD(InputEdit, newEndPoint, "L" PACKAGE "Point;");

    CACHE_CLASS(PACKAGE, Language$Metadata);
    CACHE_METHOD(Language$Metadata, init, "<init>", "(Lkotlin/Triple;)V");

    CACHE_CLASS(PACKAGE, QueryCapture);
    CACHE_METHOD(QueryCapture, init, "<init>", "(L" PACKAGE "Node;Ljava/lang/String;)V");

    CACHE_CLASS(PACKAGE, QueryMatch);
    CACHE_METHOD(QueryMatch, init, "<init>", "(ILjava/util/List;)V");

    CACHE_CLASS(PACKAGE, Parser$LogType);
    CACHE_STATIC_FIELD(Parser$LogType, LEX, "L" PACKAGE "Parser$LogType;");
    CACHE_STATIC_FIELD(Parser$LogType, PARSE, "L" PACKAGE "Parser$LogType;");

    CACHE_CLASS(PACKAGE, InputEncoding);
    CACHE_STATIC_FIELD(InputEncoding, UTF_8, "L" PACKAGE "InputEncoding;");
    CACHE_STATIC_FIELD(InputEncoding, UTF_16LE, "L" PACKAGE "InputEncoding;");
    CACHE_STATIC_FIELD(InputEncoding, UTF_16BE, "L" PACKAGE "InputEncoding;");

    CACHE_CLASS(PACKAGE, QueryError$Capture);
    CACHE_METHOD(QueryError$Capture, init, "<init>", "(IILjava/lang/String;)V");

    CACHE_CLASS(PACKAGE, QueryError$Field);
    CACHE_METHOD(QueryError$Field, init, "<init>", "(IILjava/lang/String;)V");

    CACHE_CLASS(PACKAGE, QueryError$NodeType);
    CACHE_METHOD(QueryError$NodeType, init, "<init>", "(IILjava/lang/String;)V");

    CACHE_CLASS(PACKAGE, QueryError$Syntax);
    CACHE_METHOD(QueryError$Syntax, init, "<init>", "(JJ)V");

    CACHE_CLASS(PACKAGE, QueryError$Structure);
    CACHE_METHOD(QueryError$Structure, init, "<init>", "(II)V");

    CACHE_CLASS("java/lang/", Boolean);
    CACHE_FIELD(Boolean, value, "Z");
    CACHE_METHOD(Boolean, init, "<init>", "(Z)V");

    CACHE_CLASS("java/lang/", CharSequence);
    CACHE_METHOD(CharSequence, toString, "toString", "()Ljava/lang/String;");

    CACHE_CLASS("java/util/", ArrayList);
    CACHE_METHOD(ArrayList, init, "<init>", "(I)V");
    CACHE_METHOD(ArrayList, add, "add", "(Ljava/lang/Object;)Z");

    CACHE_CLASS("java/util/", List);
    CACHE_METHOD(List, size, "size", "()I");
    CACHE_METHOD(List, get, "get", "(I)Ljava/lang/Object;");

    CACHE_CLASS("kotlin/", Pair);
    CACHE_METHOD(Pair, init, "<init>", "(Ljava/lang/Object;Ljava/lang/Object;)V");

    CACHE_CLASS("kotlin/", Triple);
    CACHE_METHOD(Triple, init, "<init>",
                 "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V");

    CACHE_CLASS("kotlin/", UInt);
    CACHE_FIELD(UInt, data, "I");

    CACHE_CLASS("kotlin/", UShort);
    CACHE_FIELD(UShort, data, "S");

    CACHE_CLASS("kotlin/jvm/functions/", Function1);
    CACHE_METHOD(Function1, invoke, "invoke",
                 "(Ljava/lang/Object;)Ljava/lang/Object;");

    CACHE_CLASS("kotlin/jvm/functions/", Function2);
    CACHE_METHOD(Function2, invoke, "invoke",
                 "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;");

    CACHE_CLASS("java/lang/", IllegalArgumentException);
    CACHE_CLASS("java/lang/", IllegalStateException);
    CACHE_CLASS("java/lang/", IndexOutOfBoundsException);

#ifdef __ANDROID__
    ts_set_allocator(malloc, calloc, realloc, free);
#endif

    return JNI_VERSION_1_6;
}

JNIEXPORT void JNI_OnUnload(JavaVM *vm, void *_reserved) {
    JNIEnv *env;
    if ((*vm)->GetEnv(vm, (void **)&env, JNI_VERSION_1_6) != JNI_OK)
        return;

    (*env)->DeleteGlobalRef(env, global_class_cache.ArrayList);
    (*env)->DeleteGlobalRef(env, global_class_cache.Boolean);
    (*env)->DeleteGlobalRef(env, global_class_cache.CharSequence);
    (*env)->DeleteGlobalRef(env, global_class_cache.Function1);
    (*env)->DeleteGlobalRef(env, global_class_cache.Function2);
    (*env)->DeleteGlobalRef(env, global_class_cache.IllegalArgumentException);
    (*env)->DeleteGlobalRef(env, global_class_cache.IllegalStateException);
    (*env)->DeleteGlobalRef(env, global_class_cache.IndexOutOfBoundsException);
    (*env)->DeleteGlobalRef(env, global_class_cache.InputEdit);
    (*env)->DeleteGlobalRef(env, global_class_cache.InputEncoding);
    (*env)->DeleteGlobalRef(env, global_class_cache.Language);
    (*env)->DeleteGlobalRef(env, global_class_cache.Language$Metadata);
    (*env)->DeleteGlobalRef(env, global_class_cache.List);
    (*env)->DeleteGlobalRef(env, global_class_cache.LookaheadIterator);
    (*env)->DeleteGlobalRef(env, global_class_cache.Node);
    (*env)->DeleteGlobalRef(env, global_class_cache.Pair);
    (*env)->DeleteGlobalRef(env, global_class_cache.Parser);
    (*env)->DeleteGlobalRef(env, global_class_cache.Point);
    (*env)->DeleteGlobalRef(env, global_class_cache.Query);
    (*env)->DeleteGlobalRef(env, global_class_cache.QueryCursor);
    (*env)->DeleteGlobalRef(env, global_class_cache.QueryError$Capture);
    (*env)->DeleteGlobalRef(env, global_class_cache.QueryError$Field);
    (*env)->DeleteGlobalRef(env, global_class_cache.QueryError$NodeType);
    (*env)->DeleteGlobalRef(env, global_class_cache.QueryError$Structure);
    (*env)->DeleteGlobalRef(env, global_class_cache.QueryError$Syntax);
    (*env)->DeleteGlobalRef(env, global_class_cache.QueryCapture);
    (*env)->DeleteGlobalRef(env, global_class_cache.QueryMatch);
    (*env)->DeleteGlobalRef(env, global_class_cache.Range);
    (*env)->DeleteGlobalRef(env, global_class_cache.Tree);
    (*env)->DeleteGlobalRef(env, global_class_cache.TreeCursor);
    (*env)->DeleteGlobalRef(env, global_class_cache.Triple);
    (*env)->DeleteGlobalRef(env, global_class_cache.UInt);
    (*env)->DeleteGlobalRef(env, global_class_cache.UShort);
}
