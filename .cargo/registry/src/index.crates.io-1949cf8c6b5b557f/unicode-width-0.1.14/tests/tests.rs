// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

macro_rules! assert_width {
    ($s:expr, $nocjk:expr, $cjk:expr $(,)?) => {{
        assert_eq!($s.width(), $nocjk, "{:?} has the wrong width", $s);
        #[cfg(feature = "cjk")]
        assert_eq!($s.width_cjk(), $cjk, "{:?} has the wrong width (CJK)", $s);
    }};
}

#[test]
fn test_str() {
    assert_width!("ｈｅｌｌｏ", 10, 10);
    assert_width!("\0\0\0\x01\x01", 5, 5);
    assert_width!("", 0, 0);
    assert_width!("\u{2081}\u{2082}\u{2083}\u{2084}", 4, 8);
}

#[test]
fn test_emoji() {
    assert_width!("👩", 2, 2); // Woman
    assert_width!("🔬", 2, 2); // Microscope
    assert_width!("👩‍🔬", 2, 2); // Woman scientist
}

// From README
#[test]
fn test_bad_devanagari() {
    assert_eq!("क".width(), 1); // Devanagari letter Ka
    assert_eq!("ष".width(), 1); // Devanagari letter Ssa
    assert_eq!("क्ष".width(), 2); // Ka + Virama + Ssa
}

#[test]
fn test_char() {
    assert_width!('ｈ', Some(2), Some(2));
    assert_width!('\x00', None, None);
    assert_width!('\x01', None, None);
    assert_width!('\u{2081}', Some(1), Some(2));
}

#[test]
fn test_char2() {
    assert_width!('\x0A', None, None);
    assert_width!('w', Some(1), Some(1));
    assert_width!('ｈ', Some(2), Some(2));
    assert_width!('\u{AD}', Some(0), Some(0));
    assert_width!('\u{1160}', Some(0), Some(0));
    assert_width!('\u{a1}', Some(1), Some(2));
    assert_width!('\u{300}', Some(0), Some(0));
}

#[test]
fn unicode_12() {
    assert_width!('\u{1F971}', Some(2), Some(2));
}

#[test]
fn test_default_ignorable() {
    assert_width!('\u{1160}', Some(0), Some(0));
    assert_width!('\u{3164}', Some(0), Some(0));
    assert_width!('\u{FFA0}', Some(0), Some(0));
    assert_width!('\u{E0000}', Some(0), Some(0));
}

#[test]
fn test_ambiguous() {
    assert_width!("\u{B7}", 1, 2);
    assert_width!("\u{0387}", 1, 2);
    assert_width!("\u{A8}", 1, 1);
    assert_width!("\u{02C9}", 1, 1);
}

#[test]
fn test_jamo() {
    assert_width!('\u{1100}', Some(2), Some(2));
    assert_width!('\u{A97C}', Some(2), Some(2));
    // Special case: U+115F HANGUL CHOSEONG FILLER
    assert_width!('\u{115F}', Some(2), Some(2));
    assert_width!('\u{1160}', Some(0), Some(0));
    assert_width!('\u{D7C6}', Some(0), Some(0));
    assert_width!('\u{11A8}', Some(0), Some(0));
    assert_width!('\u{D7FB}', Some(0), Some(0));
}

#[test]
fn test_prepended_concatenation_marks() {
    for c in [
        '\u{0600}',
        '\u{0601}',
        '\u{0602}',
        '\u{0603}',
        '\u{0604}',
        '\u{06DD}',
        '\u{110BD}',
        '\u{110CD}',
    ] {
        assert_width!(c, Some(1), Some(1));
    }

    for c in ['\u{0605}', '\u{070F}', '\u{0890}', '\u{0891}', '\u{08E2}'] {
        assert_width!(c, Some(0), Some(0));
    }
}

#[test]
fn test_gcb_prepend() {
    assert_width!("ൎഉ", 1, 1);
    assert_width!("\u{11A89}", 0, 0);
}

