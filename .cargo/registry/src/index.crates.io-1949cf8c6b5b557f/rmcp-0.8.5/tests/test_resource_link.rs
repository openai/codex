use rmcp::model::{CallToolResult, Content, RawResource};

#[test]
fn test_resource_link_in_tool_result() {
    // Test creating a tool result with resource links
    let resource = RawResource::new("file:///test/file.txt", "test.txt");

    // Create a tool result with a resource link
    let result = CallToolResult::success(vec![
        Content::text("Found a file"),
        Content::resource_link(resource),
    ]);

    // Serialize to JSON to verify format
    let json = serde_json::to_string_pretty(&result).unwrap();
    println!("Tool result with resource link:\n{}", json);

    // Verify JSON contains expected structure
    assert!(
        json.contains("\"type\":\"resource_link\"") || json.contains("\"type\": \"resource_link\"")
    );
    assert!(
        json.contains("\"uri\":\"file:///test/file.txt\"")
            || json.contains("\"uri\": \"file:///test/file.txt\"")
    );
    assert!(json.contains("\"name\":\"test.txt\"") || json.contains("\"name\": \"test.txt\""));

    // Test deserialization
    let deserialized: CallToolResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.content.len(), 2);

    // Check the text content
    assert!(deserialized.content[0].as_text().is_some());

    // Check the resource link
    let resource_link = deserialized.content[1]
        .as_resource_link()
        .expect("Expected resource link in content[1]");
    assert_eq!(resource_link.uri, "file:///test/file.txt");
    assert_eq!(resource_link.name, "test.txt");
}

#[test]
fn test_resource_link_with_full_metadata() {
    let mut resource = RawResource::new("https://example.com/data.json", "API Data");
    resource.description = Some("JSON data from external API".to_string());
    resource.mime_type = Some("application/json".to_string());
    resource.size = Some(1024);

    let result = CallToolResult::success(vec![Content::resource_link(resource)]);

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: CallToolResult = serde_json::from_str(&json).unwrap();

    let resource_link = deserialized.content[0]
        .as_resource_link()
        .expect("Expected resource link");
    assert_eq!(resource_link.uri, "https://example.com/data.json");
    assert_eq!(resource_link.name, "API Data");
    assert_eq!(
        resource_link.description,
        Some("JSON data from external API".to_string())
    );
    assert_eq!(
        resource_link.mime_type,
        Some("application/json".to_string())
    );
    assert_eq!(resource_link.size, Some(1024));
}

#[test]
fn test_mixed_content_types() {
    // Test that resource links can be mixed with other content types
    let resource = RawResource::new("file:///doc.pdf", "Document");

    let result = CallToolResult::success(vec![
        Content::text("Processing complete"),
        Content::resource_link(resource),
        Content::embedded_text("memo://result", "Analysis results here"),
    ]);

    assert_eq!(result.content.len(), 3);
    assert!(result.content[0].as_text().is_some());
    assert!(result.content[1].as_resource_link().is_some());
    assert!(result.content[2].as_resource().is_some());
}
