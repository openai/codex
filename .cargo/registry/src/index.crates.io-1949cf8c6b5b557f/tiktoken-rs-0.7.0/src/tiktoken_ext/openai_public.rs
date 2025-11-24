pub const ENDOFTEXT: &str = "<|endoftext|>";
pub const FIM_PREFIX: &str = "<|fim_prefix|>";
pub const FIM_MIDDLE: &str = "<|fim_middle|>";
pub const FIM_SUFFIX: &str = "<|fim_suffix|>";
pub const ENDOFPROMPT: &str = "<|endofprompt|>";

/// Adaptation of the tiktoken crate for use in Rust projects
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};

use rustc_hash::FxHashMap as HashMap;

use crate::{CoreBPE, Rank};

/// Use for GPT-3 models like `davinci`
/// Initializes and returns a new instance of the r50k_base tokenizer (also known as `gpt2`)
pub fn r50k_base() -> Result<CoreBPE> {
    let bpe_file = include_str!("../../assets/r50k_base.tiktoken");

    let mut encoder = HashMap::default();
    for line in bpe_file.lines() {
        let mut parts = line.split(' ');
        let token = &general_purpose::STANDARD.decode(parts.next().unwrap())?;
        let rank: Rank = parts.next().unwrap().parse().unwrap();
        encoder.insert(token.clone(), rank);
    }

    let mut special_tokens = HashMap::default();
    special_tokens.insert(String::from(ENDOFTEXT), 50256);

    let bpe = CoreBPE::new(
        encoder,
        special_tokens,
        "'s|'t|'re|'ve|'m|'ll|'d| ?\\p{L}+| ?\\p{N}+| ?[^\\s\\p{L}\\p{N}]+|\\s+(?!\\S)|\\s+",
    )?;
    Ok(bpe)
}

/// Use for Code models, `text-davinci-002`, `text-davinci-003`
/// Initializes and returns a new instance of the p50k_base tokenizer.
pub fn p50k_base() -> Result<CoreBPE> {
    let bpe_file = include_str!("../../assets/p50k_base.tiktoken");

    let mut encoder = HashMap::default();
    for line in bpe_file.lines() {
        let mut parts = line.split(' ');
        let raw = parts.next().unwrap();
        let token = &general_purpose::STANDARD.decode(raw)?;
        let rank: Rank = parts.next().unwrap().parse().unwrap();
        encoder.insert(token.clone(), rank);
    }

    let mut special_tokens = HashMap::default();
    special_tokens.insert(String::from(ENDOFTEXT), 50256);

    let bpe = CoreBPE::new(
        encoder,
        special_tokens,
        "'s|'t|'re|'ve|'m|'ll|'d| ?\\p{L}+| ?\\p{N}+| ?[^\\s\\p{L}\\p{N}]+|\\s+(?!\\S)|\\s+",
    )?;
    Ok(bpe)
}

/// Use for edit models like `text-davinci-edit-001`, `code-davinci-edit-001`
/// Initializes and returns a new instance of the p50k_base tokenizer.
pub fn p50k_edit() -> Result<CoreBPE> {
    let bpe_file = include_str!("../../assets/p50k_base.tiktoken");

    let mut encoder = HashMap::default();
    for line in bpe_file.lines() {
        let mut parts = line.split(' ');
        let raw = parts.next().unwrap();
        let token = &general_purpose::STANDARD.decode(raw)?;
        let rank: Rank = parts.next().unwrap().parse().unwrap();
        encoder.insert(token.clone(), rank);
    }

    let mut special_tokens = HashMap::default();
    special_tokens.insert(String::from(ENDOFTEXT), 50256);
    special_tokens.insert(String::from(FIM_PREFIX), 50281);
    special_tokens.insert(String::from(FIM_MIDDLE), 50282);
    special_tokens.insert(String::from(FIM_SUFFIX), 50283);

    let bpe = CoreBPE::new(
        encoder,
        special_tokens,
        "'s|'t|'re|'ve|'m|'ll|'d| ?\\p{L}+| ?\\p{N}+| ?[^\\s\\p{L}\\p{N}]+|\\s+(?!\\S)|\\s+",
    )?;
    Ok(bpe)
}

/// Use for ChatGPT models, `text-embedding-ada-002`
/// Initializes and returns a new instance of the cl100k_base tokenizer.
pub fn cl100k_base() -> Result<CoreBPE> {
    let cl100k_base = include_str!("../../assets/cl100k_base.tiktoken");

    let mut encoder = HashMap::default();
    for line in cl100k_base.lines() {
        let mut parts = line.split(' ');
        let raw = parts.next().unwrap();
        let token = &general_purpose::STANDARD.decode(raw)?;
        let rank: Rank = parts.next().unwrap().parse().unwrap();
        encoder.insert(token.clone(), rank);
    }

    let mut special_tokens = HashMap::default();
    special_tokens.insert(String::from(ENDOFTEXT), 100257);
    special_tokens.insert(String::from(FIM_PREFIX), 100258);
    special_tokens.insert(String::from(FIM_MIDDLE), 100259);
    special_tokens.insert(String::from(FIM_SUFFIX), 100260);
    special_tokens.insert(String::from(ENDOFPROMPT), 100276);

    let bpe = CoreBPE::new(
        encoder,
        special_tokens,
        "(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\\r\\n\\p{L}\\p{N}]?\\p{L}+|\\p{N}{1,3}| ?[^\\s\\p{L}\\p{N}]+[\\r\\n]*|\\s*[\\r\\n]+|\\s+(?!\\S)|\\s+",
    )?;
    Ok(bpe)
}

/// Use for GPT-4o models.
/// Initializes and returns a new instance of the o200k_base tokenizer.
pub fn o200k_base() -> Result<CoreBPE> {
    let o200k_base = include_str!("../../assets/o200k_base.tiktoken");

    let mut encoder: std::collections::HashMap<
        Vec<u8>,
        Rank,
        std::hash::BuildHasherDefault<rustc_hash::FxHasher>,
    > = HashMap::default();
    for line in o200k_base.lines() {
        let mut parts = line.split(' ');
        let raw = parts.next().unwrap();
        let token = &general_purpose::STANDARD.decode(raw)?;
        let rank: Rank = parts.next().unwrap().parse().unwrap();
        encoder.insert(token.clone(), rank);
    }

    let mut special_tokens = HashMap::default();
    special_tokens.insert(String::from(ENDOFTEXT), 199999);
    special_tokens.insert(String::from(ENDOFPROMPT), 200018);

    let bpe = CoreBPE::new(
        encoder,
        special_tokens,
        &[
            "[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]*[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)?",
            "[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]+[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)?",
            "\\p{N}{1,3}",
            " ?[^\\s\\p{L}\\p{N}]+[\\r\\n/]*",
            "\\s*[\\r\\n]+",
            "\\s+(?!\\S)",
            "\\s+",
        ].join("|"),
    )?;
    Ok(bpe)
}
