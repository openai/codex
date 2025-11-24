use regex::Regex;
use string::constants::UNACCONTABLE_WORDS;

macro_rules! special_cases{
    ($s:ident, $($singular: expr => $plural:expr), *) => {
        match &$s[..] {
            $(
                $singular => {
                    return $plural.to_owned();
                },
            )*
            _ => ()
        }
    }
}


/// Converts a `&str` to singularized `String`
///
/// ```
///     use inflector::string::singularize::to_singular;
///     let mock_string: &str = "foo_bars";
///     let expected_string: String = "foo_bar".to_owned();
///     let asserted_string: String = to_singular(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::singularize::to_singular;
///     let mock_string: &str = "oxen";
///     let expected_string: String = "ox".to_owned();
///     let asserted_string: String = to_singular(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::singularize::to_singular;
///     let mock_string: &str = "crates";
///     let expected_string: String = "crate".to_owned();
///     let asserted_string: String = to_singular(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::singularize::to_singular;
///     let mock_string: &str = "oxen";
///     let expected_string: String = "ox".to_owned();
///     let asserted_string: String = to_singular(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::singularize::to_singular;
///     let mock_string: &str = "boxes";
///     let expected_string: String = "box".to_owned();
///     let asserted_string: String = to_singular(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::singularize::to_singular;
///     let mock_string: &str = "vengeance";
///     let expected_string: String = "vengeance".to_owned();
///     let asserted_string: String = to_singular(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::singularize::to_singular;
///     let mock_string: &str = "yoga";
///     let expected_string: String = "yoga".to_owned();
///     let asserted_string: String = to_singular(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
pub fn to_singular(non_singular_string: &str) -> String {
    if UNACCONTABLE_WORDS.contains(&non_singular_string.as_ref()) {
        non_singular_string.to_owned()
    } else {
        special_cases![non_singular_string,
            "oxen" => "ox",
            "boxes" => "box",
            "men" => "man",
            "women" => "woman",
            "dice" => "die",
            "yeses" => "yes",
            "feet" => "foot",
            "eaves" => "eave",
            "geese" => "goose",
            "teeth" => "tooth",
            "quizzes" => "quiz"
        ];
        for &(ref rule, replace) in RULES.iter().rev() {
            if let Some(captures) = rule.captures(&non_singular_string) {
                if let Some(c) = captures.get(1) {
                    let mut buf = String::new();
                    captures.expand(&format!("{}{}", c.as_str(), replace), &mut buf);
                    return buf;
                }
            }
        }

        format!("{}", non_singular_string)
    }
}

macro_rules! add_rule{
    ($r:ident, $rule:expr => $replace:expr) => {
        $r.push((Regex::new($rule).unwrap(), $replace));
    }
}

macro_rules! rules{
    ($r:ident; $($rule:expr => $replace:expr), *) => {
        $(
            add_rule!{$r, $rule => $replace}
        )*
    }
}


lazy_static!{
    static ref RULES: Vec<(Regex, &'static str)> = {
    let mut r = Vec::with_capacity(27);
    rules![r;
     r"(\w*)s$" => "",
     r"(\w*)(ss)$" => "$2",
     r"(n)ews$" => "ews",
     r"(\w*)(o)es$" => "",
     r"(\w*)([ti])a$" => "um",
     r"((a)naly|(b)a|(d)iagno|(p)arenthe|(p)rogno|(s)ynop|(t)he)(sis|ses)$" => "sis",
     r"(^analy)(sis|ses)$" => "sis",
     r"(\w*)([^f])ves$" => "fe",
     r"(\w*)(hive)s$" => "",
     r"(\w*)(tive)s$" => "",
     r"(\w*)([lr])ves$" => "f",
     r"(\w*([^aeiouy]|qu))ies$" => "y",
     r"(s)eries$" => "eries",
     r"(m)ovies$" => "ovie",
     r"(\w*)(x|ch|ss|sh)es$" => "$2",
     r"(m|l)ice$" => "ouse",
     r"(bus)(es)?$" => "",
     r"(shoe)s$" => "",
     r"(cris|test)(is|es)$" => "is",
     r"^(a)x[ie]s$" => "xis",
     r"(octop|vir)(us|i)$" => "us",
     r"(alias|status)(es)?$" => "",
     r"^(ox)en" => "",
     r"(vert|ind)ices$" => "ex",
     r"(matr)ices$" => "ix",
     r"(quiz)zes$" => "",
     r"(database)s$" => ""
         ];
     r
    };
}

#[test]
fn singularize_ies_suffix() {
    assert_eq!("reply", to_singular("replies"));
    assert_eq!("lady", to_singular("ladies"));
    assert_eq!("soliloquy", to_singular("soliloquies"));
}

#[test]
fn singularize_ss_suffix() {
    assert_eq!("glass", to_singular("glass"));
    assert_eq!("access", to_singular("access"));
    assert_eq!("glass", to_singular("glasses"));
    assert_eq!("witch", to_singular("witches"));
    assert_eq!("dish", to_singular("dishes"));
}

#[test]
fn singularize_string_if_a_regex_will_match() {
    let expected_string: String = "ox".to_owned();
    let asserted_string: String = to_singular("oxen");
    assert!(expected_string == asserted_string);

}

#[test]
fn singularize_string_returns_none_option_if_no_match() {
    let expected_string: String = "bacon".to_owned();
    let asserted_string: String = to_singular("bacon");

    assert!(expected_string == asserted_string);
}
