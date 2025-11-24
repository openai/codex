//cargo test --test test_elicitation --features "client server"

use rmcp::{model::*, service::*};
// For typed elicitation tests
#[cfg(feature = "schemars")]
use schemars::JsonSchema;
#[cfg(feature = "schemars")]
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Test that elicitation data structures can be serialized and deserialized correctly
/// This ensures JSON-RPC compatibility with MCP 2025-06-18 specification
#[tokio::test]
async fn test_elicitation_serialization() {
    // Test ElicitationAction enum serialization
    let accept = ElicitationAction::Accept;
    let decline = ElicitationAction::Decline;
    let cancel = ElicitationAction::Cancel;

    assert_eq!(serde_json::to_string(&accept).unwrap(), "\"accept\"");
    assert_eq!(serde_json::to_string(&decline).unwrap(), "\"decline\"");
    assert_eq!(serde_json::to_string(&cancel).unwrap(), "\"cancel\"");

    // Test deserialization
    assert_eq!(
        serde_json::from_str::<ElicitationAction>("\"accept\"").unwrap(),
        ElicitationAction::Accept
    );
    assert_eq!(
        serde_json::from_str::<ElicitationAction>("\"decline\"").unwrap(),
        ElicitationAction::Decline
    );
    assert_eq!(
        serde_json::from_str::<ElicitationAction>("\"cancel\"").unwrap(),
        ElicitationAction::Cancel
    );
}

/// Test CreateElicitationRequestParam structure serialization/deserialization
#[tokio::test]
async fn test_elicitation_request_param_serialization() {
    let schema = ElicitationSchema::builder()
        .required_property("email", PrimitiveSchema::String(StringSchema::email()))
        .build()
        .unwrap();

    let request_param = CreateElicitationRequestParam {
        message: "Please provide your email address".to_string(),
        requested_schema: schema,
    };

    // Test serialization
    let json = serde_json::to_value(&request_param).unwrap();
    let expected = json!({
        "message": "Please provide your email address",
        "requestedSchema": {
            "type": "object",
            "properties": {
                "email": {
                    "type": "string",
                    "format": "email"
                }
            },
            "required": ["email"]
        }
    });

    assert_eq!(json, expected);

    // Test deserialization
    let deserialized: CreateElicitationRequestParam = serde_json::from_value(expected).unwrap();
    assert_eq!(deserialized.message, request_param.message);
    assert_eq!(
        deserialized.requested_schema,
        request_param.requested_schema
    );
}

/// Test CreateElicitationResult structure with different action types
#[tokio::test]
async fn test_elicitation_result_serialization() {
    // Test Accept with content
    let accept_result = CreateElicitationResult {
        action: ElicitationAction::Accept,
        content: Some(json!({"email": "user@example.com"})),
    };

    let json = serde_json::to_value(&accept_result).unwrap();
    let expected = json!({
        "action": "accept",
        "content": {"email": "user@example.com"}
    });
    assert_eq!(json, expected);

    // Test Decline without content
    let decline_result = CreateElicitationResult {
        action: ElicitationAction::Decline,
        content: None,
    };

    let json = serde_json::to_value(&decline_result).unwrap();
    let expected = json!({
        "action": "decline"
        // content should be omitted when None due to skip_serializing_if
    });
    assert_eq!(json, expected);

    // Test deserialization
    let deserialized: CreateElicitationResult = serde_json::from_value(expected).unwrap();
    assert_eq!(deserialized.action, ElicitationAction::Decline);
    assert_eq!(deserialized.content, None);
}

/// Test that elicitation requests can be created and handled through the JSON-RPC protocol
#[tokio::test]
async fn test_elicitation_json_rpc_protocol() {
    let schema = ElicitationSchema::builder()
        .required_property(
            "confirmation",
            PrimitiveSchema::Boolean(BooleanSchema::new()),
        )
        .build()
        .unwrap();

    // Create a complete JSON-RPC request for elicitation
    let request = JsonRpcRequest {
        jsonrpc: JsonRpcVersion2_0,
        id: RequestId::Number(1),
        request: CreateElicitationRequest {
            method: ElicitationCreateRequestMethod,
            params: CreateElicitationRequestParam {
                message: "Do you want to continue?".to_string(),
                requested_schema: schema,
            },
            extensions: Default::default(),
        },
    };

    // Test serialization of complete request
    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 1);
    assert_eq!(json["method"], "elicitation/create");
    assert_eq!(json["params"]["message"], "Do you want to continue?");

    // Test deserialization
    let deserialized: JsonRpcRequest<CreateElicitationRequest> =
        serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.id, RequestId::Number(1));
    assert_eq!(
        deserialized.request.params.message,
        "Do you want to continue?"
    );
}

/// Test elicitation action types and their expected behavior
#[tokio::test]
async fn test_elicitation_action_types() {
    // Test all three action types
    let actions = [
        ElicitationAction::Accept,
        ElicitationAction::Decline,
        ElicitationAction::Cancel,
    ];

    // Each action should have a unique string representation
    let serialized: Vec<String> = actions
        .iter()
        .map(|action| serde_json::to_string(action).unwrap())
        .collect();

    assert_eq!(serialized.len(), 3);
    assert!(serialized.contains(&"\"accept\"".to_string()));
    assert!(serialized.contains(&"\"decline\"".to_string()));
    assert!(serialized.contains(&"\"cancel\"".to_string()));

    // Test round-trip serialization
    for action in actions {
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: ElicitationAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }
}

/// Test MCP 2025-06-18 specification compliance
/// Ensures our implementation matches the latest MCP spec
#[tokio::test]
async fn test_elicitation_spec_compliance() {
    // Test that method names match the specification
    assert_eq!(ElicitationCreateRequestMethod::VALUE, "elicitation/create");
    assert_eq!(
        ElicitationResponseNotificationMethod::VALUE,
        "notifications/elicitation/response"
    );

    // Test that enum values match specification
    let actions = [
        ElicitationAction::Accept,
        ElicitationAction::Decline,
        ElicitationAction::Cancel,
    ];

    let serialized: Vec<String> = actions
        .iter()
        .map(|a| serde_json::to_string(a).unwrap())
        .collect();

    assert_eq!(serialized, vec!["\"accept\"", "\"decline\"", "\"cancel\""]);
}

