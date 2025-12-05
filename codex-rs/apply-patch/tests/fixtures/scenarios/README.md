# Overview
This directory is a collection of end to end tests for the apply-patch specification, meant to be easily portable to other languages or platforms.


# Specification
Each test case is one directory, composed of input state (input/), the patch operation (patch.txt), and the expected final state (expected/). This structure is designed to keep tests simple (i.e. test exactly one patch at a time) while still providing enough flexibility to test any given operation across files.

Here's what this would look like for a simple test apply-patch test case to create a new file:

```
001_add/
  input/
    foo.md
  expected/
    foo.md
    bar.md
  patch.txt
```
