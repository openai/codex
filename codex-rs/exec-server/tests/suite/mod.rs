// TODO(mbolin): Get this test working on Linux. Currently, it fails with:
//
// > Error: Mcp error: -32603: sandbox error: sandbox denied exec error,
// > exit code: 1, stdout: , stderr: Error: failed to send handshake datagram
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod accept_elicitation;
#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), target_os = "linux"))]
mod list_tools;
