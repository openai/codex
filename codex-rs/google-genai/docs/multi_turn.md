# Multi-Turn Conversation Guide

This document provides a comprehensive guide for implementing multi-turn conversations with the Google GenAI (Gemini) API, including tool calling and reasoning/thought handling.

Reference: [python-genai](https://github.com/googleapis/python-genai)

---

## Table of Contents

1. [Overview](#1-overview)
2. [History Management](#2-history-management)
3. [Tool Calling Flow](#3-tool-calling-flow)
4. [Reasoning/Thought Handling](#4-reasoningthought-handling)
5. [Wire Format Reference](#5-wire-format-reference)
6. [Rust API Reference](#6-rust-api-reference)

---

## 1. Overview

### Dual History Architecture

The SDK maintains two parallel histories to handle invalid responses gracefully:

```
┌─────────────────────────────────────────────────────────────┐
│                    Chat Session                              │
├─────────────────────────────────────────────────────────────┤
│  curated_history        │  comprehensive_history            │
│  (Valid turns only)     │  (ALL turns including invalid)    │
│  → Sent to API          │  → For debugging/logging          │
└─────────────────────────────────────────────────────────────┘
```

### Request/Response Flow

```
Turn 1: User Message
    │
    ├─► API Request: curated_history + [user_message]
    │
    ├─◄ API Response: model content (may include function_calls)
    │
    └─► Update histories based on validation

Turn 2: Function Responses (if any)
    │
    ├─► API Request: curated_history + [function_responses]
    │
    └─► Continue...
```

---

## 2. History Management

### What Gets Sent to API

**Only `curated_history`** is sent to the API. This ensures:
- Invalid responses don't pollute context
- Token budget is used efficiently
- Model sees only coherent conversation flow

### Validation Logic

A response is **valid** if:

```rust
fn is_valid_response(response: &GenerateContentResponse) -> bool {
    // Must have candidates
    // Must have content in first candidate
    // Must have non-empty parts
    // Each part must have at least one field set:
    //   - text, function_call, function_response,
    //   - inline_data, file_data, or thought=true
}
```

**Invalid responses** (excluded from curated history):
- Empty parts array
- Parts with no fields set
- Missing candidates

### History Update Flow

```rust
// After API call in send_message_with_parts():

// 1. Always add to comprehensive history (for debugging)
self.comprehensive_history.push(user_content.clone());
self.comprehensive_history.push(model_content.clone());

// 2. Only add to curated history if valid
if is_valid_response(&response) {
    self.curated_history.push(user_content);
    self.curated_history.push(model_content);
}
```

### Accessing History

```rust
// Get curated history (default, sent to API)
let history = chat.history();

// Get specific history type
let curated = chat.get_history(true);
let comprehensive = chat.get_history(false);
```

---

## 3. Tool Calling Flow

### Overview

```
┌──────────────────────────────────────────────────────────────┐
│                    Tool Calling Lifecycle                     │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│   User Message                                               │
│        │                                                     │
│        ▼                                                     │
│   ┌─────────────────┐                                        │
│   │   Model Output  │  ← Contains FunctionCall parts        │
│   │   (role=model)  │    with unique IDs                    │
│   └────────┬────────┘                                        │
│            │                                                 │
│            ▼                                                 │
│   Execute Functions Locally                                  │
│            │                                                 │
│            ▼                                                 │
│   ┌─────────────────┐                                        │
│   │  User Response  │  ← FunctionResponse parts             │
│   │   (role=user)   │    with matching IDs                  │
│   └────────┬────────┘                                        │
│            │                                                 │
│            ▼                                                 │
│   Model Continues (text or more function calls)              │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### ID Pairing Mechanism

**Critical**: `FunctionCall.id` must match `FunctionResponse.id`

```json
// Model returns:
{
  "functionCall": {
    "id": "call_weather_tokyo",  // ← Unique ID
    "name": "get_weather",
    "args": {"city": "Tokyo"}
  }
}

// User sends back:
{
  "functionResponse": {
    "id": "call_weather_tokyo",  // ← MUST MATCH
    "name": "get_weather",
    "response": {"temperature": 12}
  }
}
```

### Concurrent Tool Calls

The model can return multiple function calls in a single response:

```json
{
  "role": "model",
  "parts": [
    {"functionCall": {"id": "call_1", "name": "get_weather", "args": {...}}},
    {"functionCall": {"id": "call_2", "name": "get_weather", "args": {...}}},
    {"functionCall": {"id": "call_3", "name": "search_flights", "args": {...}}}
  ]
}
```

All responses must be sent back in a single user turn:

```json
{
  "role": "user",
  "parts": [
    {"functionResponse": {"id": "call_1", "name": "get_weather", "response": {...}}},
    {"functionResponse": {"id": "call_2", "name": "get_weather", "response": {...}}},
    {"functionResponse": {"id": "call_3", "name": "search_flights", "response": {...}}}
  ]
}
```

### Rust API for Tool Calling

```rust
// Single function response
chat.send_function_response("get_weather", json!({"temp": 12})).await?;

// Single function response with ID (recommended)
chat.send_function_response_with_id(
    "call_weather_tokyo",
    "get_weather",
    json!({"temperature": 12})
).await?;

// Batch function responses
chat.send_function_responses(vec![
    ("get_weather", json!({"temp": 12})),
    ("search_flights", json!({"flights": [...]})),
]).await?;

// Batch function responses with IDs (recommended for concurrent calls)
chat.send_function_responses_with_ids(vec![
    (Some("call_1"), "get_weather", json!({"temp": 12})),
    (Some("call_2"), "get_weather", json!({"temp": 8})),
    (Some("call_3"), "search_flights", json!({"flights": [...]})),
]).await?;
```

---

## 4. Reasoning/Thought Handling

### Thought Parts in Responses

When `ThinkingConfig` is enabled, the model may return thought parts:

```json
{
  "role": "model",
  "parts": [
    {
      "thought": true,
      "text": "User wants weather data. I should call the weather function.",
      "thoughtSignature": "sig_abc123"
    },
    {
      "functionCall": {"id": "call_1", "name": "get_weather", "args": {...}}
    }
  ]
}
```

### Filtering Thoughts from Text

The `.text()` method **automatically filters out thought parts**:

```rust
// Returns only non-thought text parts concatenated
let text = response.text();  // Excludes thought content

// To get thought content specifically:
let thought_text = response.thought_text();

// Check if response has thoughts:
if response.has_thoughts() {
    println!("Model reasoning: {}", response.thought_text().unwrap_or_default());
}
```

### Thought Signatures

**Thought signatures are opaque tokens** that allow the model to resume reasoning:

```rust
// Extract signatures from response
let signatures = response.thought_signatures();

// Signatures are automatically preserved in history when model content is added
// They will be sent back in the next request as part of the history
```

### Should Thought Signatures Be Sent Back?

**Yes, automatically.** When the model's response (including thought parts) is added to history, the `thoughtSignature` fields are preserved. On the next request, the full history is sent, including these signatures.

This allows the model to:
- Resume reasoning chains efficiently
- Avoid re-deriving context
- Maintain coherent multi-turn reasoning

### Manual Thought Signature Usage

In advanced scenarios, you can manually create parts with thought signatures:

```rust
let thought_part = Part::with_thought_signature("sig_abc123");
chat.send_message_with_parts(vec![
    Part::text("Continue from where you left off"),
    thought_part,
], None).await?;
```

---

## 5. Wire Format Reference

### Complete 3-Turn Example

This example shows a travel planning conversation with concurrent tool calls.

#### Turn 1: Initial Request

**Request:**
```json
{
  "contents": [
    {
      "role": "user",
      "parts": [{"text": "I'm planning a trip. What's the weather in Tokyo and Paris? Also search for flights."}]
    }
  ],
  "systemInstruction": {
    "parts": [{"text": "You are a helpful travel assistant."}]
  },
  "tools": [
    {
      "functionDeclarations": [
        {
          "name": "get_weather",
          "description": "Get current weather for a city",
          "parameters": {
            "type": "OBJECT",
            "properties": {
              "city": {"type": "STRING", "description": "City name"}
            },
            "required": ["city"]
          }
        },
        {
          "name": "search_flights",
          "description": "Search available flights",
          "parameters": {
            "type": "OBJECT",
            "properties": {
              "from": {"type": "STRING"},
              "to": {"type": "STRING"},
              "date": {"type": "STRING"}
            },
            "required": ["from", "to"]
          }
        }
      ]
    }
  ],
  "generationConfig": {
    "temperature": 0.7
  }
}
```

**Response (3 concurrent tool calls + thought):**
```json
{
  "candidates": [
    {
      "content": {
        "role": "model",
        "parts": [
          {
            "thought": true,
            "text": "User wants weather for two cities and flight info. I need to call get_weather twice and search_flights once.",
            "thoughtSignature": "sig_abc123_thought1"
          },
          {
            "functionCall": {
              "id": "call_weather_tokyo",
              "name": "get_weather",
              "args": {"city": "Tokyo"}
            }
          },
          {
            "functionCall": {
              "id": "call_weather_paris",
              "name": "get_weather",
              "args": {"city": "Paris"}
            }
          },
          {
            "functionCall": {
              "id": "call_flight_1",
              "name": "search_flights",
              "args": {"from": "Tokyo", "to": "Paris", "date": "2024-01-15"}
            }
          }
        ]
      },
      "finishReason": "STOP"
    }
  ],
  "usageMetadata": {
    "promptTokenCount": 150,
    "candidatesTokenCount": 80,
    "thoughtsTokenCount": 30,
    "totalTokenCount": 260
  }
}
```

#### Turn 2: Send Tool Outputs Back

**Request (with full history + function responses):**
```json
{
  "contents": [
    {
      "role": "user",
      "parts": [{"text": "I'm planning a trip. What's the weather in Tokyo and Paris? Also search for flights."}]
    },
    {
      "role": "model",
      "parts": [
        {
          "thought": true,
          "text": "User wants weather for two cities and flight info...",
          "thoughtSignature": "sig_abc123_thought1"
        },
        {
          "functionCall": {
            "id": "call_weather_tokyo",
            "name": "get_weather",
            "args": {"city": "Tokyo"}
          }
        },
        {
          "functionCall": {
            "id": "call_weather_paris",
            "name": "get_weather",
            "args": {"city": "Paris"}
          }
        },
        {
          "functionCall": {
            "id": "call_flight_1",
            "name": "search_flights",
            "args": {"from": "Tokyo", "to": "Paris", "date": "2024-01-15"}
          }
        }
      ]
    },
    {
      "role": "user",
      "parts": [
        {
          "functionResponse": {
            "id": "call_weather_tokyo",
            "name": "get_weather",
            "response": {
              "temperature": 12,
              "unit": "C",
              "conditions": "cloudy"
            }
          }
        },
        {
          "functionResponse": {
            "id": "call_weather_paris",
            "name": "get_weather",
            "response": {
              "temperature": 8,
              "unit": "C",
              "conditions": "rainy"
            }
          }
        },
        {
          "functionResponse": {
            "id": "call_flight_1",
            "name": "search_flights",
            "response": {
              "flights": [
                {"airline": "JAL", "price": 850, "departure": "10:00"},
                {"airline": "AirFrance", "price": 920, "departure": "14:30"}
              ]
            }
          }
        }
      ]
    }
  ],
  "systemInstruction": {
    "parts": [{"text": "You are a helpful travel assistant."}]
  },
  "tools": [...]
}
```

**Response (text + new tool call):**
```json
{
  "candidates": [
    {
      "content": {
        "role": "model",
        "parts": [
          {
            "thought": true,
            "text": "Got weather data and flights. Let me summarize and check hotels.",
            "thoughtSignature": "sig_def456_thought2"
          },
          {
            "text": "Tokyo is 12°C and cloudy. Paris is 8°C and rainy. Found 2 flights - JAL at $850 (10:00) or AirFrance at $920 (14:30). Would you like me to book one?"
          },
          {
            "functionCall": {
              "id": "call_hotel_1",
              "name": "search_hotels",
              "args": {"city": "Paris", "checkin": "2024-01-15", "nights": 3}
            }
          }
        ]
      },
      "finishReason": "STOP"
    }
  ]
}
```

#### Turn 3: Hotel Response + User Confirmation

**Request:**
```json
{
  "contents": [
    {"role": "user", "parts": [{"text": "I'm planning a trip..."}]},
    {
      "role": "model",
      "parts": [
        {"thought": true, "text": "...", "thoughtSignature": "sig_abc123_thought1"},
        {"functionCall": {"id": "call_weather_tokyo", "name": "get_weather", "args": {"city": "Tokyo"}}},
        {"functionCall": {"id": "call_weather_paris", "name": "get_weather", "args": {"city": "Paris"}}},
        {"functionCall": {"id": "call_flight_1", "name": "search_flights", "args": {"from": "Tokyo", "to": "Paris"}}}
      ]
    },
    {
      "role": "user",
      "parts": [
        {"functionResponse": {"id": "call_weather_tokyo", "name": "get_weather", "response": {"temperature": 12}}},
        {"functionResponse": {"id": "call_weather_paris", "name": "get_weather", "response": {"temperature": 8}}},
        {"functionResponse": {"id": "call_flight_1", "name": "search_flights", "response": {"flights": [...]}}}
      ]
    },
    {
      "role": "model",
      "parts": [
        {"thought": true, "text": "...", "thoughtSignature": "sig_def456_thought2"},
        {"text": "Tokyo is 12°C..."},
        {"functionCall": {"id": "call_hotel_1", "name": "search_hotels", "args": {"city": "Paris"}}}
      ]
    },
    {
      "role": "user",
      "parts": [
        {
          "functionResponse": {
            "id": "call_hotel_1",
            "name": "search_hotels",
            "response": {
              "hotels": [
                {"name": "Hotel Paris", "price": 150, "rating": 4.5},
                {"name": "Le Marais Inn", "price": 200, "rating": 4.8}
              ]
            }
          }
        },
        {"text": "Yes, book the JAL flight and Hotel Paris please."}
      ]
    }
  ]
}
```

**Response (booking tool calls):**
```json
{
  "candidates": [
    {
      "content": {
        "role": "model",
        "parts": [
          {
            "functionCall": {
              "id": "call_book_flight",
              "name": "book_flight",
              "args": {"flight_id": "JAL_10:00", "passenger": "user"}
            }
          },
          {
            "functionCall": {
              "id": "call_book_hotel",
              "name": "book_hotel",
              "args": {"hotel": "Hotel Paris", "nights": 3}
            }
          }
        ]
      }
    }
  ]
}
```

---

## 6. Rust API Reference

### Chat Creation

```rust
use google_genai::{Client, Chat, ChatBuilder, types::*};

// Simple chat
let mut chat = Chat::new(client, "gemini-2.0-flash");

// Chat with config
let config = GenerateContentConfig {
    temperature: Some(0.7),
    thinking_config: Some(ThinkingConfig::with_budget(1024)),
    tools: Some(vec![Tool::functions(vec![...])]),
    ..Default::default()
};
let mut chat = Chat::with_config(client, "gemini-2.0-flash", config);

// Using builder
let mut chat = ChatBuilder::new(client, "gemini-2.0-flash")
    .system_instruction("You are helpful")
    .temperature(0.7)
    .tools(vec![...])
    .build();
```

### Sending Messages

```rust
// Simple text
let response = chat.send_message("Hello").await?;

// With custom parts
let response = chat.send_message_with_parts(vec![
    Part::text("Describe this image"),
    Part::from_bytes(&image_data, "image/jpeg"),
], None).await?;
```

### Processing Function Calls

```rust
// Check for function calls
if let Some(calls) = response.function_calls() {
    // Collect responses with ID mapping
    let responses: Vec<(Option<&str>, &str, serde_json::Value)> = calls
        .iter()
        .map(|call| {
            let name = call.name.as_ref().unwrap();
            let args = call.args.as_ref().unwrap();

            // Execute function locally
            let result = match name.as_str() {
                "get_weather" => {
                    let city = args["city"].as_str().unwrap();
                    get_weather(city)
                }
                "search_flights" => {
                    search_flights(args)
                }
                _ => json!({"error": "Unknown function"})
            };

            (call.id.as_deref(), name.as_str(), result)
        })
        .collect();

    // Send all responses back
    let next_response = chat.send_function_responses_with_ids(responses).await?;
}
```

### Accessing Response Data

```rust
// Get text (excludes thoughts)
let text = response.text();

// Get thought content
let thoughts = response.thought_text();

// Get thought signatures
let signatures = response.thought_signatures();

// Check for function calls
let calls = response.function_calls();

// Get all parts
let parts = response.parts();
```

### History Access

```rust
// Get curated history (sent to API)
let history = chat.history();

// Get specific history
let curated = chat.get_history(true);
let comprehensive = chat.get_history(false);

// Clear history
chat.clear_history();

// Manually add to history
chat.add_to_history(Content::user("Manual entry"));
```

---

## Summary

| Feature | Description |
|---------|-------------|
| **Dual History** | `curated_history` (API) + `comprehensive_history` (debug) |
| **ID Pairing** | `FunctionCall.id` → `FunctionResponse.id` |
| **Concurrent Calls** | Multiple `functionCall` parts in one model Content |
| **Batch Responses** | Multiple `functionResponse` parts in one user Content |
| **Thought Filtering** | `.text()` excludes thoughts; use `.thought_text()` |
| **Signatures** | Preserved in history, sent back automatically |
| **Validation** | Invalid responses excluded from curated history |