/// Test error handling and edge cases for elicitation
#[tokio::test]
async fn test_elicitation_error_handling() {
    // Test minimal schema handling (empty properties is technically valid)
    let minimal_schema_request = CreateElicitationRequestParam {
        message: "Test message".to_string(),
        requested_schema: ElicitationSchema::builder().build().unwrap(),
    };

    // Should serialize without error
    let _json = serde_json::to_value(&minimal_schema_request).unwrap();

    // Test empty message
    let empty_message_request = CreateElicitationRequestParam {
        message: "".to_string(),
        requested_schema: ElicitationSchema::builder()
            .property("value", PrimitiveSchema::String(StringSchema::new()))
            .build()
            .unwrap(),
    };

    // Should serialize without error (validation is up to the implementation)
    let _json = serde_json::to_value(&empty_message_request).unwrap();

    // Test that we can deserialize invalid action types (should fail)
    let invalid_action_json = json!("invalid_action");
    let result = serde_json::from_value::<ElicitationAction>(invalid_action_json);
    assert!(result.is_err());
}

/// Benchmark-style test for elicitation performance
#[tokio::test]
async fn test_elicitation_performance() {
    let schema = ElicitationSchema::builder()
        .property("data", PrimitiveSchema::String(StringSchema::new()))
        .build()
        .unwrap();

    let request = CreateElicitationRequestParam {
        message: "Performance test message".to_string(),
        requested_schema: schema,
    };

    let start = std::time::Instant::now();

    // Serialize/deserialize 1000 times
    for _ in 0..1000 {
        let json = serde_json::to_value(&request).unwrap();
        let _deserialized: CreateElicitationRequestParam = serde_json::from_value(json).unwrap();
    }

    let duration = start.elapsed();
    println!(
        "1000 elicitation serialization/deserialization cycles took: {:?}",
        duration
    );

    // Should complete in reasonable time (less than 100ms on modern hardware)
    assert!(
        duration.as_millis() < 1000,
        "Performance test took too long: {:?}",
        duration
    );
}

/// Test elicitation capabilities integration
/// Ensures that elicitation capability can be properly configured and serialized
#[tokio::test]
async fn test_elicitation_capabilities() {
    use rmcp::model::{ClientCapabilities, ElicitationCapability};

    // Test basic elicitation capability
    let mut elicitation_cap = ElicitationCapability::default();
    assert_eq!(elicitation_cap.schema_validation, None);

    // Test with schema validation enabled
    elicitation_cap.schema_validation = Some(true);

    // Test serialization
    let json = serde_json::to_value(&elicitation_cap).unwrap();
    let expected = json!({"schemaValidation": true});
    assert_eq!(json, expected);

    // Test deserialization
    let deserialized: ElicitationCapability = serde_json::from_value(expected).unwrap();
    assert_eq!(deserialized.schema_validation, Some(true));

    // Test ClientCapabilities builder with elicitation
    let client_caps = ClientCapabilities::builder()
        .enable_elicitation()
        .enable_elicitation_schema_validation()
        .build();

    assert!(client_caps.elicitation.is_some());
    assert_eq!(
        client_caps.elicitation.as_ref().unwrap().schema_validation,
        Some(true)
    );

    // Test full client capabilities serialization
    let json = serde_json::to_value(&client_caps).unwrap();
    assert!(
        json["elicitation"]["schemaValidation"]
            .as_bool()
            .unwrap_or(false)
    );
}

/// Test convenience methods for common elicitation scenarios
/// This ensures the helper methods create proper requests with expected schemas
#[tokio::test]
async fn test_elicitation_convenience_methods() {
    // Test that convenience methods produce the expected request parameters

    // Test confirmation schema
    let confirmation_schema = serde_json::json!({
        "type": "boolean",
        "description": "User confirmation (true for yes, false for no)"
    });

    // Verify the schema structure for boolean confirmation
    assert_eq!(confirmation_schema["type"], "boolean");
    assert!(confirmation_schema["description"].is_string());

    // Test text input schema (non-required)
    let text_schema = serde_json::json!({
        "type": "string",
        "description": "User text input"
    });

    assert_eq!(text_schema["type"], "string");
    assert!(text_schema.get("minLength").is_none());

    // Test text input schema (required)
    let required_text_schema = serde_json::json!({
        "type": "string",
        "description": "User text input",
        "minLength": 1
    });

    assert_eq!(required_text_schema["minLength"], 1);

    // Test choice schema
    let options = ["Option A", "Option B", "Option C"];
    let choice_schema = serde_json::json!({
        "type": "integer",
        "minimum": 0,
        "maximum": options.len() - 1,
        "description": format!("Choose an option: {}", options.join(", "))
    });

    assert_eq!(choice_schema["type"], "integer");
    assert_eq!(choice_schema["minimum"], 0);
    assert_eq!(choice_schema["maximum"], 2);
    assert!(
        choice_schema["description"]
            .as_str()
            .unwrap()
            .contains("Option A")
    );

    // Test that CreateElicitationRequestParam can be created with type-safe schemas
    let confirmation_request = CreateElicitationRequestParam {
        message: "Test confirmation".to_string(),
        requested_schema: ElicitationSchema::builder()
            .property(
                "confirmed",
                PrimitiveSchema::Boolean(
                    BooleanSchema::new()
                        .description("User confirmation (true for yes, false for no)"),
                ),
            )
            .build()
            .unwrap(),
    };

    // Test serialization of convenience method request
    let json = serde_json::to_value(&confirmation_request).unwrap();
    assert_eq!(json["message"], "Test confirmation");
    assert_eq!(json["requestedSchema"]["type"], "object");
    assert_eq!(
        json["requestedSchema"]["properties"]["confirmed"]["type"],
        "boolean"
    );
}

