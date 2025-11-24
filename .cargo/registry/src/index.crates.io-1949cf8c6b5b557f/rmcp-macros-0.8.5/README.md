# rmcp-macros

`rmcp-macros` is a procedural macro library for the Rust Model Context Protocol (RMCP) SDK, providing macros that facilitate the development of RMCP applications.

## Features

This library primarily provides the following macros:

- `#[tool]`: Used to mark functions as RMCP tools, automatically generating necessary metadata and invocation mechanisms

## Usage

### tool

This macro is used to mark a function as a tool handler.

This will generate a function that return the attribute of this tool, with type `rmcp::model::Tool`.

#### Usage

| field             | type                       | usage |
| :-                | :-                         | :-    |
| `name`            | `String`                   | The name of the tool. If not provided, it defaults to the function name. |
| `description`     | `String`                   | A description of the tool. The document of this function will be used. |
| `input_schema`    | `Expr`                     | A JSON Schema object defining the expected parameters for the tool. If not provide, if will use the json schema of its argument with type `Parameters<T>` |
| `annotations`     | `ToolAnnotationsAttribute` | Additional tool information. Defaults to `None`. |

#### Example

```rust
#[tool(name = "my_tool", description = "This is my tool", annotations(title = "我的工具", read_only_hint = true))]
pub async fn my_tool(param: Parameters<MyToolParam>) {
    // handling tool request
}
```

### tool_router

This macro is used to generate a tool router based on functions marked with `#[rmcp::tool]` in an implementation block.

It creates a function that returns a `ToolRouter` instance.

In most case, you need to add a field for handler to store the router information and initialize it when creating handler, or store it with a static variable.

#### Usage

| field     | type          | usage |
| :-        | :-            | :-    |
| `router`  | `Ident`       | The name of the router function to be generated. Defaults to `tool_router`. |
| `vis`     | `Visibility`  | The visibility of the generated router function. Defaults to empty. |

#### Example

```rust
#[tool_router]
impl MyToolHandler {
    #[tool]
    pub fn my_tool() {
        
    }

    pub fn new() -> Self {
        Self {
            // the default name of tool router will be `tool_router`
            tool_router: Self::tool_router(),
        }
    }
}
```

Or specify the visibility and router name, which would be helpful when you want to combine multiple routers into one:

```rust
mod a {
    #[tool_router(router = tool_router_a, vis = "pub")]
    impl MyToolHandler {
        #[tool]
        fn my_tool_a() {
            
        }
    }
}

mod b {
    #[tool_router(router = tool_router_b, vis = "pub")]
    impl MyToolHandler {
        #[tool]
        fn my_tool_b() {
            
        }
    }
}

impl MyToolHandler {
    fn new() -> Self {
        Self {
            tool_router: self::tool_router_a() + self::tool_router_b(),
        }
    }
}
```

### tool_handler

This macro will generate the handler for `tool_call` and `list_tools` methods in the implementation block, by using an existing `ToolRouter` instance.

#### Usage

| field     | type          | usage |
| :-        | :-            | :-    |
| `router`  | `Expr`        | The expression to access the `ToolRouter` instance. Defaults to `self.tool_router`. |

#### Example
```rust
#[tool_handler]
impl ServerHandler for MyToolHandler {
    // ...implement other handler
}
```

or using a custom router expression:
```rust
#[tool_handler(router = self.get_router().await)]
impl ServerHandler for MyToolHandler {
   // ...implement other handler
}
```

#### Explained
This macro will be expended to something like this:
```rust
impl ServerHandler for MyToolHandler {
       async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let tcc = ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let items = self.tool_router.list_all();
        Ok(ListToolsResult::with_all_items(items))
    }
}
```


## Advanced Features

- Support for custom tool names and descriptions
- Automatic generation of tool descriptions from documentation comments
- JSON Schema generation for tool parameters

## License

Please refer to the LICENSE file in the project root directory. 
