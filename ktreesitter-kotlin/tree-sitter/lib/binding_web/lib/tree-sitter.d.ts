// TypeScript bindings for emscripten-generated code.  Automatically generated at compile time.
declare namespace RuntimeExports {
    function AsciiToString(ptr: number): string;
    function stringToUTF8(str: string, outPtr: number, maxBytesToWrite: number): number;
    /**
     * Given a pointer 'ptr' to a null-terminated UTF8-encoded string in the
     * emscripten HEAP, returns a copy of that string as a Javascript String object.
     *
     * @param {number} ptr
     * @param {number=} maxBytesToRead - An optional length that specifies the
     *   maximum number of bytes to read. You can omit this parameter to scan the
     *   string until the first 0 byte. If maxBytesToRead is passed, and the string
     *   at [ptr, ptr+maxBytesToReadr[ contains a null byte in the middle, then the
     *   string will cut short at that byte index (i.e. maxBytesToRead will not
     *   produce a string of exact length [ptr, ptr+maxBytesToRead[) N.B. mixing
     *   frequent uses of UTF8ToString() with and without maxBytesToRead may throw
     *   JS JIT optimizations off, so it is worth to consider consistently using one
     * @return {string}
     */
    function UTF8ToString(ptr: number, maxBytesToRead?: number): string;
    function lengthBytesUTF8(str: string): number;
    function stringToUTF16(str: string, outPtr: number, maxBytesToWrite: number): number;
    /**
     * @param {string=} libName
     * @param {Object=} localScope
     * @param {number=} handle
     */
    function loadWebAssemblyModule(
      binary: Uint8Array,
      flags: {
        allowUndefined?: boolean,
        loadAsync?: boolean,
        global?: boolean,
        nodelete?: boolean;
      },
      libName?: string,
      localScope?: Record<string, any>,
      handle?: number
    ): Promise<Record<string, () => number>>;
    /**
     * @param {number} ptr
     * @param {string} type
     */
    function getValue(ptr: number, type?: string): number;
    /**
     * @param {number} ptr
     * @param {number} value
     * @param {string} type
     */
    function setValue(ptr: number, value: number, type?: string): void;
    let currentParseCallback: ((index: number, position: {row: number, column: number}) => string | undefined) | null;
    let currentLogCallback: ((message: string, isLex: boolean) => void) | null;
    let currentProgressCallback: ((state: {currentOffset: number}) => void) | null;
    let currentQueryProgressCallback: ((state: {currentOffset: number}) => void) | null;
    let HEAPF32: Float32Array;
    let HEAPF64: Float64Array;
    let HEAP_DATA_VIEW: DataView;
    let HEAP8: Int8Array
    let HEAPU8: Uint8Array;
    let HEAP16: Int16Array;
    let HEAPU16: Uint16Array;
    let HEAP32: Int32Array;
    let HEAPU32: Uint32Array;
    let HEAP64: BigInt64Array;
    let HEAPU64: BigUint64Array;
}
interface WasmModule {
  _malloc(_0: number): number;
  _calloc(_0: number, _1: number): number;
  _realloc(_0: number, _1: number): number;
  _free(_0: number): void;
  _ts_language_symbol_count(_0: number): number;
  _ts_language_state_count(_0: number): number;
  _ts_language_version(_0: number): number;
  _ts_language_abi_version(_0: number): number;
  _ts_language_metadata(_0: number): number;
  _ts_language_name(_0: number): number;
  _ts_language_field_count(_0: number): number;
  _ts_language_next_state(_0: number, _1: number, _2: number): number;
  _ts_language_symbol_name(_0: number, _1: number): number;
  _ts_language_symbol_for_name(_0: number, _1: number, _2: number, _3: number): number;
  _strncmp(_0: number, _1: number, _2: number): number;
  _ts_language_symbol_type(_0: number, _1: number): number;
  _ts_language_field_name_for_id(_0: number, _1: number): number;
  _ts_lookahead_iterator_new(_0: number, _1: number): number;
  _ts_lookahead_iterator_delete(_0: number): void;
  _ts_lookahead_iterator_reset_state(_0: number, _1: number): number;
  _ts_lookahead_iterator_reset(_0: number, _1: number, _2: number): number;
  _ts_lookahead_iterator_next(_0: number): number;
  _ts_lookahead_iterator_current_symbol(_0: number): number;
  _memset(_0: number, _1: number, _2: number): number;
  _memcpy(_0: number, _1: number, _2: number): number;
  _ts_parser_delete(_0: number): void;
  _ts_parser_reset(_0: number): void;
  _ts_parser_set_language(_0: number, _1: number): number;
  _ts_parser_timeout_micros(_0: number): number;
  _ts_parser_set_timeout_micros(_0: number, _1: number, _2: number): void;
  _ts_parser_set_included_ranges(_0: number, _1: number, _2: number): number;
  _memmove(_0: number, _1: number, _2: number): number;
  _memcmp(_0: number, _1: number, _2: number): number;
  _ts_query_new(_0: number, _1: number, _2: number, _3: number, _4: number): number;
  _ts_query_delete(_0: number): void;
  _iswspace(_0: number): number;
  _iswalnum(_0: number): number;
  _ts_query_pattern_count(_0: number): number;
  _ts_query_capture_count(_0: number): number;
  _ts_query_string_count(_0: number): number;
  _ts_query_capture_name_for_id(_0: number, _1: number, _2: number): number;
  _ts_query_capture_quantifier_for_id(_0: number, _1: number, _2: number): number;
  _ts_query_string_value_for_id(_0: number, _1: number, _2: number): number;
  _ts_query_predicates_for_pattern(_0: number, _1: number, _2: number): number;
  _ts_query_start_byte_for_pattern(_0: number, _1: number): number;
  _ts_query_end_byte_for_pattern(_0: number, _1: number): number;
  _ts_query_is_pattern_rooted(_0: number, _1: number): number;
  _ts_query_is_pattern_non_local(_0: number, _1: number): number;
  _ts_query_is_pattern_guaranteed_at_step(_0: number, _1: number): number;
  _ts_query_disable_capture(_0: number, _1: number, _2: number): void;
  _ts_query_disable_pattern(_0: number, _1: number): void;
  _ts_tree_copy(_0: number): number;
  _ts_tree_delete(_0: number): void;
  _ts_init(): number;
  _ts_parser_new_wasm(): void;
  _ts_parser_enable_logger_wasm(_0: number, _1: number): void;
  _ts_parser_parse_wasm(_0: number, _1: number, _2: number, _3: number, _4: number): number;
  _ts_parser_included_ranges_wasm(_0: number): void;
  _ts_language_type_is_named_wasm(_0: number, _1: number): number;
  _ts_language_type_is_visible_wasm(_0: number, _1: number): number;
  _ts_language_supertypes_wasm(_0: number): void;
  _ts_language_subtypes_wasm(_0: number, _1: number): void;
  _ts_tree_root_node_wasm(_0: number): void;
  _ts_tree_root_node_with_offset_wasm(_0: number): void;
  _ts_tree_edit_wasm(_0: number): void;
  _ts_tree_included_ranges_wasm(_0: number): void;
  _ts_tree_get_changed_ranges_wasm(_0: number, _1: number): void;
  _ts_tree_cursor_new_wasm(_0: number): void;
  _ts_tree_cursor_copy_wasm(_0: number): void;
  _ts_tree_cursor_delete_wasm(_0: number): void;
  _ts_tree_cursor_reset_wasm(_0: number): void;
  _ts_tree_cursor_reset_to_wasm(_0: number, _1: number): void;
  _ts_tree_cursor_goto_first_child_wasm(_0: number): number;
  _ts_tree_cursor_goto_last_child_wasm(_0: number): number;
  _ts_tree_cursor_goto_first_child_for_index_wasm(_0: number): number;
  _ts_tree_cursor_goto_first_child_for_position_wasm(_0: number): number;
  _ts_tree_cursor_goto_next_sibling_wasm(_0: number): number;
  _ts_tree_cursor_goto_previous_sibling_wasm(_0: number): number;
  _ts_tree_cursor_goto_descendant_wasm(_0: number, _1: number): void;
  _ts_tree_cursor_goto_parent_wasm(_0: number): number;
  _ts_tree_cursor_current_node_type_id_wasm(_0: number): number;
  _ts_tree_cursor_current_node_state_id_wasm(_0: number): number;
  _ts_tree_cursor_current_node_is_named_wasm(_0: number): number;
  _ts_tree_cursor_current_node_is_missing_wasm(_0: number): number;
  _ts_tree_cursor_current_node_id_wasm(_0: number): number;
  _ts_tree_cursor_start_position_wasm(_0: number): void;
  _ts_tree_cursor_end_position_wasm(_0: number): void;
  _ts_tree_cursor_start_index_wasm(_0: number): number;
  _ts_tree_cursor_end_index_wasm(_0: number): number;
  _ts_tree_cursor_current_field_id_wasm(_0: number): number;
  _ts_tree_cursor_current_depth_wasm(_0: number): number;
  _ts_tree_cursor_current_descendant_index_wasm(_0: number): number;
  _ts_tree_cursor_current_node_wasm(_0: number): void;
  _ts_node_symbol_wasm(_0: number): number;
  _ts_node_field_name_for_child_wasm(_0: number, _1: number): number;
  _ts_node_field_name_for_named_child_wasm(_0: number, _1: number): number;
  _ts_node_children_by_field_id_wasm(_0: number, _1: number): void;
  _ts_node_first_child_for_byte_wasm(_0: number): void;
  _ts_node_first_named_child_for_byte_wasm(_0: number): void;
  _ts_node_grammar_symbol_wasm(_0: number): number;
  _ts_node_child_count_wasm(_0: number): number;
  _ts_node_named_child_count_wasm(_0: number): number;
  _ts_node_child_wasm(_0: number, _1: number): void;
  _ts_node_named_child_wasm(_0: number, _1: number): void;
  _ts_node_child_by_field_id_wasm(_0: number, _1: number): void;
  _ts_node_next_sibling_wasm(_0: number): void;
  _ts_node_prev_sibling_wasm(_0: number): void;
  _ts_node_next_named_sibling_wasm(_0: number): void;
  _ts_node_prev_named_sibling_wasm(_0: number): void;
  _ts_node_descendant_count_wasm(_0: number): number;
  _ts_node_parent_wasm(_0: number): void;
  _ts_node_child_with_descendant_wasm(_0: number): void;
  _ts_node_descendant_for_index_wasm(_0: number): void;
  _ts_node_named_descendant_for_index_wasm(_0: number): void;
  _ts_node_descendant_for_position_wasm(_0: number): void;
  _ts_node_named_descendant_for_position_wasm(_0: number): void;
  _ts_node_start_point_wasm(_0: number): void;
  _ts_node_end_point_wasm(_0: number): void;
  _ts_node_start_index_wasm(_0: number): number;
  _ts_node_end_index_wasm(_0: number): number;
  _ts_node_to_string_wasm(_0: number): number;
  _ts_node_children_wasm(_0: number): void;
  _ts_node_named_children_wasm(_0: number): void;
  _ts_node_descendants_of_type_wasm(_0: number, _1: number, _2: number, _3: number, _4: number, _5: number, _6: number): void;
  _ts_node_is_named_wasm(_0: number): number;
  _ts_node_has_changes_wasm(_0: number): number;
  _ts_node_has_error_wasm(_0: number): number;
  _ts_node_is_error_wasm(_0: number): number;
  _ts_node_is_missing_wasm(_0: number): number;
  _ts_node_is_extra_wasm(_0: number): number;
  _ts_node_parse_state_wasm(_0: number): number;
  _ts_node_next_parse_state_wasm(_0: number): number;
  _ts_query_matches_wasm(_0: number, _1: number, _2: number, _3: number, _4: number, _5: number, _6: number, _7: number, _8: number, _9: number, _10: number): void;
  _ts_query_captures_wasm(_0: number, _1: number, _2: number, _3: number, _4: number, _5: number, _6: number, _7: number, _8: number, _9: number, _10: number): void;
  _iswalpha(_0: number): number;
  _iswblank(_0: number): number;
  _iswdigit(_0: number): number;
  _iswlower(_0: number): number;
  _iswupper(_0: number): number;
  _iswxdigit(_0: number): number;
  _memchr(_0: number, _1: number, _2: number): number;
  _strlen(_0: number): number;
  _strcmp(_0: number, _1: number): number;
  _strncat(_0: number, _1: number, _2: number): number;
  _strncpy(_0: number, _1: number, _2: number): number;
  _towlower(_0: number): number;
  _towupper(_0: number): number;
  _orig$ts_parser_timeout_micros(_0: number): bigint;
  _orig$ts_parser_set_timeout_micros(_0: number, _1: bigint): void;
}

export type MainModule = WasmModule & typeof RuntimeExports;
export default function MainModuleFactory (options?: EmscriptenModule): Promise<MainModule>;