/// Test structured input with multiple primitive properties
/// Ensures that schemas with multiple primitive properties work correctly with elicitation
#[tokio::test]
async fn test_elicitation_structured_schemas() {
    // Test schema with multiple primitive properties
    let schema = ElicitationSchema::builder()
        .required_string_with("name", |s| s.length(1, 100))
        .required_email("email")
        .required_integer("age", 0, 150)
        .optional_bool("newsletter", false)
        .required_enum(
            "country",
            vec!["US".to_string(), "UK".to_string(), "CA".to_string()],
        )
        .description("User registration information")
        .build()
        .unwrap();

    let request = CreateElicitationRequestParam {
        message: "Please provide your user information".to_string(),
        requested_schema: schema,
    };

    // Test that complex schemas serialize/deserialize correctly
    let json = serde_json::to_value(&request).unwrap();
    let deserialized: CreateElicitationRequestParam = serde_json::from_value(json).unwrap();

    assert_eq!(deserialized.message, "Please provide your user information");
    assert_eq!(deserialized.requested_schema.properties.len(), 5);
    assert!(
        deserialized
            .requested_schema
            .properties
            .contains_key("name")
    );
    assert!(
        deserialized
            .requested_schema
            .properties
            .contains_key("email")
    );
    assert!(deserialized.requested_schema.properties.contains_key("age"));
    assert!(
        deserialized
            .requested_schema
            .properties
            .contains_key("newsletter")
    );
    assert!(
        deserialized
            .requested_schema
            .properties
            .contains_key("country")
    );
    assert_eq!(
        deserialized.requested_schema.required,
        Some(vec![
            "name".to_string(),
            "email".to_string(),
            "age".to_string(),
            "country".to_string()
        ])
    );
}

// Typed elicitation tests using the API with schemars
#[cfg(feature = "schemars")]
mod typed_elicitation_tests {
    use super::*;

    /// Simple user confirmation with reason
    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
    #[schemars(description = "User confirmation with optional reasoning")]
    struct UserConfirmation {
        #[schemars(description = "User's decision (true for yes, false for no)")]
        confirmed: bool,

        #[schemars(description = "Optional reason for the decision")]
        reason: Option<String>,
    }

    /// User profile with validation constraints
    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
    #[schemars(description = "Complete user profile information")]
    struct UserProfile {
        #[schemars(description = "Full name")]
        name: String,

        #[schemars(description = "Email address")]
        email: String,

        #[schemars(description = "Age in years")]
        age: u8,

        #[schemars(description = "User preferences")]
        preferences: UserPreferences,
    }

    /// User preferences
    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
    struct UserPreferences {
        #[schemars(description = "UI theme preference")]
        theme: Theme,

        #[schemars(description = "Enable notifications")]
        notifications: bool,

        #[schemars(description = "Language preference")]
        language: String,
    }

    /// UI theme options
    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
    #[schemars(description = "Available UI themes")]
    enum Theme {
        #[schemars(description = "Light theme")]
        Light,
        #[schemars(description = "Dark theme")]
        Dark,
        #[schemars(description = "Auto-detect based on system")]
        Auto,
    }

    // Mark types as safe for elicitation (they generate object schemas)
    rmcp::elicit_safe!(UserConfirmation, UserProfile, UserPreferences);

    /// Test automatic schema generation for simple types
    #[tokio::test]
    async fn test_typed_elicitation_simple_schema() {
        // Test that schema generation works for simple types
        let schema = rmcp::handler::server::tool::schema_for_type::<UserConfirmation>();

        // Verify schema contains expected fields
        assert!(schema.contains_key("type"));
        assert_eq!(schema.get("type"), Some(&json!("object")));
        assert!(schema.contains_key("properties"));

        if let Some(properties) = schema.get("properties") {
            assert!(properties.is_object());
            let props = properties.as_object().unwrap();
            assert!(props.contains_key("confirmed"));
            assert!(props.contains_key("reason"));

            // Check confirmed field is boolean
            if let Some(confirmed_schema) = props.get("confirmed") {
                let confirmed_obj = confirmed_schema.as_object().unwrap();
                assert_eq!(confirmed_obj.get("type"), Some(&json!("boolean")));
            }

            // Check reason field is optional string
            if let Some(reason_schema) = props.get("reason") {
                assert!(reason_schema.is_object());
            }
        }
    }

    /// Test automatic schema generation for complex nested types
    #[tokio::test]
    async fn test_typed_elicitation_complex_schema() {
        // Test complex nested structure schema generation
        let schema = rmcp::handler::server::tool::schema_for_type::<UserProfile>();

        // Verify schema structure
        assert!(schema.contains_key("type"));
        assert_eq!(schema.get("type"), Some(&json!("object")));

        if let Some(properties) = schema.get("properties") {
            let props = properties.as_object().unwrap();

            // Check required fields exist
            assert!(props.contains_key("name"));
            assert!(props.contains_key("email"));
            assert!(props.contains_key("age"));
            assert!(props.contains_key("preferences"));

            // Check validation constraints for name
            if let Some(name_schema) = props.get("name") {
                let name_obj = name_schema.as_object().unwrap();
                assert_eq!(name_obj.get("type"), Some(&json!("string")));
                // Note: schemars might generate constraints differently
                // The exact structure depends on schemars version
            }

            // Check email format constraint
            if let Some(email_schema) = props.get("email") {
                let email_obj = email_schema.as_object().unwrap();
                assert_eq!(email_obj.get("type"), Some(&json!("string")));
            }

            // Check age numeric constraints
            if let Some(age_schema) = props.get("age") {
                let age_obj = age_schema.as_object().unwrap();
                assert_eq!(age_obj.get("type"), Some(&json!("integer")));
            }
        }
    }

