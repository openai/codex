#![deny(warnings)]
extern crate inflector;

use inflector::Inflector;
use inflector::InflectorNumbers;

macro_rules! str_tests {
    ( $($test_name:ident => $imp_trait:ident => $to_cast:expr => $casted:expr), *) => {
        $(
            #[test]
            fn $test_name() {
                assert_eq!($to_cast.$imp_trait(), $casted)
            }
        )*
    }
}

macro_rules! string_tests {
    ( $($test_name:ident => $imp_trait:ident => $to_cast:expr => $casted:expr), *) => {
        $(
            #[test]
            fn $test_name() {
                assert_eq!($to_cast.to_string().$imp_trait(), $casted)
            }
        )*
    }
}

macro_rules! number_tests {
    ( $($test_name:ident => $imp_trait:ident => $typ:ident => $to_cast:expr => $casted:expr), *) => {
        $(
            #[test]
            fn $test_name() {
                let to_cast: $typ = $to_cast;
                assert_eq!(to_cast.$imp_trait(), $casted)
            }
        )*
    }
}

macro_rules! gated_str_tests {
    ( $($test_name:ident => $imp_trait:ident => $to_cast:expr => $casted:expr), *) => {
        $(
            #[test]
            #[cfg(feature = "heavyweight")]
            fn $test_name() {
                assert_eq!($to_cast.$imp_trait(), $casted)
            }
        )*
    }
}

macro_rules! gated_string_tests {
    ( $($test_name:ident => $imp_trait:ident => $to_cast:expr => $casted:expr), *) => {
        $(
            #[test]
            #[cfg(feature = "heavyweight")]
            fn $test_name() {
                assert_eq!($to_cast.to_string().$imp_trait(), $casted)
            }
        )*
    }
}

str_tests![
    str_to_camel => to_camel_case => "foo_bar" => "fooBar".to_string(),
    str_is_camel => is_camel_case => "fooBar" => true,
    str_is_not_camel => is_camel_case => "foo_bar" => false,
    str_to_screaming_snake => to_screaming_snake_case => "fooBar" => "FOO_BAR".to_string(),
    str_is_screaming_snake => is_screaming_snake_case => "FOO_BAR" => true,
    str_is_not_screaming_snake => is_screaming_snake_case => "foo_bar" => false,
    str_to_snake => to_snake_case => "fooBar" => "foo_bar".to_string(),
    str_is_snake => is_snake_case => "foo_bar" => true,
    str_is_not_snake => is_snake_case => "fooBar" => false,
    str_to_kebab => to_kebab_case => "fooBar" => "foo-bar".to_string(),
    str_is_kebab => is_kebab_case => "foo-bar" => true,
    str_is_not_kebab => is_kebab_case => "fooBar" => false,
    str_to_train => to_train_case => "fooBar" => "Foo-Bar".to_string(),
    str_is_train => is_train_case => "Foo-Bar" => true,
    str_is_not_train => is_train_case => "FOO-Bar" => false,
    str_to_sentence => to_sentence_case => "fooBar" => "Foo bar".to_string(),
    str_is_sentence => is_sentence_case => "Foo bar" => true,
    str_is_not_sentence => is_sentence_case => "foo_bar" => false,
    str_to_title => to_title_case => "fooBar" => "Foo Bar".to_string(),
    str_is_title => is_title_case => "Foo Bar" => true,
    str_is_not_title => is_title_case => "Foo_Bar" => false,
    str_ordinalize  => ordinalize => "1" => "1st".to_string(),
    str_deordinalize  => deordinalize => "1st" => "1".to_string(),
    str_to_foreign_key => to_foreign_key => "Foo::Bar" => "bar_id".to_string(),
    str_is_foreign_key => is_foreign_key => "bar_id" => true,
    str_is_not_foreign_key => is_foreign_key => "bar" => false
];

gated_str_tests![
    str_to_class_case => to_class_case => "foo" => "Foo".to_string(),
    str_is_class_case => is_class_case => "Foo" => true,
    str_is_not_class_case => is_class_case => "foo" => false,
    str_to_table => to_table_case => "fooBar" => "foo_bars".to_string(),
    str_is_table => is_table_case => "foo_bars" => true,
    str_is_not_table => is_table_case => "fooBars" => false,
    str_pluralize => to_plural => "crate" => "crates".to_string(),
    str_singular => to_singular => "crates" => "crate".to_string(),
    str_demodulize => demodulize => "Foo::Bar" => "Bar".to_string(),
    str_deconstantize => deconstantize => "Foo::Bar" => "Foo".to_string()
];

