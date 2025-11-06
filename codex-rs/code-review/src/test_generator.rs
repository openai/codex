//! AI-powered test generation

use anyhow::Result;
use std::path::Path;
use tracing::info;

use crate::ai_client::AIClient;
use crate::ast_analyzer::{extract_function_signatures, FunctionSignature};

/// Test generator using AI
pub struct TestGenerator {
    ai_client: AIClient,
}

impl TestGenerator {
    /// Create a new test generator
    pub fn new(ai_client: AIClient) -> Self {
        Self { ai_client }
    }

    /// Generate tests for a file
    pub async fn generate_tests(&self, source_code: &str, language: &str) -> Result<GeneratedTests> {
        info!("Generating tests for {} code", language);

        let tests = match language {
            "rust" => self.generate_rust_tests(source_code).await?,
            "python" => self.generate_python_tests(source_code).await?,
            "javascript" | "typescript" => self.generate_js_tests(source_code).await?,
            _ => {
                return Ok(GeneratedTests {
                    tests: vec![],
                    coverage_estimate: 0.0,
                });
            }
        };

        Ok(tests)
    }

    /// Generate Rust tests
    async fn generate_rust_tests(&self, source: &str) -> Result<GeneratedTests> {
        // Extract function signatures
        let signatures = extract_function_signatures(source)?;

        let mut tests = Vec::new();

        for sig in &signatures {
            let test = self.generate_rust_test_for_function(source, sig).await?;
            tests.push(test);
        }

        let coverage_estimate = if signatures.is_empty() {
            0.0
        } else {
            (tests.len() as f32 / signatures.len() as f32) * 100.0
        };

        Ok(GeneratedTests {
            tests,
            coverage_estimate,
        })
    }

    /// Generate a test for a specific Rust function
    async fn generate_rust_test_for_function(
        &self,
        source: &str,
        sig: &FunctionSignature,
    ) -> Result<Test> {
        let prompt = format!(
            r#"Generate a comprehensive unit test for this Rust function.

Function signature: {}({} parameters) -> {}

Full source code:
```rust
{}
```

Generate a test that:
1. Tests normal cases
2. Tests edge cases
3. Tests error cases if applicable

Provide ONLY the test function code in this format:
```rust
#[test]
fn test_{}() {{
    // Your test code here
}}
```
"#,
            sig.name,
            sig.inputs,
            sig.output,
            source,
            sig.name
        );

        let response = self
            .ai_client
            .call_ollama(self.ai_client.coding_model(), &prompt)
            .await?;

        // Extract code from response
        let test_code = self.extract_code_block(&response);

        Ok(Test {
            name: format!("test_{}", sig.name),
            code: test_code,
            function_tested: sig.name.clone(),
            test_type: TestType::Unit,
        })
    }

    /// Generate Python tests
    async fn generate_python_tests(&self, source: &str) -> Result<GeneratedTests> {
        let prompt = format!(
            r#"Generate comprehensive pytest tests for this Python code:

```python
{}
```

Generate tests that cover:
1. Normal cases
2. Edge cases
3. Error handling
4. Type checking (if applicable)

Provide test code only."#,
            source
        );

        let response = self
            .ai_client
            .call_ollama(self.ai_client.coding_model(), &prompt)
            .await?;

        let test_code = self.extract_code_block(&response);

        Ok(GeneratedTests {
            tests: vec![Test {
                name: "test_module".to_string(),
                code: test_code,
                function_tested: "module".to_string(),
                test_type: TestType::Unit,
            }],
            coverage_estimate: 80.0,
        })
    }

    /// Generate JavaScript tests
    async fn generate_js_tests(&self, source: &str) -> Result<GeneratedTests> {
        let prompt = format!(
            r#"Generate comprehensive Jest tests for this JavaScript/TypeScript code:

```javascript
{}
```

Generate tests that cover:
1. Happy path
2. Edge cases
3. Error scenarios
4. Async behavior (if applicable)

Provide test code only."#,
            source
        );

        let response = self
            .ai_client
            .call_ollama(self.ai_client.coding_model(), &prompt)
            .await?;

        let test_code = self.extract_code_block(&response);

        Ok(GeneratedTests {
            tests: vec![Test {
                name: "module.test.js".to_string(),
                code: test_code,
                function_tested: "module".to_string(),
                test_type: TestType::Unit,
            }],
            coverage_estimate: 75.0,
        })
    }

    /// Extract code block from AI response
    fn extract_code_block(&self, response: &str) -> String {
        // Find code block between ```
        if let Some(start) = response.find("```") {
            let after_start = &response[start + 3..];

            // Skip language identifier
            let code_start = after_start.find('\n').map(|i| i + 1).unwrap_or(0);
            let code_part = &after_start[code_start..];

            if let Some(end) = code_part.find("```") {
                return code_part[..end].trim().to_string();
            }
        }

        // If no code block found, return cleaned response
        response.trim().to_string()
    }

