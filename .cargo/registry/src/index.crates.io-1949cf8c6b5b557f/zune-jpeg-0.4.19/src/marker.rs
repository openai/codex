/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![allow(clippy::upper_case_acronyms)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    /// Start Of Frame markers
    ///
    /// - SOF(0):  Baseline DCT (Huffman coding)
    /// - SOF(1):  Extended sequential DCT (Huffman coding)
    /// - SOF(2):  Progressive DCT (Huffman coding)
    /// - SOF(3):  Lossless (sequential) (Huffman coding)
    /// - SOF(5):  Differential sequential DCT (Huffman coding)
    /// - SOF(6):  Differential progressive DCT (Huffman coding)
    /// - SOF(7):  Differential lossless (sequential) (Huffman coding)
    /// - SOF(9):  Extended sequential DCT (arithmetic coding)
    /// - SOF(10): Progressive DCT (arithmetic coding)
    /// - SOF(11): Lossless (sequential) (arithmetic coding)
    /// - SOF(13): Differential sequential DCT (arithmetic coding)
    /// - SOF(14): Differential progressive DCT (arithmetic coding)
    /// - SOF(15): Differential lossless (sequential) (arithmetic coding)
    SOF(u8),
    /// Define Huffman table(s)
    DHT,
    /// Define arithmetic coding conditioning(s)
    DAC,
    /// Restart with modulo 8 count `m`
    RST(u8),
    /// Start of image
    SOI,
    /// End of image
    EOI,
    /// Start of scan
    SOS,
    /// Define quantization table(s)
    DQT,
    /// Define number of lines
    DNL,
    /// Define restart interval
    DRI,
    /// Reserved for application segments
    APP(u8),
    /// Comment
    COM
}

impl Marker {
    pub fn from_u8(n: u8) -> Option<Marker> {
        use self::Marker::{APP, COM, DAC, DHT, DNL, DQT, DRI, EOI, RST, SOF, SOI, SOS};

        match n {
            0xFE => Some(COM),
            0xC0 => Some(SOF(0)),
            0xC1 => Some(SOF(1)),
            0xC2 => Some(SOF(2)),
            0xC4 => Some(DHT),
            0xCC => Some(DAC),
            0xD0 => Some(RST(0)),
            0xD1 => Some(RST(1)),
            0xD2 => Some(RST(2)),
            0xD3 => Some(RST(3)),
            0xD4 => Some(RST(4)),
            0xD5 => Some(RST(5)),
            0xD6 => Some(RST(6)),
            0xD7 => Some(RST(7)),
            0xD8 => Some(SOI),
            0xD9 => Some(EOI),
            0xDA => Some(SOS),
            0xDB => Some(DQT),
            0xDC => Some(DNL),
            0xDD => Some(DRI),
            0xE0 => Some(APP(0)),
            0xE1 => Some(APP(1)),
            0xE2 => Some(APP(2)),
            0xEE => Some(APP(14)),
            _ => None
        }
    }
}