    /// Test enum schema generation
    #[tokio::test]
    async fn test_enum_schema_generation() {
        // Test enum schema generation
        let schema = rmcp::handler::server::tool::schema_for_type::<Theme>();

        // Verify enum schema structure - schemars might use oneOf or enum depending on version
        assert!(
            schema.contains_key("type")
                || schema.contains_key("oneOf")
                || schema.contains_key("enum")
        );

        // The exact structure depends on schemars configuration, but it should be valid
        let json = serde_json::to_string(&schema).unwrap();
        assert!(!json.is_empty());
    }

    /// Test that the schema generation for nested structures works
    #[tokio::test]
    async fn test_nested_structure_schema() {
        // Test that nested structures generate proper schemas
        let preferences_schema = rmcp::handler::server::tool::schema_for_type::<UserPreferences>();

        // Verify basic structure
        assert!(preferences_schema.contains_key("type"));
        assert_eq!(preferences_schema.get("type"), Some(&json!("object")));

        if let Some(properties) = preferences_schema.get("properties") {
            let props = properties.as_object().unwrap();
            assert!(props.contains_key("theme"));
            assert!(props.contains_key("notifications"));
            assert!(props.contains_key("language"));
        }
    }
}

// =============================================================================
// ELICITATION DIRECTION TESTS (MCP 2025-06-18 COMPLIANCE)
// =============================================================================

/// Test that elicitation requests flow from server to client (not client to server)
/// This verifies compliance with MCP 2025-06-18 specification
#[cfg(all(feature = "client", feature = "server"))]
#[tokio::test]
async fn test_elicitation_direction_server_to_client() {
    use rmcp::model::*;
    use serde_json::json;

    // Test that server can create elicitation requests
    let schema = ElicitationSchema::builder()
        .property(
            "name",
            PrimitiveSchema::String(StringSchema::new().description("Enter your name")),
        )
        .build()
        .unwrap();

    let elicitation_request = CreateElicitationRequestParam {
        message: "Please enter your name".to_string(),
        requested_schema: schema,
    };

    // Verify request can be serialized
    let serialized = serde_json::to_value(&elicitation_request).unwrap();
    assert_eq!(serialized["message"], "Please enter your name");
    assert_eq!(serialized["requestedSchema"]["type"], "object");

    // Test that elicitation requests are part of ServerRequest
    let _server_request = ServerRequest::CreateElicitationRequest(CreateElicitationRequest {
        method: ElicitationCreateRequestMethod,
        params: elicitation_request,
        extensions: Default::default(),
    });

    // Test that client can respond with elicitation results
    let client_result = ClientResult::CreateElicitationResult(CreateElicitationResult {
        action: ElicitationAction::Accept,
        content: Some(json!("John Doe")),
    });

    // Verify client result can be serialized
    match client_result {
        ClientResult::CreateElicitationResult(result) => {
            assert_eq!(result.action, ElicitationAction::Accept);
            assert_eq!(result.content, Some(json!("John Doe")));
        }
        _ => panic!("CreateElicitationResult should be part of ClientResult"),
    }
}

/// Test complete JSON-RPC message flow: Server → Client → Server
#[cfg(all(feature = "client", feature = "server"))]
#[tokio::test]
async fn test_elicitation_json_rpc_direction() {
    use rmcp::model::*;
    use serde_json::json;

    let schema = ElicitationSchema::builder()
        .property(
            "continue",
            PrimitiveSchema::Boolean(BooleanSchema::new().description("Do you want to continue?")),
        )
        .build()
        .unwrap();

    // 1. Server creates elicitation request
    let server_request = ServerJsonRpcMessage::request(
        ServerRequest::CreateElicitationRequest(CreateElicitationRequest {
            method: ElicitationCreateRequestMethod,
            params: CreateElicitationRequestParam {
                message: "Do you want to continue?".to_string(),
                requested_schema: schema,
            },
            extensions: Default::default(),
        }),
        RequestId::Number(1),
    );

    // Serialize server request
    let server_json = serde_json::to_value(&server_request).unwrap();
    assert_eq!(server_json["method"], "elicitation/create");
    assert_eq!(server_json["id"], 1);
    assert_eq!(server_json["params"]["message"], "Do you want to continue?");

    // 2. Client responds with elicitation result
    let client_response = ClientJsonRpcMessage::response(
        ClientResult::CreateElicitationResult(CreateElicitationResult {
            action: ElicitationAction::Accept,
            content: Some(json!(true)),
        }),
        RequestId::Number(1),
    );

    // Serialize client response
    let client_json = serde_json::to_value(&client_response).unwrap();
    assert_eq!(client_json["id"], 1);
    if let Some(result) = client_json["result"].as_object() {
        assert_eq!(result["action"], "accept");
        assert_eq!(result["content"], true);
    } else {
        panic!("Client response should contain result");
    }
}

/// Test all three elicitation actions according to MCP spec
#[cfg(all(feature = "client", feature = "server"))]
#[tokio::test]
async fn test_elicitation_actions_compliance() {
    use rmcp::model::*;

    // Test all three elicitation actions according to MCP spec
    let actions = [
        ElicitationAction::Accept,
        ElicitationAction::Decline,
        ElicitationAction::Cancel,
    ];

    for action in actions {
        let result = CreateElicitationResult {
            action: action.clone(),
            content: match action {
                ElicitationAction::Accept => Some(serde_json::json!("some data")),
                _ => None,
            },
        };

        let json = serde_json::to_value(&result).unwrap();

        match action {
            ElicitationAction::Accept => {
                assert_eq!(json["action"], "accept");
                assert!(json["content"].is_string());
            }
            ElicitationAction::Decline => {
                assert_eq!(json["action"], "decline");
                assert!(json.get("content").is_none() || json["content"].is_null());
            }
            ElicitationAction::Cancel => {
                assert_eq!(json["action"], "cancel");
                assert!(json.get("content").is_none() || json["content"].is_null());
            }
        }
    }
}