#[test]
fn test_interlinear_annotation_chars() {
    assert_width!('\u{FFF9}', Some(1), Some(1));
    assert_width!('\u{FFFA}', Some(1), Some(1));
    assert_width!('\u{FFFB}', Some(1), Some(1));
}

#[test]
fn test_hieroglyph_format_controls() {
    assert_width!('\u{13430}', Some(1), Some(1));
    assert_width!('\u{13436}', Some(1), Some(1));
    assert_width!('\u{1343C}', Some(1), Some(1));
}

#[test]
fn test_marks() {
    // Nonspacing marks have 0 width
    assert_width!('\u{0301}', Some(0), Some(0));
    // Enclosing marks have 0 width
    assert_width!('\u{20DD}', Some(0), Some(0));
    // Some spacing marks have width 1
    assert_width!('\u{09CB}', Some(1), Some(1));
    // But others have width 0
    assert_width!('\u{09BE}', Some(0), Some(0));
}

#[test]
fn test_devanagari_caret() {
    assert_width!('\u{A8FA}', Some(0), Some(0));
}

#[test]
fn test_solidus_overlay() {
    assert_width!("<\u{338}", 1, 2);
    assert_width!("=\u{338}", 1, 2);
    assert_width!(">\u{338}", 1, 2);
    assert_width!("=\u{301}\u{338}", 1, 2);
    assert_width!("=\u{338}\u{301}", 1, 2);
    assert_width!("=\u{FE0F}\u{338}", 1, 2);
    assert_width!("#\u{FE0F}\u{338}", 2, 2);
    assert_width!("#\u{338}\u{FE0F}", 1, 1);

    assert_width!("\u{06B8}\u{338}\u{0627}", 1, 1);
    assert_width!("\u{06B8}\u{338}\u{FE0E}\u{0627}", 1, 1);
    assert_width!("\u{06B8}\u{338}\u{FE0F}\u{0627}", 1, 1);
    assert_width!("\u{06B8}\u{FE0E}\u{338}\u{0627}", 1, 1);
    assert_width!("\u{06B8}\u{FE0F}\u{338}\u{0627}", 1, 1);

    assert_width!("=\u{338}\u{0627}", 2, 3);
}

#[test]
fn test_emoji_presentation() {
    assert_width!('\u{0023}', Some(1), Some(1));
    assert_width!('\u{FE0F}', Some(0), Some(0));
    assert_width!("\u{0023}\u{FE0F}", 2, 2);
    assert_width!("a\u{0023}\u{FE0F}a", 4, 4);
    assert_width!("\u{0023}a\u{FE0F}", 2, 2);
    assert_width!("a\u{FE0F}", 1, 1);
    assert_width!("\u{0023}\u{0023}\u{FE0F}a", 4, 4);
    assert_width!("\u{002A}\u{FE0F}", 2, 2);
    assert_width!("\u{23F9}\u{FE0F}", 2, 2);
    assert_width!("\u{24C2}\u{FE0F}", 2, 2);
    assert_width!("\u{1F6F3}\u{FE0F}", 2, 2);
    assert_width!("\u{1F700}\u{FE0F}", 1, 1);
    assert_width!("\u{002A}\u{301}\u{FE0F}", 1, 1);
    assert_width!("\u{002A}\u{200D}\u{FE0F}", 1, 1);
    assert_width!("\u{002A}\u{FE0E}\u{FE0F}", 1, 1);
}

#[test]
fn test_text_presentation() {
    assert_width!('\u{FE0E}', Some(0), Some(0));
    assert_width!('\u{2648}', Some(2), Some(2));
    assert_width!("\u{2648}\u{FE0E}", 1, 2);
    assert_width!("\u{1F21A}\u{FE0E}", 2, 2);
    assert_width!("\u{0301}\u{FE0E}", 0, 0);
    assert_width!("a\u{FE0E}", 1, 1);
    assert_width!("𘀀\u{FE0E}", 2, 2);
    assert_width!("\u{2648}\u{0301}\u{FE0E}", 2, 2);
    assert_width!("\u{2648}\u{200D}\u{FE0E}", 2, 2);
}

