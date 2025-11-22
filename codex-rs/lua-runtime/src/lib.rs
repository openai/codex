use mlua::{Value as LuaValue, prelude::*};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LuaError {
    #[error("Lua execution error: {0}")]
    Execution(String),

    #[error("Lua timeout after {0}ms")]
    Timeout(u64),

    #[error("Failed to convert value: {0}")]
    Conversion(String),

    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),
}

impl From<mlua::Error> for LuaError {
    fn from(err: mlua::Error) -> Self {
        LuaError::Execution(err.to_string())
    }
}

/// Configuration for the Lua runtime
#[derive(Debug, Clone)]
pub struct LuaRuntimeConfig {
    /// Whether to allow file I/O operations
    pub allow_file_io: bool,

    /// Whether to allow network operations
    pub allow_network: bool,

    /// Maximum execution time in milliseconds
    pub max_execution_time_ms: u64,

    /// Maximum memory usage in bytes (0 = unlimited)
    pub max_memory_bytes: usize,
}

impl Default for LuaRuntimeConfig {
    fn default() -> Self {
        Self {
            allow_file_io: false,
            allow_network: false,
            max_execution_time_ms: 5000,
            max_memory_bytes: 0, // unlimited by default
        }
    }
}

/// A sandboxed Lua runtime
pub struct LuaRuntime {
    lua: Arc<Lua>,
    config: LuaRuntimeConfig,
}

impl LuaRuntime {
    /// Create a new Lua runtime with the given configuration
    pub fn new(config: LuaRuntimeConfig) -> Result<Self, LuaError> {
        let lua = Lua::new();

        // Apply sandboxing by removing dangerous libraries
        Self::apply_sandbox(&lua, &config)?;

        Ok(Self {
            lua: Arc::new(lua),
            config,
        })
    }

    /// Create a new Lua runtime with default configuration
    pub fn new_default() -> Result<Self, LuaError> {
        Self::new(LuaRuntimeConfig::default())
    }

    /// Apply sandbox restrictions to the Lua environment
    fn apply_sandbox(lua: &Lua, config: &LuaRuntimeConfig) -> Result<(), LuaError> {
        lua.load(
            r#"
            -- Remove dangerous libraries
            os = nil
            io = nil
            debug = nil
            dofile = nil
            loadfile = nil

            -- Provide safe alternatives
            _SANDBOXED = true
        "#,
        )
        .exec()?;

        // Set up memory limits if configured
        if config.max_memory_bytes > 0 {
            // Note: mlua doesn't expose memory limits directly in safe API
            // This would require unsafe code or custom allocator
            tracing::warn!("Memory limits are configured but not enforced in this version");
        }

        Ok(())
    }

