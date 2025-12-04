#include "utils.h"

jlong JNICALL tree_copy CRITICAL_ARGS(jlong self) { return (jlong)ts_tree_copy((TSTree *)self); }

void JNICALL tree_delete CRITICAL_ARGS(jlong self) { ts_tree_delete((TSTree *)self); }

jobject JNICALL tree_get_root_node(JNIEnv *env, jobject this) {
    TSTree *self = GET_POINTER(TSTree, this, Tree_self);
    return marshal_node(env, ts_tree_root_node(self), this);
}

jobject JNICALL tree_root_node_with_offset(JNIEnv *env, jobject this, jint bytes, jobject extent) {
    TSTree *self = GET_POINTER(TSTree, this, Tree_self);
    TSPoint ts_point = unmarshal_point(env, extent);
    TSNode ts_node = ts_tree_root_node_with_offset(self, (uint32_t)bytes, ts_point);
    return marshal_node(env, ts_node, this);
}

void JNICALL tree_edit(JNIEnv *env, jobject this, jobject edit) {
    TSTree *self = GET_POINTER(TSTree, this, Tree_self);
    TSInputEdit input_edit = unmarshal_input_edit(env, edit);
    ts_tree_edit(self, &input_edit);
    (*env)->SetObjectField(env, this, global_field_cache.Tree_source, NULL);
}

jobject JNICALL tree_changed_ranges(JNIEnv *env, jobject this, jobject new_tree) {
    TSTree *self = GET_POINTER(TSTree, this, Tree_self);
    TSTree *other = GET_POINTER(TSTree, new_tree, Tree_self);
    uint32_t length;
    TSRange *ts_ranges = ts_tree_get_changed_ranges(self, other, &length);
    if (length == 0 || ts_ranges == NULL)
        return NEW_OBJECT(ArrayList, 0);

    TSRange *ts_range = ts_ranges;
    jobject ranges = NEW_OBJECT(ArrayList, (jint)length);
    for (uint32_t i = 0; i < length; ++i) {
        jobject range_obj = marshal_range(env, ts_range++);
        CALL_METHOD(Boolean, ranges, ArrayList_add, range_obj);
        (*env)->DeleteLocalRef(env, range_obj);
    }
    free(ts_ranges);
    return ranges;
}

jobject JNICALL tree_native_included_ranges(JNIEnv *env, jobject this) {
    TSTree *self = GET_POINTER(TSTree, this, Tree_self);
    uint32_t length;
    TSRange *ts_ranges = ts_tree_included_ranges(self, &length);
    if (length == 0 || ts_ranges == NULL)
        return NEW_OBJECT(ArrayList, 0);

    jobject ranges = NEW_OBJECT(ArrayList, (jint)length);
    for (uint32_t i = 0; i < length; ++i) {
        jobject range_obj = marshal_range(env, ts_ranges + i);
        CALL_METHOD(Boolean, ranges, ArrayList_add, range_obj);
        (*env)->DeleteLocalRef(env, range_obj);
    }
    free(ts_ranges);
    return ranges;
}

const JNINativeMethod Tree_methods[] = {
    {"copy", "(J)J", (void *)&tree_copy},
    {"delete", "(J)V", (void *)&tree_delete},
    {"getRootNode", "()L" PACKAGE "Node;", (void *)&tree_get_root_node},
    {"rootNodeWithOffset", "(IL" PACKAGE "Point;)L" PACKAGE "Node;",
     (void *)&tree_root_node_with_offset},
    {"edit", "(L" PACKAGE "InputEdit;)V", (void *)&tree_edit},
    {"changedRanges", "(L" PACKAGE "Tree;)Ljava/util/List;", (void *)&tree_changed_ranges},
    {"nativeIncludedRanges", "()Ljava/util/List;", (void *)&tree_native_included_ranges},
};

const size_t Tree_methods_size = sizeof Tree_methods / sizeof(JNINativeMethod);
