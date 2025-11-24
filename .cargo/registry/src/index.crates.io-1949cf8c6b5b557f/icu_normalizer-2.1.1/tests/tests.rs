// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use icu_normalizer::properties::CanonicalCombiningClassMap;
use icu_normalizer::properties::CanonicalCombiningClassMapBorrowed;
use icu_normalizer::properties::CanonicalComposition;
use icu_normalizer::properties::CanonicalCompositionBorrowed;
use icu_normalizer::properties::CanonicalDecomposition;
use icu_normalizer::properties::CanonicalDecompositionBorrowed;
use icu_normalizer::properties::Decomposed;
use icu_normalizer::uts46::Uts46Mapper;
use icu_normalizer::uts46::Uts46MapperBorrowed;
use icu_normalizer::ComposingNormalizer;
use icu_normalizer::ComposingNormalizerBorrowed;
use icu_normalizer::DecomposingNormalizer;
use icu_normalizer::DecomposingNormalizerBorrowed;

#[test]
fn test_nfd_basic() {
    let normalizer = DecomposingNormalizerBorrowed::new_nfd();
    assert_eq!(normalizer.normalize("ä"), "a\u{0308}");
    assert_eq!(normalizer.normalize("Ä"), "A\u{0308}");
    assert_eq!(normalizer.normalize("ệ"), "e\u{0323}\u{0302}");
    assert_eq!(normalizer.normalize("Ệ"), "E\u{0323}\u{0302}");
    assert_eq!(normalizer.normalize("𝅗𝅥"), "𝅗\u{1D165}");
    assert_eq!(normalizer.normalize("\u{2126}"), "Ω"); // ohm sign
    assert_eq!(normalizer.normalize("ﾍﾞ"), "ﾍﾞ"); // half-width unchanged
    assert_eq!(normalizer.normalize("ﾍﾟ"), "ﾍﾟ"); // half-width unchanged
    assert_eq!(normalizer.normalize("ﬁ"), "ﬁ"); // ligature unchanged
    assert_eq!(normalizer.normalize("\u{FDFA}"), "\u{FDFA}"); // ligature unchanged
    assert_eq!(normalizer.normalize("㈎"), "㈎"); // parenthetical unchanged
    assert_eq!(normalizer.normalize("\u{0345}"), "\u{0345}"); // Iota subscript
}

#[test]
fn test_nfd_owned() {
    let owned =
        DecomposingNormalizer::try_new_nfd_unstable(&icu_normalizer::provider::Baked).unwrap();
    let normalizer = owned.as_borrowed();
    assert_eq!(normalizer.normalize("ä"), "a\u{0308}");
    assert_eq!(normalizer.normalize("Ä"), "A\u{0308}");
    assert_eq!(normalizer.normalize("ệ"), "e\u{0323}\u{0302}");
    assert_eq!(normalizer.normalize("Ệ"), "E\u{0323}\u{0302}");
    assert_eq!(normalizer.normalize("𝅗𝅥"), "𝅗\u{1D165}");
    assert_eq!(normalizer.normalize("\u{2126}"), "Ω"); // ohm sign
    assert_eq!(normalizer.normalize("ﾍﾞ"), "ﾍﾞ"); // half-width unchanged
    assert_eq!(normalizer.normalize("ﾍﾟ"), "ﾍﾟ"); // half-width unchanged
    assert_eq!(normalizer.normalize("ﬁ"), "ﬁ"); // ligature unchanged
    assert_eq!(normalizer.normalize("\u{FDFA}"), "\u{FDFA}"); // ligature unchanged
    assert_eq!(normalizer.normalize("㈎"), "㈎"); // parenthetical unchanged
    assert_eq!(normalizer.normalize("\u{0345}"), "\u{0345}"); // Iota subscript
}

#[test]
fn test_nfkd_basic() {
    let normalizer = DecomposingNormalizerBorrowed::new_nfkd();
    assert_eq!(normalizer.normalize("ä"), "a\u{0308}");
    assert_eq!(normalizer.normalize("Ä"), "A\u{0308}");
    assert_eq!(normalizer.normalize("ệ"), "e\u{0323}\u{0302}");
    assert_eq!(normalizer.normalize("Ệ"), "E\u{0323}\u{0302}");
    assert_eq!(normalizer.normalize("𝅗𝅥"), "𝅗\u{1D165}");
    assert_eq!(normalizer.normalize("\u{2126}"), "Ω"); // ohm sign
    assert_eq!(normalizer.normalize("ﾍﾞ"), "ヘ\u{3099}"); // half-width to full-width
    assert_eq!(normalizer.normalize("ﾍﾟ"), "ヘ\u{309A}"); // half-width to full-width
    assert_eq!(normalizer.normalize("ﬁ"), "fi"); // ligature expanded
    assert_eq!(normalizer.normalize("\u{FDFA}"), "\u{635}\u{644}\u{649} \u{627}\u{644}\u{644}\u{647} \u{639}\u{644}\u{64A}\u{647} \u{648}\u{633}\u{644}\u{645}");
    // ligature expanded
    assert_eq!(normalizer.normalize("㈎"), "(\u{1100}\u{1161})"); // parenthetical expanded
    assert_eq!(normalizer.normalize("\u{0345}"), "\u{0345}"); // Iota subscript
}

#[test]
fn test_nfkd_owned() {
    let owned =
        DecomposingNormalizer::try_new_nfkd_unstable(&icu_normalizer::provider::Baked).unwrap();
    let normalizer = owned.as_borrowed();
    assert_eq!(normalizer.normalize("ä"), "a\u{0308}");
    assert_eq!(normalizer.normalize("Ä"), "A\u{0308}");
    assert_eq!(normalizer.normalize("ệ"), "e\u{0323}\u{0302}");
    assert_eq!(normalizer.normalize("Ệ"), "E\u{0323}\u{0302}");
    assert_eq!(normalizer.normalize("𝅗𝅥"), "𝅗\u{1D165}");
    assert_eq!(normalizer.normalize("\u{2126}"), "Ω"); // ohm sign
    assert_eq!(normalizer.normalize("ﾍﾞ"), "ヘ\u{3099}"); // half-width to full-width
    assert_eq!(normalizer.normalize("ﾍﾟ"), "ヘ\u{309A}"); // half-width to full-width
    assert_eq!(normalizer.normalize("ﬁ"), "fi"); // ligature expanded
    assert_eq!(normalizer.normalize("\u{FDFA}"), "\u{635}\u{644}\u{649} \u{627}\u{644}\u{644}\u{647} \u{639}\u{644}\u{64A}\u{647} \u{648}\u{633}\u{644}\u{645}");
    // ligature expanded
    assert_eq!(normalizer.normalize("㈎"), "(\u{1100}\u{1161})"); // parenthetical expanded
    assert_eq!(normalizer.normalize("\u{0345}"), "\u{0345}"); // Iota subscript
}

#[test]
fn test_nfc_basic() {
    let normalizer = ComposingNormalizerBorrowed::new_nfc();
    assert_eq!(normalizer.normalize("a\u{0308}"), "ä");
    assert_eq!(normalizer.normalize("A\u{0308}"), "Ä");
    assert_eq!(normalizer.normalize("e\u{0323}\u{0302}"), "ệ");
    assert_eq!(normalizer.normalize("E\u{0323}\u{0302}"), "Ệ");
    assert_eq!(normalizer.normalize("𝅗𝅥"), "𝅗\u{1D165}"); // Composition exclusion

    assert_eq!(normalizer.normalize("\u{2126}"), "Ω"); // ohm sign
    assert_eq!(normalizer.normalize("ﾍﾞ"), "ﾍﾞ"); // half-width unchanged
    assert_eq!(normalizer.normalize("ﾍﾟ"), "ﾍﾟ"); // half-width unchanged
    assert_eq!(normalizer.normalize("ﬁ"), "ﬁ"); // ligature unchanged
    assert_eq!(normalizer.normalize("\u{FDFA}"), "\u{FDFA}"); // ligature unchanged
    assert_eq!(normalizer.normalize("㈎"), "㈎"); // parenthetical unchanged
    assert_eq!(normalizer.normalize("\u{0345}"), "\u{0345}"); // Iota subscript
}

#[test]
fn test_nfc_owned() {
    let owned =
        ComposingNormalizer::try_new_nfc_unstable(&icu_normalizer::provider::Baked).unwrap();
    let normalizer = owned.as_borrowed();
    assert_eq!(normalizer.normalize("a\u{0308}"), "ä");
    assert_eq!(normalizer.normalize("A\u{0308}"), "Ä");
    assert_eq!(normalizer.normalize("e\u{0323}\u{0302}"), "ệ");
    assert_eq!(normalizer.normalize("E\u{0323}\u{0302}"), "Ệ");
    assert_eq!(normalizer.normalize("𝅗𝅥"), "𝅗\u{1D165}"); // Composition exclusion

    assert_eq!(normalizer.normalize("\u{2126}"), "Ω"); // ohm sign
    assert_eq!(normalizer.normalize("ﾍﾞ"), "ﾍﾞ"); // half-width unchanged
    assert_eq!(normalizer.normalize("ﾍﾟ"), "ﾍﾟ"); // half-width unchanged
    assert_eq!(normalizer.normalize("ﬁ"), "ﬁ"); // ligature unchanged
    assert_eq!(normalizer.normalize("\u{FDFA}"), "\u{FDFA}"); // ligature unchanged
    assert_eq!(normalizer.normalize("㈎"), "㈎"); // parenthetical unchanged
    assert_eq!(normalizer.normalize("\u{0345}"), "\u{0345}"); // Iota subscript
}

