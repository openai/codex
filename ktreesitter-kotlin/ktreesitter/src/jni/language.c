#include "utils.h"

jlong JNICALL language_copy CRITICAL_ARGS(jlong self) {
    return (jlong)ts_language_copy((TSLanguage *)self);
}

jint JNICALL language_get_abi_version(JNIEnv *env, jobject this) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    return (jint)ts_language_abi_version(self);
}

jint JNICALL language_get_version(JNIEnv *env, jobject this) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    return (jint)ts_language_abi_version(self);
}

jint JNICALL language_get_symbol_count(JNIEnv *env, jobject this) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    return (jint)ts_language_symbol_count(self);
}

jint JNICALL language_get_state_count(JNIEnv *env, jobject this) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    return (jint)ts_language_state_count(self);
}

jint JNICALL language_get_field_count(JNIEnv *env, jobject this) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    return (jint)ts_language_field_count(self);
}

jstring JNICALL language_get_name(JNIEnv *env, jobject this) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    const char *name = ts_language_name(self);
    return name ? (*env)->NewStringUTF(env, name) : NULL;
}

jobject JNICALL language_get_metadata(JNIEnv *env, jobject this) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    const TSLanguageMetadata *metadata = ts_language_metadata(self);
    if (metadata == NULL)
        return NULL;

    jobject major = (*env)->AllocObject(env, global_class_cache.UShort);
    (*env)->SetShortField(env, major, global_field_cache.UShort_data,
                          (jshort)metadata->major_version);
    jobject minor = (*env)->AllocObject(env, global_class_cache.UShort);
    (*env)->SetShortField(env, minor, global_field_cache.UShort_data,
                          (jshort)metadata->minor_version);
    jobject patch = (*env)->AllocObject(env, global_class_cache.UShort);
    (*env)->SetShortField(env, patch, global_field_cache.UShort_data,
                          (jshort)metadata->patch_version);
    jobject version = NEW_OBJECT(Triple, major, minor, patch);
    return NEW_OBJECT(Language$Metadata, version);
}

jshortArray JNICALL language_get_supertypes(JNIEnv *env, jobject this) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    uint32_t length;
    const TSSymbol *supertypes = ts_language_supertypes(self, &length);
    jshortArray result = (*env)->NewShortArray(env, length);
    if (length > 0) {
        (*env)->SetShortArrayRegion(env, result, 0, length, (const jshort *)supertypes);
    }
    return result;
}

jstring JNICALL language_symbol_name(JNIEnv *env, jobject this, jshort symbol) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    const char *name = ts_language_symbol_name(self, (uint16_t)symbol);
    return name ? (*env)->NewStringUTF(env, name) : NULL;
}

jshort JNICALL language_symbol_for_name(JNIEnv *env, jobject this, jstring name,
                                        jboolean is_named) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    const char *symbol_name = (*env)->GetStringUTFChars(env, name, NULL);
    uint32_t length = (uint32_t)(*env)->GetStringUTFLength(env, name);
    uint16_t symbol = ts_language_symbol_for_name(self, symbol_name, length, (bool)is_named);
    (*env)->ReleaseStringUTFChars(env, name, symbol_name);
    return (jshort)symbol;
}

jshortArray JNICALL language_subtypes(JNIEnv *env, jobject this, jshort supertype) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    uint32_t length;
    const TSSymbol *subtypes = ts_language_subtypes(self, supertype, &length);
    jshortArray result = (*env)->NewShortArray(env, length);
    if (length > 0) {
        (*env)->SetShortArrayRegion(env, result, 0, length, (const jshort *)subtypes);
    }
    return result;
}

jboolean JNICALL language_is_named(JNIEnv *env, jobject this, jshort symbol) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    TSSymbolType symbol_type = ts_language_symbol_type(self, symbol);
    return (jboolean)(symbol_type == TSSymbolTypeRegular);
}

jboolean JNICALL language_is_visible(JNIEnv *env, jobject this, jshort symbol) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    TSSymbolType symbol_type = ts_language_symbol_type(self, symbol);
    return (jboolean)(symbol_type <= TSSymbolTypeAnonymous);
}

jboolean JNICALL language_is_supertype(JNIEnv *env, jobject this, jshort symbol) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    TSSymbolType symbol_type = ts_language_symbol_type(self, symbol);
    return (jboolean)(symbol_type == TSSymbolTypeSupertype);
}

jstring JNICALL language_field_name_for_id(JNIEnv *env, jobject this, jshort id) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    const char *name = ts_language_field_name_for_id(self, (uint16_t)id);
    return name ? (*env)->NewStringUTF(env, name) : NULL;
}

jint JNICALL language_field_id_for_name(JNIEnv *env, jobject this, jstring name) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    const char *field_name = (*env)->GetStringUTFChars(env, name, NULL);
    uint32_t length = (uint32_t)(*env)->GetStringUTFLength(env, name);
    uint16_t field_id = ts_language_field_id_for_name(self, field_name, length);
    (*env)->ReleaseStringUTFChars(env, name, field_name);
    return (jint)field_id;
}

jshort JNICALL language_next_state(JNIEnv *env, jobject this, jshort state, jshort symbol) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    return (jshort)ts_language_next_state(self, (uint16_t)state, (uint16_t)symbol);
}

void JNICALL language_check_version(JNIEnv *env, jobject this) {
    TSLanguage *self = GET_POINTER(TSLanguage, this, Language_self);
    uint32_t version = ts_language_version(self);
    if (version < TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION ||
        version > TREE_SITTER_LANGUAGE_VERSION) {
        const char *fmt = "Incompatible language version %u. Must be between %u and %u.";
        char buffer[70] = {0}; // length(fmt) + digits(UINT32_MAX)
        sprintf_s(buffer, 70, fmt, version, TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION,
                  TREE_SITTER_LANGUAGE_VERSION);
        THROW(IllegalArgumentException, (const char *)buffer);
    }
}

const JNINativeMethod Language_methods[] = {
    {"copy", "(J)J", (void *)&language_copy},
    {"getAbiVersion", "()I", (void *)&language_get_abi_version},
    {"getVersion", "()I", (void *)&language_get_version},
    {"getSymbolCount", "()I", (void *)&language_get_symbol_count},
    {"getStateCount", "()I", (void *)&language_get_state_count},
    {"getFieldCount", "()I", (void *)&language_get_field_count},
    {"getName", "()Ljava/lang/String;", (void *)&language_get_name},
    {"getMetadata", "()L" PACKAGE "Language$Metadata;", (void *)&language_get_metadata},
    {"getSupertypes", "()[S", (void *)&language_get_supertypes},
    {"symbolName", "(S)Ljava/lang/String;", (void *)&language_symbol_name},
    {"symbolForName", "(Ljava/lang/String;Z)S", (void *)&language_symbol_for_name},
    {"subtypes", "(S)[S", (void *)&language_subtypes},
    {"isNamed", "(S)Z", (void *)&language_is_named},
    {"isVisible", "(S)Z", (void *)&language_is_visible},
    {"isSupertype", "(S)Z", (void *)&language_is_supertype},
    {"fieldNameForId", "(S)Ljava/lang/String;", (void *)&language_field_name_for_id},
    {"fieldIdForName", "(Ljava/lang/String;)S", (void *)&language_field_id_for_name},
    {"nextState", "(SS)S", (void *)&language_next_state},
    {"checkVersion", "()V", (void *)&language_check_version},
};

const size_t Language_methods_size = sizeof Language_methods / sizeof(JNINativeMethod);