string_tests![
    string_to_camel => to_camel_case => "foo_bar".to_string() => "fooBar".to_string(),
    string_is_camel => is_camel_case => "fooBar".to_string() => true,
    string_is_not_camel => is_camel_case => "foo_bar".to_string() => false,
    string_to_screaming_snake => to_screaming_snake_case => "fooBar".to_string() => "FOO_BAR".to_string(),
    string_is_screaming_snake => is_screaming_snake_case => "FOO_BAR".to_string() => true,
    string_is_not_screaming_snake => is_screaming_snake_case => "foo_bar".to_string() => false,
    string_to_snake => to_snake_case => "fooBar".to_string() => "foo_bar".to_string(),
    string_is_snake => is_snake_case => "foo_bar".to_string() => true,
    string_is_not_snake => is_snake_case => "fooBar".to_string() => false,
    string_to_kebab => to_kebab_case => "fooBar".to_string() => "foo-bar".to_string(),
    string_is_kebab => is_kebab_case => "foo-bar".to_string() => true,
    string_is_not_kebab => is_kebab_case => "fooBar".to_string() => false,
    string_to_train => to_train_case => "fooBar".to_string() => "Foo-Bar".to_string(),
    string_is_train => is_train_case => "Foo-Bar".to_string() => true,
    string_is_not_train => is_train_case => "foo-Bar".to_string() => false,
    string_to_sentence => to_sentence_case => "fooBar".to_string() => "Foo bar".to_string(),
    string_is_sentence => is_sentence_case => "Foo bar".to_string() => true,
    string_is_not_sentence => is_sentence_case => "fooBar".to_string() => false,
    string_to_title => to_title_case => "fooBar".to_string() => "Foo Bar".to_string(),
    string_is_title => is_title_case => "Foo Bar".to_string() => true,
    string_is_not_title => is_title_case => "fooBar".to_string() => false,
    string_ordinalize  => ordinalize => "1".to_string() => "1st".to_string(),
    string_deordinalize  => deordinalize => "1st".to_string() => "1".to_string(),
    string_to_foreign_key => to_foreign_key => "Foo::Bar".to_string() => "bar_id".to_string(),
    string_is_foreign_key => is_foreign_key => "bar_id".to_string() => true,
    string_is_not_foreign_key => is_foreign_key => "bar".to_string() => false
];

gated_string_tests![
    string_to_class_case => to_class_case => "foo".to_string() => "Foo".to_string(),
    string_is_class_case => is_class_case => "Foo".to_string() => true,
    string_is_not_class_case => is_class_case => "ooBar".to_string() => false,
    string_to_table => to_table_case => "fooBar".to_string() => "foo_bars".to_string(),
    string_is_table => is_table_case => "foo_bars".to_string() => true,
    string_is_not_table => is_table_case => "fooBar".to_string() => false,
    string_pluralize => to_plural => "crate".to_string() => "crates".to_string(),
    string_singular => to_singular => "crates".to_string() => "crate".to_string(),
    string_demodulize => demodulize => "Foo::Bar".to_string() => "Bar".to_string(),
    string_deconstantize => deconstantize => "Foo::Bar".to_string() => "Foo".to_string()
];

number_tests![
    i8_ordinalize   => ordinalize => i8  => 1 => "1st".to_string(),
    i16_ordinalize  => ordinalize => i16 => 1 => "1st".to_string(),
    i32_ordinalize  => ordinalize => i32 => 1 => "1st".to_string(),
    i64_ordinalize  => ordinalize => i64 => 1 => "1st".to_string(),
    u8_ordinalize   => ordinalize => u8  => 1 => "1st".to_string(),
    u16_ordinalize  => ordinalize => u16 => 1 => "1st".to_string(),
    u32_ordinalize  => ordinalize => u32 => 1 => "1st".to_string(),
    u64_ordinalize  => ordinalize => u64 => 1 => "1st".to_string(),
    isize_ordinalize  => ordinalize => isize => 1 => "1st".to_string(),
    usize_ordinalize  => ordinalize => usize => 1 => "1st".to_string(),
    f32_ordinalize  => ordinalize => f32 => 1.0 => "1st".to_string(),
    f64_ordinalize  => ordinalize => f64 => 1.0 => "1st".to_string()
];
