# Root-Cause Analysis: OAuth Tokens Too Long for Windows Keyring (Issue #10353)

## Problem

When using OAuth authentication with keyring storage on Windows, users encounter the error:
```
Unable to persist auth file: failed to write OAuth tokens to keyring: 
Attribute 'password encoded as UTF-16' is longer than platform limit of 2560 chars
```

This prevents users from logging in via OAuth on Windows when using the keyring storage option.

## Technical Root Cause

### Windows Credential Manager Limit

Windows Credential Manager has a hard limit of **2560 UTF-16 code units** for the password field. OAuth tokens (especially JWT tokens with claims) frequently exceed this limit.

### Previous Chunking Implementation

A chunking implementation was added to handle large tokens by splitting them into multiple credential entries:
- Values exceeding `MAX_KEYRING_VALUE_LEN` are split into chunks
- Main key stores a header with format: `CODEX_CHUNKED:{count}:{first_chunk}`
- Additional chunks stored in keys like `account:1`, `account:2`, etc.

### The Bug

The previous implementation set `MAX_KEYRING_VALUE_LEN = 512`. While this value itself is well under 2560, the issue is that the **header chunk** (which includes the prefix `CODEX_CHUNKED:`, the count, a colon, AND the first chunk) could still approach the limit under certain conditions:

1. **UTF-16 Encoding Expansion**: Some characters (non-BMP Unicode) require 2 UTF-16 code units per character
2. **Header Overhead Not Accounted**: The header adds ~20 characters on top of the first chunk
3. **Edge Cases**: With special characters in tokens, the effective UTF-16 length could exceed limits

## Resolution

Updated `MAX_KEYRING_VALUE_LEN` from `512` to `1200` characters. This provides:

- **Header chunk total**: ~1220 UTF-16 code units (1200 data + 20 overhead)
- **Safety margin**: This is less than half the 2560 limit
- **UTF-16 expansion room**: Even if every character expanded to 2 UTF-16 units, we'd be at ~2440, still under the limit

### Code Changes

```rust
// Old value
pub(crate) const MAX_KEYRING_VALUE_LEN: usize = 512;

// New value with documentation
/// Maximum characters per chunk for keyring storage.
/// Windows Credential Manager has a 2560 UTF-16 code unit limit.
/// The header chunk format is: `CODEX_CHUNKED:{count}:{first_chunk}`
/// We use 1200 chars per chunk to ensure even the header chunk (with ~20 char overhead)
/// stays well under the 2560 limit, with room for UTF-16 encoding expansion.
pub(crate) const MAX_KEYRING_VALUE_LEN: usize = 1200;
```

## Verification

1. Unit tests pass with the new chunk size
2. The chunking logic correctly splits tokens > 1200 chars into multiple entries
3. Load/delete operations properly reassemble chunked values

## Files Modified

- `codex-rs/keyring-store/src/lib.rs`

## Testing Recommendations

1. Test with OAuth tokens of varying lengths (up to 10,000+ characters)
2. Test with tokens containing non-ASCII characters (emojis, CJK characters)
3. Verify login/logout cycles properly clean up chunked credentials
