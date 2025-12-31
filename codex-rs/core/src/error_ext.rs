//! Error conversions for extension modules.
//!
//! This module contains error type conversions for optional/extension
//! features like retrieval, keeping them separate from core error.rs
//! to minimize invasive changes during upstream syncs.

use crate::error::CodexErr;

impl From<codex_retrieval::RetrievalErr> for CodexErr {
    fn from(err: codex_retrieval::RetrievalErr) -> Self {
        CodexErr::Fatal(err.to_string())
    }
}
