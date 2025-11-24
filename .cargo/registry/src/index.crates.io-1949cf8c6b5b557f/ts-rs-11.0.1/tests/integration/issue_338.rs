use std::collections::HashMap;

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "issue_338/")]
pub struct MyType {
    pub my_field_0: bool,
    pub my_field_1: HashMap<MyEnum, MyStruct>,
}

#[derive(TS)]
#[ts(export, export_to = "issue_338/")]
pub enum MyEnum {
    Variant0,
    Variant1,
    Variant2,
    Variant3,
}

#[derive(TS)]
#[ts(export, export_to = "issue_338/")]
pub struct MyStruct {
    pub my_field_0: bool,
    pub my_field_1: u32,
    pub my_field_2: Option<u32>,
    pub my_field_3: Option<u32>,
    pub my_field_4: Option<u32>,
    pub my_field_5: String,
}

#[test]
fn test() {
    assert_eq!(
        MyType::inline(),
        "{ my_field_0: boolean, my_field_1: { [key in MyEnum]?: MyStruct }, }"
    );
}
