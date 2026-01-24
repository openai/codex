# cocode-error

Unified error utilities with status codes, retry semantics, and virtual stack traces.

## Quick Start

```rust
use cocode_error::{ErrorExt, StatusCode, Location, stack_trace_debug};
use snafu::{ResultExt, Snafu};

#[stack_trace_debug]  // Must be BEFORE #[derive(Snafu)]
#[derive(Snafu)]
pub enum Error {
    #[snafu(display("Failed to read file: {path}"))]
    ReadFile {
        path: String,
        #[snafu(source)]
        source: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Rate limited"))]
    RateLimited {
        retry_after: std::time::Duration,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ErrorExt for Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::ReadFile { .. } => StatusCode::IoError,
            Self::RateLimited { .. } => StatusCode::RateLimited,
        }
    }

    fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Self::RateLimited { retry_after, .. } => Some(*retry_after),
            _ => None,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

## Location

```rust
#[non_exhaustive]
pub struct Location {
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}
```

Add to error variants with `#[snafu(implicit)]` - auto-captured at error creation site.

## StatusCode Categories

| Category | Range | Examples |
|----------|-------|----------|
| Success | 00_xxx | Success |
| Common | 01_xxx | Unknown, Internal, Cancelled |
| Input | 02_xxx | InvalidArguments, ParseError |
| IO | 03_xxx | IoError, FileNotFound |
| Network | 04_xxx | NetworkError, ConnectionFailed |
| Auth | 05_xxx | AuthenticationFailed, PermissionDenied |
| Config | 10_xxx | InvalidConfig |
| Provider | 11_xxx | ProviderNotFound, ModelNotFound |
| Resource | 12_xxx | RateLimited, Timeout, QuotaExceeded |