#[test]
fn test_control_line_break() {
    assert_width!('\u{2028}', Some(1), Some(1));
    assert_width!('\u{2029}', Some(1), Some(1));
    assert_width!('\r', None, None);
    assert_width!('\n', None, None);
    assert_width!("\r", 1, 1);
    // This is 0 due to #60
    assert_width!("\n", 0, 0);
    assert_width!("\r\n", 0, 0);
    assert_width!("\0", 1, 1);
    assert_width!("1\t2\r\n3\u{85}4", 6, 6);
    assert_width!("\r\u{FE0F}\n", 1, 1);
    assert_width!("\r\u{200D}\n", 1, 1);
}

#[test]
fn char_str_consistent() {
    let mut s = String::with_capacity(4);
    for c in '\0'..=char::MAX {
        // Newlines are special cased (#60)
        if c == '\n' {
            continue;
        }
        s.clear();
        s.push(c);
        assert_eq!(c.width().unwrap_or(1), s.width());
        #[cfg(feature = "cjk")]
        assert_eq!(c.width_cjk().unwrap_or(1), s.width_cjk());
    }
}

#[test]
fn test_lisu_tones() {
    for c in '\u{A4F8}'..='\u{A4FD}' {
        assert_width!(c, Some(1), Some(1));
        assert_width!(String::from(c), 1, 1);
    }
    for c1 in '\u{A4F8}'..='\u{A4FD}' {
        for c2 in '\u{A4F8}'..='\u{A4FD}' {
            let mut s = String::with_capacity(8);
            s.push(c1);
            s.push(c2);
            match (c1, c2) {
                ('\u{A4F8}'..='\u{A4FB}', '\u{A4FC}'..='\u{A4FD}') => assert_width!(s, 1, 1),
                _ => assert_width!(s, 2, 2),
            }
        }
    }

    assert_width!("ꓪꓹ", 2, 2);
    assert_width!("ꓪꓹꓼ", 2, 2);
    assert_width!("ꓪꓹ\u{FE0F}ꓼ", 2, 2);
    assert_width!("ꓪꓹ\u{200D}ꓼ", 2, 2);
    assert_width!("ꓪꓹꓼ\u{FE0F}", 2, 2);
    assert_width!("ꓪꓹ\u{0301}ꓼ", 3, 3);
    assert_width!("ꓪꓹꓹ", 3, 3);
    assert_width!("ꓪꓼꓼ", 3, 3);
}

#[test]
fn test_hebrew_alef_lamed() {
    assert_width!("\u{05D0}", 1, 1);
    assert_width!("\u{05DC}", 1, 1);
    assert_width!("\u{05D0}\u{05DC}", 2, 2);
    assert_width!("\u{05D0}\u{200D}\u{05DC}", 1, 1);
    assert_width!(
        "\u{05D0}\u{200D}\u{200D}\u{200D}\u{200D}\u{200D}\u{200D}\u{200D}\u{05DC}",
        1,
        1,
    );
    assert_width!("\u{05D0}\u{05D0}\u{200D}\u{05DC}", 2, 2);
    assert_width!(
        "\u{05D0}\u{05D0}\u{200D}\u{200D}\u{200D}\u{200D}\u{200D}\u{200D}\u{05DC}",
        2,
        2,
    );
    assert_width!("\u{05D0}\u{FE0F}\u{200D}\u{FE0F}\u{05DC}\u{FE0F}", 1, 1);
    assert_width!("\u{05D0}\u{FE0E}\u{200D}\u{FE0E}\u{05DC}\u{FE0E}", 1, 1);
}

