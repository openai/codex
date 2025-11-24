#[cfg(test)]
mod tests {
    use rmcp::{ServerHandler, handler::server::router::tool::ToolRouter, tool, tool_handler};

    #[derive(Debug, Clone, Default)]
    pub struct AnnotatedServer {
        tool_router: ToolRouter<AnnotatedServer>,
    }

    impl AnnotatedServer {
        // Tool with inline comments for documentation
        /// Direct annotation test tool
        /// This is used to test tool annotations
        #[tool(
            name = "direct-annotated-tool",
            annotations(title = "Annotated Tool", read_only_hint = true)
        )]
        pub async fn direct_annotated_tool(&self, input: String) -> String {
            format!("Direct: {}", input)
        }
    }
    #[tool_handler]
    impl ServerHandler for AnnotatedServer {}

    #[test]
    fn test_direct_tool_attributes() {
        // Get the tool definition
        let tool = AnnotatedServer::direct_annotated_tool_tool_attr();

        // Verify basic properties
        assert_eq!(tool.name, "direct-annotated-tool");

        // Verify description is extracted from doc comments
        assert!(tool.description.is_some());
        assert!(
            tool.description
                .as_ref()
                .unwrap()
                .contains("Direct annotation test tool")
        );

        let annotations = tool.annotations.unwrap();
        assert_eq!(annotations.title.as_ref().unwrap(), "Annotated Tool");
        assert_eq!(annotations.read_only_hint, Some(true));
    }
}
