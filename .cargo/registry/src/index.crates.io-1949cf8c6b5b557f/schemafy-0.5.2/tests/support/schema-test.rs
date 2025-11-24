extern crate schema;
extern crate serde_json;

fn main() {
    let s = include_str!("../../src/schema.json");
    let _: schema::Schema = serde_json::from_str(s).unwrap();
}