#[test]
fn test_arabic_lam_alef() {
    assert_width!("\u{0644}", 1, 1);
    assert_width!("\u{06B8}", 1, 1);

    assert_width!("\u{0623}", 1, 1);
    assert_width!("\u{0627}", 1, 1);

    assert_width!("\u{0644}\u{0623}", 1, 1);
    assert_width!("\u{0644}\u{0627}", 1, 1);
    assert_width!("\u{06B8}\u{0623}", 1, 1);
    assert_width!("\u{06B8}\u{0627}", 1, 1);

    assert_width!("\u{0644}\u{065F}\u{065E}\u{0623}", 1, 1);
    assert_width!("\u{0644}\u{065F}\u{065E}\u{0627}", 1, 1);
    assert_width!("\u{06B8}\u{065F}\u{065E}\u{0623}", 1, 1);
    assert_width!("\u{06B8}\u{065F}\u{065E}\u{0627}", 1, 1);

    assert_width!("\u{06B8}\u{FE0E}\u{0627}", 1, 1);
    assert_width!("\u{06B8}\u{FE0F}\u{0627}", 1, 1);
    assert_width!("\u{06B8}\u{17B5}\u{0627}", 1, 1);

    assert_width!("\u{0644}\u{0644}\u{0623}", 2, 2);
    assert_width!("\u{0644}\u{0644}\u{0627}", 2, 2);
    assert_width!("\u{06B8}\u{06B8}\u{0623}", 2, 2);
    assert_width!("\u{06B8}\u{06B8}\u{0627}", 2, 2);

    assert_width!("\u{0644}\u{200D}\u{0623}", 2, 2);
    assert_width!("\u{0644}\u{200D}\u{0627}", 2, 2);
    assert_width!("\u{06B8}\u{200D}\u{0623}", 2, 2);
    assert_width!("\u{06B8}\u{200D}\u{0627}", 2, 2);

    assert_width!("\u{0644}\u{1E94B}\u{0623}", 3, 3);
    assert_width!("\u{0644}\u{1E94B}\u{0627}", 3, 3);
    assert_width!("\u{06B8}\u{1E94B}\u{0623}", 3, 3);
    assert_width!("\u{06B8}\u{1E94B}\u{0627}", 3, 3);
}

#[test]
fn test_buginese_a_i_ya() {
    assert_width!("\u{1A15}", 1, 1);
    assert_width!("\u{1A17}", 0, 0);
    assert_width!("\u{1A10}", 1, 1);

    assert_width!("\u{1A15}\u{1A17}\u{200D}\u{1A10}", 1, 1);
    assert_width!(
        "\u{1A15}\u{1A17}\u{200D}\u{200D}\u{200D}\u{200D}\u{1A10}",
        1,
        1,
    );
    assert_width!("\u{1A15}\u{1A17}\u{200D}\u{338}", 1, 1);
    assert_width!("\u{1A15}\u{FE0E}\u{1A17}\u{200D}", 1, 1);
    assert_width!("\u{1A15}\u{FE0F}\u{1A17}\u{200D}", 1, 1);
    assert_width!("\u{1A15}\u{1A17}\u{FE0E}\u{200D}", 1, 1);
    assert_width!("\u{1A15}\u{1A17}\u{FE0F}\u{200D}", 1, 1);
    assert_width!("\u{1A15}\u{1A17}\u{200D}\u{FE0E}", 1, 1);
    assert_width!("\u{1A15}\u{1A17}\u{200D}\u{FE0F}", 1, 1);
    assert_width!(
        "\u{1A15}\u{17B5}\u{200D}\u{FE0E}\u{1A17}\u{200D}\u{FE0F}\u{200D}\u{FE0F}",
        1,
        1,
    );

    assert_width!("\u{1A15}\u{1A15}\u{1A17}\u{200D}\u{1A10}", 2, 2);
    assert_width!(
        "\u{1A15}\u{1A15}\u{1A17}\u{200D}\u{200D}\u{200D}\u{200D}\u{1A10}",
        2,
        2,
    );

    assert_width!("\u{1A15}\u{1A17}\u{1A10}", 2, 2);
    assert_width!("\u{1A15}\u{200D}\u{1A10}", 2, 2);
    assert_width!("\u{1A15}\u{1A10}", 2, 2);
    assert_width!("\u{1A15}\u{1A17}\u{1A17}\u{200D}\u{1A10}", 2, 2);
    assert_width!("\u{1A15}\u{1A17}\u{338}\u{200D}\u{1A10}", 2, 2);
}

