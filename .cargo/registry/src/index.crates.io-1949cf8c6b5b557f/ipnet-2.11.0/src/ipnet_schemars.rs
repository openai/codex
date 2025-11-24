use crate::Ipv4Net;
use crate::Ipv6Net;
use crate::IpNet;

use alloc::{
    boxed::Box,
    string::{
        String,
        ToString
    },
    vec,
};

use schemars::{JsonSchema, gen::SchemaGenerator, schema::{SubschemaValidation, Schema, SchemaObject, StringValidation, Metadata, SingleOrVec, InstanceType}};

impl JsonSchema for Ipv4Net {
    fn schema_name() -> String {
        "Ipv4Net".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some("IPv4 network".to_string()),
                description: Some("An IPv4 address with prefix length".to_string()),
                examples: vec![
                    schemars::_serde_json::Value::String("0.0.0.0/0".to_string()),
                    schemars::_serde_json::Value::String("192.168.0.0/24".to_string()),
                ],
                ..Default::default()
            })),
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
            string: Some(Box::new(StringValidation {
                max_length: Some(18),
                min_length: None,
                pattern: Some(r#"^(?:(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\/(?:3[0-2]|[1-2][0-9]|[0-9])$"#.to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }) 
    }
}
impl JsonSchema for Ipv6Net {
    fn schema_name() -> String {
        "Ipv6Net".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some("IPv6 network".to_string()),
                description: Some("An IPv6 address with prefix length".to_string()),
                examples: vec![
                    schemars::_serde_json::Value::String("::/0".to_string()),
                    schemars::_serde_json::Value::String("fd00::/32".to_string()),
                ],
                ..Default::default()
            })),
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
            string: Some(Box::new(StringValidation {
                max_length: Some(43),
                min_length: None,
                pattern: Some(r#"^[0-9A-Fa-f:\.]+\/(?:[0-9]|[1-9][0-9]|1[0-1][0-9]|12[0-8])$"#.to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }) 
    }
}
impl JsonSchema for IpNet {
    fn schema_name() -> String {
        "IpNet".to_string()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some("IP network".to_string()),
                description: Some("An IPv4 or IPv6 address with prefix length".to_string()),
                examples: vec![
                    schemars::_serde_json::Value::String("192.168.0.0/24".to_string()),
                    schemars::_serde_json::Value::String("fd00::/32".to_string()),
                ],
                ..Default::default()
            })),
            subschemas: Some(Box::new(
                SubschemaValidation {
                    one_of: Some(vec![Ipv4Net::json_schema(gen), Ipv6Net::json_schema(gen)]),
                    ..Default::default()
                }
            )),
            ..Default::default()
        }) 
    }
}
