//cargo test --test test_prompt_macro_annotations --features "client server"
#![allow(dead_code)]

use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{GetPromptResult, Prompt, PromptMessage, PromptMessageRole},
    prompt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
struct TestServer;

impl ServerHandler for TestServer {}

#[derive(Serialize, Deserialize, JsonSchema)]
struct TestArgs {
    /// The input text to process
    input: String,
    /// Optional configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct ComplexArgs {
    /// Required field
    required_field: String,
    /// Optional string field
    #[schemars(description = "An optional string parameter")]
    optional_string: Option<String>,
    /// Optional number field
    optional_number: Option<i64>,
    /// Array field
    items: Vec<String>,
}

// Test basic prompt attribute generation
#[prompt]
async fn basic_prompt(_server: &TestServer) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Basic response",
    )]
}

// Test prompt with custom name
#[prompt(name = "custom_name")]
async fn named_prompt(_server: &TestServer) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Named response",
    )]
}

// Test prompt with custom description
#[prompt(description = "This is a custom description")]
async fn described_prompt(_server: &TestServer) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Described response",
    )]
}

// Test prompt with both name and description
#[prompt(name = "full_custom", description = "Fully customized prompt")]
async fn fully_custom_prompt(_server: &TestServer) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Fully custom response",
    )]
}

// Test prompt with doc comments
/// This is a doc comment description
/// that spans multiple lines
#[prompt]
async fn doc_comment_prompt(_server: &TestServer) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Doc comment response",
    )]
}

// Test prompt with doc comments and explicit description (explicit wins)
/// This is a doc comment
#[prompt(description = "This overrides the doc comment")]
async fn override_doc_prompt(_server: &TestServer) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Override response",
    )]
}

// Test prompt with arguments
#[prompt]
async fn args_prompt(_server: &TestServer, _args: Parameters<TestArgs>) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Args response",
    )]
}

// Test prompt with complex arguments
#[prompt]
async fn complex_args_prompt(
    _server: &TestServer,
    _args: Parameters<ComplexArgs>,
) -> GetPromptResult {
    GetPromptResult {
        description: Some("Complex args result".to_string()),
        messages: vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            "Complex response",
        )],
    }
}

// Test sync prompt
#[prompt]
fn sync_prompt(_server: &TestServer) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Sync response",
    )]
}

#[test]
fn test_basic_prompt_attr() {
    let attr = basic_prompt_prompt_attr();
    assert_eq!(attr.name, "basic_prompt");
    assert_eq!(attr.description, None);
    assert!(attr.arguments.is_none());
}

#[test]
fn test_named_prompt_attr() {
    let attr = named_prompt_prompt_attr();
    assert_eq!(attr.name, "custom_name");
    assert_eq!(attr.description, None);
    assert!(attr.arguments.is_none());
}

#[test]
fn test_described_prompt_attr() {
    let attr = described_prompt_prompt_attr();
    assert_eq!(attr.name, "described_prompt");
    assert_eq!(
        attr.description.as_deref(),
        Some("This is a custom description")
    );
    assert!(attr.arguments.is_none());
}

#[test]
fn test_fully_custom_prompt_attr() {
    let attr = fully_custom_prompt_prompt_attr();
    assert_eq!(attr.name, "full_custom");
    assert_eq!(attr.description.as_deref(), Some("Fully customized prompt"));
    assert!(attr.arguments.is_none());
}

#[test]
fn test_doc_comment_prompt_attr() {
    let attr = doc_comment_prompt_prompt_attr();
    assert_eq!(attr.name, "doc_comment_prompt");
    assert!(attr.description.is_some());
    let desc = attr.description.unwrap();
    assert!(desc.contains("This is a doc comment description"));
    assert!(desc.contains("that spans multiple lines"));
}

#[test]
fn test_override_doc_prompt_attr() {
    let attr = override_doc_prompt_prompt_attr();
    assert_eq!(attr.name, "override_doc_prompt");
    assert_eq!(
        attr.description.as_deref(),
        Some("This overrides the doc comment")
    );
}

#[test]
fn test_args_prompt_attr() {
    let attr = args_prompt_prompt_attr();
    assert_eq!(attr.name, "args_prompt");

    let args = attr.arguments.as_ref().unwrap();
    assert_eq!(args.len(), 2);

    // Check input field
    let input_arg = args.iter().find(|a| a.name == "input").unwrap();
    assert_eq!(input_arg.required, Some(true));
    assert_eq!(
        input_arg.description.as_deref(),
        Some("The input text to process")
    );

    // Check config field
    let config_arg = args.iter().find(|a| a.name == "config").unwrap();
    assert_eq!(config_arg.required, Some(false));
    assert_eq!(
        config_arg.description.as_deref(),
        Some("Optional configuration")
    );
}

#[test]
fn test_complex_args_prompt_attr() {
    let attr = complex_args_prompt_prompt_attr();
    assert_eq!(attr.name, "complex_args_prompt");

    let args = attr.arguments.as_ref().unwrap();
    assert_eq!(args.len(), 4);

    // Check required_field
    let required_arg = args.iter().find(|a| a.name == "required_field").unwrap();
    assert_eq!(required_arg.required, Some(true));
    assert_eq!(required_arg.description.as_deref(), Some("Required field"));

    // Check optional_string
    let optional_string_arg = args.iter().find(|a| a.name == "optional_string").unwrap();
    assert_eq!(optional_string_arg.required, Some(false));
    assert_eq!(
        optional_string_arg.description.as_deref(),
        Some("An optional string parameter")
    );

    // Check optional_number
    let optional_number_arg = args.iter().find(|a| a.name == "optional_number").unwrap();
    assert_eq!(optional_number_arg.required, Some(false));
    assert_eq!(
        optional_number_arg.description.as_deref(),
        Some("Optional number field")
    );

    // Check items
    let items_arg = args.iter().find(|a| a.name == "items").unwrap();
    assert_eq!(items_arg.required, Some(true));
    assert_eq!(items_arg.description.as_deref(), Some("Array field"));
}

#[test]
fn test_sync_prompt_attr() {
    let attr = sync_prompt_prompt_attr();
    assert_eq!(attr.name, "sync_prompt");
    assert!(attr.arguments.is_none());
}

#[test]
fn test_prompt_attr_function_type() {
    // Test that the generated function returns the correct type
    fn assert_prompt_attr_fn(_: impl Fn() -> Prompt) {}

    assert_prompt_attr_fn(basic_prompt_prompt_attr);
    assert_prompt_attr_fn(named_prompt_prompt_attr);
    assert_prompt_attr_fn(described_prompt_prompt_attr);
    assert_prompt_attr_fn(fully_custom_prompt_prompt_attr);
    assert_prompt_attr_fn(doc_comment_prompt_prompt_attr);
    assert_prompt_attr_fn(override_doc_prompt_prompt_attr);
    assert_prompt_attr_fn(args_prompt_prompt_attr);
    assert_prompt_attr_fn(complex_args_prompt_prompt_attr);
    assert_prompt_attr_fn(sync_prompt_prompt_attr);
}

// Test generic prompts
#[derive(Debug, Clone)]
struct GenericServer<T: Send + Sync + 'static> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: Send + Sync + 'static> ServerHandler for GenericServer<T> {}

#[prompt]
async fn generic_prompt<T: Send + Sync + 'static>(
    _server: &GenericServer<T>,
) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Generic response",
    )]
}

#[test]
fn test_generic_prompt_attr() {
    let attr = generic_prompt_prompt_attr();
    assert_eq!(attr.name, "generic_prompt");
    assert!(attr.arguments.is_none());
}
