## Overview
`read_api_key.rs` securely reads the upstream Authorization header from stdin, returning a `&'static str` that is locked in memory (on Unix) to minimize exposure of the API key used by the proxy.

## Detailed Behavior
- Constants:
  - `BUFFER_SIZE` (1024) accommodates future token lengths.
  - `AUTH_HEADER_PREFIX` (`"Bearer "`) ensures the returned header string always starts with the prefix expected by OpenAI.
- `read_auth_header_from_stdin` delegates to `read_auth_header_with`, using a low-level `read(2)` loop on Unix to avoid `std::io::stdin()`’s buffered copies. Windows currently uses `stdin().read` with a TODO to adopt an equivalent low-level API.
- `read_auth_header_with`:
  - Reads into a stack-allocated buffer, trimming newline/CRLF suffixes, and validating size limits.
  - Calls `validate_auth_header_bytes` (not shown here) to ensure the token contains only ASCII tokens expected by OpenAI.
  - Converts the buffer to a `String`, immediately leaks it (`String::leak`) to `'static`, and zeroizes the temporary buffer with `zeroize`.
  - Invokes `mlock_str` on Unix to lock the leaked string’s pages in memory (failures are ignored but best-effort).
- `mlock_str` aligns the string’s memory to page boundaries and uses `libc::mlock` to prevent swapping.
- Windows variant notes the lack of `mlock` and plans to revisit when an equivalent is available.

## Broader Context
- Ensures the proxy does not accidentally retain multiple copies of the API key in heap buffers or logs. All subsequent request forwarding uses the leaked header reference.

## Technical Debt
- Windows implementation still relies on buffered stdin and lacks a secure memory-locking equivalent; code comments track this gap.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Replace Windows stdin handling with a low-level reader and introduce a secure memory-locking approach when available.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