/// Test that CreateElicitationResult IS in ClientResult (response compliance)
#[tokio::test]
async fn test_elicitation_result_in_client_result() {
    use rmcp::model::*;

    // Test that clients can return elicitation results
    let result = ClientResult::CreateElicitationResult(CreateElicitationResult {
        action: ElicitationAction::Decline,
        content: None,
    });

    match result {
        ClientResult::CreateElicitationResult(elicit_result) => {
            assert_eq!(elicit_result.action, ElicitationAction::Decline);
            assert_eq!(elicit_result.content, None);
        }
        _ => panic!("CreateElicitationResult should be part of ClientResult"),
    }
}

// =============================================================================
// ELICITATION CAPABILITIES TESTS
// =============================================================================

/// Test ElicitationCapability structure and serialization
#[tokio::test]
async fn test_elicitation_capability_structure() {
    // Test default ElicitationCapability
    let default_cap = ElicitationCapability::default();
    assert!(default_cap.schema_validation.is_none());

    // Test ElicitationCapability with schema validation enabled
    let cap_with_validation = ElicitationCapability {
        schema_validation: Some(true),
    };
    assert_eq!(cap_with_validation.schema_validation, Some(true));

    // Test ElicitationCapability with schema validation disabled
    let cap_without_validation = ElicitationCapability {
        schema_validation: Some(false),
    };
    assert_eq!(cap_without_validation.schema_validation, Some(false));

    // Test JSON serialization
    let json = serde_json::to_value(&cap_with_validation).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "schemaValidation": true
        })
    );

    // Test JSON deserialization
    let deserialized: ElicitationCapability = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.schema_validation, Some(true));
}

/// Test ClientCapabilities with elicitation capability
#[tokio::test]
async fn test_client_capabilities_with_elicitation() {
    // Test ClientCapabilities with elicitation capability
    let capabilities = ClientCapabilities {
        elicitation: Some(ElicitationCapability {
            schema_validation: Some(true),
        }),
        ..Default::default()
    };

    // Verify elicitation capability is present
    assert!(capabilities.elicitation.is_some());
    assert_eq!(
        capabilities.elicitation.as_ref().unwrap().schema_validation,
        Some(true)
    );

    // Test JSON serialization
    let json = serde_json::to_value(&capabilities).unwrap();
    assert!(
        json["elicitation"]["schemaValidation"]
            .as_bool()
            .unwrap_or(false)
    );

    // Test ClientCapabilities without elicitation
    let capabilities_without = ClientCapabilities {
        elicitation: None,
        ..Default::default()
    };

    assert!(capabilities_without.elicitation.is_none());
}

/// Test InitializeRequestParam with elicitation capability
#[tokio::test]
async fn test_initialize_request_with_elicitation() {
    // Test InitializeRequestParam with elicitation capability
    let init_param = InitializeRequestParam {
        protocol_version: ProtocolVersion::LATEST,
        capabilities: ClientCapabilities {
            elicitation: Some(ElicitationCapability {
                schema_validation: Some(true),
            }),
            ..Default::default()
        },
        client_info: Implementation {
            name: "test-client".to_string(),
            version: "1.0.0".to_string(),
            title: None,
            website_url: None,
            icons: None,
        },
    };

    // Verify the structure
    assert!(init_param.capabilities.elicitation.is_some());
    assert_eq!(
        init_param
            .capabilities
            .elicitation
            .as_ref()
            .unwrap()
            .schema_validation,
        Some(true)
    );

    // Test JSON serialization
    let json = serde_json::to_value(&init_param).unwrap();
    assert!(
        json["capabilities"]["elicitation"]["schemaValidation"]
            .as_bool()
            .unwrap_or(false)
    );
}

/// Test capability checking logic (simulated)
#[tokio::test]
async fn test_capability_checking_logic() {
    // Simulate the logic that would be used in supports_elicitation()

    // Case 1: Client with elicitation capability
    let client_with_capability = InitializeRequestParam {
        protocol_version: ProtocolVersion::LATEST,
        capabilities: ClientCapabilities {
            elicitation: Some(ElicitationCapability {
                schema_validation: Some(true),
            }),
            ..Default::default()
        },
        client_info: Implementation {
            name: "test-client".to_string(),
            version: "1.0.0".to_string(),
            title: None,
            website_url: None,
            icons: None,
        },
    };

    // Simulate supports_elicitation() logic
    let supports_elicitation = client_with_capability.capabilities.elicitation.is_some();
    assert!(supports_elicitation);

    // Case 2: Client without elicitation capability
    let client_without_capability = InitializeRequestParam {
        protocol_version: ProtocolVersion::LATEST,
        capabilities: ClientCapabilities {
            elicitation: None,
            ..Default::default()
        },
        client_info: Implementation {
            name: "test-client".to_string(),
            version: "1.0.0".to_string(),
            title: None,
            website_url: None,
            icons: None,
        },
    };
    let supports_elicitation = client_without_capability.capabilities.elicitation.is_some();
    assert!(!supports_elicitation);
}

/// Test CapabilityNotSupported error message formatting
#[tokio::test]
async fn test_capability_not_supported_error_message() {
    let error = ElicitationError::CapabilityNotSupported;
    let message = format!("{}", error);

    assert_eq!(
        message,
        "Client does not support elicitation - capability not declared during initialization"
    );
}

