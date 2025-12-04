#include "tree_sitter/parser.h"

enum TokenType {
  COMMENT
};

void *tree_sitter_epsilon_external_extra_tokens_external_scanner_create(void) {
  return NULL;
}

bool tree_sitter_epsilon_external_extra_tokens_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  lexer->result_symbol = COMMENT;
  return true;
}

unsigned tree_sitter_epsilon_external_extra_tokens_external_scanner_serialize(
  void *payload,
  char *buffer
) {
  return 0;
}

void tree_sitter_epsilon_external_extra_tokens_external_scanner_deserialize(
  void *payload,
  const char *buffer,
  unsigned length
) {}

void tree_sitter_epsilon_external_extra_tokens_external_scanner_destroy(void *payload) {}