#[test]
fn test_tifinagh_biconsonants() {
    assert_width!("\u{2D4F}", 1, 1);
    assert_width!("\u{2D3E}", 1, 1);
    assert_width!("\u{2D7F}", 1, 1);

    assert_width!("\u{2D4F}\u{200D}\u{2D3E}", 1, 1);
    assert_width!("\u{2D4F}\u{2D7F}\u{2D3E}", 1, 1);
    assert_width!("\u{2D4F}\u{200D}\u{2D3E}", 1, 1);
    assert_width!(
        "\u{2D4F}\u{FE0F}\u{200D}\u{2D7F}\u{FE0E}\u{200D}\u{17B5}\u{2D3E}",
        1,
        1,
    );

    assert_width!("\u{2D4F}\u{301}\u{2D7F}\u{2D3E}", 3, 3);
    assert_width!("\u{2D4F}\u{301}\u{200D}\u{2D3E}", 2, 2);
    assert_width!("\u{2D4F}\u{2D3E}", 2, 2);
    assert_width!("\u{2D4F}\u{2D7F}\u{2D7F}\u{2D3E}", 4, 4);
    assert_width!("\u{2D7F}\u{2D3E}", 2, 2);
    assert_width!("\u{2D7F}\u{2D7F}\u{2D66}", 3, 3);
    assert_width!("\u{2D66}\u{2D7F}\u{2D3E}", 3, 3);
}

#[test]
fn test_old_turkic_ligature() {
    assert_width!("\u{10C32}", 1, 1);
    assert_width!("\u{10C03}", 1, 1);
    assert_width!("\u{10C32}\u{10C03}", 2, 2);

    assert_width!("\u{10C32}\u{200D}\u{10C03}", 1, 1);
    assert_width!("\u{10C32}\u{FE0F}\u{200D}\u{FE0E}\u{10C03}", 1, 1);

    assert_width!("\u{10C32}\u{2D7F}\u{10C03}", 3, 3);
    assert_width!("\u{10C32}\u{0301}\u{200D}\u{10C03}", 2, 2);
    assert_width!("\u{10C03}\u{200D}\u{10C32}", 2, 2);
    assert_width!("\u{200D}\u{10C32}", 1, 1);
}

#[test]
fn test_khmer_coeng() {
    assert_width!("ល", 1, 1);
    assert_width!("ង", 1, 1);
    assert_width!("លង", 2, 2);
    assert_width!("ល្ង", 1, 1);

    for c in '\0'..=char::MAX {
        if matches!(
            c,
            '\u{1780}'..='\u{1782}' | '\u{1784}'..='\u{1787}'
            | '\u{1789}'..='\u{178C}'  | '\u{178E}'..='\u{1793}'
            | '\u{1795}'..='\u{1798}' | '\u{179B}'..='\u{179D}'
            | '\u{17A0}' | '\u{17A2}'  | '\u{17A7}'
            | '\u{17AB}'..='\u{17AC}' | '\u{17AF}'
        ) {
            assert_width!(format!("\u{17D2}{c}"), 0, 0);
            assert_width!(format!("\u{17D2}\u{200D}\u{200D}{c}"), 0, 0);
        } else {
            // Newlines are special cased (#60)
            if c == '\n' {
                continue;
            }
            assert_width!(
                format!("\u{17D2}{c}"),
                c.width().unwrap_or(1),
                c.width_cjk().unwrap_or(1)
            );
        }
    }
}

#[test]
fn test_khmer_qaa() {
    assert_width!("\u{17A4}", 2, 2);
    assert_width!("\u{17A2}\u{17A6}", 2, 2);
}

