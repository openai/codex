extern fn tree_sitter_PARSER_NAME() callconv(.c) *const anyopaque;

pub fn language() *const anyopaque {
    return tree_sitter_PARSER_NAME();
}
