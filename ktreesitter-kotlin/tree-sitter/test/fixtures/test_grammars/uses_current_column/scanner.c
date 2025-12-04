#include "tree_sitter/alloc.h"
#include "tree_sitter/parser.h"

#include <stdlib.h>
#include <wctype.h>

enum TokenType {
  INDENT,
  DEDENT,
  NEWLINE,
};

typedef struct {
  uint8_t queued_dedent_count;
  uint8_t indent_count;
  int8_t indents[32];
} Scanner;

void *tree_sitter_uses_current_column_external_scanner_create() {
  Scanner *self = ts_malloc(sizeof(Scanner));
  self->queued_dedent_count = 0;
  self->indent_count = 1;
  self->indents[0] = 0;
  return (void *)self;
}

void tree_sitter_uses_current_column_external_scanner_destroy(void *payload) {
  ts_free(payload);
}

unsigned tree_sitter_uses_current_column_external_scanner_serialize(
  void *payload,
  char *buffer
) {
  Scanner *self = (Scanner *)payload;
  buffer[0] = self->queued_dedent_count;
  for (unsigned i = 0; i < self->indent_count; i++) {
    buffer[i + 1] = self->indents[i];
  }
  return self->indent_count + 1;
}

void tree_sitter_uses_current_column_external_scanner_deserialize(
  void *payload,
  const char *buffer,
  unsigned length
) {
  Scanner *self = (Scanner *)payload;
  if (length > 0) {
    self->queued_dedent_count = buffer[0];
    self->indent_count = length - 1;
    for (unsigned i = 0; i < self->indent_count; i++) {
      self->indents[i] = buffer[i + 1];
    }
  } else {
    self->queued_dedent_count = 0;
    self->indent_count = 1;
    self->indents[0] = 0;
  }
}

bool tree_sitter_uses_current_column_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  Scanner *self = (Scanner *)payload;
  lexer->mark_end(lexer);

  // If dedents were found in a previous run, and are valid now,
  // then return a dedent.
  if (self->queued_dedent_count > 0 && valid_symbols[DEDENT]) {
    lexer->result_symbol = DEDENT;
    self->queued_dedent_count--;
    return true;
  }

  // If an indent is valid, then add an entry to the indent stack
  // for the current column, and return an indent.
  if (valid_symbols[INDENT]) {
    while (iswspace(lexer->lookahead)) {
      lexer->advance(lexer, false);
    }
    uint32_t column = lexer->get_column(lexer);
    if (column > self->indents[self->indent_count - 1]) {
      self->indents[self->indent_count++] = column - 2;
      lexer->result_symbol = INDENT;
      return true;
    } else {
      return false;
    }
  }

  // If at the end of a statement, then get the current indent
  // level and pop some number of entries off of the indent stack.
  if (valid_symbols[NEWLINE] || valid_symbols[DEDENT]) {
    while (iswspace(lexer->lookahead) && lexer->lookahead != '\n') {
      lexer->advance(lexer, false);
    }

    if (lexer->lookahead == '\n') {
      lexer->advance(lexer, false);

      uint32_t next_column = 0;
      for (;;) {
        if (lexer->lookahead == ' ') {
          next_column++;
          lexer->advance(lexer, false);
        } else if (lexer->lookahead == '\n') {
          next_column = 0;
          lexer->advance(lexer, false);
        } else {
          break;
        }
      }

      unsigned dedent_count = 0;
      while (next_column < self->indents[self->indent_count - 1]) {
        dedent_count++;
        self->indent_count--;
      }

      if (dedent_count > 0 && valid_symbols[DEDENT]) {
        lexer->result_symbol = DEDENT;
        return true;
      } else if (valid_symbols[NEWLINE]) {
        self->queued_dedent_count += dedent_count;
        lexer->result_symbol = NEWLINE;
        return true;
      }
    }
  }

  return false;
}
