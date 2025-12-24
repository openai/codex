This directory exists solely to give Buck2 a stable in-repo path for the
`prelude` cell declared in `.buckconfig`.

The actual prelude implementation is provided by the Buck2 binary itself via
`[external_cells] prelude = bundled`.