#[test]
fn test_khmer_sign_beyyal() {
    assert_width!("\u{17D8}", 3, 3);
    assert_width!("\u{17D4}\u{179B}\u{17D4}", 3, 3);
}

#[test]
fn test_emoji_modifier() {
    assert_width!("\u{1F46A}", 2, 2);
    assert_width!("\u{1F3FB}", 2, 2);
    assert_width!("\u{1F46A}\u{1F3FB}", 2, 2);
    assert_width!("\u{1F46A}\u{200D}\u{200D}\u{1F3FB}", 4, 4);
}

#[test]
fn test_emoji_zwj() {
    assert_width!("🧑‍🤝‍🧑", 2, 2);

    assert_width!("🇮🇱🕊️🇵🇸", 6, 6);
    assert_width!("🇵🇸\u{200D}🕊️\u{200D}🇮🇱", 2, 2);
    assert_width!("🇮🇱\u{200D}🕊️\u{200D}\u{200D}🇵🇸", 4, 4);
    assert_width!("🇵🇸\u{200D}\u{200D}🕊️\u{200D}🇮🇱", 4, 4);

    assert_width!("🇦🇦\u{200D}🇦🇦", 2, 2);
    assert_width!("🇦🇦\u{200D}🇦🇦🇦", 3, 3);
    assert_width!("🇦🇦\u{200D}🇦🇦🇦", 3, 3);

    assert_width!("🇦🇦\u{200D}\u{200D}🇦🇦", 4, 4);
    assert_width!("🇦🇦\u{200D}🇦\u{200D}🇦🇦", 5, 5);
    assert_width!("🇦🇦\u{200D}🇦🇦\u{200D}🇦🇦", 2, 2);
    assert_width!("🇦🇦\u{200D}🇦🇦🇦\u{200D}🇦🇦", 5, 5);
    assert_width!("🇦🇦\u{200D}🇦🇦🇦🇦\u{200D}🇦🇦", 4, 4);
    assert_width!("🇦🇦\u{200D}🇦🇦🇦🇦🇦\u{200D}🇦🇦", 7, 7);
    assert_width!("🇦🇦\u{200D}🇦🇦🇦🇦🇦🇦\u{200D}🇦🇦", 6, 6);
    assert_width!("🇦🇦\u{200D}🇦🇦🇦🇦🇦🇦🇦\u{200D}🇦🇦", 9, 9);

    assert_width!("🏴󠁧󠁢󠁷󠁬󠁳󠁿", 2, 2);
    assert_width!("🏴󠁧󠁢󠁥󠁮󠁧󠁿\u{200D}🏴󠁧󠁢󠁳󠁣󠁴󠁿\u{200D}🏴󠁧󠁢󠁷󠁬󠁳󠁿", 2, 2);

    assert_width!("🇦👪\u{200D}🏿", 3, 3);
    assert_width!("🇦🏿\u{200D}🏿", 3, 3);

    assert_width!('🏴', Some(2), Some(2));
    assert_width!("\u{E0031}", 0, 0);
    assert_width!("\u{E0063}", 0, 0);
    assert_width!("\u{E007F}", 0, 0);
    assert_width!("🏴\u{200D}Ⓜ️", 2, 2);
    assert_width!("🏴\u{E0031}\u{200D}Ⓜ️", 4, 4);
    assert_width!("🏴\u{E0063}\u{200D}Ⓜ️", 4, 4);
    assert_width!("🏴\u{E007F}\u{200D}Ⓜ️", 4, 4);
    assert_width!("🏴\u{E0031}\u{E007F}\u{200D}Ⓜ️", 4, 4);
    assert_width!("🏴\u{E0031}\u{E0031}\u{E007F}\u{200D}Ⓜ️", 4, 4);
    assert_width!("🏴\u{E0031}\u{E0031}\u{E0031}\u{E007F}\u{200D}Ⓜ️", 2, 2);
    assert_width!(
        "🏴\u{E0031}\u{E0031}\u{E0031}\u{E0031}\u{E007F}\u{200D}Ⓜ️",
        4,
        4,
    );
    assert_width!(
        "🏴\u{E0031}\u{E0031}\u{E0031}\u{E0063}\u{E007F}\u{200D}Ⓜ️",
        2,
        2,
    );
    assert_width!(
        "🏴\u{E0031}\u{E0031}\u{E0031}\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️",
        2,
        2,
    );
    assert_width!(
        "🏴\u{E0031}\u{E0031}\u{E0031}\u{E0063}\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️",
        2,
        2,
    );
    assert_width!(
        "🏴\u{E0031}\u{E0031}\u{E0031}\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️",
        2,
        2,
    );
    assert_width!(
        "🏴\u{E0031}\u{E0031}\u{E0031}\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️",
        4,
        4,
    );
    assert_width!("🏴\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️", 4, 4);
    assert_width!("🏴\u{E0063}\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️", 2, 2);
    assert_width!(
        "🏴\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️",
        2,
        2,
    );
    assert_width!(
        "🏴\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️",
        2,
        2,
    );
    assert_width!(
        "🏴\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️",
        2,
        2,
    );
    assert_width!(
        "🏴\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E0063}\u{E007F}\u{200D}Ⓜ️",
        4,
        4,
    );

    assert_width!("a\u{200D}🏴󠁧󠁢󠁷󠁬󠁳󠁿", 3, 3);
    assert_width!("👪\u{200D}a", 3, 3);
    assert_width!("a\u{200D}a", 2, 2);

    assert_width!("*\u{FE0F}", 2, 2);
    assert_width!("*\u{20E3}", 1, 1);
    assert_width!("*️⃣", 2, 2);
    assert_width!("*\u{FE0F}", 2, 2);
    assert_width!("*\u{20E3}\u{FE0F}", 1, 1);
    assert_width!("*️⃣\u{200D}👪", 2, 2);
    assert_width!("*\u{20E3}\u{FE0F}\u{200D}👪", 3, 3);
    assert_width!("*\u{20E3}\u{200D}👪", 3, 3);
    assert_width!("*\u{FE0F}\u{200D}👪", 2, 2);
    assert_width!("*️⃣\u{20E3}\u{200D}👪", 4, 4);
    assert_width!("*\u{FE0F}\u{FE0F}\u{20E3}\u{200D}👪", 4, 4);

    assert_width!(
        "🇦👪\u{200D}🏿\u{200D}👪🏻\u{200D}Ⓜ️\u{200D}*\u{FE0F}\u{200D}🇦🇦\u{200D}🏴󠁧󠁢󠁷󠁬󠁳󠁿\u{200D}👪",
        3,
        3,
    );
}

