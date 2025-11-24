// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

macro_rules! decoder_function {
    ($preamble:block,
     $loop_preable:block,
     $eof:block,
     $body:block,
     $slf:ident,
     $src_consumed:ident,
     $dest:ident,
     $source:ident,
     $b:ident,
     $destination_handle:ident,
     $unread_handle:ident,
     $destination_check:ident,
     $name:ident,
     $code_unit:ty,
     $dest_struct:ident) => (
    pub fn $name(&mut $slf,
                 src: &[u8],
                 dst: &mut [$code_unit],
                 last: bool)
                 -> (DecoderResult, usize, usize) {
        let mut $source = ByteSource::new(src);
        let mut $dest = $dest_struct::new(dst);
        loop { // TODO: remove this loop
            {
                // Start non-boilerplate
                $preamble
                // End non-boilerplate
            }
            loop {
                {
                    $loop_preable
                }
                match $source.check_available() {
                    Space::Full($src_consumed) => {
                        if last {
                            // Start non-boilerplate
                            $eof
                            // End non-boilerplate
                        }
                        return (DecoderResult::InputEmpty, $src_consumed, $dest.written());
                    }
                    Space::Available(source_handle) => {
                        match $dest.$destination_check() {
                            Space::Full(dst_written) => {
                                return (DecoderResult::OutputFull,
                                        source_handle.consumed(),
                                        dst_written);
                            }
                            Space::Available($destination_handle) => {
                                let ($b, $unread_handle) = source_handle.read();
                                // Start non-boilerplate
                                $body
                                // End non-boilerplate
                            }
                        }
                    }
                }
            }
        }
    });
}

macro_rules! decoder_functions {
    (
        $preamble:block,
        $loop_preable:block,
        $eof:block,
        $body:block,
        $slf:ident,
        $src_consumed:ident,
        $dest:ident,
        $source:ident,
        $b:ident,
        $destination_handle:ident,
        $unread_handle:ident,
        $destination_check:ident
    ) => {
        decoder_function!(
            $preamble,
            $loop_preable,
            $eof,
            $body,
            $slf,
            $src_consumed,
            $dest,
            $source,
            $b,
            $destination_handle,
            $unread_handle,
            $destination_check,
            decode_to_utf8_raw,
            u8,
            Utf8Destination
        );
        decoder_function!(
            $preamble,
            $loop_preable,
            $eof,
            $body,
            $slf,
            $src_consumed,
            $dest,
            $source,
            $b,
            $destination_handle,
            $unread_handle,
            $destination_check,
            decode_to_utf16_raw,
            u16,
            Utf16Destination
        );
    };
}

