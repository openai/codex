use lazy_static::lazy_static;

use crate::vendor_tiktoken::CoreBPE;

use crate::{cl100k_base, o200k_base, p50k_base, p50k_edit, r50k_base};

/// Returns a singleton instance of the r50k_base tokenizer. (also known as `gpt2`)
/// Use for GPT-3 models like `davinci`
///
/// This function will only initialize the tokenizer once, and then return a reference the tokenizer
pub fn r50k_base_singleton() -> &'static CoreBPE {
    lazy_static! {
        static ref R50K_BASE: CoreBPE = r50k_base().unwrap();
    }
    &R50K_BASE
}

/// Returns a singleton instance of the p50k_base tokenizer.
/// Use for Code models, `text-davinci-002`, `text-davinci-003`
///
/// This function will only initialize the tokenizer once, and then return a reference the tokenizer.
pub fn p50k_base_singleton() -> &'static CoreBPE {
    lazy_static! {
        static ref P50K_BASE: CoreBPE = p50k_base().unwrap();
    }
    &P50K_BASE
}

/// Returns a singleton instance of the p50k_edit tokenizer.
/// Use for edit models like `text-davinci-edit-001`, `code-davinci-edit-001`
///
/// This function will only initialize the tokenizer once, and then return a reference the tokenizer.
pub fn p50k_edit_singleton() -> &'static CoreBPE {
    lazy_static! {
        static ref P50K_EDIT: CoreBPE = p50k_edit().unwrap();
    }
    &P50K_EDIT
}

/// Returns a singleton instance of the cl100k_base tokenizer.
/// Use for ChatGPT models, `text-embedding-ada-002`
///
/// This function will only initialize the tokenizer once, and then return a reference the tokenizer
pub fn cl100k_base_singleton() -> &'static CoreBPE {
    lazy_static! {
        static ref CL100K_BASE: CoreBPE = cl100k_base().unwrap();
    }
    &CL100K_BASE
}

/// Returns a singleton instance of the o200k_base tokenizer.
/// Use for GPT-4o models.
///
/// This function will only initialize the tokenizer once, and then return a reference the tokenizer
pub fn o200k_base_singleton() -> &'static CoreBPE {
    lazy_static! {
        static ref O200K_BASE: CoreBPE = o200k_base().unwrap();
    }
    &O200K_BASE
}