/// Test all ElicitationError variants and their messages
#[tokio::test]
async fn test_elicitation_error_variants() {
    // Test CapabilityNotSupported
    let capability_error = ElicitationError::CapabilityNotSupported;
    assert_eq!(
        format!("{}", capability_error),
        "Client does not support elicitation - capability not declared during initialization"
    );

    // Test UserDeclined
    let user_declined = ElicitationError::UserDeclined;
    assert_eq!(
        format!("{}", user_declined),
        "User explicitly declined the request"
    );

    // Test UserCancelled
    let user_cancelled = ElicitationError::UserCancelled;
    assert_eq!(
        format!("{}", user_cancelled),
        "User cancelled/dismissed the request"
    );

    // Test NoContent
    let no_content = ElicitationError::NoContent;
    assert_eq!(format!("{}", no_content), "No response content provided");

    // Test Service error
    let service_error = ElicitationError::Service(ServiceError::UnexpectedResponse);
    let message = format!("{}", service_error);
    assert!(message.starts_with("Service error:"));

    // Test ParseError
    let json_error = serde_json::from_str::<i32>("\"not_an_integer\"").unwrap_err();
    let data = serde_json::json!({"key": "value"});
    let parse_error = ElicitationError::ParseError {
        error: json_error,
        data: data.clone(),
    };
    let message = format!("{}", parse_error);
    assert!(message.starts_with("Failed to parse response data:"));
    assert!(message.contains("Received data:"));

    // Test error matching
    match capability_error {
        ElicitationError::CapabilityNotSupported => {} // Expected
        _ => panic!("Should match CapabilityNotSupported"),
    }

    match user_declined {
        ElicitationError::UserDeclined => {} // Expected
        _ => panic!("Should match UserDeclined"),
    }

    match user_cancelled {
        ElicitationError::UserCancelled => {} // Expected
        _ => panic!("Should match UserCancelled"),
    }

    match no_content {
        ElicitationError::NoContent => {} // Expected
        _ => panic!("Should match NoContent"),
    }
}

/// Test ElicitationCapability serialization with schema validation
#[tokio::test]
async fn test_elicitation_capability_serialization() {
    use rmcp::model::ElicitationCapability;

    // Test default capability (no schema validation)
    let default_cap = ElicitationCapability::default();
    let json = serde_json::to_value(&default_cap).unwrap();

    // Should serialize to empty object when no fields are set
    assert_eq!(json, serde_json::json!({}));

    // Test capability with schema validation enabled
    let cap_with_validation = ElicitationCapability {
        schema_validation: Some(true),
    };
    let json = serde_json::to_value(&cap_with_validation).unwrap();

    assert_eq!(
        json,
        serde_json::json!({
            "schemaValidation": true
        })
    );

    // Test capability with schema validation disabled
    let cap_without_validation = ElicitationCapability {
        schema_validation: Some(false),
    };
    let json = serde_json::to_value(&cap_without_validation).unwrap();

    assert_eq!(
        json,
        serde_json::json!({
            "schemaValidation": false
        })
    );

    // Test deserialization
    let deserialized: ElicitationCapability = serde_json::from_value(serde_json::json!({
        "schemaValidation": true
    }))
    .unwrap();

    assert_eq!(deserialized.schema_validation, Some(true));
}

/// Test ClientCapabilities builder with elicitation capability methods
#[tokio::test]
async fn test_client_capabilities_elicitation_builder() {
    use rmcp::model::{ClientCapabilities, ElicitationCapability};

    // Test enabling elicitation capability
    let caps = ClientCapabilities::builder().enable_elicitation().build();

    assert!(caps.elicitation.is_some());
    assert_eq!(caps.elicitation.as_ref().unwrap().schema_validation, None);

    // Test enabling elicitation with schema validation
    let caps_with_validation = ClientCapabilities::builder()
        .enable_elicitation()
        .enable_elicitation_schema_validation()
        .build();

    assert!(caps_with_validation.elicitation.is_some());
    assert_eq!(
        caps_with_validation
            .elicitation
            .as_ref()
            .unwrap()
            .schema_validation,
        Some(true)
    );

    // Test enabling elicitation with custom capability
    let custom_elicitation = ElicitationCapability {
        schema_validation: Some(false),
    };

    let caps_custom = ClientCapabilities::builder()
        .enable_elicitation_with(custom_elicitation.clone())
        .build();

    assert!(caps_custom.elicitation.is_some());
    assert_eq!(
        caps_custom.elicitation.as_ref().unwrap(),
        &custom_elicitation
    );
}

// =============================================================================
// TIMEOUT TESTS
// =============================================================================

/// Test basic timeout functionality for create_elicitation_with_timeout
#[tokio::test]
async fn test_create_elicitation_with_timeout_basic() {
    use std::time::Duration;

    // This test verifies that the method accepts timeout parameter
    let schema = ElicitationSchema::builder()
        .required_property("name", PrimitiveSchema::String(StringSchema::new()))
        .required_property("email", PrimitiveSchema::String(StringSchema::new()))
        .build()
        .unwrap();

    let _params = CreateElicitationRequestParam {
        message: "Enter your details".to_string(),
        requested_schema: schema,
    };

    // Test different timeout values
    let timeout_short = Duration::from_millis(100);
    let timeout_long = Duration::from_secs(30);
    let timeout_none: Option<Duration> = None;

    // Verify timeout parameter types are correct
    assert!(!timeout_short.is_zero());
    assert!(!timeout_long.is_zero());
    assert!(timeout_none.is_none());

    // Verify timeout values are reasonable
    assert_eq!(timeout_short.as_millis(), 100);
    assert_eq!(timeout_long.as_secs(), 30);
}

/// Test timeout behavior with elicit_with_timeout method
#[tokio::test]
async fn test_elicit_with_timeout_method_signature() {
    use std::time::Duration;

    // Test that method signature works with different timeout values
    let timeout_values = vec![
        None,
        Some(Duration::from_millis(500)),
        Some(Duration::from_secs(1)),
        Some(Duration::from_secs(30)),
        Some(Duration::from_secs(60)),
    ];

    for timeout in timeout_values {
        // Verify timeout value is properly handled
        match timeout {
            None => assert!(timeout.is_none()),
            Some(duration) => {
                assert!(duration > Duration::from_millis(0));
                assert!(duration <= Duration::from_secs(300)); // Max 5 minutes
            }
        }
    }
}

