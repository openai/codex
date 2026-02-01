//! Unified status codes for error classification.
//!
//! Status code format: XX_YYY (5-digit)
//! - XX = Category (00-99)
//! - YYY = Code within category (000-999)
//!
//! Category layout:
//! - General/Core (01-09): Common, Input, IO, Network, Auth
//! - Business (10-19): Config, Provider, Resource

use strum::AsRefStr;
use strum::EnumIter;
use strum::FromRepr;

/// Status code metadata.
#[derive(Debug, Clone, Copy)]
pub struct StatusMeta {
    pub retryable: bool,
    pub log_error: bool,
    pub category: StatusCategory,
}

/// Status code category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCategory {
    // ====== General/Core (00-05) ======
    /// Success (00_xxx)
    Success,
    /// Common/internal errors (01_xxx)
    Common,
    /// Parameter/validation errors (02_xxx)
    Input,
    /// IO/storage errors (03_xxx)
    IO,
    /// Network/transport errors (04_xxx)
    Network,
    /// Authentication/authorization errors (05_xxx)
    Auth,

    // ====== Business (10-12) ======
    /// Configuration errors (10_xxx)
    Config,
    /// LLM provider/model errors (11_xxx)
    Provider,
    /// Resource limits (12_xxx)
    Resource,
}

