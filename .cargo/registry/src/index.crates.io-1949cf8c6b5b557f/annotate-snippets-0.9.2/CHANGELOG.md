# Changelog
 
## Unreleased
 
  - …

## annotate-snippets 0.9.2 (October 30, 2023)

  - Remove parsing of __ in title strings, fixes (#53)
  - Origin line number is not correct when using a slice with fold: true (#52)

## annotate-snippets 0.9.1 (September 4, 2021)

  - Fix character split when strip code. (#37)
  - Fix off by one error in multiline highlighting. (#42)
  - Fix display of annotation for double width characters. (#46)

## annotate-snippets 0.9.0 (June 28, 2020)

  - Add strip code to the left and right of long lines. (#36)

## annotate-snippets 0.8.0 (April 14, 2020)

  - Replace `ansi_term` with `yansi-term` for improved performance. (#30)
  - Turn `Snippet` and `Slice` to work on borrowed slices, rather than Strings. (#32)
  - Fix `\r\n` end of lines. (#29)

## annotate-snippets 0.7.0 (March 30, 2020)

  - Refactor API to use `fmt::Display` (#27)
  - Fix SourceAnnotation range (#27)
  - Fix column numbers (#22)
  - Derive `PartialEq` for `AnnotationType` (#19)
  - Update `ansi_term` to 0.12.

## annotate-snippets 0.6.1 (July 23, 2019)

  - Fix too many anonymized line numbers (#5)
 
## annotate-snippets 0.6.0 (June 26, 2019)
 
  - Add an option to anonymize line numbers (#3)
  - Transition the crate to rust-lang org.
  - Update the syntax to Rust 2018 idioms. (#4)