    /// Generate integration tests
    pub async fn generate_integration_tests(
        &self,
        modules: &[&str],
        language: &str,
    ) -> Result<Test> {
        let prompt = format!(
            r#"Generate integration tests for these {} modules:

{}

Create tests that verify:
1. Module interactions
2. Data flow between components
3. End-to-end scenarios

Provide test code only."#,
            language,
            modules.join("\n")
        );

        let response = self
            .ai_client
            .call_ollama(self.ai_client.coding_model(), &prompt)
            .await?;

        let test_code = self.extract_code_block(&response);

        Ok(Test {
            name: "integration_test".to_string(),
            code: test_code,
            function_tested: "multiple".to_string(),
            test_type: TestType::Integration,
        })
    }
}

/// Generated tests result
#[derive(Debug, Clone)]
pub struct GeneratedTests {
    pub tests: Vec<Test>,
    pub coverage_estimate: f32,
}

/// A generated test
#[derive(Debug, Clone)]
pub struct Test {
    pub name: String,
    pub code: String,
    pub function_tested: String,
    pub test_type: TestType,
}

/// Type of test
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestType {
    Unit,
    Integration,
    EndToEnd,
}

/// Test file writer
pub struct TestFileWriter;

impl TestFileWriter {
    /// Write tests to a file
    pub async fn write_tests(
        &self,
        tests: &GeneratedTests,
        output_path: &Path,
        language: &str,
    ) -> Result<()> {
        let mut content = String::new();

        match language {
            "rust" => {
                content.push_str("#[cfg(test)]\nmod tests {\n");
                content.push_str("    use super::*;\n\n");

                for test in &tests.tests {
                    content.push_str("    ");
                    content.push_str(&test.code.replace('\n', "\n    "));
                    content.push_str("\n\n");
                }

                content.push_str("}\n");
            }
            "python" => {
                content.push_str("import pytest\n\n");
                for test in &tests.tests {
                    content.push_str(&test.code);
                    content.push_str("\n\n");
                }
            }
            "javascript" | "typescript" => {
                content.push_str("const { describe, it, expect } = require('@jest/globals');\n\n");
                for test in &tests.tests {
                    content.push_str(&test.code);
                    content.push_str("\n\n");
                }
            }
            _ => {}
        }

        tokio::fs::write(output_path, content).await?;
        info!("Wrote tests to {:?}", output_path);

        Ok(())
    }

    /// Append tests to existing file
    pub async fn append_tests(
        &self,
        tests: &GeneratedTests,
        file_path: &Path,
        language: &str,
    ) -> Result<()> {
        let existing = tokio::fs::read_to_string(file_path)
            .await
            .unwrap_or_default();

        let mut content = existing;

        match language {
            "rust" => {
                // Find or create #[cfg(test)] mod
                if !content.contains("#[cfg(test)]") {
                    content.push_str("\n#[cfg(test)]\nmod tests {\n");
                    content.push_str("    use super::*;\n\n");
                    for test in &tests.tests {
                        content.push_str("    ");
                        content.push_str(&test.code.replace('\n', "\n    "));
                        content.push_str("\n\n");
                    }
                    content.push_str("}\n");
                } else {
                    // Append to existing test module
                    if let Some(pos) = content.rfind('}') {
                        for test in &tests.tests {
                            let indented = format!("    {}\n\n", test.code.replace('\n', "\n    "));
                            content.insert_str(pos, &indented);
                        }
                    }
                }
            }
            _ => {
                content.push_str("\n\n");
                for test in &tests.tests {
                    content.push_str(&test.code);
                    content.push_str("\n\n");
                }
            }
        }

        tokio::fs::write(file_path, content).await?;
        info!("Appended tests to {:?}", file_path);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_code_block() {
        let ai_client = AIClient::new(crate::config::LocalAIConfig::default()).unwrap();
        let generator = TestGenerator::new(ai_client);

        let response = r#"
Here's the test:

```rust
#[test]
fn test_add() {
    assert_eq!(add(2, 2), 4);
}
```
"#;

        let code = generator.extract_code_block(response);
        assert!(code.contains("#[test]"));
        assert!(code.contains("test_add"));
    }

    #[test]
    fn test_extract_code_block_no_markers() {
        let ai_client = AIClient::new(crate::config::LocalAIConfig::default()).unwrap();
        let generator = TestGenerator::new(ai_client);

        let response = "just some text";
        let code = generator.extract_code_block(response);
        assert_eq!(code, "just some text");
    }
}
