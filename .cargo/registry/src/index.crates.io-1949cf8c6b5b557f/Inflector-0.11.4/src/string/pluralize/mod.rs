#![deny(warnings)]
use regex::Regex;
use string::constants::UNACCONTABLE_WORDS;

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
        let mut r = Vec::with_capacity(24);
        rules![r;
               r"(\w*)s$" => "s",
               r"(\w*([^aeiou]ese))$" => "",
               r"(\w*(ax|test))is$" => "es",
               r"(\w*(alias|[^aou]us|tlas|gas|ris))$" => "es",
               r"(\w*(e[mn]u))s?$" => "s",
               r"(\w*([^l]ias|[aeiou]las|[emjzr]as|[iu]am))$" => "",
               r"(\w*(alumn|syllab|octop|vir|radi|nucle|fung|cact|stimul|termin|bacill|foc|uter|loc|strat))(?:us|i)$" => "i",
               r"(\w*(alumn|alg|vertebr))(?:a|ae)$" => "ae",
               r"(\w*(seraph|cherub))(?:im)?$" => "im",
               r"(\w*(her|at|gr))o$" => "oes",
               r"(\w*(agend|addend|millenni|dat|extrem|bacteri|desiderat|strat|candelabr|errat|ov|symposi|curricul|automat|quor))(?:a|um)$" => "a",
               r"(\w*(apheli|hyperbat|periheli|asyndet|noumen|phenomen|criteri|organ|prolegomen|hedr|automat))(?:a|on)$" => "a",
               r"(\w*)sis$" => "ses",
               r"(\w*(kni|wi|li))fe$" => "ves",
               r"(\w*(ar|l|ea|eo|oa|hoo))f$" => "ves",
               r"(\w*([^aeiouy]|qu))y$" => "ies",
               r"(\w*([^ch][ieo][ln]))ey$" => "ies",
               r"(\w*(x|ch|ss|sh|zz)es)$" => "",
               r"(\w*(x|ch|ss|sh|zz))$" => "es",
               r"(\w*(matr|cod|mur|sil|vert|ind|append))(?:ix|ex)$" => "ices",
               r"(\w*(m|l)(?:ice|ouse))$" => "ice",
               r"(\w*(pe)(?:rson|ople))$" => "ople",
               r"(\w*(child))(?:ren)?$" => "ren",
               r"(\w*eaux)$" => ""
        ];
        r
    };
}

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


/// Converts a `&str` to pluralized `String`
///
/// ```
///     use inflector::string::pluralize::to_plural;
///     let mock_string: &str = "foo_bar";
///     let expected_string: String = "foo_bars".to_owned();
///     let asserted_string: String = to_plural(mock_string);
///     assert_eq!(asserted_string, expected_string);
///
/// ```
/// ```
///     use inflector::string::pluralize::to_plural;
///     let mock_string: &str = "ox";
///     let expected_string: String = "oxen".to_owned();
///     let asserted_string: String = to_plural(mock_string);
///     assert_eq!(asserted_string, expected_string);
///
/// ```
/// ```
///     use inflector::string::pluralize::to_plural;
///     let mock_string: &str = "crate";
///     let expected_string: String = "crates".to_owned();
///     let asserted_string: String = to_plural(mock_string);
///     assert_eq!(asserted_string, expected_string);
///
/// ```
/// ```
///     use inflector::string::pluralize::to_plural;
///     let mock_string: &str = "boxes";
///     let expected_string: String = "boxes".to_owned();
///     let asserted_string: String = to_plural(mock_string);
///     assert_eq!(asserted_string, expected_string);
///
/// ```
/// ```
///     use inflector::string::pluralize::to_plural;
///     let mock_string: &str = "vengeance";
///     let expected_string: String = "vengeance".to_owned();
///     let asserted_string: String = to_plural(mock_string);
///     assert_eq!(asserted_string, expected_string);
///
/// ```
/// ```
///     use inflector::string::pluralize::to_plural;
///     let mock_string: &str = "yoga";
///     let expected_string: String = "yoga".to_owned();
///     let asserted_string: String = to_plural(mock_string);
///     assert_eq!(asserted_string, expected_string);
///
/// ```
/// ```
///     use inflector::string::pluralize::to_plural;
///     let mock_string: &str = "geometry";
///     let expected_string: String = "geometries".to_owned();
///     let asserted_string: String = to_plural(mock_string);
///     assert_eq!(asserted_string, expected_string);
///
/// ```
///
pub fn to_plural(non_plural_string: &str) -> String {
    if UNACCONTABLE_WORDS.contains(&non_plural_string.as_ref()) {
        non_plural_string.to_owned()
    } else {
        special_cases![non_plural_string,
            "ox" => "oxen",
            "man" => "men",
            "woman" => "women",
            "die" => "dice",
            "yes" => "yeses",
            "foot" => "feet",
            "eave" => "eaves",
            "goose" => "geese",
            "tooth" => "teeth",
            "quiz" => "quizzes"
        ];
        for &(ref rule, replace) in RULES.iter().rev() {
            if let Some(c) = rule.captures(&non_plural_string) {
                if let Some(c) = c.get(1) {
                    return format!("{}{}", c.as_str(), replace);
                }
            }
        }

        format!("{}s", non_plural_string)
    }
}


#[cfg(test)]
mod tests {

    macro_rules! as_item {
        ($i:item) => { $i };
    }

    macro_rules! make_tests{
        ($($singular:ident => $plural:ident); *) =>{
            $(
                   as_item! {
                       #[test]
                       fn $singular(){
                           assert_eq!(
                               stringify!($plural),
                               super::to_plural(stringify!($singular))
                               );
                       }
                   }
            )*
        }
    }

    #[test]
    fn boxes() {
        assert_eq!("boxes", super::to_plural("box"));
    }

    make_tests!{
        geometry => geometries;
        ox => oxen;
        woman => women;
        test => tests;
        axis => axes;
        knife => knives;
        agendum => agenda;
        elf => elves;
        zoology => zoology
    }
}
