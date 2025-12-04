#include "tree_sitter/parser.h"

enum {
  COMMENT,
};

void *tree_sitter_external_extra_tokens_external_scanner_create() {
  return NULL;
}

void tree_sitter_external_extra_tokens_external_scanner_destroy(void *payload) {}

unsigned tree_sitter_external_extra_tokens_external_scanner_serialize(
  void *payload,
  char *buffer
) { return 0; }

void tree_sitter_external_extra_tokens_external_scanner_deserialize(
  void *payload,
  const char *buffer,
  unsigned length
) {}

bool tree_sitter_external_extra_tokens_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  while (lexer->lookahead == ' ') {
    lexer->advance(lexer, true);
  }

  if (lexer->lookahead == '#') {
    lexer->advance(lexer, false);
    while (lexer->lookahead != '\n') {
      lexer->advance(lexer, false);
    }

    lexer->result_symbol = COMMENT;
    return true;
  }

  return false;
}