#[test]
fn test_nfkc_basic() {
    let normalizer = ComposingNormalizerBorrowed::new_nfkc();
    assert_eq!(normalizer.normalize("a\u{0308}"), "ä");
    assert_eq!(normalizer.normalize("A\u{0308}"), "Ä");
    assert_eq!(normalizer.normalize("e\u{0323}\u{0302}"), "ệ");
    assert_eq!(normalizer.normalize("E\u{0323}\u{0302}"), "Ệ");
    assert_eq!(normalizer.normalize("𝅗𝅥"), "𝅗\u{1D165}"); // Composition exclusion

    assert_eq!(normalizer.normalize("\u{2126}"), "Ω"); // ohm sign
    assert_eq!(normalizer.normalize("ﾍﾞ"), "ベ"); // half-width to full-width, the compose
    assert_eq!(normalizer.normalize("ﾍﾟ"), "ペ"); // half-width to full-width, the compose
    assert_eq!(normalizer.normalize("ﬁ"), "fi"); // ligature expanded
    assert_eq!(normalizer.normalize("\u{FDFA}"), "\u{0635}\u{0644}\u{0649} \u{0627}\u{0644}\u{0644}\u{0647} \u{0639}\u{0644}\u{064A}\u{0647} \u{0648}\u{0633}\u{0644}\u{0645}");
    // ligature expanded
    assert_eq!(normalizer.normalize("㈎"), "(가)"); // parenthetical expanded and partially recomposed
    assert_eq!(normalizer.normalize("\u{0345}"), "\u{0345}"); // Iota subscript
}

#[test]
fn test_nfkc_owned() {
    let owned =
        ComposingNormalizer::try_new_nfkc_unstable(&icu_normalizer::provider::Baked).unwrap();
    let normalizer = owned.as_borrowed();
    assert_eq!(normalizer.normalize("a\u{0308}"), "ä");
    assert_eq!(normalizer.normalize("A\u{0308}"), "Ä");
    assert_eq!(normalizer.normalize("e\u{0323}\u{0302}"), "ệ");
    assert_eq!(normalizer.normalize("E\u{0323}\u{0302}"), "Ệ");
    assert_eq!(normalizer.normalize("𝅗𝅥"), "𝅗\u{1D165}"); // Composition exclusion

    assert_eq!(normalizer.normalize("\u{2126}"), "Ω"); // ohm sign
    assert_eq!(normalizer.normalize("ﾍﾞ"), "ベ"); // half-width to full-width, the compose
    assert_eq!(normalizer.normalize("ﾍﾟ"), "ペ"); // half-width to full-width, the compose
    assert_eq!(normalizer.normalize("ﬁ"), "fi"); // ligature expanded
    assert_eq!(normalizer.normalize("\u{FDFA}"), "\u{0635}\u{0644}\u{0649} \u{0627}\u{0644}\u{0644}\u{0647} \u{0639}\u{0644}\u{064A}\u{0647} \u{0648}\u{0633}\u{0644}\u{0645}");
    // ligature expanded
    assert_eq!(normalizer.normalize("㈎"), "(가)"); // parenthetical expanded and partially recomposed
    assert_eq!(normalizer.normalize("\u{0345}"), "\u{0345}"); // Iota subscript
}

#[test]
fn test_uts46_map_normalize() {
    let mapper = Uts46MapperBorrowed::new();
    assert_eq!(
        mapper
            .map_normalize("a\u{0308}".chars())
            .collect::<String>(),
        "ä"
    );
    assert_eq!(
        mapper
            .map_normalize("A\u{0308}".chars())
            .collect::<String>(),
        "ä"
    );
    assert_eq!(
        mapper
            .map_normalize("e\u{0323}\u{0302}".chars())
            .collect::<String>(),
        "ệ"
    );
    assert_eq!(
        mapper
            .map_normalize("E\u{0323}\u{0302}".chars())
            .collect::<String>(),
        "ệ"
    );
    assert_eq!(
        mapper.map_normalize("𝅗𝅥".chars()).collect::<String>(),
        "𝅗\u{1D165}"
    ); // Composition exclusion

    assert_eq!(
        mapper.map_normalize("\u{2126}".chars()).collect::<String>(),
        "ω"
    ); // ohm sign
    assert_eq!(mapper.map_normalize("ﾍﾞ".chars()).collect::<String>(), "ベ"); // half-width to full-width, the compose
    assert_eq!(mapper.map_normalize("ﾍﾟ".chars()).collect::<String>(), "ペ"); // half-width to full-width, the compose
    assert_eq!(mapper.map_normalize("ﬁ".chars()).collect::<String>(), "fi"); // ligature expanded
    assert_eq!(mapper.map_normalize("\u{FDFA}".chars()).collect::<String>(), "\u{0635}\u{0644}\u{0649} \u{0627}\u{0644}\u{0644}\u{0647} \u{0639}\u{0644}\u{064A}\u{0647} \u{0648}\u{0633}\u{0644}\u{0645}");
    // ligature expanded
    assert_eq!(
        mapper.map_normalize("㈎".chars()).collect::<String>(),
        "(가)"
    ); // parenthetical expanded and partially recomposed

    // Deviations (UTS 46, 6 Mapping Table Derivation, Step 4)
    assert_eq!(
        mapper.map_normalize("\u{200C}".chars()).collect::<String>(),
        "\u{200C}"
    );
    assert_eq!(
        mapper.map_normalize("\u{200D}".chars()).collect::<String>(),
        "\u{200D}"
    );
    assert_eq!(mapper.map_normalize("ß".chars()).collect::<String>(), "ß");
    assert_eq!(mapper.map_normalize("ς".chars()).collect::<String>(), "ς");

    // Iota subscript
    assert_eq!(
        mapper.map_normalize("\u{0345}".chars()).collect::<String>(),
        "ι"
    );

    // Disallowed
    assert_eq!(
        mapper.map_normalize("\u{061C}".chars()).collect::<String>(),
        "\u{FFFD}"
    );

    // Ignored
    assert_eq!(
        mapper
            .map_normalize("a\u{180B}b".chars())
            .collect::<String>(),
        "ab"
    );
}

#[test]
fn test_uts46_owned() {
    let owned = Uts46Mapper::try_new(&icu_normalizer::provider::Baked).unwrap();
    let mapper = owned.as_borrowed();
    assert_eq!(
        mapper
            .map_normalize("a\u{0308}".chars())
            .collect::<String>(),
        "ä"
    );
    assert_eq!(
        mapper
            .map_normalize("A\u{0308}".chars())
            .collect::<String>(),
        "ä"
    );
    assert_eq!(
        mapper
            .map_normalize("e\u{0323}\u{0302}".chars())
            .collect::<String>(),
        "ệ"
    );
    assert_eq!(
        mapper
            .map_normalize("E\u{0323}\u{0302}".chars())
            .collect::<String>(),
        "ệ"
    );
    assert_eq!(
        mapper.map_normalize("𝅗𝅥".chars()).collect::<String>(),
        "𝅗\u{1D165}"
    ); // Composition exclusion

    assert_eq!(
        mapper.map_normalize("\u{2126}".chars()).collect::<String>(),
        "ω"
    ); // ohm sign
    assert_eq!(mapper.map_normalize("ﾍﾞ".chars()).collect::<String>(), "ベ"); // half-width to full-width, the compose
    assert_eq!(mapper.map_normalize("ﾍﾟ".chars()).collect::<String>(), "ペ"); // half-width to full-width, the compose
    assert_eq!(mapper.map_normalize("ﬁ".chars()).collect::<String>(), "fi"); // ligature expanded
    assert_eq!(mapper.map_normalize("\u{FDFA}".chars()).collect::<String>(), "\u{0635}\u{0644}\u{0649} \u{0627}\u{0644}\u{0644}\u{0647} \u{0639}\u{0644}\u{064A}\u{0647} \u{0648}\u{0633}\u{0644}\u{0645}");
    // ligature expanded
    assert_eq!(
        mapper.map_normalize("㈎".chars()).collect::<String>(),
        "(가)"
    ); // parenthetical expanded and partially recomposed

    // Deviations (UTS 46, 6 Mapping Table Derivation, Step 4)
    assert_eq!(
        mapper.map_normalize("\u{200C}".chars()).collect::<String>(),
        "\u{200C}"
    );
    assert_eq!(
        mapper.map_normalize("\u{200D}".chars()).collect::<String>(),
        "\u{200D}"
    );
    assert_eq!(mapper.map_normalize("ß".chars()).collect::<String>(), "ß");
    assert_eq!(mapper.map_normalize("ς".chars()).collect::<String>(), "ς");

    // Iota subscript
    assert_eq!(
        mapper.map_normalize("\u{0345}".chars()).collect::<String>(),
        "ι"
    );

    // Disallowed
    assert_eq!(
        mapper.map_normalize("\u{061C}".chars()).collect::<String>(),
        "\u{FFFD}"
    );

    // Ignored
    assert_eq!(
        mapper
            .map_normalize("a\u{180B}b".chars())
            .collect::<String>(),
        "ab"
    );
}

