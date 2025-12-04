#include "utils.h"

jshort JNICALL node_symbol(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jshort)ts_node_symbol(self);
}

jshort JNICALL node_grammar_symbol(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jshort)ts_node_grammar_symbol(self);
}

jstring JNICALL node_type(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    const char *type = ts_node_type(self);
    return (*env)->NewStringUTF(env, type);
}

jstring JNICALL node_grammar_type(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    const char *type = ts_node_grammar_type(self);
    return (*env)->NewStringUTF(env, type);
}

jboolean JNICALL node_is_named(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jboolean)ts_node_is_named(self);
}

jboolean JNICALL node_is_extra(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jboolean)ts_node_is_extra(self);
}

jboolean JNICALL node_is_error(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jboolean)ts_node_is_error(self);
}

jboolean JNICALL node_is_missing(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jboolean)ts_node_is_missing(self);
}

jboolean JNICALL node_has_error(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jboolean)ts_node_has_error(self);
}

jboolean JNICALL node_has_changes(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jboolean)ts_node_has_changes(self);
}

jshort JNICALL node_get_parse_state(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jshort)ts_node_parse_state(self);
}

jshort JNICALL node_get_next_parse_state(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jshort)ts_node_next_parse_state(self);
}

jint JNICALL node_get_start_byte(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jint)ts_node_start_byte(self);
}

jint JNICALL node_get_end_byte(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jint)ts_node_end_byte(self);
}

jobject JNICALL node_get_start_point(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return marshal_point(env, ts_node_start_point(self));
}

jobject JNICALL node_get_end_point(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return marshal_point(env, ts_node_end_point(self));
}

jint JNICALL node_get_child_count(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jint)ts_node_child_count(self);
}

jint JNICALL node_get_named_child_count(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jint)ts_node_named_child_count(self);
}

jint JNICALL node_get_descendant_count(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    return (jint)ts_node_descendant_count(self);
}

