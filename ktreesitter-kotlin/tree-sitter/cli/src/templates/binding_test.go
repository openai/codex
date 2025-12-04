package tree_sitter_LOWER_PARSER_NAME_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_LOWER_PARSER_NAME "PARSER_URL_STRIPPED/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_LOWER_PARSER_NAME.Language())
	if language == nil {
		t.Errorf("Error loading TITLE_PARSER_NAME grammar")
	}
}
