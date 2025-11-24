// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use serde::Deserialize;

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct SubtagData {
    pub valid: Vec<String>,
    pub invalid: Vec<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Subtags {
    pub language: SubtagData,
    pub script: SubtagData,
    pub region: SubtagData,
    pub variant: SubtagData,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct LocaleList {
    pub canonicalized: Vec<String>,
    pub casing: Vec<String>,
}