#[test]
fn test_uts46_normalize_validate() {
    let mapper = Uts46MapperBorrowed::new();
    assert_eq!(
        mapper
            .normalize_validate("a\u{0308}".chars())
            .collect::<String>(),
        "ä"
    );
    assert_eq!(
        mapper
            .normalize_validate("A\u{0308}".chars())
            .collect::<String>(),
        "ä"
    );
    assert_eq!(
        mapper
            .normalize_validate("e\u{0323}\u{0302}".chars())
            .collect::<String>(),
        "ệ"
    );
    assert_eq!(
        mapper
            .normalize_validate("E\u{0323}\u{0302}".chars())
            .collect::<String>(),
        "ệ"
    );
    assert_eq!(
        mapper.normalize_validate("𝅗𝅥".chars()).collect::<String>(),
        "𝅗\u{1D165}"
    ); // Composition exclusion

    assert_eq!(
        mapper
            .normalize_validate("\u{2126}".chars())
            .collect::<String>(),
        "ω"
    ); // ohm sign
    assert_eq!(
        mapper.normalize_validate("ﾍﾞ".chars()).collect::<String>(),
        "ベ"
    ); // half-width to full-width, the compose
    assert_eq!(
        mapper.normalize_validate("ﾍﾟ".chars()).collect::<String>(),
        "ペ"
    ); // half-width to full-width, the compose
    assert_eq!(
        mapper.normalize_validate("ﬁ".chars()).collect::<String>(),
        "fi"
    ); // ligature expanded
    assert_eq!(mapper.normalize_validate("\u{FDFA}".chars()).collect::<String>(), "\u{0635}\u{0644}\u{0649} \u{0627}\u{0644}\u{0644}\u{0647} \u{0639}\u{0644}\u{064A}\u{0647} \u{0648}\u{0633}\u{0644}\u{0645}");
    // ligature expanded
    assert_eq!(
        mapper.normalize_validate("㈎".chars()).collect::<String>(),
        "(가)"
    ); // parenthetical expanded and partially recomposed

    // Deviations (UTS 46, 6 Mapping Table Derivation, Step 4)
    assert_eq!(
        mapper
            .normalize_validate("\u{200C}".chars())
            .collect::<String>(),
        "\u{200C}"
    );
    assert_eq!(
        mapper
            .normalize_validate("\u{200D}".chars())
            .collect::<String>(),
        "\u{200D}"
    );
    assert_eq!(
        mapper.normalize_validate("ß".chars()).collect::<String>(),
        "ß"
    );
    assert_eq!(
        mapper.normalize_validate("ς".chars()).collect::<String>(),
        "ς"
    );

    // Iota subscript
    assert_eq!(
        mapper
            .normalize_validate("\u{0345}".chars())
            .collect::<String>(),
        "ι"
    );

    // Disallowed
    assert_eq!(
        mapper
            .normalize_validate("\u{061C}".chars())
            .collect::<String>(),
        "\u{FFFD}"
    );

    // Ignored
    assert_eq!(
        mapper
            .normalize_validate("a\u{180B}b".chars())
            .collect::<String>(),
        "a\u{FFFD}b"
    );
}

type StackString = arraystring::ArrayString<arraystring::typenum::U48>;

#[test]
fn test_nfd_str_to() {
    let normalizer = DecomposingNormalizerBorrowed::new_nfd();

    let mut buf = StackString::new();
    assert!(normalizer.normalize_to("ä", &mut buf).is_ok());
    assert_eq!(&buf, "a\u{0308}");

    buf.clear();
    assert!(normalizer.normalize_to("ệ", &mut buf).is_ok());
    assert_eq!(&buf, "e\u{0323}\u{0302}");
}

#[test]
fn test_nfd_utf8_to() {
    let normalizer = DecomposingNormalizerBorrowed::new_nfd();

    let mut buf = StackString::new();
    assert!(normalizer
        .normalize_utf8_to("ä".as_bytes(), &mut buf)
        .is_ok());
    assert_eq!(&buf, "a\u{0308}");

    buf.clear();
    assert!(normalizer
        .normalize_utf8_to("ệ".as_bytes(), &mut buf)
        .is_ok());
    assert_eq!(&buf, "e\u{0323}\u{0302}");
}

type StackVec = arrayvec::ArrayVec<u16, 32>;

