/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Decoding results for images
use alloc::vec::Vec;

/// A simple enum that can hold decode
/// results of most images
#[non_exhaustive]
pub enum DecodingResult {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>)
}

impl DecodingResult {
    /// Return the contents if the enum stores `Vec<u8>` or otherwise
    /// return `None`.
    ///
    /// Useful for de-sugaring the result of a decoding operation
    /// into raw bytes
    ///
    /// # Example
    /// ```
    /// use zune_core::result::DecodingResult;
    /// let data = DecodingResult::U8(vec![0;100]);
    /// // we know this won't fail because we created it with u8
    /// assert!(data.u8().is_some());
    ///
    /// let data = DecodingResult::U16(vec![0;100]);
    /// // it should now return nothing since the type is u18
    /// assert!(data.u8().is_none());
    ///
    /// ```
    pub fn u8(self) -> Option<Vec<u8>> {
        match self {
            DecodingResult::U8(data) => Some(data),
            _ => None
        }
    }

    /// Return the contents if the enum stores `Vec<u16>` or otherwise
    /// return `None`.
    ///
    /// Useful for de-sugaring the result of a decoding operation
    /// into raw bytes
    ///
    /// # Example
    /// ```
    /// use zune_core::result::DecodingResult;
    /// let data = DecodingResult::U8(vec![0;100]);
    /// // we know this will fail because we created it with u16
    /// assert!(data.u16().is_none());
    ///
    ///
    /// let data = DecodingResult::U16(vec![0;100]);
    /// // it should now return something since the type is u16
    /// assert!(data.u16().is_some());
    ///
    /// ```
    pub fn u16(self) -> Option<Vec<u16>> {
        match self {
            DecodingResult::U16(data) => Some(data),
            _ => None
        }
    }
}
