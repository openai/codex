#include "tree_sitter/parser.h"

unsigned tree_sitter_get_col_should_hang_not_crash_external_scanner_serialize() { return 0; }

void tree_sitter_get_col_should_hang_not_crash_external_scanner_deserialize() {}

void *tree_sitter_get_col_should_hang_not_crash_external_scanner_create() { return NULL; }

void tree_sitter_get_col_should_hang_not_crash_external_scanner_destroy() {}

bool tree_sitter_get_col_should_hang_not_crash_external_scanner_scan(void *payload, TSLexer *lexer,
                                            const bool *valid_symbols) {
    while (true) {
        lexer->advance(lexer, false);
        lexer->get_column(lexer);
    }
}
