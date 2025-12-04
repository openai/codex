#include "utils.h"

#define SET_INTERNAL_NODE(value)                                                                   \
    (*env)->SetObjectField(env, this, global_field_cache.TreeCursor_internalNode, value);

static inline TSTreeCursor *tree_cursor_alloc(TSTreeCursor cursor) {
    TSTreeCursor *cursor_ptr = (TSTreeCursor *)malloc(sizeof(TSTreeCursor));
    cursor_ptr->id = cursor.id;
    cursor_ptr->tree = cursor.tree;
    cursor_ptr->context[0] = cursor.context[0];
    cursor_ptr->context[1] = cursor.context[1];
    cursor_ptr->context[2] = cursor.context[2];
    return cursor_ptr;
}

jlong JNICALL tree_cursor_init(JNIEnv *env, jclass _class, jobject node) {
    TSNode ts_node = unmarshal_node(env, node);
    TSTreeCursor cursor = ts_tree_cursor_new(ts_node);
    return (jlong)tree_cursor_alloc(cursor);
}

jlong JNICALL tree_cursor_copy CRITICAL_ARGS(jlong self) {
    TSTreeCursor copy = ts_tree_cursor_copy((TSTreeCursor *)self);
    return (jlong)tree_cursor_alloc(copy);
}

void JNICALL tree_cursor_delete CRITICAL_ARGS(jlong self) {
    ts_tree_cursor_delete((TSTreeCursor *)self);
    free((TSTreeCursor *)self);
}

jobject JNICALL tree_cursor_get_current_node(JNIEnv *env, jobject this) {
    jobject node = GET_FIELD(Object, this, TreeCursor_internalNode);
    if (node != NULL)
        return node;

    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    jobject tree = GET_FIELD(Object, this, TreeCursor_tree);
    TSNode ts_node = ts_tree_cursor_current_node(self);
    node = marshal_node(env, ts_node, tree);
    SET_INTERNAL_NODE(node);
    return node;
}

jint JNICALL tree_cursor_get_current_depth(JNIEnv *env, jobject this) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    return (jint)ts_tree_cursor_current_depth(self);
}

jshort JNICALL tree_cursor_get_current_field_id(JNIEnv *env, jobject this) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    return (short)ts_tree_cursor_current_field_id(self);
}

jstring JNICALL tree_cursor_get_current_field_name(JNIEnv *env, jobject this) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    const char *name = ts_tree_cursor_current_field_name(self);
    return name ? (*env)->NewStringUTF(env, name) : NULL;
}

jint JNICALL tree_cursor_get_current_descendant_index(JNIEnv *env, jobject this) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    return (jint)ts_tree_cursor_current_descendant_index(self);
}

void JNICALL tree_cursor_reset__node(JNIEnv *env, jobject this, jobject node) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    TSNode ts_node = unmarshal_node(env, node);
    ts_tree_cursor_reset(self, ts_node);
    SET_INTERNAL_NODE(NULL);
}

void JNICALL tree_cursor_reset__cursor(JNIEnv *env, jobject this, jobject cursor) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    TSTreeCursor *other = GET_POINTER(TSTreeCursor, cursor, TreeCursor_self);
    ts_tree_cursor_reset_to(self, other);
    SET_INTERNAL_NODE(NULL);
}

jboolean JNICALL tree_cursor_goto_first_child(JNIEnv *env, jobject this) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    bool result = ts_tree_cursor_goto_first_child(self);
    if (result)
        SET_INTERNAL_NODE(NULL);
    return (jboolean)result;
}

jboolean JNICALL tree_cursor_goto_last_child(JNIEnv *env, jobject this) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    bool result = ts_tree_cursor_goto_last_child(self);
    if (result)
        SET_INTERNAL_NODE(NULL);
    return (jboolean)result;
}

jboolean JNICALL tree_cursor_goto_parent(JNIEnv *env, jobject this) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    bool result = ts_tree_cursor_goto_parent(self);
    if (result)
        SET_INTERNAL_NODE(NULL);
    return (jboolean)result;
}

jboolean JNICALL tree_cursor_goto_next_sibling(JNIEnv *env, jobject this) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    bool result = ts_tree_cursor_goto_next_sibling(self);
    if (result)
        SET_INTERNAL_NODE(NULL);
    return (jboolean)result;
}

jboolean JNICALL tree_cursor_goto_previous_sibling(JNIEnv *env, jobject this) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    bool result = ts_tree_cursor_goto_previous_sibling(self);
    if (result)
        SET_INTERNAL_NODE(NULL);
    return (jboolean)result;
}

void JNICALL tree_cursor_goto_descendant(JNIEnv *env, jobject this, jint index) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    ts_tree_cursor_goto_descendant(self, (uint32_t)index);
    SET_INTERNAL_NODE(NULL);
}

jlong JNICALL tree_cursor_native_goto_first_child_for_byte(JNIEnv *env, jobject this, jint byte) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    return (jlong)ts_tree_cursor_goto_first_child_for_byte(self, (uint32_t)byte);
}

jlong JNICALL tree_cursor_native_goto_first_child_for_point(JNIEnv *env, jobject this,
                                                            jobject point) {
    TSTreeCursor *self = GET_POINTER(TSTreeCursor, this, TreeCursor_self);
    TSPoint ts_point = unmarshal_point(env, point);
    return (jlong)ts_tree_cursor_goto_first_child_for_point(self, ts_point);
}

const JNINativeMethod TreeCursor_methods[] = {
    {"init", "(L" PACKAGE "Node;)J", (void *)&tree_cursor_init},
    {"copy", "(J)J", (void *)&tree_cursor_copy},
    {"delete", "(J)V", (void *)&tree_cursor_delete},
    {"getCurrentNode", "()L" PACKAGE "Node;", (void *)&tree_cursor_get_current_node},
    {"getCurrentDepth", "()I", (void *)&tree_cursor_get_current_depth},
    {"getCurrentFieldId", "()S", (void *)&tree_cursor_get_current_field_id},
    {"getCurrentFieldName", "()Ljava/lang/String;", (void *)&tree_cursor_get_current_field_name},
    {"getCurrentDescendantIndex", "()I", (void *)&tree_cursor_get_current_descendant_index},
    {"reset", "(L" PACKAGE "Node;)V", (void *)&tree_cursor_reset__node},
    {"reset", "(L" PACKAGE "TreeCursor;)V", (void *)&tree_cursor_reset__cursor},
    {"gotoFirstChild", "()Z", (void *)&tree_cursor_goto_first_child},
    {"gotoLastChild", "()Z", (void *)&tree_cursor_goto_last_child},
    {"gotoParent", "()Z", (void *)&tree_cursor_goto_parent},
    {"gotoNextSibling", "()Z", (void *)&tree_cursor_goto_next_sibling},
    {"gotoPreviousSibling", "()Z", (void *)&tree_cursor_goto_previous_sibling},
    {"gotoDescendant", "(I)V", (void *)&tree_cursor_goto_descendant},
    {"nativeGotoFirstChildForByte", "(I)J", (void *)&tree_cursor_native_goto_first_child_for_byte},
    {"nativeGotoFirstChildForPoint", "(L" PACKAGE "Point;)J",
     (void *)&tree_cursor_native_goto_first_child_for_point},
};

const size_t TreeCursor_methods_size = sizeof TreeCursor_methods / sizeof(JNINativeMethod);
