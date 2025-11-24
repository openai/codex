use crate::prelude::*;

mod a {
    use super::*;

    #[derive(JsonSchema, Deserialize, Serialize, Default)]
    pub struct Config {
        test: String,
    }
}

mod b {
    use super::*;

    #[derive(JsonSchema, Deserialize, Serialize, Default)]
    pub struct Config {
        test2: String,
    }
}

mod c {
    use super::*;

    #[derive(JsonSchema, Deserialize, Serialize, Default)]
    #[schemars(rename = "Config")]
    pub struct Configuration {
        test3: String,
    }
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
pub struct Config2 {
    a_cfg: a::Config,
    b_cfg: b::Config,
    c_cfg: c::Configuration,
}

#[test]
fn same_name() {
    test!(Config2)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}
