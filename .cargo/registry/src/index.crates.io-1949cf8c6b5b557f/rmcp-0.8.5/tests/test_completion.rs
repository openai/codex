use std::collections::HashMap;

use rmcp::model::*;
use serde_json::json;

#[test]
fn test_completion_context_serialization() {
    let mut args = HashMap::new();
    args.insert("key1".to_string(), "value1".to_string());
    args.insert("key2".to_string(), "value2".to_string());

    let context = CompletionContext::with_arguments(args);

    // Test serialization
    let json = serde_json::to_value(&context).unwrap();
    let expected = json!({
        "arguments": {
            "key1": "value1",
            "key2": "value2"
        }
    });
    assert_eq!(json, expected);

    // Test deserialization
    let deserialized: CompletionContext = serde_json::from_value(expected).unwrap();
    assert_eq!(deserialized, context);
}

#[test]
fn test_completion_context_methods() {
    let mut args = HashMap::new();
    args.insert("city".to_string(), "San Francisco".to_string());
    args.insert("country".to_string(), "USA".to_string());

    let context = CompletionContext::with_arguments(args);

    assert!(context.has_arguments());
    assert_eq!(
        context.get_argument("city"),
        Some(&"San Francisco".to_string())
    );
    assert_eq!(context.get_argument("missing"), None);

    let names: Vec<&str> = context.argument_names().collect();
    assert!(names.contains(&"city"));
    assert!(names.contains(&"country"));
    assert_eq!(names.len(), 2);
}

#[test]
fn test_complete_request_param_serialization() {
    let mut args = HashMap::new();
    args.insert("previous_input".to_string(), "test".to_string());

    let request = CompleteRequestParam {
        r#ref: Reference::for_prompt("weather_prompt"),
        argument: ArgumentInfo {
            name: "location".to_string(),
            value: "San".to_string(),
        },
        context: Some(CompletionContext::with_arguments(args)),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert!(json["ref"]["name"].as_str().unwrap() == "weather_prompt");
    assert!(json["argument"]["name"].as_str().unwrap() == "location");
    assert!(json["argument"]["value"].as_str().unwrap() == "San");
    assert!(
        json["context"]["arguments"]["previous_input"]
            .as_str()
            .unwrap()
            == "test"
    );
}

#[test]
fn test_completion_info_validation() {
    // Valid completion with less than max values
    let values = vec!["option1".to_string(), "option2".to_string()];
    let completion = CompletionInfo::new(values.clone()).unwrap();
    assert_eq!(completion.values, values);
    assert!(completion.validate().is_ok());

    // Test max values limit
    let many_values: Vec<String> = (0..=CompletionInfo::MAX_VALUES)
        .map(|i| format!("option_{}", i))
        .collect();
    let result = CompletionInfo::new(many_values);
    assert!(result.is_err());
}

#[test]
fn test_completion_info_helper_methods() {
    let values = vec!["test1".to_string(), "test2".to_string()];

    // Test with_all_values
    let completion = CompletionInfo::with_all_values(values.clone()).unwrap();
    assert_eq!(completion.values, values);
    assert_eq!(completion.total, Some(2));
    assert_eq!(completion.has_more, Some(false));
    assert!(!completion.has_more_results());
    assert_eq!(completion.total_available(), Some(2));

    // Test with_pagination
    let paginated = CompletionInfo::with_pagination(values.clone(), Some(10), true).unwrap();
    assert_eq!(paginated.values, values);
    assert_eq!(paginated.total, Some(10));
    assert_eq!(paginated.has_more, Some(true));
    assert!(paginated.has_more_results());
    assert_eq!(paginated.total_available(), Some(10));
}

#[test]
fn test_completion_info_bounds() {
    // Test exactly at the limit
    let max_values: Vec<String> = (0..CompletionInfo::MAX_VALUES)
        .map(|i| format!("value_{}", i))
        .collect();
    assert!(CompletionInfo::new(max_values).is_ok());

    // Test over the limit
    let over_limit: Vec<String> = (0..=CompletionInfo::MAX_VALUES)
        .map(|i| format!("value_{}", i))
        .collect();
    assert!(CompletionInfo::new(over_limit).is_err());
}

#[test]
fn test_reference_convenience_methods() {
    let prompt_ref = Reference::for_prompt("test_prompt");
    assert_eq!(prompt_ref.reference_type(), "ref/prompt");
    assert_eq!(prompt_ref.as_prompt_name(), Some("test_prompt"));
    assert_eq!(prompt_ref.as_resource_uri(), None);

    let resource_ref = Reference::for_resource("file://path/to/resource");
    assert_eq!(resource_ref.reference_type(), "ref/resource");
    assert_eq!(
        resource_ref.as_resource_uri(),
        Some("file://path/to/resource")
    );
    assert_eq!(resource_ref.as_prompt_name(), None);
}

#[test]
fn test_completion_serialization_format() {
    // Test that completion follows MCP 2025-06-18 specification format
    let completion = CompletionInfo {
        values: vec!["value1".to_string(), "value2".to_string()],
        total: Some(2),
        has_more: Some(false),
    };

    let json = serde_json::to_value(&completion).unwrap();

    // Verify JSON structure matches specification
    assert!(json.is_object());
    assert!(json["values"].is_array());
    assert_eq!(json["values"].as_array().unwrap().len(), 2);
    assert_eq!(json["total"].as_u64().unwrap(), 2);
    assert!(!json["hasMore"].as_bool().unwrap());
}

#[test]
fn test_resource_reference() {
    // Test that ResourceReference works correctly
    let resource_ref = ResourceReference {
        uri: "test://uri".to_string(),
    };

    // Test that ResourceReference works correctly
    let another_ref = ResourceReference {
        uri: "test://uri".to_string(),
    };

    // They should be equivalent
    assert_eq!(resource_ref.uri, another_ref.uri);
}

#[test]
fn test_complete_result_default() {
    let result = CompleteResult::default();
    assert!(result.completion.values.is_empty());
    assert_eq!(result.completion.total, None);
    assert_eq!(result.completion.has_more, None);
}

#[test]
fn test_completion_context_empty() {
    let context = CompletionContext::new();
    assert!(!context.has_arguments());
    assert_eq!(context.get_argument("any"), None);
    assert!(context.argument_names().count() == 0);
}

#[test]
fn test_mcp_schema_compliance() {
    // Test that our types serialize correctly according to MCP specification
    let request = CompleteRequestParam {
        r#ref: Reference::for_resource("file://{path}"),
        argument: ArgumentInfo {
            name: "path".to_string(),
            value: "src/".to_string(),
        },
        context: None,
    };

    let json_str = serde_json::to_string(&request).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // Verify key structure matches MCP spec
    assert!(parsed["ref"].is_object());
    assert!(parsed["argument"].is_object());
    assert!(parsed["argument"]["name"].is_string());
    assert!(parsed["argument"]["value"].is_string());

    // Verify type tag is correct
    assert_eq!(parsed["ref"]["type"].as_str().unwrap(), "ref/resource");
}
