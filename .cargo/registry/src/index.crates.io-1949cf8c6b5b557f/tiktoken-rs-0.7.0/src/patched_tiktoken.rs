use super::vendor_tiktoken::*;
use anyhow::anyhow;
use anyhow::Result;
use fancy_regex::Regex;
use rustc_hash::FxHashMap as HashMap;

/// Rust API
impl CoreBPE {
    // ====================
    // Encoding
    // ====================

    // This function a copy of the similar function in python API, but it return
    // Rust's results and errors
    pub fn new(
        encoder: HashMap<Vec<u8>, Rank>,
        special_tokens_encoder: HashMap<String, Rank>,
        pattern: &str,
    ) -> Result<Self> {
        let regex = Regex::new(pattern)?;

        let special_regex = {
            let parts = special_tokens_encoder
                .keys()
                .map(|s| fancy_regex::escape(s))
                .collect::<Vec<_>>();
            Regex::new(&parts.join("|"))?
        };

        let decoder: HashMap<Rank, Vec<u8>> =
            encoder.iter().map(|(k, v)| (*v, k.clone())).collect();

        assert!(
            encoder.len() == decoder.len(),
            "Encoder and decoder must be of equal length; maybe you had duplicate token indices in your encoder?"
        );

        let special_tokens_decoder: HashMap<Rank, Vec<u8>> = special_tokens_encoder
            .iter()
            .map(|(k, v)| (*v, k.as_bytes().to_vec()))
            .collect();

        // Clone because I don't know how to tell Rust I'm not going to change the map
        let mut sorted_token_bytes: Vec<Vec<u8>> = encoder.keys().cloned().collect();
        sorted_token_bytes.sort();

        Ok(Self {
            encoder,
            special_tokens_encoder,
            decoder,
            special_tokens_decoder,
            regex_tls: (0..MAX_NUM_THREADS).map(|_| regex.clone()).collect(),
            special_regex_tls: (0..MAX_NUM_THREADS)
                .map(|_| special_regex.clone())
                .collect(),
            sorted_token_bytes,
        })
    }

    // ====================
    // Decoding
    // ====================

    /// Decode a vector of tokens into a valid UTF-8 String
    ///
    /// If unicode validation is not wanted, see _decode_native.
    pub fn decode(&self, tokens: Vec<Rank>) -> Result<String> {
        match String::from_utf8(self.decode_bytes(&tokens)?) {
            Ok(text) => Ok(text),
            Err(e) => Err(anyhow!("Unable to decode into a valid UTF-8 string: {}", e)),
        }
    }

    pub fn _decode_native_and_split(
        &self,
        tokens: Vec<Rank>,
    ) -> impl Iterator<Item = Vec<u8>> + '_ {
        tokens.into_iter().map(|token| {
            let token_bytes = self
                .decoder
                .get(&token)
                .unwrap_or_else(|| &self.special_tokens_decoder[&token]);
            token_bytes.clone()
        })
    }

    /// Tokenize a string and return the decoded tokens using the correct BPE model.
    ///
    /// This method takes a string, encodes it using the BPE model, and decodes the encoded tokens into
    /// a vector of strings. It can be used to tokenize a string and return the decoded tokens using the
    /// correct BPE model.
    ///
    /// # Examples
    ///
    /// ```
    ///     use tiktoken_rs::cl100k_base;
    ///     let bpe = cl100k_base().unwrap();
    ///     let tokenized: Result<Vec<_>, _> = bpe
    ///         .split_by_token("This is a test         with a lot of spaces", true);
    ///     let tokenized = tokenized.unwrap();
    ///     assert_eq!(
    ///         tokenized,
    ///         vec!["This", " is", " a", " test", "        ", " with", " a", " lot", " of", " spaces"]
    ///     );
    /// ```
    ///
    /// # Arguments
    ///
    /// * text: A string slice containing the text to be tokenized.
    /// * use_special_tokens: A boolean indicating whether to use the special tokens in the BPE model.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<String>>`: A Result containing a vector of decoded tokens as strings, or an error
    ///   if the string cannot be converted into a valid UTF-8 string.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    ///
    /// * The input text cannot be converted into a valid UTF-8 string during the decoding process.
    ///
    pub fn split_by_token<'a>(
        &'a self,
        text: &'a str,
        use_special_tokens: bool,
    ) -> Result<Vec<String>> {
        self.split_by_token_iter(text, use_special_tokens).collect()
    }

    /// Iterator for decoding and splitting a String.
    /// See `split_by_token` for more details.
    pub fn split_by_token_iter<'a>(
        &'a self,
        text: &'a str,
        use_special_tokens: bool,
    ) -> impl Iterator<Item = Result<String>> + 'a {
        // First, encode the text using the BPE model
        let encoded = match use_special_tokens {
            true => self.encode_with_special_tokens(text),
            false => self.encode_ordinary(text),
        };

        self._decode_native_and_split(encoded).map(|token| {
            // Map each token to a Result<String>
            Ok(String::from_utf8_lossy(token.as_slice()).to_string())
        })
    }

    /// Tokenize a string and return the decoded tokens using the correct BPE model.
    /// This method is equivalent to `split_by_token(text, false)`.
    pub fn split_by_token_ordinary<'a>(&'a self, text: &'a str) -> Result<Vec<String>> {
        self.split_by_token(text, false)
    }

    /// Iterator for decoding and splitting a String.
    /// This method is equivalent to `split_by_token_iter(text, false)`.
    pub fn split_by_token_ordinary_iter<'a>(
        &'a self,
        text: &'a str,
    ) -> impl Iterator<Item = Result<String>> + 'a {
        self.split_by_token_iter(text, false)
    }
}
