# markdownlint style rules for OpenSSL
# See https://github.com/markdownlint/markdownlint/blob/master/docs/RULES.md

all

# Use --- and === for H1 and H2.
rule 'MD003', :style => :setext_with_atx
# Code blocks may be fenced or indented, both are OK...
# but they must be consistent throughout each file.
rule 'MD046', :style => :consistent

# Bug in mdl, https://github.com/markdownlint/markdownlint/issues/313
exclude_rule 'MD007'

# Not possible to line-break tables (:tables => false)
# Not possible to line-break headers (currently cannot be selectively exempted)
exclude_rule 'MD013'

exclude_rule 'MD004' # Unordered list style TODO(fix?)
exclude_rule 'MD005' # Inconsistent indentation for list items at the same level
exclude_rule 'MD006' # Consider starting bulleted lists at the beginning of the line
exclude_rule 'MD014' # Dollar signs used before commands without showing output
exclude_rule 'MD023' # Headers must start at the beginning of the line
exclude_rule 'MD024' # Multiple headers with the same content
exclude_rule 'MD025' # Multiple top level headers in the same document
exclude_rule 'MD026' # Trailing punctuation in header
exclude_rule 'MD029' # Ordered list item prefix
exclude_rule 'MD030' # Spaces after list markers (default: 1!)
exclude_rule 'MD033' # Allow inline HTML (complex tables are impossible without it)
