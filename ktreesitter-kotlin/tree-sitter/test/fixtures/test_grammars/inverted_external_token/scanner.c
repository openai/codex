#include "tree_sitter/parser.h"

enum {
  LINE_BREAK
};

void *tree_sitter_inverted_external_token_external_scanner_create() { return NULL; }

void tree_sitter_inverted_external_token_external_scanner_destroy(void *payload) {}

unsigned tree_sitter_inverted_external_token_external_scanner_serialize(
  void *payload,
  char *buffer
) { return true; }

void tree_sitter_inverted_external_token_external_scanner_deserialize(
  void *payload,
  const char *buffer,
  unsigned length
) {}

bool tree_sitter_inverted_external_token_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  while (lexer->lookahead == ' ' || lexer->lookahead == '\r') {
    lexer->advance(lexer, true);
  }

  if (lexer->lookahead == '\n') {
    lexer->advance(lexer, false);

    // Mark the end of the line break token.
    lexer->mark_end(lexer);

    // Skip whitespace *after* having marked the end.
    while (lexer->lookahead == ' ' || lexer->lookahead == '\n' || lexer->lookahead == '\r') {
      lexer->advance(lexer, true);
    }

    if (lexer->lookahead != '.') {
      lexer->result_symbol = LINE_BREAK;
      return true;
    }
  }

  return false;
}