macro_rules! ascii_compatible_two_byte_decoder_function {
    ($lead:block,
     $trail:block,
     $slf:ident,
     $non_ascii:ident,
     $byte:ident,
     $lead_minus_offset:ident,
     $unread_handle_trail:ident,
     $source:ident,
     $handle:ident,
     $outermost:tt,
     $copy_ascii:ident,
     $destination_check:ident,
     $name:ident,
     $code_unit:ty,
     $dest_struct:ident,
     $ascii_punctuation:expr) => (
    pub fn $name(&mut $slf,
                 src: &[u8],
                 dst: &mut [$code_unit],
                 last: bool)
                 -> (DecoderResult, usize, usize) {
        let mut $source = ByteSource::new(src);
        let mut dest_prolog = $dest_struct::new(dst);
        let dest = match $slf.lead {
            Some(lead) => {
                let $lead_minus_offset = lead;
                $slf.lead = None;
                // Since we don't have `goto` we could use to jump into the trail
                // handling part of the main loop, we need to repeat trail handling
                // here.
                match $source.check_available() {
                    Space::Full(src_consumed_prolog) => {
                        if last {
                            return (DecoderResult::Malformed(1, 0),
                                    src_consumed_prolog,
                                    dest_prolog.written());
                        }
                        return (DecoderResult::InputEmpty, src_consumed_prolog, dest_prolog.written());
                    }
                    Space::Available(source_handle_prolog) => {
                        match dest_prolog.$destination_check() {
                            Space::Full(dst_written_prolog) => {
                                return (DecoderResult::OutputFull,
                                        source_handle_prolog.consumed(),
                                        dst_written_prolog);
                            }
                            Space::Available($handle) => {
                                let ($byte, $unread_handle_trail) = source_handle_prolog.read();
                                // Start non-boilerplate
                                $trail
                                // End non-boilerplate
                            }
                        }
                    }
                }
            },
            None => {
                &mut dest_prolog
            }
        };
        $outermost: loop {
            match dest.$copy_ascii(&mut $source) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((mut $non_ascii, mut $handle)) => {
                    'middle: loop {
                        let dest_again = {
                            let $lead_minus_offset = {
                                // Start non-boilerplate
                                $lead
                                // End non-boilerplate
                            };
                            match $source.check_available() {
                                Space::Full(src_consumed_trail) => {
                                    if last {
                                        return (DecoderResult::Malformed(1, 0),
                                                src_consumed_trail,
                                                $handle.written());
                                    }
                                    $slf.lead = Some($lead_minus_offset);
                                    return (DecoderResult::InputEmpty,
                                            src_consumed_trail,
                                            $handle.written());
                                }
                                Space::Available(source_handle_trail) => {
                                    let ($byte, $unread_handle_trail) = source_handle_trail.read();
                                    // Start non-boilerplate
                                    $trail
                                    // End non-boilerplate
                                }
                            }
                        };
                        match $source.check_available() {
                            Space::Full(src_consumed) => {
                                return (DecoderResult::InputEmpty,
                                        src_consumed,
                                        dest_again.written());
                            }
                            Space::Available(source_handle) => {
                                match dest_again.$destination_check() {
                                    Space::Full(dst_written) => {
                                        return (DecoderResult::OutputFull,
                                                source_handle.consumed(),
                                                dst_written);
                                    }
                                    Space::Available(mut destination_handle) => {
                                        let (mut b, unread_handle) = source_handle.read();
                                        let source_again = unread_handle.commit();
                                        'innermost: loop {
                                            if b > 127 {
                                                $non_ascii = b;
                                                $handle = destination_handle;
                                                continue 'middle;
                                            }
                                            // Testing on Haswell says that we should write the
                                            // byte unconditionally instead of trying to unread it
                                            // to make it part of the next SIMD stride.
                                            let dest_again_again =
                                                destination_handle.write_ascii(b);
                                            if $ascii_punctuation && b < 60 {
                                                // We've got punctuation
                                                match source_again.check_available() {
                                                    Space::Full(src_consumed_again) => {
                                                        return (DecoderResult::InputEmpty,
                                                                src_consumed_again,
                                                                dest_again_again.written());
                                                    }
                                                    Space::Available(source_handle_again) => {
                                                        match dest_again_again.$destination_check() {
                                                            Space::Full(dst_written_again) => {
                                                                return (DecoderResult::OutputFull,
                                                                        source_handle_again.consumed(),
                                                                        dst_written_again);
                                                            }
                                                            Space::Available(destination_handle_again) => {
                                                                {
                                                                    let (b_again, _unread_handle_again) =
                                                                        source_handle_again.read();
                                                                    b = b_again;
                                                                    destination_handle = destination_handle_again;
                                                                    continue 'innermost;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            // We've got markup or ASCII text
                                            continue $outermost;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

macro_rules! ascii_compatible_two_byte_decoder_functions {
    (
        $lead:block,
        $trail:block,
        $slf:ident,
        $non_ascii:ident,
        $byte:ident,
        $lead_minus_offset:ident,
        $unread_handle_trail:ident,
        $source:ident,
        $handle:ident,
        $outermost:tt,
        $copy_ascii:ident,
        $destination_check:ident,
        $ascii_punctuation:expr
    ) => {
        ascii_compatible_two_byte_decoder_function!(
            $lead,
            $trail,
            $slf,
            $non_ascii,
            $byte,
            $lead_minus_offset,
            $unread_handle_trail,
            $source,
            $handle,
            $outermost,
            $copy_ascii,
            $destination_check,
            decode_to_utf8_raw,
            u8,
            Utf8Destination,
            $ascii_punctuation
        );
        ascii_compatible_two_byte_decoder_function!(
            $lead,
            $trail,
            $slf,
            $non_ascii,
            $byte,
            $lead_minus_offset,
            $unread_handle_trail,
            $source,
            $handle,
            $outermost,
            $copy_ascii,
            $destination_check,
            decode_to_utf16_raw,
            u16,
            Utf16Destination,
            $ascii_punctuation
        );
    };
}

macro_rules! gb18030_decoder_function {
    ($first_body:block,
     $second_body:block,
     $third_body:block,
     $fourth_body:block,
     $slf:ident,
     $non_ascii:ident,
     $first_minus_offset:ident,
     $second:ident,
     $second_minus_offset:ident,
     $unread_handle_second:ident,
     $third:ident,
     $third_minus_offset:ident,
     $unread_handle_third:ident,
     $fourth:ident,
     $fourth_minus_offset:ident,
     $unread_handle_fourth:ident,
     $source:ident,
     $handle:ident,
     $outermost:tt,
     $name:ident,
     $code_unit:ty,
     $dest_struct:ident) => (
    #[cfg_attr(feature = "cargo-clippy", allow(never_loop))]
    pub fn $name(&mut $slf,
                 src: &[u8],
                 dst: &mut [$code_unit],
                 last: bool)
                 -> (DecoderResult, usize, usize) {
        let mut $source = ByteSource::new(src);
        let mut dest = $dest_struct::new(dst);
        {
            if let Some(ascii) = $slf.pending_ascii {
                match dest.check_space_bmp() {
                    Space::Full(_) => {
                        return (DecoderResult::OutputFull, 0, 0);
                    }
                    Space::Available(pending_ascii_handle) => {
                        $slf.pending_ascii = None;
                        pending_ascii_handle.write_ascii(ascii);
                    }
                }
            }
        }
        while !$slf.pending.is_none() {
            match $source.check_available() {
                Space::Full(src_consumed) => {
                    if last {
                        // Start non-boilerplate
                        let count = $slf.pending.count();
                        $slf.pending = Gb18030Pending::None;
                        return (DecoderResult::Malformed(count as u8, 0),
                                src_consumed,
                                dest.written());
                        // End non-boilerplate
                    }
                    return (DecoderResult::InputEmpty, src_consumed, dest.written());
                }
                Space::Available(source_handle) => {
                    match dest.check_space_astral() {
                        Space::Full(dst_written) => {
                            return (DecoderResult::OutputFull,
                                    source_handle.consumed(),
                                    dst_written);
                        }
                        Space::Available($handle) => {
                            let (byte, unread_handle) = source_handle.read();
                            match $slf.pending {
                                Gb18030Pending::One($first_minus_offset) => {
                                    $slf.pending = Gb18030Pending::None;
                                    let $second = byte;
                                    let $unread_handle_second = unread_handle;
                                    // If second is between 0x40 and 0x7E,
                                    // inclusive, subtract offset 0x40. Else if
                                    // second is between 0x80 and 0xFE, inclusive,
                                    // subtract offset 0x41. In both cases,
                                    // handle as a two-byte sequence.
                                    // Else if second is between 0x30 and 0x39,
                                    // inclusive, subtract offset 0x30 and
                                    // handle as a four-byte sequence.
                                    let $second_minus_offset = $second.wrapping_sub(0x30);
                                    // It's not optimal to do this check first,
                                    // but this results in more readable code.
                                    if $second_minus_offset > (0x39 - 0x30) {
                                        // Start non-boilerplate
                                        $second_body
                                        // End non-boilerplate
                                    } else {
                                        // Four-byte!
                                        $slf.pending = Gb18030Pending::Two($first_minus_offset,
                                                                           $second_minus_offset);
                                        $handle.commit()
                                    }
                                }
                                Gb18030Pending::Two($first_minus_offset, $second_minus_offset) => {
                                    $slf.pending = Gb18030Pending::None;
                                    let $third = byte;
                                    let $unread_handle_third = unread_handle;
                                    let $third_minus_offset = {
                                        // Start non-boilerplate
                                        $third_body
                                        // End non-boilerplate
                                    };
                                    $slf.pending = Gb18030Pending::Three($first_minus_offset,
                                                                         $second_minus_offset,
                                                                         $third_minus_offset);
                                    $handle.commit()
                                }
                                Gb18030Pending::Three($first_minus_offset,
                                                      $second_minus_offset,
                                                      $third_minus_offset) => {
                                    $slf.pending = Gb18030Pending::None;
                                    let $fourth = byte;
                                    let $unread_handle_fourth = unread_handle;
                                    // Start non-boilerplate
                                    $fourth_body
                                    // End non-boilerplate
                                }
                                Gb18030Pending::None => unreachable!("Checked in loop condition"),
                            };
                        }
                    }
                }
            }
        }
        $outermost: loop {
            match dest.copy_ascii_from_check_space_astral(&mut $source) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((mut $non_ascii, mut $handle)) => {
                    'middle: loop {
                        let dest_again = {
                            let $first_minus_offset = {
                                // Start non-boilerplate
                                $first_body
                                // End non-boilerplate
                            };
                            match $source.check_available() {
                                Space::Full(src_consumed_trail) => {
                                    if last {
                                        return (DecoderResult::Malformed(1, 0),
                                                src_consumed_trail,
                                                $handle.written());
                                    }
                                    $slf.pending = Gb18030Pending::One($first_minus_offset);
                                    return (DecoderResult::InputEmpty,
                                            src_consumed_trail,
                                            $handle.written());
                                }
                                Space::Available(source_handle_trail) => {
                                    let ($second, $unread_handle_second) = source_handle_trail.read();
                                    // Start non-boilerplate
                                    // If second is between 0x40 and 0x7E,
                                    // inclusive, subtract offset 0x40. Else if
                                    // second is between 0x80 and 0xFE, inclusive,
                                    // subtract offset 0x41. In both cases,
                                    // handle as a two-byte sequence.
                                    // Else if second is between 0x30 and 0x39,
                                    // inclusive, subtract offset 0x30 and
                                    // handle as a four-byte sequence.
                                    let $second_minus_offset = $second.wrapping_sub(0x30);
                                    // It's not optimal to do this check first,
                                    // but this results in more readable code.
                                    if $second_minus_offset > (0x39 - 0x30) {
                                        // Start non-boilerplate
                                        $second_body
                                        // End non-boilerplate
                                    } else {
                                        // Four-byte!
                                        match $unread_handle_second.commit().check_available() {
                                            Space::Full(src_consumed_third) => {
                                                if last {
                                                    return (DecoderResult::Malformed(2, 0),
                                                            src_consumed_third,
                                                            $handle.written());
                                                }
                                                $slf.pending =
                                                    Gb18030Pending::Two($first_minus_offset,
                                                                        $second_minus_offset);
                                                return (DecoderResult::InputEmpty,
                                                        src_consumed_third,
                                                        $handle.written());
                                            }
                                            Space::Available(source_handle_third) => {
                                                let ($third, $unread_handle_third) =
                                                    source_handle_third.read();
                                                let $third_minus_offset = {
                                                    // Start non-boilerplate
                                                    $third_body
                                                    // End non-boilerplate
                                                };
                                                match $unread_handle_third.commit()
                                                                         .check_available() {
                                                    Space::Full(src_consumed_fourth) => {
                                                        if last {
                                                            return (DecoderResult::Malformed(3, 0),
                                                                    src_consumed_fourth,
                                                                    $handle.written());
                                                        }
                                                        $slf.pending = Gb18030Pending::Three($first_minus_offset, $second_minus_offset, $third_minus_offset);
                                                        return (DecoderResult::InputEmpty,
                                                                src_consumed_fourth,
                                                                $handle.written());
                                                    }
                                                    Space::Available(source_handle_fourth) => {
                                                        let ($fourth, $unread_handle_fourth) =
                                                            source_handle_fourth.read();
                                                        // Start non-boilerplate
                                                        $fourth_body
                                                        // End non-boilerplate
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // End non-boilerplate
                                }
                            }
                        };
                        match $source.check_available() {
                            Space::Full(src_consumed) => {
                                return (DecoderResult::InputEmpty,
                                        src_consumed,
                                        dest_again.written());
                            }
                            Space::Available(source_handle) => {
                                match dest_again.check_space_astral() {
                                    Space::Full(dst_written) => {
                                        return (DecoderResult::OutputFull,
                                                source_handle.consumed(),
                                                dst_written);
                                    }
                                    Space::Available(destination_handle) => {
                                        let (b, _) = source_handle.read();
                                        loop {
                                            if b > 127 {
                                                $non_ascii = b;
                                                $handle = destination_handle;
                                                continue 'middle;
                                            }
                                            // Testing on Haswell says that we should write the
                                            // byte unconditionally instead of trying to unread it
                                            // to make it part of the next SIMD stride.
                                            destination_handle.write_ascii(b);
                                            // We've got markup or ASCII text
                                            continue $outermost;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

macro_rules! gb18030_decoder_functions {
    (
        $first_body:block,
        $second_body:block,
        $third_body:block,
        $fourth_body:block,
        $slf:ident,
        $non_ascii:ident,
        $first_minus_offset:ident,
        $second:ident,
        $second_minus_offset:ident,
        $unread_handle_second:ident,
        $third:ident,
        $third_minus_offset:ident,
        $unread_handle_third:ident,
        $fourth:ident,
        $fourth_minus_offset:ident,
        $unread_handle_fourth:ident,
        $source:ident,
        $handle:ident,
        $outermost:tt
    ) => {
        gb18030_decoder_function!(
            $first_body,
            $second_body,
            $third_body,
            $fourth_body,
            $slf,
            $non_ascii,
            $first_minus_offset,
            $second,
            $second_minus_offset,
            $unread_handle_second,
            $third,
            $third_minus_offset,
            $unread_handle_third,
            $fourth,
            $fourth_minus_offset,
            $unread_handle_fourth,
            $source,
            $handle,
            $outermost,
            decode_to_utf8_raw,
            u8,
            Utf8Destination
        );
        gb18030_decoder_function!(
            $first_body,
            $second_body,
            $third_body,
            $fourth_body,
            $slf,
            $non_ascii,
            $first_minus_offset,
            $second,
            $second_minus_offset,
            $unread_handle_second,
            $third,
            $third_minus_offset,
            $unread_handle_third,
            $fourth,
            $fourth_minus_offset,
            $unread_handle_fourth,
            $source,
            $handle,
            $outermost,
            decode_to_utf16_raw,
            u16,
            Utf16Destination
        );
    };
}

macro_rules! euc_jp_decoder_function {
    ($jis0802_trail_body:block,
     $jis0812_lead_body:block,
     $jis0812_trail_body:block,
     $half_width_katakana_body:block,
     $slf:ident,
     $non_ascii:ident,
     $jis0208_lead_minus_offset:ident,
     $byte:ident,
     $unread_handle_trail:ident,
     $jis0212_lead_minus_offset:ident,
     $lead:ident,
     $unread_handle_jis0212:ident,
     $source:ident,
     $handle:ident,
     $name:ident,
     $code_unit:ty,
     $dest_struct:ident) => (
    #[cfg_attr(feature = "cargo-clippy", allow(never_loop))]
    pub fn $name(&mut $slf,
                 src: &[u8],
                 dst: &mut [$code_unit],
                 last: bool)
                 -> (DecoderResult, usize, usize) {
        let mut $source = ByteSource::new(src);
        let mut dest = $dest_struct::new(dst);
        while !$slf.pending.is_none() {
            match $source.check_available() {
                Space::Full(src_consumed) => {
                    if last {
                        // Start non-boilerplate
                        let count = $slf.pending.count();
                        $slf.pending = EucJpPending::None;
                        return (DecoderResult::Malformed(count as u8, 0),
                                src_consumed,
                                dest.written());
                        // End non-boilerplate
                    }
                    return (DecoderResult::InputEmpty, src_consumed, dest.written());
                }
                Space::Available(source_handle) => {
                    match dest.check_space_bmp() {
                        Space::Full(dst_written) => {
                            return (DecoderResult::OutputFull,
                                    source_handle.consumed(),
                                    dst_written);
                        }
                        Space::Available($handle) => {
                            let ($byte, $unread_handle_trail) = source_handle.read();
                            match $slf.pending {
                                EucJpPending::Jis0208Lead($jis0208_lead_minus_offset) => {
                                    $slf.pending = EucJpPending::None;
                                    // Start non-boilerplate
                                    $jis0802_trail_body
                                    // End non-boilerplate
                                }
                                EucJpPending::Jis0212Shift => {
                                    $slf.pending = EucJpPending::None;
                                    let $lead = $byte;
                                    let $unread_handle_jis0212 = $unread_handle_trail;
                                    let $jis0212_lead_minus_offset = {
                                        // Start non-boilerplate
                                        $jis0812_lead_body
                                        // End non-boilerplate
                                    };
                                    $slf.pending =
                                        EucJpPending::Jis0212Lead($jis0212_lead_minus_offset);
                                    $handle.commit()
                                }
                                EucJpPending::Jis0212Lead($jis0212_lead_minus_offset) => {
                                    $slf.pending = EucJpPending::None;
                                    // Start non-boilerplate
                                    $jis0812_trail_body
                                    // End non-boilerplate
                                }
                                EucJpPending::HalfWidthKatakana => {
                                    $slf.pending = EucJpPending::None;
                                    // Start non-boilerplate
                                    $half_width_katakana_body
                                    // End non-boilerplate
                                }
                                EucJpPending::None => unreachable!("Checked in loop condition"),
                            };
                        }
                    }
                }
            }
        }
        'outermost: loop {
            match dest.copy_ascii_from_check_space_bmp(&mut $source) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((mut $non_ascii, mut $handle)) => {
                    'middle: loop {
                        let dest_again = {
                            // If lead is between 0xA1 and 0xFE, inclusive,
                            // subtract 0xA1. Else if lead is 0x8E, handle the
                            // next byte as half-width Katakana. Else if lead is
                            // 0x8F, expect JIS 0212.
                            let $jis0208_lead_minus_offset = $non_ascii.wrapping_sub(0xA1);
                            if $jis0208_lead_minus_offset <= (0xFE - 0xA1) {
                                // JIS 0208
                                match $source.check_available() {
                                    Space::Full(src_consumed_trail) => {
                                        if last {
                                            return (DecoderResult::Malformed(1, 0),
                                                    src_consumed_trail,
                                                    $handle.written());
                                        }
                                        $slf.pending =
                                            EucJpPending::Jis0208Lead($jis0208_lead_minus_offset);
                                        return (DecoderResult::InputEmpty,
                                                src_consumed_trail,
                                                $handle.written());
                                    }
                                    Space::Available(source_handle_trail) => {
                                        let ($byte, $unread_handle_trail) =
                                            source_handle_trail.read();
                                        // Start non-boilerplate
                                        $jis0802_trail_body
                                        // End non-boilerplate
                                    }
                                }
                            } else if $non_ascii == 0x8F {
                                match $source.check_available() {
                                    Space::Full(src_consumed_jis0212) => {
                                        if last {
                                            return (DecoderResult::Malformed(1, 0),
                                                    src_consumed_jis0212,
                                                    $handle.written());
                                        }
                                        $slf.pending = EucJpPending::Jis0212Shift;
                                        return (DecoderResult::InputEmpty,
                                                src_consumed_jis0212,
                                                $handle.written());
                                    }
                                    Space::Available(source_handle_jis0212) => {
                                        let ($lead, $unread_handle_jis0212) =
                                            source_handle_jis0212.read();
                                        let $jis0212_lead_minus_offset = {
                                            // Start non-boilerplate
                                            $jis0812_lead_body
                                            // End non-boilerplate
                                        };
                                        match $unread_handle_jis0212.commit().check_available() {
                                            Space::Full(src_consumed_trail) => {
                                                if last {
                                                    return (DecoderResult::Malformed(2, 0),
                                                            src_consumed_trail,
                                                            $handle.written());
                                                }
                                                $slf.pending = EucJpPending::Jis0212Lead($jis0212_lead_minus_offset);
                                                return (DecoderResult::InputEmpty,
                                                        src_consumed_trail,
                                                        $handle.written());
                                            }
                                            Space::Available(source_handle_trail) => {
                                                let ($byte, $unread_handle_trail) =
                                                    source_handle_trail.read();
                                                // Start non-boilerplate
                                                $jis0812_trail_body
                                                // End non-boilerplate
                                            }
                                        }
                                    }
                                }
                            } else if $non_ascii == 0x8E {
                                match $source.check_available() {
                                    Space::Full(src_consumed_trail) => {
                                        if last {
                                            return (DecoderResult::Malformed(1, 0),
                                                    src_consumed_trail,
                                                    $handle.written());
                                        }
                                        $slf.pending = EucJpPending::HalfWidthKatakana;
                                        return (DecoderResult::InputEmpty,
                                                src_consumed_trail,
                                                $handle.written());
                                    }
                                    Space::Available(source_handle_trail) => {
                                        let ($byte, $unread_handle_trail) =
                                            source_handle_trail.read();
                                        // Start non-boilerplate
                                        $half_width_katakana_body
                                        // End non-boilerplate
                                    }
                                }
                            } else {
                                return (DecoderResult::Malformed(1, 0),
                                        $source.consumed(),
                                        $handle.written());
                            }
                        };
                        match $source.check_available() {
                            Space::Full(src_consumed) => {
                                return (DecoderResult::InputEmpty,
                                        src_consumed,
                                        dest_again.written());
                            }
                            Space::Available(source_handle) => {
                                match dest_again.check_space_bmp() {
                                    Space::Full(dst_written) => {
                                        return (DecoderResult::OutputFull,
                                                source_handle.consumed(),
                                                dst_written);
                                    }
                                    Space::Available(destination_handle) => {
                                        let (b, _) = source_handle.read();
                                        loop {
                                            if b > 127 {
                                                $non_ascii = b;
                                                $handle = destination_handle;
                                                continue 'middle;
                                            }
                                            // Testing on Haswell says that we should write the
                                            // byte unconditionally instead of trying to unread it
                                            // to make it part of the next SIMD stride.
                                            destination_handle.write_ascii(b);
                                            // We've got markup or ASCII text
                                            continue 'outermost;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

macro_rules! euc_jp_decoder_functions {
    (
        $jis0802_trail_body:block,
        $jis0812_lead_body:block,
        $jis0812_trail_body:block,
        $half_width_katakana_body:block,
        $slf:ident,
        $non_ascii:ident,
        $jis0208_lead_minus_offset:ident,
        $byte:ident,
        $unread_handle_trail:ident,
        $jis0212_lead_minus_offset:ident,
        $lead:ident,
        $unread_handle_jis0212:ident,
        $source:ident,
        $handle:ident
    ) => {
        euc_jp_decoder_function!(
            $jis0802_trail_body,
            $jis0812_lead_body,
            $jis0812_trail_body,
            $half_width_katakana_body,
            $slf,
            $non_ascii,
            $jis0208_lead_minus_offset,
            $byte,
            $unread_handle_trail,
            $jis0212_lead_minus_offset,
            $lead,
            $unread_handle_jis0212,
            $source,
            $handle,
            decode_to_utf8_raw,
            u8,
            Utf8Destination
        );
        euc_jp_decoder_function!(
            $jis0802_trail_body,
            $jis0812_lead_body,
            $jis0812_trail_body,
            $half_width_katakana_body,
            $slf,
            $non_ascii,
            $jis0208_lead_minus_offset,
            $byte,
            $unread_handle_trail,
            $jis0212_lead_minus_offset,
            $lead,
            $unread_handle_jis0212,
            $source,
            $handle,
            decode_to_utf16_raw,
            u16,
            Utf16Destination
        );
    };
}

macro_rules! encoder_function {
    ($eof:block,
     $body:block,
     $slf:ident,
     $src_consumed:ident,
     $source:ident,
     $dest:ident,
     $c:ident,
     $destination_handle:ident,
     $unread_handle:ident,
     $destination_check:ident,
     $name:ident,
     $input:ty,
     $source_struct:ident) => (
    pub fn $name(&mut $slf,
                 src: &$input,
                 dst: &mut [u8],
                 last: bool)
                 -> (EncoderResult, usize, usize) {
        let mut $source = $source_struct::new(src);
        let mut $dest = ByteDestination::new(dst);
        loop {
            match $source.check_available() {
                Space::Full($src_consumed) => {
                    if last {
                        // Start non-boilerplate
                        $eof
                        // End non-boilerplate
                    }
                    return (EncoderResult::InputEmpty, $src_consumed, $dest.written());
                }
                Space::Available(source_handle) => {
                    match $dest.$destination_check() {
                        Space::Full(dst_written) => {
                            return (EncoderResult::OutputFull,
                                    source_handle.consumed(),
                                    dst_written);
                        }
                        Space::Available($destination_handle) => {
                            let ($c, $unread_handle) = source_handle.read();
                            // Start non-boilerplate
                            $body
                            // End non-boilerplate
                        }
                    }
                }
            }
        }
    });
}

macro_rules! encoder_functions {
    (
        $eof:block,
        $body:block,
        $slf:ident,
        $src_consumed:ident,
        $source:ident,
        $dest:ident,
        $c:ident,
        $destination_handle:ident,
        $unread_handle:ident,
        $destination_check:ident
    ) => {
        encoder_function!(
            $eof,
            $body,
            $slf,
            $src_consumed,
            $source,
            $dest,
            $c,
            $destination_handle,
            $unread_handle,
            $destination_check,
            encode_from_utf8_raw,
            str,
            Utf8Source
        );
        encoder_function!(
            $eof,
            $body,
            $slf,
            $src_consumed,
            $source,
            $dest,
            $c,
            $destination_handle,
            $unread_handle,
            $destination_check,
            encode_from_utf16_raw,
            [u16],
            Utf16Source
        );
    };
}

macro_rules! ascii_compatible_encoder_function {
    ($bmp_body:block,
     $astral_body:block,
     $bmp:ident,
     $astral:ident,
     $slf:ident,
     $source:ident,
     $handle:ident,
     $copy_ascii:ident,
     $destination_check:ident,
     $name:ident,
     $input:ty,
     $source_struct:ident,
     $ascii_punctuation:expr) => (
    pub fn $name(&mut $slf,
                 src: &$input,
                 dst: &mut [u8],
                 _last: bool)
                 -> (EncoderResult, usize, usize) {
        let mut $source = $source_struct::new(src);
        let mut dest = ByteDestination::new(dst);
        'outermost: loop {
            match $source.$copy_ascii(&mut dest) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((mut non_ascii, mut $handle)) => {
                    'middle: loop {
                        let dest_again = match non_ascii {
                            NonAscii::BmpExclAscii($bmp) => {
                                // Start non-boilerplate
                                $bmp_body
                                // End non-boilerplate
                            }
                            NonAscii::Astral($astral) => {
                                // Start non-boilerplate
                                $astral_body
                                // End non-boilerplate
                            }
                        };
                        match $source.check_available() {
                            Space::Full(src_consumed) => {
                                return (EncoderResult::InputEmpty,
                                        src_consumed,
                                        dest_again.written());
                            }
                            Space::Available(source_handle) => {
                                match dest_again.$destination_check() {
                                    Space::Full(dst_written) => {
                                        return (EncoderResult::OutputFull,
                                                source_handle.consumed(),
                                                dst_written);
                                    }
                                    Space::Available(mut destination_handle) => {
                                        let (mut c, unread_handle) = source_handle.read_enum();
                                        let source_again = unread_handle.commit();
                                        'innermost: loop {
                                            let ascii = match c {
                                                Unicode::NonAscii(non_ascii_again) => {
                                                    non_ascii = non_ascii_again;
                                                    $handle = destination_handle;
                                                    continue 'middle;
                                                }
                                                Unicode::Ascii(a) => a,
                                            };
                                            // Testing on Haswell says that we should write the
                                            // byte unconditionally instead of trying to unread it
                                            // to make it part of the next SIMD stride.
                                            let dest_again_again =
                                                destination_handle.write_one(ascii);
                                            if $ascii_punctuation && ascii < 60 {
                                                // We've got punctuation
                                                match source_again.check_available() {
                                                    Space::Full(src_consumed_again) => {
                                                        return (EncoderResult::InputEmpty,
                                                                src_consumed_again,
                                                                dest_again_again.written());
                                                    }
                                                    Space::Available(source_handle_again) => {
                                                        match dest_again_again.$destination_check() {
                                                            Space::Full(dst_written_again) => {
                                                                return (EncoderResult::OutputFull,
                                                                        source_handle_again.consumed(),
                                                                        dst_written_again);
                                                            }
                                                            Space::Available(destination_handle_again) => {
                                                                {
                                                                    let (c_again, _unread_handle_again) =
                                                                        source_handle_again.read_enum();
                                                                    c = c_again;
                                                                    destination_handle = destination_handle_again;
                                                                    continue 'innermost;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            // We've got markup or ASCII text
                                            continue 'outermost;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

macro_rules! ascii_compatible_encoder_functions {
    (
        $bmp_body:block,
        $astral_body:block,
        $bmp:ident,
        $astral:ident,
        $slf:ident,
        $source:ident,
        $handle:ident,
        $copy_ascii:ident,
        $destination_check:ident,
        $ascii_punctuation:expr
    ) => {
        ascii_compatible_encoder_function!(
            $bmp_body,
            $astral_body,
            $bmp,
            $astral,
            $slf,
            $source,
            $handle,
            $copy_ascii,
            $destination_check,
            encode_from_utf8_raw,
            str,
            Utf8Source,
            $ascii_punctuation
        );
        ascii_compatible_encoder_function!(
            $bmp_body,
            $astral_body,
            $bmp,
            $astral,
            $slf,
            $source,
            $handle,
            $copy_ascii,
            $destination_check,
            encode_from_utf16_raw,
            [u16],
            Utf16Source,
            $ascii_punctuation
        );
    };
}

macro_rules! ascii_compatible_bmp_encoder_function {
    (
        $bmp_body:block,
        $bmp:ident,
        $slf:ident,
        $source:ident,
        $handle:ident,
        $copy_ascii:ident,
        $destination_check:ident,
        $name:ident,
        $input:ty,
        $source_struct:ident,
        $ascii_punctuation:expr
    ) => {
        ascii_compatible_encoder_function!(
            $bmp_body,
            {
                return (
                    EncoderResult::Unmappable(astral),
                    $source.consumed(),
                    $handle.written(),
                );
            },
            $bmp,
            astral,
            $slf,
            $source,
            $handle,
            $copy_ascii,
            $destination_check,
            $name,
            $input,
            $source_struct,
            $ascii_punctuation
        );
    };
}

macro_rules! ascii_compatible_bmp_encoder_functions {
    (
        $bmp_body:block,
        $bmp:ident,
        $slf:ident,
        $source:ident,
        $handle:ident,
        $copy_ascii:ident,
        $destination_check:ident,
        $ascii_punctuation:expr
    ) => {
        ascii_compatible_encoder_functions!(
            $bmp_body,
            {
                return (
                    EncoderResult::Unmappable(astral),
                    $source.consumed(),
                    $handle.written(),
                );
            },
            $bmp,
            astral,
            $slf,
            $source,
            $handle,
            $copy_ascii,
            $destination_check,
            $ascii_punctuation
        );
    };
}

macro_rules! public_decode_function{
    ($(#[$meta:meta])*,
     $decode_to_utf:ident,
     $decode_to_utf_raw:ident,
     $decode_to_utf_checking_end:ident,
     $decode_to_utf_after_one_potential_bom_byte:ident,
     $decode_to_utf_after_two_potential_bom_bytes:ident,
     $decode_to_utf_checking_end_with_offset:ident,
     $code_unit:ty) => (
    $(#[$meta])*
    pub fn $decode_to_utf(&mut self,
                           src: &[u8],
                           dst: &mut [$code_unit],
                           last: bool)
                           -> (DecoderResult, usize, usize) {
        let mut offset = 0usize;
        loop {
            match self.life_cycle {
                // The common case. (Post-sniffing.)
                DecoderLifeCycle::Converting => {
                    return self.$decode_to_utf_checking_end(src, dst, last);
                }
                // The rest is all BOM sniffing!
                DecoderLifeCycle::AtStart => {
                    debug_assert_eq!(offset, 0usize);
                    if src.is_empty() {
                        return (DecoderResult::InputEmpty, 0, 0);
                    }
                    match src[0] {
                        0xEFu8 => {
                            self.life_cycle = DecoderLifeCycle::SeenUtf8First;
                            offset += 1;
                            continue;
                        }
                        0xFEu8 => {
                            self.life_cycle = DecoderLifeCycle::SeenUtf16BeFirst;
                            offset += 1;
                            continue;
                        }
                        0xFFu8 => {
                            self.life_cycle = DecoderLifeCycle::SeenUtf16LeFirst;
                            offset += 1;
                            continue;
                        }
                        _ => {
                            self.life_cycle = DecoderLifeCycle::Converting;
                            continue;
                        }
                    }
                }
                DecoderLifeCycle::AtUtf8Start => {
                    debug_assert_eq!(offset, 0usize);
                    if src.is_empty() {
                        return (DecoderResult::InputEmpty, 0, 0);
                    }
                    match src[0] {
                        0xEFu8 => {
                            self.life_cycle = DecoderLifeCycle::SeenUtf8First;
                            offset += 1;
                            continue;
                        }
                        _ => {
                            self.life_cycle = DecoderLifeCycle::Converting;
                            continue;
                        }
                    }
                }
                DecoderLifeCycle::AtUtf16BeStart => {
                    debug_assert_eq!(offset, 0usize);
                    if src.is_empty() {
                        return (DecoderResult::InputEmpty, 0, 0);
                    }
                    match src[0] {
                        0xFEu8 => {
                            self.life_cycle = DecoderLifeCycle::SeenUtf16BeFirst;
                            offset += 1;
                            continue;
                        }
                        _ => {
                            self.life_cycle = DecoderLifeCycle::Converting;
                            continue;
                        }
                    }
                }
                DecoderLifeCycle::AtUtf16LeStart => {
                    debug_assert_eq!(offset, 0usize);
                    if src.is_empty() {
                        return (DecoderResult::InputEmpty, 0, 0);
                    }
                    match src[0] {
                        0xFFu8 => {
                            self.life_cycle = DecoderLifeCycle::SeenUtf16LeFirst;
                            offset += 1;
                            continue;
                        }
                        _ => {
                            self.life_cycle = DecoderLifeCycle::Converting;
                            continue;
                        }
                    }
                }
                DecoderLifeCycle::SeenUtf8First => {
                    if offset >= src.len() {
                        if last {
                            return self.$decode_to_utf_after_one_potential_bom_byte(src,
                                                                                    dst,
                                                                                    last,
                                                                                    offset,
                                                                                    0xEFu8);
                        }
                        return (DecoderResult::InputEmpty, offset, 0);
                    }
                    if src[offset] == 0xBBu8 {
                        self.life_cycle = DecoderLifeCycle::SeenUtf8Second;
                        offset += 1;
                        continue;
                    }
                    return self.$decode_to_utf_after_one_potential_bom_byte(src,
                                                                            dst,
                                                                            last,
                                                                            offset,
                                                                            0xEFu8);
                }
                DecoderLifeCycle::SeenUtf8Second => {
                    if offset >= src.len() {
                        if last {
                            return self.$decode_to_utf_after_two_potential_bom_bytes(src,
                                                                                     dst,
                                                                                     last,
                                                                                     offset);
                        }
                        return (DecoderResult::InputEmpty, offset, 0);
                    }
                    if src[offset] == 0xBFu8 {
                        self.life_cycle = DecoderLifeCycle::Converting;
                        offset += 1;
                        if self.encoding != UTF_8 {
                            self.encoding = UTF_8;
                            self.variant = UTF_8.new_variant_decoder();
                        }
                        return self.$decode_to_utf_checking_end_with_offset(src,
                                                                            dst,
                                                                            last,
                                                                            offset);
                    }
                    return self.$decode_to_utf_after_two_potential_bom_bytes(src,
                                                                             dst,
                                                                             last,
                                                                             offset);
                }
                DecoderLifeCycle::SeenUtf16BeFirst => {
                    if offset >= src.len() {
                        if last {
                            return self.$decode_to_utf_after_one_potential_bom_byte(src,
                                                                                    dst,
                                                                                    last,
                                                                                    offset,
                                                                                    0xFEu8);
                        }
                        return (DecoderResult::InputEmpty, offset, 0);
                    }
                    if src[offset] == 0xFFu8 {
                        self.life_cycle = DecoderLifeCycle::Converting;
                        offset += 1;
                        if self.encoding != UTF_16BE {
                            self.encoding = UTF_16BE;
                            self.variant = UTF_16BE.new_variant_decoder();
                        }
                        return self.$decode_to_utf_checking_end_with_offset(src,
                                                                            dst,
                                                                            last,
                                                                            offset);
                    }
                    return self.$decode_to_utf_after_one_potential_bom_byte(src,
                                                                            dst,
                                                                            last,
                                                                            offset,
                                                                            0xFEu8);
                }
                DecoderLifeCycle::SeenUtf16LeFirst => {
                    if offset >= src.len() {
                        if last {
                            return self.$decode_to_utf_after_one_potential_bom_byte(src,
                                                                                    dst,
                                                                                    last,
                                                                                    offset,
                                                                                    0xFFu8);
                        }
                        return (DecoderResult::InputEmpty, offset, 0);
                    }
                    if src[offset] == 0xFEu8 {
                        self.life_cycle = DecoderLifeCycle::Converting;
                        offset += 1;
                        if self.encoding != UTF_16LE {
                            self.encoding = UTF_16LE;
                            self.variant = UTF_16LE.new_variant_decoder();
                        }
                        return self.$decode_to_utf_checking_end_with_offset(src,
                                                                            dst,
                                                                            last,
                                                                            offset);
                    }
                    return self.$decode_to_utf_after_one_potential_bom_byte(src,
                                                                            dst,
                                                                            last,
                                                                            offset,
                                                                            0xFFu8);
                }
                DecoderLifeCycle::ConvertingWithPendingBB => {
                    debug_assert_eq!(offset, 0usize);
                    return self.$decode_to_utf_after_one_potential_bom_byte(src,
                                                                            dst,
                                                                            last,
                                                                            0usize,
                                                                            0xBBu8);
                }
                DecoderLifeCycle::Finished => panic!("Must not use a decoder that has finished."),
            }
        }
    }

    fn $decode_to_utf_after_one_potential_bom_byte(&mut self,
                                                   src: &[u8],
                                                   dst: &mut [$code_unit],
                                                   last: bool,
                                                   offset: usize,
                                                   first_byte: u8)
                                                   -> (DecoderResult, usize, usize) {
        self.life_cycle = DecoderLifeCycle::Converting;
        if offset == 0usize {
            // First byte was seen previously.
            let first = [first_byte];
            let mut out_read = 0usize;
            let (mut first_result, _, mut first_written) =
                self.variant
                    .$decode_to_utf_raw(&first[..], dst, false);
            match first_result {
                DecoderResult::InputEmpty => {
                    let (result, read, written) =
                        self.$decode_to_utf_checking_end(src, &mut dst[first_written..], last);
                    first_result = result;
                    out_read = read; // Overwrite, don't add!
                    first_written += written;
                }
                DecoderResult::Malformed(_, _) => {
                    // Wasn't read from `src`!, leave out_read to 0
                }
                DecoderResult::OutputFull => {
                    panic!("Output buffer must have been too small.");
                }
            }
            return (first_result, out_read, first_written);
        }
        debug_assert_eq!(offset, 1usize);
        // The first byte is in `src`, so no need to push it separately.
        self.$decode_to_utf_checking_end(src, dst, last)
    }

    fn $decode_to_utf_after_two_potential_bom_bytes(&mut self,
                                                    src: &[u8],
                                                    dst: &mut [$code_unit],
                                                    last: bool,
                                                    offset: usize)
                                                    -> (DecoderResult, usize, usize) {
        self.life_cycle = DecoderLifeCycle::Converting;
        if offset == 0usize {
            // The first two bytes are not in the current buffer..
            let ef_bb = [0xEFu8, 0xBBu8];
            let (mut first_result, mut first_read, mut first_written) =
                self.variant
                    .$decode_to_utf_raw(&ef_bb[..], dst, false);
            match first_result {
                DecoderResult::InputEmpty => {
                    let (result, read, written) =
                        self.$decode_to_utf_checking_end(src, &mut dst[first_written..], last);
                    first_result = result;
                    first_read = read; // Overwrite, don't add!
                    first_written += written;
                }
                DecoderResult::Malformed(_, _) => {
                    if first_read == 1usize {
                        // The first byte was malformed. We need to handle
                        // the second one, which isn't in `src`, later.
                        self.life_cycle = DecoderLifeCycle::ConvertingWithPendingBB;
                    }
                    first_read = 0usize; // Wasn't read from `src`!
                }
                DecoderResult::OutputFull => {
                    panic!("Output buffer must have been too small.");
                }
            }
            return (first_result, first_read, first_written);
        }
        if offset == 1usize {
            // The first byte isn't in the current buffer but the second one
            // is.
            return self.$decode_to_utf_after_one_potential_bom_byte(src,
                                                                    dst,
                                                                    last,
                                                                    0usize,
                                                                    0xEFu8);

        }
        debug_assert_eq!(offset, 2usize);
        // The first two bytes are in `src`, so no need to push them separately.
        self.$decode_to_utf_checking_end(src, dst, last)
    }

    /// Calls `$decode_to_utf_checking_end` with `offset` bytes omitted from
    /// the start of `src` but adjusting the return values to show those bytes
    /// as having been consumed.
    fn $decode_to_utf_checking_end_with_offset(&mut self,
                                               src: &[u8],
                                               dst: &mut [$code_unit],
                                               last: bool,
                                               offset: usize)
                                               -> (DecoderResult, usize, usize) {
        debug_assert_eq!(self.life_cycle, DecoderLifeCycle::Converting);
        let (result, read, written) = self.$decode_to_utf_checking_end(&src[offset..], dst, last);
        (result, read + offset, written)
    }

    /// Calls through to the delegate and adjusts life cycle iff `last` is
    /// `true` and result is `DecoderResult::InputEmpty`.
    fn $decode_to_utf_checking_end(&mut self,
                                   src: &[u8],
                                   dst: &mut [$code_unit],
                                   last: bool)
                                   -> (DecoderResult, usize, usize) {
        debug_assert_eq!(self.life_cycle, DecoderLifeCycle::Converting);
        let (result, read, written) = self.variant
                                          .$decode_to_utf_raw(src, dst, last);
        if last {
            if let DecoderResult::InputEmpty = result {
                self.life_cycle = DecoderLifeCycle::Finished;
            }
        }
        (result, read, written)
    });
}