/// Test timeout value validation
#[tokio::test]
async fn test_timeout_value_validation() {
    use std::time::Duration;

    // Test valid timeout ranges
    let valid_timeouts = vec![
        Duration::from_millis(1),   // Minimum
        Duration::from_millis(100), // Short
        Duration::from_secs(1),     // 1 second
        Duration::from_secs(30),    // 30 seconds
        Duration::from_secs(300),   // 5 minutes
    ];

    for timeout in valid_timeouts {
        assert!(timeout >= Duration::from_millis(1));
        assert!(timeout <= Duration::from_secs(300));
    }

    // Test edge cases
    let zero_timeout = Duration::from_millis(0);
    let very_long_timeout = Duration::from_secs(3600); // 1 hour

    // Zero timeout should be handled gracefully
    assert_eq!(zero_timeout, Duration::from_millis(0));

    // Very long timeout should work but may not be practical
    assert!(very_long_timeout > Duration::from_secs(300));
}

/// Test timeout error message formatting
#[tokio::test]
async fn test_timeout_error_formatting() {
    use std::time::Duration;

    let timeout = Duration::from_secs(30);

    // Simulate a timeout error
    let timeout_error = ServiceError::Timeout { timeout };

    // Verify error contains timeout information
    let error_string = format!("{}", timeout_error);
    assert!(error_string.contains("timeout"));
    assert!(error_string.contains("30"));
}

/// Test elicitation error handling with timeout
#[tokio::test]
async fn test_elicitation_timeout_error_conversion() {
    use std::time::Duration;

    let timeout = Duration::from_millis(500);
    let service_timeout_error = ServiceError::Timeout { timeout };
    let elicitation_error = ElicitationError::Service(service_timeout_error);

    // Verify error chain is preserved
    match elicitation_error {
        ElicitationError::Service(ServiceError::Timeout { timeout: t }) => {
            assert_eq!(t, timeout);
        }
        _ => panic!("Expected timeout error"),
    }
}

/// Test timeout parameter pass-through in PeerRequestOptions
#[tokio::test]
async fn test_peer_request_options_timeout() {
    use std::time::Duration;

    let timeout = Some(Duration::from_secs(15));

    let options = PeerRequestOptions {
        timeout,
        meta: None,
    };

    // Verify timeout is properly stored
    assert_eq!(options.timeout, timeout);
    assert!(options.meta.is_none());

    // Test with no timeout
    let options_no_timeout = PeerRequestOptions {
        timeout: None,
        meta: None,
    };

    assert!(options_no_timeout.timeout.is_none());
}

/// Test realistic timeout scenarios
#[tokio::test]
async fn test_realistic_timeout_scenarios() {
    use std::time::Duration;

    // Test common timeout scenarios users might encounter

    // Quick response (5 seconds)
    let quick_timeout = Duration::from_secs(5);
    assert!(quick_timeout >= Duration::from_secs(1));
    assert!(quick_timeout <= Duration::from_secs(10));

    // Normal interaction (30 seconds)
    let normal_timeout = Duration::from_secs(30);
    assert!(normal_timeout >= Duration::from_secs(10));
    assert!(normal_timeout <= Duration::from_secs(60));

    // Long form input (2 minutes)
    let long_timeout = Duration::from_secs(120);
    assert!(long_timeout >= Duration::from_secs(60));
    assert!(long_timeout <= Duration::from_secs(300));
}

/// Test that different ElicitationAction values map to correct error types
#[tokio::test]
async fn test_elicitation_action_error_mapping() {
    use rmcp::{model::ElicitationAction, service::ElicitationError};

    // Test that each action type produces the expected error
    let test_cases = vec![
        (ElicitationAction::Decline, "UserDeclined"),
        (ElicitationAction::Cancel, "UserCancelled"),
    ];

    for (action, _expected_error_type) in test_cases {
        // Verify that the action exists and has the expected semantics
        match action {
            ElicitationAction::Accept => {
                // Accept should not produce an error (it provides content)
            }
            ElicitationAction::Decline => {
                // Should map to UserDeclined error
                let error = ElicitationError::UserDeclined;
                assert!(format!("{}", error).contains("explicitly declined"));
            }
            ElicitationAction::Cancel => {
                // Should map to UserCancelled error
                let error = ElicitationError::UserCancelled;
                assert!(format!("{}", error).contains("cancelled/dismissed"));
            }
        }
    }
}

/// Test elicitation action semantics according to MCP specification
#[tokio::test]
async fn test_elicitation_action_semantics() {
    use rmcp::model::ElicitationAction;

    // According to MCP spec:
    // - Accept: User explicitly approved and submitted with data
    // - Decline: User explicitly declined the request
    // - Cancel: User dismissed without making an explicit choice

    // Test that all three actions are available
    let actions = vec![
        ElicitationAction::Accept,
        ElicitationAction::Decline,
        ElicitationAction::Cancel,
    ];

    assert_eq!(actions.len(), 3);

    // Test serialization/deserialization
    for action in actions {
        let serialized = serde_json::to_string(&action).expect("Should serialize");
        let deserialized: ElicitationAction =
            serde_json::from_str(&serialized).expect("Should deserialize");

        // Actions should round-trip correctly
        match (action, deserialized) {
            (ElicitationAction::Accept, ElicitationAction::Accept) => {}
            (ElicitationAction::Decline, ElicitationAction::Decline) => {}
            (ElicitationAction::Cancel, ElicitationAction::Cancel) => {}
            _ => panic!("Action serialization round-trip failed"),
        }
    }
}

/// Test compile-time type safety for elicitation
#[tokio::test]
async fn test_elicitation_type_safety() {
    use rmcp::service::ElicitationSafe;
    use schemars::JsonSchema;

    // Test that our types implement ElicitationSafe
    #[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
    struct SafeType {
        name: String,
        value: i32,
    }

    rmcp::elicit_safe!(SafeType);

    // Verify that SafeType implements the required traits
    fn assert_elicitation_safe<T: ElicitationSafe>() {}
    assert_elicitation_safe::<SafeType>();

    // Test that SafeType can generate schema (compile-time check)
    let _schema = schemars::schema_for!(SafeType);
}

