/// Special argv[1] flag used when the Codex executable self-invokes to run the
/// internal sandbox-backed filesystem helper path.
pub const CODEX_CORE_FS_OPS_ARG1: &str = "--codex-run-as-fs-ops";

/// When passed as argv[2] to the Codex filesystem helper, it should be followed
/// by a single path argument, and the helper will read the contents of the file
/// at that path and write it to stdout.
pub const READ_FILE_OPERATION_ARG2: &str = "read";