#[test]
fn test_nfd_utf16_to() {
    let normalizer = DecomposingNormalizerBorrowed::new_nfd();

    let mut buf = StackVec::new();
    assert!(normalizer
        .normalize_utf16_to([0x00E4u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(&buf, [0x0061u16, 0x0308u16].as_slice());

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to([0x1EC7u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(&buf, [0x0065u16, 0x0323u16, 0x0302u16].as_slice());
}

#[test]
fn test_nfc_str_to() {
    let normalizer = ComposingNormalizerBorrowed::new_nfc();

    let mut buf = StackString::new();
    assert!(normalizer.normalize_to("a\u{0308}", &mut buf).is_ok());
    assert_eq!(&buf, "ä");

    buf.clear();
    assert!(normalizer
        .normalize_to("e\u{0323}\u{0302}", &mut buf)
        .is_ok());
    assert_eq!(&buf, "ệ");
}

#[test]
fn test_nfc_utf8_to() {
    let normalizer = ComposingNormalizerBorrowed::new_nfc();

    let mut buf = StackString::new();
    assert!(normalizer
        .normalize_utf8_to("a\u{0308}".as_bytes(), &mut buf)
        .is_ok());
    assert_eq!(&buf, "ä");

    buf.clear();
    assert!(normalizer
        .normalize_utf8_to("e\u{0323}\u{0302}".as_bytes(), &mut buf)
        .is_ok());
    assert_eq!(&buf, "ệ");
}

#[test]
fn test_nfc_utf16_to() {
    let normalizer = ComposingNormalizerBorrowed::new_nfc();

    let mut buf = StackVec::new();
    assert!(normalizer
        .normalize_utf16_to([0x0061u16, 0x0308u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(&buf, [0x00E4u16].as_slice());

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to([0x0065u16, 0x0323u16, 0x0302u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(&buf, [0x1EC7u16].as_slice());
}

#[test]
fn test_nfc_utf8_to_errors() {
    let normalizer = ComposingNormalizerBorrowed::new_nfc();

    let mut buf = StackString::new();
    assert!(normalizer
        .normalize_utf8_to(b"\xFFa\xCC\x88\xFF", &mut buf)
        .is_ok());
    assert_eq!(&buf, "\u{FFFD}ä\u{FFFD}");

    buf.clear();
    assert!(normalizer
        .normalize_utf8_to(b"\x80e\xCC\xA3\xCC\x82\x80", &mut buf)
        .is_ok());
    assert_eq!(&buf, "\u{FFFD}ệ\u{FFFD}");

    buf.clear();
    assert!(normalizer
        .normalize_utf8_to(b"aaa\xFFaaa\xFFaaa", &mut buf)
        .is_ok());
    assert_eq!(&buf, "aaa\u{FFFD}aaa\u{FFFD}aaa");

    buf.clear();
    assert!(normalizer
        .normalize_utf8_to(b"aaa\xE2\x98aaa\xE2\x98aaa", &mut buf)
        .is_ok());
    assert_eq!(&buf, "aaa\u{FFFD}aaa\u{FFFD}aaa");
}

#[test]
fn test_nfd_utf8_to_errors() {
    let normalizer = DecomposingNormalizerBorrowed::new_nfd();

    let mut buf = StackString::new();
    assert!(normalizer
        .normalize_utf8_to(b"\xFF\xC3\xA4\xFF", &mut buf)
        .is_ok());
    assert_eq!(&buf, "\u{FFFD}a\u{0308}\u{FFFD}");

    buf.clear();
    assert!(normalizer
        .normalize_utf8_to(b"\x80\xE1\xBB\x87\x80", &mut buf)
        .is_ok());
    assert_eq!(&buf, "\u{FFFD}e\u{0323}\u{0302}\u{FFFD}");

    buf.clear();
    assert!(normalizer
        .normalize_utf8_to(b"aaa\xFFaaa\xFFaaa", &mut buf)
        .is_ok());
    assert_eq!(&buf, "aaa\u{FFFD}aaa\u{FFFD}aaa");

    buf.clear();
    assert!(normalizer
        .normalize_utf8_to(b"aaa\xE2\x98aaa\xE2\x98aaa", &mut buf)
        .is_ok());
    assert_eq!(&buf, "aaa\u{FFFD}aaa\u{FFFD}aaa");
}

#[test]
fn test_nfc_utf16_to_errors() {
    let normalizer = ComposingNormalizerBorrowed::new_nfc();

    let mut buf = StackVec::new();
    assert!(normalizer
        .normalize_utf16_to([0xD800u16, 0x0061u16, 0x0308u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(&buf, [0xFFFDu16, 0x00E4u16].as_slice());

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to([0xDC00u16, 0x0061u16, 0x0308u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(&buf, [0xFFFDu16, 0x00E4u16].as_slice());

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xD800u16, 0x0061u16, 0x0308u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(&buf, [0x0061u16, 0xFFFDu16, 0x00E4u16].as_slice());

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xDC00u16, 0x0061u16, 0x0308u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(&buf, [0x0061u16, 0xFFFDu16, 0x00E4u16].as_slice());

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xD800u16, 0x0061u16, 0x0308u16, 0xD800u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x00E4u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xDC00u16, 0x0061u16, 0x0308u16, 0xDC00u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x00E4u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xD800u16, 0x0061u16, 0x0061u16, 0xD800u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0061u16, 0x0061u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xDC00u16, 0x0061u16, 0x0061u16, 0xDC00u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0061u16, 0x0061u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xD800u16, 0x0308u16, 0xD800u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0308u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xDC00u16, 0x0308u16, 0xDC00u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0308u16, 0xFFFDu16].as_slice()
    );
}

#[test]
fn test_nfd_utf16_to_errors() {
    let normalizer = DecomposingNormalizerBorrowed::new_nfd();

    let mut buf = StackVec::new();
    assert!(normalizer
        .normalize_utf16_to([0xD800u16, 0x00E4u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(&buf, [0xFFFDu16, 0x0061u16, 0x0308u16].as_slice());

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to([0xDC00u16, 0x00E4u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(&buf, [0xFFFDu16, 0x0061u16, 0x0308u16].as_slice());

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to([0x0061u16, 0xD800u16, 0x00E4u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0061u16, 0x0308u16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to([0x0061u16, 0xDC00u16, 0x00E4u16].as_slice(), &mut buf)
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0061u16, 0x0308u16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xD800u16, 0x00E4u16, 0xD800u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0061u16, 0x0308u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xDC00u16, 0x00E4u16, 0xDC00u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0061u16, 0x0308u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xD800u16, 0x0061u16, 0x0061u16, 0xD800u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0061u16, 0x0061u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xDC00u16, 0x0061u16, 0x0061u16, 0xDC00u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0061u16, 0x0061u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xD800u16, 0x0308u16, 0xD800u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0308u16, 0xFFFDu16].as_slice()
    );

    buf.clear();
    assert!(normalizer
        .normalize_utf16_to(
            [0x0061u16, 0xDC00u16, 0x0308u16, 0xDC00u16].as_slice(),
            &mut buf
        )
        .is_ok());
    assert_eq!(
        &buf,
        [0x0061u16, 0xFFFDu16, 0x0308u16, 0xFFFDu16].as_slice()
    );
}

use atoi::FromRadix16;
use icu_properties::props::CanonicalCombiningClass;

/// Parse five semicolon-terminated strings consisting of space-separated hexadecimal scalar values
fn parse_hex(mut hexes: &[u8]) -> [StackString; 5] {
    let mut strings = [
        StackString::new(),
        StackString::new(),
        StackString::new(),
        StackString::new(),
        StackString::new(),
    ];
    let mut current = 0;
    loop {
        let (scalar, mut offset) = u32::from_radix_16(hexes);
        let c = core::char::from_u32(scalar).unwrap();
        strings[current].try_push(c).unwrap();
        match hexes[offset] {
            b';' => {
                current += 1;
                if current == strings.len() {
                    return strings;
                }
                offset += 1;
            }
            b' ' => {
                offset += 1;
            }
            _ => {
                panic!("Bad format: Garbage");
            }
        }
        hexes = &hexes[offset..];
    }
}

#[test]
fn test_conformance() {
    let nfd = DecomposingNormalizerBorrowed::new_nfd();
    let nfkd = DecomposingNormalizerBorrowed::new_nfkd();
    let nfc = ComposingNormalizerBorrowed::new_nfc();
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();

    let mut prev = 0u32;
    let mut part = 0u8;
    let data = include_bytes!("data/NormalizationTest.txt");
    let lines = data.split(|b| b == &b'\n');
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(b"#") {
            continue;
        }
        if line.starts_with(&b"@Part"[..]) {
            part = line[5] - b'0';
            if part == 2 {
                for u in prev + 1..=0x10FFFF {
                    if let Some(c) = char::from_u32(u) {
                        assert!(nfd
                            .normalize_iter(core::iter::once(c))
                            .eq(core::iter::once(c)));
                        assert!(nfkd
                            .normalize_iter(core::iter::once(c))
                            .eq(core::iter::once(c)));
                        assert!(nfc
                            .normalize_iter(core::iter::once(c))
                            .eq(core::iter::once(c)));
                        assert!(nfkc
                            .normalize_iter(core::iter::once(c))
                            .eq(core::iter::once(c)));
                    }
                }
            }
            continue;
        }
        let strings = parse_hex(line);
        // 0: source
        // 1: NFC
        // 2: NFD
        // 3: NFKC
        // 4: NFKD
        if part == 1 {
            let mut iter = strings[0].chars();
            let current = iter.next().unwrap();
            assert_eq!(iter.next(), None);
            let current_u = u32::from(current);
            for u in prev + 1..current_u {
                if let Some(c) = char::from_u32(u) {
                    assert!(nfd
                        .normalize_iter(core::iter::once(c))
                        .eq(core::iter::once(c)));
                    assert!(nfkd
                        .normalize_iter(core::iter::once(c))
                        .eq(core::iter::once(c)));
                    assert!(nfc
                        .normalize_iter(core::iter::once(c))
                        .eq(core::iter::once(c)));
                    assert!(nfkc
                        .normalize_iter(core::iter::once(c))
                        .eq(core::iter::once(c)));
                }
            }
            prev = current_u;
        }
        // NFC
        assert!(nfc
            .normalize_iter(strings[0].chars())
            .eq(strings[1].chars()));
        assert!(nfc
            .normalize_iter(strings[1].chars())
            .eq(strings[1].chars()));
        assert!(nfc
            .normalize_iter(strings[2].chars())
            .eq(strings[1].chars()));

        assert!(nfc
            .normalize_iter(strings[3].chars())
            .eq(strings[3].chars()));
        assert!(nfc
            .normalize_iter(strings[4].chars())
            .eq(strings[3].chars()));

        // NFD
        assert!(nfd
            .normalize_iter(strings[0].chars())
            .eq(strings[2].chars()));
        assert!(nfd
            .normalize_iter(strings[1].chars())
            .eq(strings[2].chars()));
        assert!(nfd
            .normalize_iter(strings[2].chars())
            .eq(strings[2].chars()));

        assert!(nfd
            .normalize_iter(strings[3].chars())
            .eq(strings[4].chars()));
        assert!(nfd
            .normalize_iter(strings[4].chars())
            .eq(strings[4].chars()));

        // NFKC
        assert!(nfkc
            .normalize_iter(strings[0].chars())
            .eq(strings[3].chars()));
        assert!(nfkc
            .normalize_iter(strings[1].chars())
            .eq(strings[3].chars()));
        assert!(nfkc
            .normalize_iter(strings[2].chars())
            .eq(strings[3].chars()));
        assert!(nfkc
            .normalize_iter(strings[3].chars())
            .eq(strings[3].chars()));
        assert!(nfkc
            .normalize_iter(strings[4].chars())
            .eq(strings[3].chars()));

        // NFKD
        assert!(nfkd
            .normalize_iter(strings[0].chars())
            .eq(strings[4].chars()));
        assert!(nfkd
            .normalize_iter(strings[1].chars())
            .eq(strings[4].chars()));
        assert!(nfkd
            .normalize_iter(strings[2].chars())
            .eq(strings[4].chars()));
        assert!(nfkd
            .normalize_iter(strings[3].chars())
            .eq(strings[4].chars()));
        assert!(nfkd
            .normalize_iter(strings[4].chars())
            .eq(strings[4].chars()));
    }
}

// Commented out, because we don't currently have a way to force a no-op set for testing.
// #[test]
// fn test_hangul() {
//     use icu_collections::codepointinvlist::{CodePointSet, CodePointSetBuilder};
//     use zerofrom::ZeroFrom;
//     let builder = CodePointSetBuilder::new();
//     let set: CodePointSet = builder.build();

//     let normalizer: ComposingNormalizer = ComposingNormalizerBorrowed::new_nfc();
//     {
//         let mut norm_iter = normalizer.normalize_iter("A\u{AC00}\u{11A7}".chars());
//         // Pessimize passthrough to avoid hiding bugs.
//         norm_iter
//             .decomposition
//             .potential_passthrough_and_not_backward_combining = Some(ZeroFrom::zero_from(&set));
//         assert!(norm_iter.eq("A\u{AC00}\u{11A7}".chars()));
//     }
//     {
//         let mut norm_iter = normalizer.normalize_iter("A\u{AC00}\u{11C2}".chars());
//         // Pessimize passthrough to avoid hiding bugs.
//         norm_iter
//             .decomposition
//             .potential_passthrough_and_not_backward_combining = Some(ZeroFrom::zero_from(&set));
//         assert!(norm_iter.eq("A\u{AC1B}".chars()));
//     }
// }

fn str_to_utf16(s: &str, sink: &mut StackVec) {
    sink.clear();
    let mut buf = [0u16; 2];
    for c in s.chars() {
        sink.try_extend_from_slice(c.encode_utf16(&mut buf))
            .unwrap();
    }
}

fn char_to_utf16(c: char, sink: &mut StackVec) {
    sink.clear();
    let mut buf = [0u16; 2];
    sink.try_extend_from_slice(c.encode_utf16(&mut buf))
        .unwrap();
}

fn str_to_str(s: &str, sink: &mut StackString) {
    sink.clear();
    sink.try_push_str(s).unwrap();
}

fn char_to_str(c: char, sink: &mut StackString) {
    sink.clear();
    sink.try_push(c).unwrap();
}

#[test]
fn test_conformance_utf16() {
    let nfd = DecomposingNormalizerBorrowed::new_nfd();
    let nfkd = DecomposingNormalizerBorrowed::new_nfkd();
    let nfc = ComposingNormalizerBorrowed::new_nfc();
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();

    let mut input = StackVec::new();
    let mut normalized = StackVec::new();
    let mut expected = StackVec::new();

    let mut prev = 0u32;
    let mut part = 0u8;
    let data = include_bytes!("data/NormalizationTest.txt");
    let lines = data.split(|b| b == &b'\n');
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(b"#") {
            continue;
        }
        if line.starts_with(&b"@Part"[..]) {
            part = line[5] - b'0';
            if part == 2 {
                for u in prev + 1..=0x10FFFF {
                    if let Some(c) = char::from_u32(u) {
                        normalized.clear();
                        char_to_utf16(c, &mut input);
                        assert!(nfd.normalize_utf16_to(&input, &mut normalized).is_ok());
                        assert_eq!(&normalized, &input);

                        normalized.clear();
                        char_to_utf16(c, &mut input);
                        assert!(nfkd.normalize_utf16_to(&input, &mut normalized).is_ok());
                        assert_eq!(&normalized, &input);

                        normalized.clear();
                        char_to_utf16(c, &mut input);
                        assert!(nfc.normalize_utf16_to(&input, &mut normalized).is_ok());
                        assert_eq!(&normalized, &input);

                        normalized.clear();
                        char_to_utf16(c, &mut input);
                        assert!(nfkc.normalize_utf16_to(&input, &mut normalized).is_ok());
                        assert_eq!(&normalized, &input);
                    }
                }
            }
            continue;
        }
        let strings = parse_hex(line);
        // 0: source
        // 1: NFC
        // 2: NFD
        // 3: NFKC
        // 4: NFKD
        if part == 1 {
            let mut iter = strings[0].chars();
            let current = iter.next().unwrap();
            assert_eq!(iter.next(), None);
            let current_u = u32::from(current);
            for u in prev + 1..current_u {
                if let Some(c) = char::from_u32(u) {
                    normalized.clear();
                    char_to_utf16(c, &mut input);
                    assert!(nfd.normalize_utf16_to(&input, &mut normalized).is_ok());
                    assert_eq!(&normalized, &input);

                    normalized.clear();
                    char_to_utf16(c, &mut input);
                    assert!(nfkd.normalize_utf16_to(&input, &mut normalized).is_ok());
                    assert_eq!(&normalized, &input);

                    normalized.clear();
                    char_to_utf16(c, &mut input);
                    assert!(nfc.normalize_utf16_to(&input, &mut normalized).is_ok());
                    assert_eq!(&normalized, &input);

                    normalized.clear();
                    char_to_utf16(c, &mut input);
                    assert!(nfkc.normalize_utf16_to(&input, &mut normalized).is_ok());
                    assert_eq!(&normalized, &input);
                }
            }
            prev = current_u;
        }
        // NFC
        normalized.clear();
        str_to_utf16(&strings[0], &mut input);
        str_to_utf16(&strings[1], &mut expected);
        assert!(nfc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[1], &mut input);
        str_to_utf16(&strings[1], &mut expected);
        assert!(nfc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[2], &mut input);
        str_to_utf16(&strings[1], &mut expected);
        assert!(nfc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[3], &mut input);
        str_to_utf16(&strings[3], &mut expected);
        assert!(nfc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[4], &mut input);
        str_to_utf16(&strings[3], &mut expected);
        assert!(nfc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        // NFD
        normalized.clear();
        str_to_utf16(&strings[0], &mut input);
        str_to_utf16(&strings[2], &mut expected);
        assert!(nfd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[1], &mut input);
        str_to_utf16(&strings[2], &mut expected);
        assert!(nfd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[2], &mut input);
        str_to_utf16(&strings[2], &mut expected);
        assert!(nfd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[3], &mut input);
        str_to_utf16(&strings[4], &mut expected);
        assert!(nfd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[4], &mut input);
        str_to_utf16(&strings[4], &mut expected);
        assert!(nfd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        // NFKC
        normalized.clear();
        str_to_utf16(&strings[0], &mut input);
        str_to_utf16(&strings[3], &mut expected);
        assert!(nfkc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[1], &mut input);
        str_to_utf16(&strings[3], &mut expected);
        assert!(nfkc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[2], &mut input);
        str_to_utf16(&strings[3], &mut expected);
        assert!(nfkc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[3], &mut input);
        str_to_utf16(&strings[3], &mut expected);
        assert!(nfkc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[4], &mut input);
        str_to_utf16(&strings[3], &mut expected);
        assert!(nfkc.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        // NFKD
        normalized.clear();
        str_to_utf16(&strings[0], &mut input);
        str_to_utf16(&strings[4], &mut expected);
        assert!(nfkd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[1], &mut input);
        str_to_utf16(&strings[4], &mut expected);
        assert!(nfkd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[2], &mut input);
        str_to_utf16(&strings[4], &mut expected);
        assert!(nfkd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[3], &mut input);
        str_to_utf16(&strings[4], &mut expected);
        assert!(nfkd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_utf16(&strings[4], &mut input);
        str_to_utf16(&strings[4], &mut expected);
        assert!(nfkd.normalize_utf16_to(&input, &mut normalized).is_ok());
        assert_eq!(&normalized, &expected);
    }
}

#[test]
fn test_conformance_utf8() {
    let nfd = DecomposingNormalizerBorrowed::new_nfd();
    let nfkd = DecomposingNormalizerBorrowed::new_nfkd();
    let nfc = ComposingNormalizerBorrowed::new_nfc();
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();

    let mut input = StackString::new();
    let mut normalized = StackString::new();
    let mut expected = StackString::new();

    let mut prev = 0u32;
    let mut part = 0u8;
    let data = include_bytes!("data/NormalizationTest.txt");
    let lines = data.split(|b| b == &b'\n');
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(b"#") {
            continue;
        }
        if line.starts_with(&b"@Part"[..]) {
            part = line[5] - b'0';
            if part == 2 {
                for u in prev + 1..=0x10FFFF {
                    if let Some(c) = char::from_u32(u) {
                        normalized.clear();
                        char_to_str(c, &mut input);
                        assert!(nfd
                            .normalize_utf8_to(input.as_bytes(), &mut normalized)
                            .is_ok());
                        assert_eq!(&normalized, &input);

                        normalized.clear();
                        char_to_str(c, &mut input);
                        assert!(nfkd
                            .normalize_utf8_to(input.as_bytes(), &mut normalized)
                            .is_ok());
                        assert_eq!(&normalized, &input);

                        normalized.clear();
                        char_to_str(c, &mut input);
                        assert!(nfc
                            .normalize_utf8_to(input.as_bytes(), &mut normalized)
                            .is_ok());
                        assert_eq!(&normalized, &input);

                        normalized.clear();
                        char_to_str(c, &mut input);
                        assert!(nfkc
                            .normalize_utf8_to(input.as_bytes(), &mut normalized)
                            .is_ok());
                        assert_eq!(&normalized, &input);
                    }
                }
            }
            continue;
        }
        let strings = parse_hex(line);
        // 0: source
        // 1: NFC
        // 2: NFD
        // 3: NFKC
        // 4: NFKD
        if part == 1 {
            let mut iter = strings[0].chars();
            let current = iter.next().unwrap();
            assert_eq!(iter.next(), None);
            let current_u = u32::from(current);
            for u in prev + 1..current_u {
                if let Some(c) = char::from_u32(u) {
                    normalized.clear();
                    char_to_str(c, &mut input);
                    assert!(nfd
                        .normalize_utf8_to(input.as_bytes(), &mut normalized)
                        .is_ok());
                    assert_eq!(&normalized, &input);

                    normalized.clear();
                    char_to_str(c, &mut input);
                    assert!(nfkd
                        .normalize_utf8_to(input.as_bytes(), &mut normalized)
                        .is_ok());
                    assert_eq!(&normalized, &input);

                    normalized.clear();
                    char_to_str(c, &mut input);
                    assert!(nfc
                        .normalize_utf8_to(input.as_bytes(), &mut normalized)
                        .is_ok());
                    assert_eq!(&normalized, &input);

                    normalized.clear();
                    char_to_str(c, &mut input);
                    assert!(nfkc
                        .normalize_utf8_to(input.as_bytes(), &mut normalized)
                        .is_ok());
                    assert_eq!(&normalized, &input);
                }
            }
            prev = current_u;
        }
        // NFC
        normalized.clear();
        str_to_str(&strings[0], &mut input);
        str_to_str(&strings[1], &mut expected);
        assert!(nfc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[1], &mut input);
        str_to_str(&strings[1], &mut expected);
        assert!(nfc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[2], &mut input);
        str_to_str(&strings[1], &mut expected);
        assert!(nfc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[3], &mut input);
        str_to_str(&strings[3], &mut expected);
        assert!(nfc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[4], &mut input);
        str_to_str(&strings[3], &mut expected);
        assert!(nfc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        // NFD
        normalized.clear();
        str_to_str(&strings[0], &mut input);
        str_to_str(&strings[2], &mut expected);
        assert!(nfd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[1], &mut input);
        str_to_str(&strings[2], &mut expected);
        assert!(nfd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[2], &mut input);
        str_to_str(&strings[2], &mut expected);
        assert!(nfd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[3], &mut input);
        str_to_str(&strings[4], &mut expected);
        assert!(nfd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[4], &mut input);
        str_to_str(&strings[4], &mut expected);
        assert!(nfd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        // NFKC
        normalized.clear();
        str_to_str(&strings[0], &mut input);
        str_to_str(&strings[3], &mut expected);
        assert!(nfkc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[1], &mut input);
        str_to_str(&strings[3], &mut expected);
        assert!(nfkc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[2], &mut input);
        str_to_str(&strings[3], &mut expected);
        assert!(nfkc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[3], &mut input);
        str_to_str(&strings[3], &mut expected);
        assert!(nfkc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[4], &mut input);
        str_to_str(&strings[3], &mut expected);
        assert!(nfkc
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        // NFKD
        normalized.clear();
        str_to_str(&strings[0], &mut input);
        str_to_str(&strings[4], &mut expected);
        assert!(nfkd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[1], &mut input);
        str_to_str(&strings[4], &mut expected);
        assert!(nfkd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[2], &mut input);
        str_to_str(&strings[4], &mut expected);
        assert!(nfkd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[3], &mut input);
        str_to_str(&strings[4], &mut expected);
        assert!(nfkd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);

        normalized.clear();
        str_to_str(&strings[4], &mut input);
        str_to_str(&strings[4], &mut expected);
        assert!(nfkd
            .normalize_utf8_to(input.as_bytes(), &mut normalized)
            .is_ok());
        assert_eq!(&normalized, &expected);
    }
}

#[test]
fn test_canonical_composition() {
    let comp = CanonicalCompositionBorrowed::new();

    assert_eq!(comp.compose('a', 'b'), None); // Just two starters

    assert_eq!(comp.compose('a', '\u{0308}'), Some('ä'));
    assert_eq!(comp.compose('A', '\u{0308}'), Some('Ä'));
    assert_eq!(comp.compose('ẹ', '\u{0302}'), Some('ệ'));
    assert_eq!(comp.compose('Ẹ', '\u{0302}'), Some('Ệ'));
    assert_eq!(comp.compose('\u{1D157}', '\u{1D165}'), None); // Composition exclusion

    assert_eq!(comp.compose('ে', 'া'), Some('ো')); // Second is starter; BMP
    assert_eq!(comp.compose('𑄱', '𑄧'), Some('𑄮')); // Second is starter; non-BMP

    assert_eq!(comp.compose('ᄀ', 'ᅡ'), Some('가')); // Hangul LV
    assert_eq!(comp.compose('가', 'ᆨ'), Some('각')); // Hangul LVT
}

#[test]
fn test_canonical_composition_owned() {
    let owned = CanonicalComposition::try_new_unstable(&icu_normalizer::provider::Baked).unwrap();
    let comp = owned.as_borrowed();

    assert_eq!(comp.compose('a', 'b'), None); // Just two starters

    assert_eq!(comp.compose('a', '\u{0308}'), Some('ä'));
    assert_eq!(comp.compose('A', '\u{0308}'), Some('Ä'));
    assert_eq!(comp.compose('ẹ', '\u{0302}'), Some('ệ'));
    assert_eq!(comp.compose('Ẹ', '\u{0302}'), Some('Ệ'));
    assert_eq!(comp.compose('\u{1D157}', '\u{1D165}'), None); // Composition exclusion

    assert_eq!(comp.compose('ে', 'া'), Some('ো')); // Second is starter; BMP
    assert_eq!(comp.compose('𑄱', '𑄧'), Some('𑄮')); // Second is starter; non-BMP

    assert_eq!(comp.compose('ᄀ', 'ᅡ'), Some('가')); // Hangul LV
    assert_eq!(comp.compose('가', 'ᆨ'), Some('각')); // Hangul LVT
}

#[test]
fn test_canonical_decomposition() {
    let decomp = CanonicalDecompositionBorrowed::new();

    assert_eq!(
        decomp.decompose('ä'),
        Decomposed::Expansion('a', '\u{0308}')
    );
    assert_eq!(
        decomp.decompose('Ä'),
        Decomposed::Expansion('A', '\u{0308}')
    );
    assert_eq!(
        decomp.decompose('ệ'),
        Decomposed::Expansion('ẹ', '\u{0302}')
    );
    assert_eq!(
        decomp.decompose('Ệ'),
        Decomposed::Expansion('Ẹ', '\u{0302}')
    );
    assert_eq!(
        decomp.decompose('\u{1D15E}'),
        Decomposed::Expansion('\u{1D157}', '\u{1D165}')
    );
    assert_eq!(decomp.decompose('ো'), Decomposed::Expansion('ে', 'া'));
    assert_eq!(decomp.decompose('𑄮'), Decomposed::Expansion('𑄱', '𑄧'));
    assert_eq!(decomp.decompose('가'), Decomposed::Expansion('ᄀ', 'ᅡ'));
    assert_eq!(decomp.decompose('각'), Decomposed::Expansion('가', 'ᆨ'));

    assert_eq!(decomp.decompose('\u{212B}'), Decomposed::Singleton('Å')); // ANGSTROM SIGN
    assert_eq!(decomp.decompose('\u{2126}'), Decomposed::Singleton('Ω')); // OHM SIGN

    assert_eq!(decomp.decompose('\u{1F71}'), Decomposed::Singleton('ά')); // oxia
    assert_eq!(
        decomp.decompose('\u{1F72}'),
        Decomposed::Expansion('ε', '\u{0300}')
    ); // not oxia but in the oxia range
    assert_eq!(
        decomp.decompose('ά'),
        Decomposed::Expansion('α', '\u{0301}')
    ); // tonos
}

#[test]
fn test_canonical_decomposition_owned() {
    let owned = CanonicalDecomposition::try_new_unstable(&icu_normalizer::provider::Baked).unwrap();
    let decomp = owned.as_borrowed();

    assert_eq!(
        decomp.decompose('ä'),
        Decomposed::Expansion('a', '\u{0308}')
    );
    assert_eq!(
        decomp.decompose('Ä'),
        Decomposed::Expansion('A', '\u{0308}')
    );
    assert_eq!(
        decomp.decompose('ệ'),
        Decomposed::Expansion('ẹ', '\u{0302}')
    );
    assert_eq!(
        decomp.decompose('Ệ'),
        Decomposed::Expansion('Ẹ', '\u{0302}')
    );
    assert_eq!(
        decomp.decompose('\u{1D15E}'),
        Decomposed::Expansion('\u{1D157}', '\u{1D165}')
    );
    assert_eq!(decomp.decompose('ো'), Decomposed::Expansion('ে', 'া'));
    assert_eq!(decomp.decompose('𑄮'), Decomposed::Expansion('𑄱', '𑄧'));
    assert_eq!(decomp.decompose('가'), Decomposed::Expansion('ᄀ', 'ᅡ'));
    assert_eq!(decomp.decompose('각'), Decomposed::Expansion('가', 'ᆨ'));

    assert_eq!(decomp.decompose('\u{212B}'), Decomposed::Singleton('Å')); // ANGSTROM SIGN
    assert_eq!(decomp.decompose('\u{2126}'), Decomposed::Singleton('Ω')); // OHM SIGN

    assert_eq!(decomp.decompose('\u{1F71}'), Decomposed::Singleton('ά')); // oxia
    assert_eq!(
        decomp.decompose('\u{1F72}'),
        Decomposed::Expansion('ε', '\u{0300}')
    ); // not oxia but in the oxia range
    assert_eq!(
        decomp.decompose('ά'),
        Decomposed::Expansion('α', '\u{0301}')
    ); // tonos
}

#[test]
fn test_ccc() {
    let map = CanonicalCombiningClassMapBorrowed::new();
    for u in 0..=0x10FFFF {
        assert_eq!(
            map.get32(u),
            icu_properties::CodePointMapData::<CanonicalCombiningClass>::new().get32(u)
        );
    }
}

#[test]
fn test_ccc_owned() {
    let owned =
        CanonicalCombiningClassMap::try_new_unstable(&icu_normalizer::provider::Baked).unwrap();
    let map = owned.as_borrowed();
    for u in 0..=0x10FFFF {
        assert_eq!(
            map.get32(u),
            icu_properties::CodePointMapData::<CanonicalCombiningClass>::new().get32(u)
        );
    }
}

#[test]
fn test_utf16_basic() {
    let normalizer = ComposingNormalizerBorrowed::new_nfc();

    assert_eq!(
        normalizer.normalize_utf16(&[0x0061]).as_ref(),
        [0x0061].as_slice()
    );
    assert_eq!(
        normalizer.normalize_utf16(&[0x0300, 0x0323]).as_ref(),
        [0x0323, 0x0300].as_slice()
    );
}

#[test]
fn test_accented_digraph() {
    let normalizer = DecomposingNormalizerBorrowed::new_nfkd();
    assert_eq!(
        normalizer.normalize("\u{01C4}\u{0323}"),
        "DZ\u{0323}\u{030C}"
    );
    assert_eq!(
        normalizer.normalize("DZ\u{030C}\u{0323}"),
        "DZ\u{0323}\u{030C}"
    );
}

#[test]
fn test_ddd() {
    let normalizer = DecomposingNormalizerBorrowed::new_nfd();
    assert_eq!(
        normalizer.normalize("\u{0DDD}\u{0334}"),
        "\u{0DD9}\u{0DCF}\u{0334}\u{0DCA}"
    );
}

#[test]
fn test_is_normalized() {
    let nfd = DecomposingNormalizerBorrowed::new_nfd();
    let nfkd = DecomposingNormalizerBorrowed::new_nfkd();
    let nfc = ComposingNormalizerBorrowed::new_nfc();
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();

    let aaa = "aaa";
    assert!(nfd.is_normalized(aaa));
    assert!(nfkd.is_normalized(aaa));
    assert!(nfc.is_normalized(aaa));
    assert!(nfkc.is_normalized(aaa));

    assert!(nfd.is_normalized_utf8(aaa.as_bytes()));
    assert!(nfkd.is_normalized_utf8(aaa.as_bytes()));
    assert!(nfc.is_normalized_utf8(aaa.as_bytes()));
    assert!(nfkc.is_normalized_utf8(aaa.as_bytes()));

    let aaa16 = [0x0061u16, 0x0061u16, 0x0061u16].as_slice();
    assert!(nfd.is_normalized_utf16(aaa16));
    assert!(nfkd.is_normalized_utf16(aaa16));
    assert!(nfc.is_normalized_utf16(aaa16));
    assert!(nfkc.is_normalized_utf16(aaa16));

    let affa = b"a\xFFa";
    assert!(nfd.is_normalized_utf8(affa));
    assert!(nfkd.is_normalized_utf8(affa));
    assert!(nfc.is_normalized_utf8(affa));
    assert!(nfkc.is_normalized_utf8(affa));

    let a_surrogate_a = [0x0061u16, 0xD800u16, 0x0061u16].as_slice();
    assert!(nfd.is_normalized_utf16(a_surrogate_a));
    assert!(nfkd.is_normalized_utf16(a_surrogate_a));
    assert!(nfc.is_normalized_utf16(a_surrogate_a));
    assert!(nfkc.is_normalized_utf16(a_surrogate_a));

    let note = "a𝅗\u{1D165}a";
    assert!(nfd.is_normalized(note));
    assert!(nfkd.is_normalized(note));
    assert!(nfc.is_normalized(note));
    assert!(nfkc.is_normalized(note));

    assert!(nfd.is_normalized_utf8(note.as_bytes()));
    assert!(nfkd.is_normalized_utf8(note.as_bytes()));
    assert!(nfc.is_normalized_utf8(note.as_bytes()));
    assert!(nfkc.is_normalized_utf8(note.as_bytes()));

    let note16 = [
        0x0061u16, 0xD834u16, 0xDD57u16, 0xD834u16, 0xDD65u16, 0x0061u16,
    ]
    .as_slice();
    assert!(nfd.is_normalized_utf16(note16));
    assert!(nfkd.is_normalized_utf16(note16));
    assert!(nfc.is_normalized_utf16(note16));
    assert!(nfkc.is_normalized_utf16(note16));

    let umlaut = "aäa";
    assert!(!nfd.is_normalized(umlaut));
    assert!(!nfkd.is_normalized(umlaut));
    assert!(nfc.is_normalized(umlaut));
    assert!(nfkc.is_normalized(umlaut));

    assert!(!nfd.is_normalized_utf8(umlaut.as_bytes()));
    assert!(!nfkd.is_normalized_utf8(umlaut.as_bytes()));
    assert!(nfc.is_normalized_utf8(umlaut.as_bytes()));
    assert!(nfkc.is_normalized_utf8(umlaut.as_bytes()));

    let umlaut16 = [0x0061u16, 0x00E4u16, 0x0061u16].as_slice();
    assert!(!nfd.is_normalized_utf16(umlaut16));
    assert!(!nfkd.is_normalized_utf16(umlaut16));
    assert!(nfc.is_normalized_utf16(umlaut16));
    assert!(nfkc.is_normalized_utf16(umlaut16));

    let fraction = "a½a";
    assert!(nfd.is_normalized(fraction));
    assert!(!nfkd.is_normalized(fraction));
    assert!(nfc.is_normalized(fraction));
    assert!(!nfkc.is_normalized(fraction));

    assert!(nfd.is_normalized_utf8(fraction.as_bytes()));
    assert!(!nfkd.is_normalized_utf8(fraction.as_bytes()));
    assert!(nfc.is_normalized_utf8(fraction.as_bytes()));
    assert!(!nfkc.is_normalized_utf8(fraction.as_bytes()));

    let fraction16 = [0x0061u16, 0x00BDu16, 0x0061u16].as_slice();
    assert!(nfd.is_normalized_utf16(fraction16));
    assert!(!nfkd.is_normalized_utf16(fraction16));
    assert!(nfc.is_normalized_utf16(fraction16));
    assert!(!nfkc.is_normalized_utf16(fraction16));
}

#[test]
fn test_is_normalized_up_to() {
    let nfd = DecomposingNormalizerBorrowed::new_nfd();
    let nfkd = DecomposingNormalizerBorrowed::new_nfkd();
    let nfc = ComposingNormalizerBorrowed::new_nfc();
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();

    // Check a string slice is normalized up to where is_normalized_up_to reports
    let check_str = |input: &str| {
        // Check nfd
        let (head, tail) = nfd.split_normalized(input);
        let mut normalized = String::from(head);
        let _ = nfd.normalize_to(tail, &mut normalized);
        assert!(nfd.is_normalized(&normalized));

        // Check nfkd
        let (head, tail) = nfkd.split_normalized(input);
        let mut normalized = String::from(head);
        let _ = nfkd.normalize_to(tail, &mut normalized);
        assert!(nfkd.is_normalized(&normalized));

        // Check nfc
        let (head, tail) = nfc.split_normalized(input);
        let mut normalized = String::from(head);
        let _ = nfc.normalize_to(tail, &mut normalized);
        assert!(nfc.is_normalized(&normalized));

        // Check nfkc
        let (head, tail) = nfkc.split_normalized(input);
        let mut normalized = String::from(head);
        let _ = nfkc.normalize_to(tail, &mut normalized);
        assert!(nfkc.is_normalized(&normalized));
    };

    // Check a string of UTF8 bytes is normalized up to where is_normalized_up_to reports
    // note: from_utf8 can panic with invalid UTF8 input
    let check_utf8 = |input: &[u8]| {
        // Check nfd
        let (head, tail) = nfd.split_normalized_utf8(input);
        let mut normalized = String::from(head);
        let _ = nfd.normalize_utf8_to(tail, &mut normalized);
        assert!(nfd.is_normalized(&normalized));

        // Check nfkd
        let (head, tail) = nfkd.split_normalized_utf8(input);
        let mut normalized = String::from(head);
        let _ = nfkd.normalize_utf8_to(tail, &mut normalized);
        assert!(nfkd.is_normalized(&normalized));

        // Check nfc
        let (head, tail) = nfc.split_normalized_utf8(input);
        let mut normalized = String::from(head);
        let _ = nfc.normalize_utf8_to(tail, &mut normalized);
        assert!(nfc.is_normalized(&normalized));

        // Check nfkc
        let (head, tail) = nfkc.split_normalized_utf8(input);
        let mut normalized = String::from(head);
        let _ = nfkc.normalize_utf8_to(tail, &mut normalized);
        assert!(nfkc.is_normalized(&normalized));
    };

    // Check a string of UTF-16 code units is normalized up to where is_normalized_up_to reports
    let check_utf16 = |input: &[u16]| {
        // Check nfd
        let (head, tail) = nfd.split_normalized_utf16(input);
        let mut normalized = head.to_vec();
        let _ = nfd.normalize_utf16_to(tail, &mut normalized);
        assert!(nfd.is_normalized_utf16(&normalized));

        // Check nfkd
        let (head, tail) = nfkd.split_normalized_utf16(input);
        let mut normalized = head.to_vec();
        let _ = nfkd.normalize_utf16_to(tail, &mut normalized);
        assert!(nfkd.is_normalized_utf16(&normalized));

        // Check nfc
        let (head, tail) = nfc.split_normalized_utf16(input);
        let mut normalized = head.to_vec();
        let _ = nfc.normalize_utf16_to(tail, &mut normalized);
        assert!(nfc.is_normalized_utf16(&normalized));

        // Check nfkc
        let (head, tail) = nfkc.split_normalized_utf16(input);
        let mut normalized = head.to_vec();
        let _ = nfkc.normalize_utf16_to(tail, &mut normalized);
        assert!(nfkc.is_normalized_utf16(&normalized));
    };

    let aaa = "aaa";
    check_str(aaa);

    let aaa_utf8 = aaa.as_bytes();
    check_utf8(aaa_utf8);

    let aaa_utf16: Vec<u16> = aaa.encode_utf16().collect();
    check_utf16(&aaa_utf16);

    assert!(nfd.split_normalized(aaa).0.len() == aaa.len());
    assert!(nfkd.split_normalized(aaa).0.len() == aaa.len());
    assert!(nfc.split_normalized(aaa).0.len() == aaa.len());
    assert!(nfkc.split_normalized(aaa).0.len() == aaa.len());
    assert!(nfd.split_normalized_utf8(aaa_utf8).0.len() == aaa_utf8.len());
    assert!(nfkd.split_normalized_utf8(aaa_utf8).0.len() == aaa_utf8.len());
    assert!(nfc.split_normalized_utf8(aaa_utf8).0.len() == aaa_utf8.len());
    assert!(nfkc.split_normalized_utf8(aaa_utf8).0.len() == aaa_utf8.len());
    assert!(nfd.split_normalized_utf16(&aaa_utf16).0.len() == aaa_utf16.len());
    assert!(nfkd.split_normalized_utf16(&aaa_utf16).0.len() == aaa_utf16.len());
    assert!(nfc.split_normalized_utf16(&aaa_utf16).0.len() == aaa_utf16.len());
    assert!(nfkc.split_normalized_utf16(&aaa_utf16).0.len() == aaa_utf16.len());

    let note = "a𝅗\u{1D165}a";
    check_str(note);

    let note_utf8 = note.as_bytes();
    check_utf8(note_utf8);

    let note_utf16: Vec<u16> = note.encode_utf16().collect();
    check_utf16(&note_utf16);

    assert!(nfd.split_normalized(note).0.len() == note.len());
    assert!(nfkd.split_normalized(note).0.len() == note.len());
    assert!(nfc.split_normalized(note).0.len() == note.len());
    assert!(nfkc.split_normalized(note).0.len() == note.len());
    assert!(nfd.split_normalized_utf8(note_utf8).0.len() == note_utf8.len());
    assert!(nfkd.split_normalized_utf8(note_utf8).0.len() == note_utf8.len());
    assert!(nfc.split_normalized_utf8(note_utf8).0.len() == note_utf8.len());
    assert!(nfkc.split_normalized_utf8(note_utf8).0.len() == note_utf8.len());
    assert!(nfd.split_normalized_utf16(&note_utf16).0.len() == note_utf16.len());
    assert!(nfkd.split_normalized_utf16(&note_utf16).0.len() == note_utf16.len());
    assert!(nfc.split_normalized_utf16(&note_utf16).0.len() == note_utf16.len());
    assert!(nfkc.split_normalized_utf16(&note_utf16).0.len() == note_utf16.len());

    let umlaut = "aäa";
    check_str(umlaut);

    let umlaut_utf8 = umlaut.as_bytes();
    check_utf8(umlaut_utf8);

    let umlaut_utf16: Vec<u16> = umlaut.encode_utf16().collect();
    check_utf16(&umlaut_utf16);

    assert_eq!(nfd.split_normalized(umlaut).0.len(), 1);
    assert_eq!(nfkd.split_normalized(umlaut).0.len(), 1);
    assert_eq!(nfc.split_normalized(umlaut).0.len(), 4);
    assert_eq!(nfkc.split_normalized(umlaut).0.len(), 4);
    assert_eq!(nfd.split_normalized_utf8(umlaut_utf8).0.len(), 1);
    assert_eq!(nfkd.split_normalized_utf8(umlaut_utf8).0.len(), 1);
    assert_eq!(nfc.split_normalized_utf8(umlaut_utf8).0.len(), 4);
    assert_eq!(nfkc.split_normalized_utf8(umlaut_utf8).0.len(), 4);
    assert_eq!(nfd.split_normalized_utf16(&umlaut_utf16).0.len(), 1);
    assert_eq!(nfkd.split_normalized_utf16(&umlaut_utf16).0.len(), 1);
    assert_eq!(nfc.split_normalized_utf16(&umlaut_utf16).0.len(), 3);
    assert_eq!(nfkc.split_normalized_utf16(&umlaut_utf16).0.len(), 3);

    let fraction = "a½a";
    check_str(fraction);

    let fraction_utf8 = fraction.as_bytes();
    check_utf8(fraction_utf8);

    let fraction_utf16: Vec<u16> = fraction.encode_utf16().collect();
    check_utf16(&fraction_utf16);

    assert_eq!(nfd.split_normalized(fraction).0.len(), 4);
    assert_eq!(nfkd.split_normalized(fraction).0.len(), 1);
    assert_eq!(nfc.split_normalized(fraction).0.len(), 4);
    assert_eq!(nfkc.split_normalized(fraction).0.len(), 1);
    assert_eq!(nfd.split_normalized_utf8(fraction_utf8).0.len(), 4);
    assert_eq!(nfkd.split_normalized_utf8(fraction_utf8).0.len(), 1);
    assert_eq!(nfc.split_normalized_utf8(fraction_utf8).0.len(), 4);
    assert_eq!(nfkc.split_normalized_utf8(fraction_utf8).0.len(), 1);
    assert_eq!(nfd.split_normalized_utf16(&fraction_utf16).0.len(), 3);
    assert_eq!(nfkd.split_normalized_utf16(&fraction_utf16).0.len(), 1);
    assert_eq!(nfc.split_normalized_utf16(&fraction_utf16).0.len(), 3);
    assert_eq!(nfkc.split_normalized_utf16(&fraction_utf16).0.len(), 1);

    let reversed_vietnamese = "e\u{0302}\u{0323}";
    check_str(reversed_vietnamese);

    let reversed_vietnamese_utf8 = reversed_vietnamese.as_bytes();
    check_utf8(reversed_vietnamese_utf8);

    let reversed_vietnamese_utf16: Vec<u16> = reversed_vietnamese.encode_utf16().collect();
    check_utf16(&reversed_vietnamese_utf16);

    assert_eq!(nfd.split_normalized(reversed_vietnamese).0.len(), 1);
    assert_eq!(nfkd.split_normalized(reversed_vietnamese).0.len(), 1);
    assert_eq!(nfc.split_normalized(reversed_vietnamese).0.len(), 0);
    assert_eq!(nfkc.split_normalized(reversed_vietnamese).0.len(), 0);
    assert_eq!(
        nfd.split_normalized_utf8(reversed_vietnamese_utf8).0.len(),
        1
    );
    assert_eq!(
        nfkd.split_normalized_utf8(reversed_vietnamese_utf8).0.len(),
        1
    );
    assert_eq!(
        nfc.split_normalized_utf8(reversed_vietnamese_utf8).0.len(),
        0
    );
    assert_eq!(
        nfkc.split_normalized_utf8(reversed_vietnamese_utf8).0.len(),
        0
    );
    assert_eq!(
        nfd.split_normalized_utf16(&reversed_vietnamese_utf16)
            .0
            .len(),
        1
    );
    assert_eq!(
        nfkd.split_normalized_utf16(&reversed_vietnamese_utf16)
            .0
            .len(),
        1
    );
    assert_eq!(
        nfc.split_normalized_utf16(&reversed_vietnamese_utf16)
            .0
            .len(),
        0
    );
    assert_eq!(
        nfkc.split_normalized_utf16(&reversed_vietnamese_utf16)
            .0
            .len(),
        0
    );

    let truncated_vietnamese = "e\u{0302}";
    check_str(truncated_vietnamese);

    let truncated_vietnamese_utf8 = truncated_vietnamese.as_bytes();
    check_utf8(truncated_vietnamese_utf8);

    let truncated_vietnamese_utf16: Vec<u16> = truncated_vietnamese.encode_utf16().collect();
    check_utf16(&truncated_vietnamese_utf16);

    assert_eq!(nfd.split_normalized(truncated_vietnamese).0.len(), 3);
    assert_eq!(nfkd.split_normalized(truncated_vietnamese).0.len(), 3);
    assert_eq!(nfc.split_normalized(truncated_vietnamese).0.len(), 0);
    assert_eq!(nfkc.split_normalized(truncated_vietnamese).0.len(), 0);
    assert_eq!(
        nfd.split_normalized_utf8(truncated_vietnamese_utf8).0.len(),
        3
    );
    assert_eq!(
        nfkd.split_normalized_utf8(truncated_vietnamese_utf8)
            .0
            .len(),
        3
    );
    assert_eq!(
        nfc.split_normalized_utf8(truncated_vietnamese_utf8).0.len(),
        0
    );
    assert_eq!(
        nfkc.split_normalized_utf8(truncated_vietnamese_utf8)
            .0
            .len(),
        0
    );
    assert_eq!(
        nfd.split_normalized_utf16(&truncated_vietnamese_utf16)
            .0
            .len(),
        2
    );
    assert_eq!(
        nfkd.split_normalized_utf16(&truncated_vietnamese_utf16)
            .0
            .len(),
        2
    );
    assert_eq!(
        nfc.split_normalized_utf16(&truncated_vietnamese_utf16)
            .0
            .len(),
        0
    );
    assert_eq!(
        nfkc.split_normalized_utf16(&truncated_vietnamese_utf16)
            .0
            .len(),
        0
    );
}
