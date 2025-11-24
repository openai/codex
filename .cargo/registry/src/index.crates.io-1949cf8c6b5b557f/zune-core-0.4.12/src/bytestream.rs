/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! A simple implementation of a bytestream reader
//! and writer.
//!
//! This module contains two main structs that help in
//! byte reading and byte writing
//!
//! Useful for a lot of image readers and writers, it's put
//! here to minimize code reuse
pub use reader::ZByteReader;
pub use traits::*;
pub use writer::ZByteWriter;

mod reader;
mod traits;
mod writer;
