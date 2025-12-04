from unittest import TestCase

from tree_sitter import Language, Parser
import tree_sitter_LOWER_PARSER_NAME


class TestLanguage(TestCase):
    def test_can_load_grammar(self):
        try:
            Parser(Language(tree_sitter_LOWER_PARSER_NAME.language()))
        except Exception:
            self.fail("Error loading TITLE_PARSER_NAME grammar")
