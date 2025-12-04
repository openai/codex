The Error Corpus
================

This directory contains corpus tests that exercise error recovery in a variety of languages.

These corpus tests provide a simple way of asserting that error recoveries are "reasonable" in a variety of situations. But they are also somewhat *overspecified*. It isn't critical that error recovery behaves *exactly* as these tests specify, just that most of the syntax tree is preserved despite the error.

Sometimes these tests can start failing when changes are pushed to the parser repositories like `tree-sitter-ruby`, `tree-sitter-javascript`, etc. Usually, we just need to tweak the expected syntax tree.
