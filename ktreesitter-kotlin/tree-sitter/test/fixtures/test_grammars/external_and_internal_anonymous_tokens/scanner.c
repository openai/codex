#include "tree_sitter/parser.h"

enum {
  STRING,
  LINE_BREAK
};

void *tree_sitter_external_and_internal_anonymous_tokens_external_scanner_create() {
  return NULL;
}

void tree_sitter_external_and_internal_anonymous_tokens_external_scanner_destroy(
  void *payload
) {}

unsigned tree_sitter_external_and_internal_anonymous_tokens_external_scanner_serialize(
  void *payload,
  char *buffer
) { return 0; }

void tree_sitter_external_and_internal_anonymous_tokens_external_scanner_deserialize(
  void *payload,
  const char *buffer,
  unsigned length
) {}

bool tree_sitter_external_and_internal_anonymous_tokens_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  // If a line-break is a valid lookahead token, only skip spaces.
  if (valid_symbols[LINE_BREAK]) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\r') {
      lexer->advance(lexer, true);
    }

    if (lexer->lookahead == '\n') {
      lexer->advance(lexer, false);
      lexer->result_symbol = LINE_BREAK;
      return true;
    }
  }

  // If a line-break is not a valid lookahead token, skip line breaks as well
  // as spaces.
  if (valid_symbols[STRING]) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\r' || lexer->lookahead == '\n') {
      lexer->advance(lexer, true);
    }

    if (lexer->lookahead == '\'') {
      lexer->advance(lexer, false);

      while (lexer->lookahead != '\'') {
        lexer->advance(lexer, false);
      }

      lexer->advance(lexer, false);
      lexer->result_symbol = STRING;
      return true;
    }
  }

  return false;
}
