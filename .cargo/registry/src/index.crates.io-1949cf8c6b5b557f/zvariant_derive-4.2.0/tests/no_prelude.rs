#![no_implicit_prelude]
#![allow(dead_code)]

use ::zvariant_derive::{DeserializeDict, SerializeDict, Type};

#[derive(Type)]
struct FooF(f64);

#[derive(Type)]
struct TestStruct {
    name: ::std::string::String,
    age: u8,
    blob: ::std::vec::Vec<u8>,
}

#[repr(u32)]
#[derive(Type)]
enum RequestNameFlags {
    AllowReplacement = 0x01,
    ReplaceExisting = 0x02,
    DoNotQueue = 0x04,
}

#[derive(SerializeDict, DeserializeDict, Type)]
#[zvariant(deny_unknown_fields, signature = "a{sv}")]
struct Test {
    field_a: ::std::option::Option<u32>,
    #[zvariant(rename = "field-b")]
    field_b: ::std::string::String,
    field_c: ::std::vec::Vec<u8>,
}