#[test]
fn emoji_test_file() {
    let norm_file = BufReader::new(
        File::open("tests/emoji-test.txt")
            .expect("run `unicode.py` first to download `emoji-test.txt`"),
    );
    for line in norm_file.lines() {
        let line = line.unwrap();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (cps, status) = line.split_once(';').unwrap();
        let status = status.trim();
        if status.starts_with("fully-qualified") || status.starts_with("component") {
            let emoji: String = cps
                .trim()
                .split(' ')
                .map(|s| char::try_from(u32::from_str_radix(s, 16).unwrap()).unwrap())
                .collect();
            dbg!(&emoji);
            assert_width!(emoji, 2, 2);
        }
    }
}

#[test]
fn test_newline_zero_issue_60() {
    assert_width!("a\na", 2, 2);
}

// Test traits are unsealed

#[cfg(feature = "cjk")]
#[allow(dead_code)]
struct Foo;

#[cfg(feature = "cjk")]
impl UnicodeWidthChar for Foo {
    fn width(self) -> Option<usize> {
        Some(0)
    }

    fn width_cjk(self) -> Option<usize> {
        Some(0)
    }
}

#[cfg(feature = "cjk")]
impl UnicodeWidthStr for Foo {
    fn width(&self) -> usize {
        0
    }

    fn width_cjk(&self) -> usize {
        0
    }
}