jobject JNICALL node_get_parent(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_parent(self);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_get_next_sibling(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_next_sibling(self);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_get_prev_sibling(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_prev_sibling(self);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_get_next_named_sibling(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_next_named_sibling(self);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_get_prev_named_sibling(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_prev_named_sibling(self);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_get_children(JNIEnv *env, jobject this) {
    jobject children = GET_FIELD(Object, this, Node_internalChildren);
    if (children != NULL)
        return children;

    TSNode self = unmarshal_node(env, this);
    uint32_t count = ts_node_child_count(self);
    children = NEW_OBJECT(ArrayList, (jint)count);
    if (count == 0)
        return children;

    jobject tree = GET_FIELD(Object, this, Node_tree);
    TSTreeCursor cursor = ts_tree_cursor_new(self);
    (void)ts_tree_cursor_goto_first_child(&cursor);
    for (uint32_t i = 0; i < count; ++i) {
        TSNode ts_node = ts_tree_cursor_current_node(&cursor);
        jobject node_obj = marshal_node(env, ts_node, tree);
        CALL_METHOD(Boolean, children, ArrayList_add, node_obj);
        (*env)->DeleteLocalRef(env, node_obj);
        ts_tree_cursor_goto_next_sibling(&cursor);
    }
    ts_tree_cursor_delete(&cursor);
    (*env)->SetObjectField(env, this, global_field_cache.Node_internalChildren, children);
    return children;
}

jobject JNICALL node_child(JNIEnv *env, jobject this, jint index) {
    TSNode self = unmarshal_node(env, this);
    if (ts_node_child_count(self) <= (uint32_t)index) {
        const char *fmt = "Child index %u is out of bounds";
        char buffer[40] = {0};
        sprintf_s(buffer, 40, fmt, (uint32_t)index);
        THROW(IndexOutOfBoundsException, (const char *)buffer);
        return NULL;
    }

    TSNode result = ts_node_child(self, (uint32_t)index);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_named_child(JNIEnv *env, jobject this, jint index) {
    TSNode self = unmarshal_node(env, this);
    if (ts_node_child_count(self) <= (uint32_t)index) {
        const char *fmt = "Child index %u is out of bounds";
        char buffer[40] = {0};
        sprintf_s(buffer, 40, fmt, (uint32_t)index);
        THROW(IndexOutOfBoundsException, (const char *)buffer);
        return NULL;
    }

    TSNode result = ts_node_named_child(self, (uint32_t)index);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_child_by_field_id(JNIEnv *env, jobject this, jshort id) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_child_by_field_id(self, (uint16_t)id);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_child_by_field_name(JNIEnv *env, jobject this, jstring name) {
    TSNode self = unmarshal_node(env, this);
    const char *field_name = (*env)->GetStringUTFChars(env, name, NULL);
    uint32_t length = (uint32_t)(*env)->GetStringUTFLength(env, name);
    TSNode result = ts_node_child_by_field_name(self, field_name, length);
    (*env)->ReleaseStringUTFChars(env, name, field_name);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_children_by_field_id(JNIEnv *env, jobject this, jshort id) {
    if (id == 0)
        return NEW_OBJECT(ArrayList, 0);

    TSNode self = unmarshal_node(env, this);
    uint32_t count = ts_node_child_count(self);
    jobject children = NEW_OBJECT(ArrayList, (jint)count);
    jobject tree = GET_FIELD(Object, this, Node_tree);
    TSTreeCursor cursor = ts_tree_cursor_new(self);
    bool ok = ts_tree_cursor_goto_first_child(&cursor);
    while (ok) {
        uint16_t field_id = ts_tree_cursor_current_field_id(&cursor);
        if (field_id == (uint16_t)id) {
            TSNode ts_node = ts_tree_cursor_current_node(&cursor);
            jobject node_obj = marshal_node(env, ts_node, tree);
            CALL_METHOD(Boolean, children, ArrayList_add, node_obj);
            (*env)->DeleteLocalRef(env, node_obj);
        }
        ok = ts_tree_cursor_goto_next_sibling(&cursor);
    }
    ts_tree_cursor_delete(&cursor);
    return children;
}

jobject JNICALL node_first_child_for_byte(JNIEnv *env, jobject this, jint byte) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_first_child_for_byte(self, byte);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_first_named_child_for_byte(JNIEnv *env, jobject this, jint byte) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_first_named_child_for_byte(self, byte);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jstring JNICALL node_field_name_for_child(JNIEnv *env, jobject this, jint index) {
    TSNode self = unmarshal_node(env, this);
    if (ts_node_child_count(self) <= (uint32_t)index) {
        const char *fmt = "Child index %u is out of bounds";
        char buffer[40] = {0};
        sprintf_s(buffer, 40, fmt, (uint32_t)index);
        THROW(IndexOutOfBoundsException, (const char *)buffer);
        return NULL;
    }

    const char *field_name = ts_node_field_name_for_child(self, (uint32_t)index);
    return field_name ? (*env)->NewStringUTF(env, field_name) : NULL;
}

jstring JNICALL node_field_name_for_named_child(JNIEnv *env, jobject this, jint index) {
    TSNode self = unmarshal_node(env, this);
    if (ts_node_child_count(self) <= (uint32_t)index) {
        const char *fmt = "Child index %u is out of bounds";
        char buffer[40] = {0};
        sprintf_s(buffer, 40, fmt, (uint32_t)index);
        THROW(IndexOutOfBoundsException, (const char *)buffer);
        return NULL;
    }

    const char *field_name = ts_node_field_name_for_named_child(self, (uint32_t)index);
    return field_name ? (*env)->NewStringUTF(env, field_name) : NULL;
}

jobject JNICALL node_child_with_descendant(JNIEnv *env, jobject this, jobject descendant) {
    TSNode self = unmarshal_node(env, this);
    TSNode other = unmarshal_node(env, descendant);
    TSNode result = ts_node_child_with_descendant(self, other);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_descendant__bytes(JNIEnv *env, jobject this, jint start, jint end) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_descendant_for_byte_range(self, (uint32_t)start, (uint32_t)end);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_descendant__points(JNIEnv *env, jobject this, jobject start, jobject end) {
    TSNode self = unmarshal_node(env, this);
    TSPoint start_point = unmarshal_point(env, start);
    TSPoint end_point = unmarshal_point(env, end);
    TSNode result = ts_node_descendant_for_point_range(self, start_point, end_point);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_named_descendant__bytes(JNIEnv *env, jobject this, jint start, jint end) {
    TSNode self = unmarshal_node(env, this);
    TSNode result = ts_node_named_descendant_for_byte_range(self, (uint32_t)start, (uint32_t)end);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

jobject JNICALL node_named_descendant__points(JNIEnv *env, jobject this, jobject start,
                                              jobject end) {
    TSNode self = unmarshal_node(env, this);
    TSPoint start_point = unmarshal_point(env, start);
    TSPoint end_point = unmarshal_point(env, end);
    TSNode result = ts_node_named_descendant_for_point_range(self, start_point, end_point);
    if (ts_node_is_null(result))
        return NULL;
    jobject tree = GET_FIELD(Object, this, Node_tree);
    return marshal_node(env, result, tree);
}

void JNICALL node_edit(JNIEnv *env, jobject this, jobject edit) {
    TSNode self = unmarshal_node(env, this);
    TSInputEdit input_edit = unmarshal_input_edit(env, edit);
    ts_node_edit(&self, &input_edit);
    jobject context = (jintArray)GET_FIELD(Object, this, Node_context);
    (*env)->SetIntArrayRegion(env, context, 0, 4, (jint *)self.context);
}

jstring JNICALL node_sexp(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    char *sexp = ts_node_string(self);
    jobject result = (*env)->NewStringUTF(env, sexp);
    free(sexp);
    return result;
}

jint JNICALL node_hash_code(JNIEnv *env, jobject this) {
    TSNode self = unmarshal_node(env, this);
    uintptr_t id = (uintptr_t)self.id;
    uintptr_t tree = (uintptr_t)self.tree;
    return (jint)(id == tree ? id : id ^ tree);
}

jboolean JNICALL node_native_equals(JNIEnv *env, jobject this, jobject that) {
    TSNode self = unmarshal_node(env, this);
    TSNode other = unmarshal_node(env, that);
    return (jboolean)ts_node_eq(self, other);
}

const JNINativeMethod Node_methods[] = {
    {"getSymbol", "()S", (void *)&node_symbol},
    {"getGrammarSymbol", "()S", (void *)&node_grammar_symbol},
    {"getType", "()Ljava/lang/String;", (void *)&node_type},
    {"getGrammarType", "()Ljava/lang/String;", (void *)&node_grammar_type},
    {"isNamed", "()Z", (void *)&node_is_named},
    {"isExtra", "()Z", (void *)&node_is_extra},
    {"isError", "()Z", (void *)&node_is_error},
    {"isMissing", "()Z", (void *)&node_is_missing},
    {"hasError", "()Z", (void *)&node_has_error},
    {"hasChanges", "()Z", (void *)&node_has_changes},
    {"getParseState", "()S", (void *)&node_get_parse_state},
    {"getNextParseState", "()S", (void *)&node_get_next_parse_state},
    {"getStartByte", "()I", (void *)&node_get_start_byte},
    {"getEndByte", "()I", (void *)&node_get_end_byte},
    {"getStartPoint", "()L" PACKAGE "Point;", (void *)&node_get_start_point},
    {"getEndPoint", "()L" PACKAGE "Point;", (void *)&node_get_end_point},
    {"getChildCount", "()I", (void *)&node_get_child_count},
    {"getNamedChildCount", "()I", (void *)&node_get_named_child_count},
    {"getDescendantCount", "()I", (void *)&node_get_descendant_count},
    {"getParent", "()L" PACKAGE "Node;", (void *)&node_get_parent},
    {"getNextSibling", "()L" PACKAGE "Node;", (void *)&node_get_next_sibling},
    {"getNextNamedSibling", "()L" PACKAGE "Node;", (void *)&node_get_next_named_sibling},
    {"getPrevSibling", "()L" PACKAGE "Node;", (void *)&node_get_prev_sibling},
    {"getPrevNamedSibling", "()L" PACKAGE "Node;", (void *)&node_get_prev_named_sibling},
    {"getChildren", "()Ljava/util/List;", (void *)&node_get_children},
    {"child", "(I)L" PACKAGE "Node;", (void *)&node_child},
    {"namedChild", "(I)L" PACKAGE "Node;", (void *)&node_named_child},
    {"firstChildForByte", "(I)L" PACKAGE "Node;", (void *)&node_first_child_for_byte},
    {"firstNamedChildForByte", "(I)L" PACKAGE "Node;", (void *)&node_first_named_child_for_byte},
    {"childByFieldId", "(S)L" PACKAGE "Node;", (void *)&node_child_by_field_id},
    {"childByFieldName", "(Ljava/lang/String;)L" PACKAGE "Node;",
     (void *)&node_child_by_field_name},
    {"childrenByFieldId", "(S)Ljava/util/List;", (void *)&node_children_by_field_id},
    {"fieldNameForChild", "(I)Ljava/lang/String;", (void *)&node_field_name_for_child},
    {"fieldNameForNamedChild", "(I)Ljava/lang/String;", (void *)&node_field_name_for_named_child},
    {"childWithDescendant", "(L" PACKAGE "Node;)L" PACKAGE "Node;",
     (void *)&node_child_with_descendant},
    {"descendant", "(II)L" PACKAGE "Node;", (void *)&node_descendant__bytes},
    {"descendant", "(L" PACKAGE "Point;L" PACKAGE "Point;)L" PACKAGE "Node;",
     (void *)&node_descendant__points},
    {"namedDescendant", "(II)L" PACKAGE "Node;", (void *)&node_named_descendant__bytes},
    {"namedDescendant", "(L" PACKAGE "Point;L" PACKAGE "Point;)L" PACKAGE "Node;",
     (void *)&node_named_descendant__points},
    {"edit", "(L" PACKAGE "InputEdit;)V", (void *)&node_edit},
    {"sexp", "()Ljava/lang/String;", (void *)&node_sexp},
    {"hashCode", "()I", (void *)&node_hash_code},
    {"nativeEquals", "(L" PACKAGE "Node;)Z", (void *)&node_native_equals},
};

const size_t Node_methods_size = sizeof Node_methods / sizeof(JNINativeMethod);
