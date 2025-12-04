#include "tree_sitter/alloc.h"
#include "tree_sitter/parser.h"

#include <wctype.h>
#include <string.h>

enum {
  LIST_START,
  LIST_ITEM,
  LIST_END
};

typedef struct {
  int32_t column;
} Scanner;

void *tree_sitter_external_unicode_column_alignment_external_scanner_create() {
  Scanner *scanner = ts_malloc(sizeof(Scanner));
  *scanner = (Scanner){
    .column = -1
  };
  return scanner;
}

void tree_sitter_external_unicode_column_alignment_external_scanner_destroy(void *payload) {
  ts_free(payload);
}

unsigned tree_sitter_external_unicode_column_alignment_external_scanner_serialize(
  void *payload,
  char *buffer
) {
  Scanner *scanner = payload;
  unsigned copied = sizeof(int32_t);
  memcpy(buffer, &(scanner->column), copied);
  return copied;
}

void tree_sitter_external_unicode_column_alignment_external_scanner_deserialize(
  void *payload,
  const char *buffer,
  unsigned length
) {
  Scanner *scanner = payload;
  scanner->column = -1;
  if (length > 0) {
    memcpy(&(scanner->column), buffer, sizeof(int32_t));
  }
}

bool tree_sitter_external_unicode_column_alignment_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  Scanner *scanner = payload;
  // U+25A1 is unicode codepoint □
  while (iswspace(lexer->lookahead) || 0x25A1 == lexer->lookahead) {
    lexer->advance(lexer, true);
  } 
  if ('-' == lexer->lookahead) {
    const int32_t column = lexer->get_column(lexer);
    if (-1 == scanner->column) {
      lexer->result_symbol = LIST_START;
      scanner->column = column;
      return true;
    } else {
      if (column == scanner->column) {
        lexer->result_symbol = LIST_ITEM;
        lexer->advance(lexer, false);
        return true;
      } else {
        lexer->result_symbol = LIST_END;
        scanner->column = -1;
        return true;
      }
    }
  }
  
  if (lexer->eof(lexer) && -1 != scanner->column) {
    lexer->result_symbol = LIST_END;
    scanner->column = -1;
    return true;
  }
  
  return false;
}
