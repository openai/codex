#include "utils.h"

jlong JNICALL lookahead_iterator_init CRITICAL_ARGS(jlong language, jshort state) {
    TSLookaheadIterator *lookahead =
        ts_lookahead_iterator_new((TSLanguage *)language, (uint16_t)state);
    return (jlong)lookahead;
}

void JNICALL lookahead_iterator_delete CRITICAL_ARGS(jlong self) {
    ts_lookahead_iterator_delete((TSLookaheadIterator *)self);
}

jobject JNICALL lookahead_iterator_get_language(JNIEnv *env, jobject this) {
    TSLookaheadIterator *self = GET_POINTER(TSLookaheadIterator, this, LookaheadIterator_self);
    const TSLanguage *language = ts_lookahead_iterator_language(self);
    jobject language_obj = (*env)->AllocObject(env, global_class_cache.Language);
    (*env)->SetLongField(env, language_obj, global_field_cache.Language_self, (jlong)language);
    return language_obj;
};

jshort JNICALL lookahead_iterator_get_current_symbol(JNIEnv *env, jobject this) {
    TSLookaheadIterator *self = GET_POINTER(TSLookaheadIterator, this, LookaheadIterator_self);
    return (jshort)ts_lookahead_iterator_current_symbol(self);
}

jstring JNICALL lookahead_iterator_get_current_symbol_name(JNIEnv *env, jobject this) {
    TSLookaheadIterator *self = GET_POINTER(TSLookaheadIterator, this, LookaheadIterator_self);
    const char *name = ts_lookahead_iterator_current_symbol_name(self);
    return (*env)->NewStringUTF(env, name);
}

jboolean JNICALL lookahead_iterator_reset(JNIEnv *env, jobject this, jshort state,
                                          jobject language) {
    TSLookaheadIterator *self = GET_POINTER(TSLookaheadIterator, this, LookaheadIterator_self);
    if (language == NULL) {
        return (jboolean)ts_lookahead_iterator_reset_state(self, (uint16_t)state);
    }
    TSLanguage *language_ptr = GET_POINTER(TSLanguage, language, Language_self);
    return (jboolean)ts_lookahead_iterator_reset(self, language_ptr, (uint16_t)state);
}

jboolean JNICALL lookahead_iterator_native_next(JNIEnv *env, jobject this) {
    TSLookaheadIterator *self = GET_POINTER(TSLookaheadIterator, this, LookaheadIterator_self);
    return (jboolean)ts_lookahead_iterator_next(self);
}

const JNINativeMethod LookaheadIterator_methods[] = {
    {"init", "(JS)J", (void *)&lookahead_iterator_init},
    {"delete", "(J)V", (void *)&lookahead_iterator_delete},
    {"getLanguage", "()L" PACKAGE "Language;", (void *)&lookahead_iterator_get_language},
    {"getCurrentSymbol", "()S", (void *)&lookahead_iterator_get_current_symbol},
    {"getCurrentSymbolName", "()Ljava/lang/String;",
     (void *)&lookahead_iterator_get_current_symbol_name},
    {"reset", "(SL" PACKAGE "Language;)Z", (void *)&lookahead_iterator_reset},
    {"nativeNext", "()Z", (void *)&lookahead_iterator_native_next},
};

const size_t LookaheadIterator_methods_size =
    sizeof LookaheadIterator_methods / sizeof(JNINativeMethod);
