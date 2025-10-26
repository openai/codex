## Overview
`md-events` is a small diagnostic binary that reads markdown from stdin, parses it with `pulldown_cmark`, and prints the resulting event stream (`Debug` format). It helps developers inspect how markdown input is tokenized when debugging rendering quirks in the TUI.