    /// Execute a Lua script and return the result as JSON
    pub async fn execute_script(
        &self,
        script: &str,
        args: Option<JsonValue>,
    ) -> Result<JsonValue, LuaError> {
        let lua = Arc::clone(&self.lua);
        let script = script.to_string();
        let timeout_ms = self.config.max_execution_time_ms;

        // Execute with timeout
        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            tokio::task::spawn_blocking(move || Self::execute_script_sync(&lua, &script, args)),
        )
        .await
        .map_err(|_| LuaError::Timeout(timeout_ms))?
        .map_err(|e| LuaError::Execution(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    pub async fn execute_script_from_file(
        &self,
        file_path: &str,
        args: Option<JsonValue>,
    ) -> Result<JsonValue, LuaError> {
        let lua = Arc::clone(&self.lua);
        let file_path = file_path.to_string();
        let timeout_ms = self.config.max_execution_time_ms;

        // Read the script from file
        let script = std::fs::read_to_string(&file_path).map_err(|e| {
            LuaError::Execution(format!("Failed to read Lua script from file: {}", e))
        })?;

        // Execute with timeout
        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            tokio::task::spawn_blocking(move || Self::execute_script_sync(&lua, &script, args)),
        )
        .await
        .map_err(|_| LuaError::Timeout(timeout_ms))?
        .map_err(|e| LuaError::Execution(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    /// Synchronous script execution (runs in blocking context)
    fn execute_script_sync(
        lua: &Lua,
        script: &str,
        args: Option<JsonValue>,
    ) -> Result<JsonValue, LuaError> {
        // Set up arguments in global scope if provided
        if let Some(args_value) = args {
            let lua_value = json_to_lua(lua, &args_value)?;
            lua.globals().set("args", lua_value)?;
        }

        // Execute the script
        let result: LuaValue = lua.load(script).eval()?;

        // Convert result back to JSON
        lua_to_json(&result)
    }

    /// Load a script from a string and return a callable function
    pub fn load_function(&self, script: &str, name: &str) -> Result<(), LuaError> {
        self.lua.load(script).set_name(name).exec()?;
        Ok(())
    }

    /// Call a previously loaded Lua function
    pub async fn call_function(
        &self,
        name: &str,
        args: Option<JsonValue>,
    ) -> Result<JsonValue, LuaError> {
        let lua = Arc::clone(&self.lua);
        let name = name.to_string();
        let timeout_ms = self.config.max_execution_time_ms;

        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            tokio::task::spawn_blocking(move || Self::call_function_sync(&lua, &name, args)),
        )
        .await
        .map_err(|_| LuaError::Timeout(timeout_ms))?
        .map_err(|e| LuaError::Execution(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    /// Synchronous function call (runs in blocking context)
    fn call_function_sync(
        lua: &Lua,
        name: &str,
        args: Option<JsonValue>,
    ) -> Result<JsonValue, LuaError> {
        let func: LuaFunction = lua.globals().get(name)?;

        let result = if let Some(args_value) = args {
            let lua_args = json_to_lua(lua, &args_value)?;
            func.call::<LuaValue>(lua_args)?
        } else {
            func.call::<LuaValue>(())?
        };

        lua_to_json(&result)
    }

    /// Get the current configuration
    pub fn config(&self) -> &LuaRuntimeConfig {
        &self.config
    }
}

/// Convert JSON value to Lua value
pub fn json_to_lua(lua: &Lua, value: &JsonValue) -> Result<LuaValue, LuaError> {
    match value {
        JsonValue::Null => Ok(LuaValue::Nil),
        JsonValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Err(LuaError::Conversion(format!("Invalid number: {}", n)))
            }
        }
        JsonValue::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        JsonValue::Array(arr) => {
            let table = lua.create_table()?;
            for (i, item) in arr.iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, item)?)?;
            }
            Ok(LuaValue::Table(table))
        }
        JsonValue::Object(obj) => {
            let table = lua.create_table()?;
            for (key, val) in obj {
                table.set(key.as_str(), json_to_lua(lua, val)?)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

/// Convert Lua value to JSON value
pub fn lua_to_json(value: &LuaValue) -> Result<JsonValue, LuaError> {
    match value {
        LuaValue::Nil => Ok(JsonValue::Null),
        LuaValue::Boolean(b) => Ok(JsonValue::Bool(*b)),
        LuaValue::Integer(i) => Ok(JsonValue::Number((*i).into())),
        LuaValue::Number(n) => serde_json::Number::from_f64(*n)
            .map(JsonValue::Number)
            .ok_or_else(|| LuaError::Conversion(format!("Invalid number: {}", n))),
        LuaValue::String(s) => Ok(JsonValue::String(
            s.to_str()
                .map_err(|e| LuaError::Conversion(format!("Invalid UTF-8: {}", e)))?
                .to_string(),
        )),
        LuaValue::Table(table) => {
            // Try to detect if it's an array or object
            let mut is_array = true;
            let mut max_index = 0;
            let mut count = 0;

            for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                let (key, _) = pair.map_err(|e| LuaError::Conversion(e.to_string()))?;
                count += 1;

                if let LuaValue::Integer(i) = key {
                    if i > 0 {
                        max_index = max_index.max(i as usize);
                    } else {
                        is_array = false;
                        break;
                    }
                } else {
                    is_array = false;
                    break;
                }
            }

            // Check if indices are consecutive starting from 1
            if is_array && count == max_index {
                let mut arr = Vec::new();
                for i in 1..=max_index {
                    let val: LuaValue = table
                        .get(i)
                        .map_err(|e| LuaError::Conversion(e.to_string()))?;
                    arr.push(lua_to_json(&val)?);
                }
                Ok(JsonValue::Array(arr))
            } else {
                let mut obj = serde_json::Map::new();
                for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                    let (key, val) = pair.map_err(|e| LuaError::Conversion(e.to_string()))?;
                    let key_str = match key {
                        LuaValue::String(s) => s
                            .to_str()
                            .map_err(|e| LuaError::Conversion(format!("Invalid UTF-8 key: {}", e)))?
                            .to_string(),
                        LuaValue::Integer(i) => i.to_string(),
                        LuaValue::Number(n) => n.to_string(),
                        _ => {
                            return Err(LuaError::Conversion(
                                "Only string/number keys are supported".to_string(),
                            ));
                        }
                    };
                    obj.insert(key_str, lua_to_json(&val)?);
                }
                Ok(JsonValue::Object(obj))
            }
        }
        LuaValue::Function(_) => Err(LuaError::Conversion(
            "Functions cannot be converted to JSON".to_string(),
        )),
        LuaValue::Thread(_) => Err(LuaError::Conversion(
            "Threads cannot be converted to JSON".to_string(),
        )),
        LuaValue::UserData(_) => Err(LuaError::Conversion(
            "UserData cannot be converted to JSON".to_string(),
        )),
        LuaValue::LightUserData(_) => Err(LuaError::Conversion(
            "LightUserData cannot be converted to JSON".to_string(),
        )),
        LuaValue::Error(e) => Err(LuaError::Conversion(format!("Lua error: {}", e))),
        LuaValue::Other(_) => Err(LuaError::Conversion(
            "Other value type cannot be converted to JSON".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_basic_execution() {
        let runtime = LuaRuntime::new_default().unwrap();
        let result = runtime.execute_script("return 1 + 1", None).await.unwrap();
        assert_eq!(result, json!(2));
    }

    #[tokio::test]
    async fn test_with_args() {
        let runtime = LuaRuntime::new_default().unwrap();
        let result = runtime
            .execute_script("return args.x + args.y", Some(json!({"x": 10, "y": 20})))
            .await
            .unwrap();
        assert_eq!(result, json!(30));
    }

    #[tokio::test]
    async fn test_array_return() {
        let runtime = LuaRuntime::new_default().unwrap();
        let result = runtime
            .execute_script("return {1, 2, 3}", None)
            .await
            .unwrap();
        assert_eq!(result, json!([1, 2, 3]));
    }

    #[tokio::test]
    async fn test_object_return() {
        let runtime = LuaRuntime::new_default().unwrap();
        let result = runtime
            .execute_script("return {foo = 'bar', num = 42}", None)
            .await
            .unwrap();
        assert_eq!(result, json!({"foo": "bar", "num": 42}));
    }

    #[tokio::test]
    async fn test_sandbox_no_io() {
        let runtime = LuaRuntime::new_default().unwrap();
        let result = runtime.execute_script("return io", None).await.unwrap();
        assert_eq!(result, JsonValue::Null);
    }

    #[tokio::test]
    async fn test_sandbox_no_os() {
        let runtime = LuaRuntime::new_default().unwrap();
        let result = runtime.execute_script("return os", None).await.unwrap();
        assert_eq!(result, JsonValue::Null);
    }

    #[tokio::test]
    async fn test_function_loading() {
        let runtime = LuaRuntime::new_default().unwrap();
        runtime
            .load_function("function double(x) return x * 2 end", "test")
            .unwrap();
        let result = runtime
            .call_function("double", Some(json!(21)))
            .await
            .unwrap();
        assert_eq!(result, json!(42));
    }

    #[tokio::test]
    async fn test_string_conversion() {
        let runtime = LuaRuntime::new_default().unwrap();
        let result = runtime
            .execute_script("return 'hello world'", None)
            .await
            .unwrap();
        assert_eq!(result, json!("hello world"));
    }
}
