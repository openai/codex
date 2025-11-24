mod util;
use schemars::JsonSchema;
use util::*;

mod a {
    use super::*;

    #[allow(dead_code)]
    #[derive(JsonSchema)]
    pub struct Config {
        test: String,
    }
}

mod b {
    use super::*;

    #[allow(dead_code)]
    #[derive(JsonSchema)]
    pub struct Config {
        test2: String,
    }
}

#[allow(dead_code)]
#[derive(JsonSchema)]
pub struct Config2 {
    a_cfg: a::Config,
    b_cfg: b::Config,
}

#[test]
fn same_name() -> TestResult {
    test_default_generated_schema::<Config2>("same_name")
}
