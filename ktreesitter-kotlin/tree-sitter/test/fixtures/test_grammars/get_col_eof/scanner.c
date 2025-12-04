#include "tree_sitter/parser.h"

enum TokenType { CHAR };

void *tree_sitter_get_col_eof_external_scanner_create(void) { return NULL; }

void tree_sitter_get_col_eof_external_scanner_destroy(void *scanner) {}

unsigned tree_sitter_get_col_eof_external_scanner_serialize(void *scanner,
                                                            char *buffer) {
  return 0;
}

void tree_sitter_get_col_eof_external_scanner_deserialize(void *scanner,
                                                          const char *buffer,
                                                          unsigned length) {}

bool tree_sitter_get_col_eof_external_scanner_scan(void *scanner,
                                                   TSLexer *lexer,
                                                   const bool *valid_symbols) {
  if (lexer->eof(lexer)) {
    return false;
  }

  if (valid_symbols[CHAR]) {
    lexer->advance(lexer, false);
    lexer->get_column(lexer);
    lexer->result_symbol = CHAR;
    lexer->mark_end(lexer);
    return true;
  }

  return false;
}