macro_rules! define_status_codes {
    ($(
        $(#[$attr:meta])*
        $name:ident = $value:expr => {
            retryable: $retry:expr,
            log_error: $log:expr,
            category: $cat:ident $(,)?
        }
    ),* $(,)?) => {
        /// Status codes for error classification.
        ///
        /// Format: XX_YYY (5-digit)
        /// - XX = Category (00-99)
        /// - YYY = Code within category (000-999)
        ///
        /// Ranges:
        /// - 00_000: Success
        /// - 01_xxx: Common/Generic errors
        /// - 02_xxx: Input/Validation errors
        /// - 03_xxx: IO/Storage errors
        /// - 04_xxx: Network/Transport errors
        /// - 05_xxx: Auth errors
        /// - 10_xxx: Config errors
        /// - 11_xxx: Provider/Model errors
        /// - 12_xxx: Resource/Limit errors
        #[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, EnumIter, FromRepr)]
        #[repr(i32)]
        pub enum StatusCode {
            $($(#[$attr])* $name = $value,)*
        }

        impl StatusCode {
            /// Returns the metadata for this status code.
            pub const fn meta(&self) -> StatusMeta {
                match self {
                    $(Self::$name => StatusMeta {
                        retryable: $retry,
                        log_error: $log,
                        category: StatusCategory::$cat,
                    },)*
                }
            }

            /// Returns the string name of this status code.
            pub const fn name(&self) -> &'static str {
                match self {
                    $(Self::$name => stringify!($name),)*
                }
            }
        }

        // Compile-time check for duplicate status code values
        const _: () = {
            const CODES: &[i32] = &[$($value),*];
            const fn check_unique() {
                let mut i = 0;
                while i < CODES.len() {
                    let mut j = i + 1;
                    while j < CODES.len() {
                        if CODES[i] == CODES[j] {
                            panic!("Duplicate status code value detected");
                        }
                        j += 1;
                    }
                    i += 1;
                }
            }
            check_unique();
        };
    };
}

define_status_codes! {
    // ====== Success (00_xxx) ======
    /// Operation succeeded.
    Success = 00_000 => { retryable: false, log_error: false, category: Success },

    // ====== Common errors (01_xxx) ======
    /// Unknown error.
    Unknown = 01_000 => { retryable: false, log_error: true, category: Common },
    /// Internal error, unexpected BUG.
    Internal = 01_001 => { retryable: true, log_error: true, category: Common },
    /// Unsupported operation.
    Unsupported = 01_002 => { retryable: false, log_error: false, category: Common },
    /// Task was cancelled.
    Cancelled = 01_003 => { retryable: false, log_error: false, category: Common },
    /// Caused by external system.
    External = 01_004 => { retryable: false, log_error: true, category: Common },

    // ====== Input/Validation errors (02_xxx) ======
    /// Invalid arguments.
    InvalidArguments = 02_000 => { retryable: false, log_error: false, category: Input },
    /// Invalid request format.
    InvalidRequest = 02_001 => { retryable: false, log_error: false, category: Input },
    /// Parse/Deserialize error.
    ParseError = 02_002 => { retryable: false, log_error: false, category: Input },
    /// Invalid JSON.
    InvalidJson = 02_003 => { retryable: false, log_error: false, category: Input },

    // ====== IO/Storage errors (03_xxx) ======
    /// IO error.
    IoError = 03_000 => { retryable: false, log_error: false, category: IO },
    /// File not found.
    FileNotFound = 03_001 => { retryable: false, log_error: false, category: IO },

    // ====== Network/Transport errors (04_xxx) ======
    /// Network error.
    NetworkError = 04_000 => { retryable: true, log_error: false, category: Network },
    /// Connection failed.
    ConnectionFailed = 04_001 => { retryable: true, log_error: false, category: Network },
    /// Service unavailable.
    ServiceUnavailable = 04_002 => { retryable: true, log_error: false, category: Network },

    // ====== Auth errors (05_xxx) ======
    /// Authentication failed (invalid credentials).
    AuthenticationFailed = 05_000 => { retryable: false, log_error: false, category: Auth },
    /// Permission denied.
    PermissionDenied = 05_001 => { retryable: false, log_error: false, category: Auth },
    /// Access denied.
    AccessDenied = 05_002 => { retryable: false, log_error: false, category: Auth },
    /// Auth header not found.
    AuthHeaderNotFound = 05_003 => { retryable: false, log_error: false, category: Auth },
    /// Invalid auth header.
    InvalidAuthHeader = 05_004 => { retryable: false, log_error: false, category: Auth },

    // ====== Config errors (10_xxx) ======
    /// Invalid configuration.
    InvalidConfig = 10_000 => { retryable: false, log_error: false, category: Config },
    /// Config file error.
    ConfigFileError = 10_001 => { retryable: false, log_error: false, category: Config },

    // ====== Provider/Model errors (11_xxx) ======
    /// Provider not found or not configured.
    ProviderNotFound = 11_000 => { retryable: false, log_error: false, category: Provider },
    /// Model not found or not available.
    ModelNotFound = 11_001 => { retryable: false, log_error: false, category: Provider },
    /// Unsupported capability for this provider/model.
    UnsupportedCapability = 11_002 => { retryable: false, log_error: false, category: Provider },
    /// Context window exceeded.
    ContextWindowExceeded = 11_003 => { retryable: false, log_error: false, category: Provider },
    /// Provider returned an error.
    ProviderError = 11_004 => { retryable: false, log_error: true, category: Provider },
    /// Streaming error.
    StreamError = 11_005 => { retryable: true, log_error: true, category: Provider },

    // ====== Resource/Limit errors (12_xxx) ======
    /// Rate limit exceeded.
    RateLimited = 12_000 => { retryable: true, log_error: false, category: Resource },
    /// Quota/Usage limit exceeded.
    QuotaExceeded = 12_001 => { retryable: false, log_error: false, category: Resource },
    /// Runtime resources exhausted.
    ResourcesExhausted = 12_002 => { retryable: true, log_error: false, category: Resource },
    /// Request timeout.
    Timeout = 12_003 => { retryable: true, log_error: false, category: Resource },
    /// Deadline exceeded.
    DeadlineExceeded = 12_004 => { retryable: false, log_error: false, category: Resource },
}

impl StatusCode {
    /// Returns true if `code` is success.
    pub fn is_success(code: i32) -> bool {
        Self::Success as i32 == code
    }

    /// Returns true if the error is retryable.
    pub const fn is_retryable(&self) -> bool {
        self.meta().retryable
    }

    /// Returns true if the error should be logged.
    pub const fn should_log_error(&self) -> bool {
        self.meta().log_error
    }

    /// Returns the category of this status code.
    pub const fn category(&self) -> StatusCategory {
        self.meta().category
    }

    /// Convert from i32.
    pub fn from_i32(value: i32) -> Option<Self> {
        Self::from_repr(value)
    }
}

impl std::fmt::Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_status_code_values() {
        // General categories (01-05)
        assert_eq!(StatusCode::Success as i32, 00_000);
        assert_eq!(StatusCode::Unknown as i32, 01_000);
        assert_eq!(StatusCode::InvalidArguments as i32, 02_000);
        assert_eq!(StatusCode::IoError as i32, 03_000);
        assert_eq!(StatusCode::NetworkError as i32, 04_000);
        assert_eq!(StatusCode::AuthenticationFailed as i32, 05_000);

        // Business categories (10-12)
        assert_eq!(StatusCode::InvalidConfig as i32, 10_000);
        assert_eq!(StatusCode::ProviderNotFound as i32, 11_000);
        assert_eq!(StatusCode::RateLimited as i32, 12_000);
    }

    #[test]
    fn test_is_success() {
        assert!(StatusCode::is_success(0));
        assert!(!StatusCode::is_success(01_000));
    }

    #[test]
    fn test_is_retryable() {
        assert!(StatusCode::NetworkError.is_retryable());
        assert!(StatusCode::RateLimited.is_retryable());
        assert!(StatusCode::Timeout.is_retryable());
        assert!(!StatusCode::InvalidArguments.is_retryable());
        assert!(!StatusCode::AuthenticationFailed.is_retryable());
    }

    #[test]
    fn test_should_log_error() {
        assert!(StatusCode::Unknown.should_log_error());
        assert!(StatusCode::Internal.should_log_error());
        assert!(!StatusCode::InvalidArguments.should_log_error());
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", StatusCode::Success), "Success");
        assert_eq!(format!("{}", StatusCode::NetworkError), "NetworkError");
    }

    #[test]
    fn test_name() {
        assert_eq!(StatusCode::Success.name(), "Success");
        assert_eq!(StatusCode::NetworkError.name(), "NetworkError");
        assert_eq!(StatusCode::InvalidArguments.name(), "InvalidArguments");
        assert_eq!(
            StatusCode::AuthenticationFailed.name(),
            "AuthenticationFailed"
        );
    }

    #[test]
    fn test_category() {
        // General categories
        assert_eq!(StatusCode::Success.category(), StatusCategory::Success);
        assert_eq!(StatusCode::Unknown.category(), StatusCategory::Common);
        assert_eq!(
            StatusCode::InvalidArguments.category(),
            StatusCategory::Input
        );
        assert_eq!(StatusCode::IoError.category(), StatusCategory::IO);
        assert_eq!(StatusCode::NetworkError.category(), StatusCategory::Network);
        assert_eq!(
            StatusCode::AuthenticationFailed.category(),
            StatusCategory::Auth
        );

        // Business categories
        assert_eq!(StatusCode::InvalidConfig.category(), StatusCategory::Config);
        assert_eq!(
            StatusCode::ProviderNotFound.category(),
            StatusCategory::Provider
        );
        assert_eq!(StatusCode::RateLimited.category(), StatusCategory::Resource);
    }

    #[test]
    fn test_metadata_consistency() {
        for code in StatusCode::iter() {
            let meta = code.meta();
            let value = code as i32;

            // Verify category matches code range (XX_YYY format)
            match meta.category {
                StatusCategory::Success => assert_eq!(value, 0),
                StatusCategory::Common => assert!((01_000..02_000).contains(&value)),
                StatusCategory::Input => assert!((02_000..03_000).contains(&value)),
                StatusCategory::IO => assert!((03_000..04_000).contains(&value)),
                StatusCategory::Network => assert!((04_000..05_000).contains(&value)),
                StatusCategory::Auth => assert!((05_000..06_000).contains(&value)),
                StatusCategory::Config => assert!((10_000..11_000).contains(&value)),
                StatusCategory::Provider => assert!((11_000..12_000).contains(&value)),
                StatusCategory::Resource => assert!((12_000..13_000).contains(&value)),
            }
        }
    }

    #[test]
    fn test_retryable_rules() {
        // All network errors should be retryable
        assert!(StatusCode::NetworkError.is_retryable());
        assert!(StatusCode::ConnectionFailed.is_retryable());
        assert!(StatusCode::ServiceUnavailable.is_retryable());

        // Rate limits and timeouts should be retryable
        assert!(StatusCode::RateLimited.is_retryable());
        assert!(StatusCode::Timeout.is_retryable());
        assert!(StatusCode::ResourcesExhausted.is_retryable());

        // Internal errors might be transient
        assert!(StatusCode::Internal.is_retryable());

        // Stream errors can be retried
        assert!(StatusCode::StreamError.is_retryable());

        // Auth errors should NOT be retryable
        assert!(!StatusCode::AuthenticationFailed.is_retryable());
        assert!(!StatusCode::PermissionDenied.is_retryable());

        // Input errors should NOT be retryable
        assert!(!StatusCode::InvalidArguments.is_retryable());
        assert!(!StatusCode::InvalidConfig.is_retryable());

        // QuotaExceeded should NOT be retryable (needs user action)
        assert!(!StatusCode::QuotaExceeded.is_retryable());
    }

    #[test]
    fn test_log_error_rules() {
        // Unexpected errors should be logged
        assert!(StatusCode::Unknown.should_log_error());
        assert!(StatusCode::Internal.should_log_error());
        assert!(StatusCode::External.should_log_error());

        // Provider errors should be logged for debugging
        assert!(StatusCode::ProviderError.should_log_error());
        assert!(StatusCode::StreamError.should_log_error());

        // User errors should NOT be logged
        assert!(!StatusCode::InvalidArguments.should_log_error());
        assert!(!StatusCode::AuthenticationFailed.should_log_error());
    }
}