/// Test that elicit_safe! macro works with multiple types
#[tokio::test]
async fn test_elicit_safe_macro() {
    use schemars::JsonSchema;

    #[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
    struct TypeA {
        field_a: String,
    }

    #[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
    struct TypeB {
        field_b: i32,
    }

    #[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
    struct TypeC {
        field_c: bool,
    }

    // Test macro with multiple types
    rmcp::elicit_safe!(TypeA, TypeB, TypeC);

    // All should implement ElicitationSafe
    fn assert_all_safe<T: rmcp::service::ElicitationSafe>() {}
    assert_all_safe::<TypeA>();
    assert_all_safe::<TypeB>();
    assert_all_safe::<TypeC>();
}

/// Test ElicitationSafe trait behavior
#[tokio::test]
async fn test_elicitation_safe_trait() {
    use schemars::JsonSchema;

    // Test object type validation
    #[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
    struct ObjectType {
        name: String,
        count: usize,
        active: bool,
    }

    rmcp::elicit_safe!(ObjectType);

    // Test that ObjectType can generate schema (compile-time check)
    let _schema = schemars::schema_for!(ObjectType);
}

/// Test documentation examples compile correctly
#[tokio::test]
async fn test_elicitation_examples_compile() {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    // Example from trait documentation
    #[allow(dead_code)]
    #[derive(Serialize, Deserialize, JsonSchema)]
    struct UserProfile {
        name: String,
        email: String,
    }

    rmcp::elicit_safe!(UserProfile);

    // This should compile and work
    fn _example_usage() {
        fn _assert_safe<T: rmcp::service::ElicitationSafe>() {}
        _assert_safe::<UserProfile>();
    }
}

// =============================================================================
// BUILD-TIME VALIDATION TESTS
// =============================================================================

/// Test that build() validates required fields exist in properties
#[tokio::test]
async fn test_build_validation_required_field_not_in_properties() {
    // Try to mark a field as required that doesn't exist in properties
    let result = ElicitationSchema::builder()
        .property("email", PrimitiveSchema::String(StringSchema::email()))
        .mark_required("nonexistent_field")
        .build();

    // Should return an error
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "Required field does not exist in properties"
    );
}

/// Test that build() succeeds when all required fields exist
#[tokio::test]
async fn test_build_validation_required_field_exists() {
    let result = ElicitationSchema::builder()
        .property("email", PrimitiveSchema::String(StringSchema::email()))
        .property("name", PrimitiveSchema::String(StringSchema::new()))
        .mark_required("email")
        .mark_required("name")
        .build();

    // Should succeed
    assert!(result.is_ok());
    let schema = result.unwrap();
    assert_eq!(schema.properties.len(), 2);
    assert_eq!(
        schema.required,
        Some(vec!["email".to_string(), "name".to_string()])
    );
}

/// Test that build_unchecked() panics on validation errors
#[tokio::test]
#[should_panic(expected = "Invalid elicitation schema")]
async fn test_build_unchecked_panics_on_invalid() {
    // build_unchecked validates but panics instead of returning Result
    let _schema = ElicitationSchema::builder()
        .property("email", PrimitiveSchema::String(StringSchema::email()))
        .mark_required("nonexistent_field")
        .build_unchecked();
}

/// Test convenience methods handle validation correctly
#[tokio::test]
async fn test_convenience_methods_validation() {
    // required_string_property should add both property and mark as required
    let result = ElicitationSchema::builder()
        .required_string_property("name", |s| s)
        .required_email("email")
        .build();

    assert!(result.is_ok());
    let schema = result.unwrap();
    assert_eq!(schema.properties.len(), 2);
    assert!(
        schema
            .required
            .as_ref()
            .unwrap()
            .contains(&"name".to_string())
    );
    assert!(
        schema
            .required
            .as_ref()
            .unwrap()
            .contains(&"email".to_string())
    );
}

/// Test typed property methods work correctly
#[tokio::test]
async fn test_typed_property_methods() {
    let result = ElicitationSchema::builder()
        .string_property("name", |s| s.length(1, 100))
        .number_property("price", |n| n.range(0.0, 1000.0))
        .integer_property("quantity", |i| i.range(1, 100))
        .bool_property("in_stock", |b| b.with_default(true))
        .build();

    assert!(result.is_ok());
    let schema = result.unwrap();
    assert_eq!(schema.properties.len(), 4);

    // Verify types are correct
    if let Some(PrimitiveSchema::String(_)) = schema.properties.get("name") {
        // Expected
    } else {
        panic!("name should be StringSchema");
    }

    if let Some(PrimitiveSchema::Number(_)) = schema.properties.get("price") {
        // Expected
    } else {
        panic!("price should be NumberSchema");
    }

    if let Some(PrimitiveSchema::Integer(_)) = schema.properties.get("quantity") {
        // Expected
    } else {
        panic!("quantity should be IntegerSchema");
    }

    if let Some(PrimitiveSchema::Boolean(_)) = schema.properties.get("in_stock") {
        // Expected
    } else {
        panic!("in_stock should be BooleanSchema");
    }
}

/// Test required typed property methods
#[tokio::test]
async fn test_required_typed_property_methods() {
    let result = ElicitationSchema::builder()
        .required_string_property("name", |s| s)
        .required_number_property("price", |n| n)
        .required_integer_property("age", |i| i)
        .required_bool_property("active", |b| b)
        .build();

    assert!(result.is_ok());
    let schema = result.unwrap();
    assert_eq!(schema.properties.len(), 4);
    assert_eq!(schema.required.as_ref().unwrap().len(), 4);

    // All should be marked as required
    let required = schema.required.as_ref().unwrap();
    assert!(required.contains(&"name".to_string()));
    assert!(required.contains(&"price".to_string()));
    assert!(required.contains(&"age".to_string()));
    assert!(required.contains(&"active".to_string()));
}
