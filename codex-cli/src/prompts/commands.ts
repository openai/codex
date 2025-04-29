/**
 * Special command prompt responses for Codex CLI
 */

/**
 * Response text for the new task tool command
 */
export function newTaskToolResponse(): string {
  return `<explicit_instructions type="new_task">
The user has explicitly asked you to help them create a new task with preloaded context, which you will generate. The user may have provided instructions or additional information for you to consider when summarizing existing work and creating the context for the new task.
Irrespective of whether additional information or instructions are given, you are ONLY allowed to respond to this message by calling the new_task tool.

The new_task tool is defined below:

Description:
Your task is to create a detailed summary of the conversation so far, paying close attention to the user's explicit requests and your previous actions. This summary should be thorough in capturing technical details, code patterns, and architectural decisions that would be essential for continuing with the new task.
The user will be presented with a preview of your generated context and can choose to create a new task or keep chatting in the current conversation.

Parameters:
- Context: (required) The context to preload the new task with. If applicable based on the current task, this should include:
  1. Current Work: Describe in detail what was being worked on prior to this request to create a new task. Pay special attention to the more recent messages / conversation.
  2. Key Technical Concepts: List all important technical concepts, technologies, coding conventions, and frameworks discussed, which might be relevant for the new task.
  3. Relevant Files and Code: If applicable, enumerate specific files and code sections examined, modified, or created for the task continuation. Pay special attention to the most recent messages and changes.
  4. Problem Solving: Document problems solved thus far and any ongoing troubleshooting efforts.
  5. Pending Tasks and Next Steps: Outline all pending tasks that you have explicitly been asked to work on, as well as list the next steps you will take for all outstanding work, if applicable. Include code snippets where they add clarity. For any next steps, include direct quotes from the most recent conversation showing exactly what task you were working on and where you left off. This should be verbatim to ensure there's no information loss in context between tasks.

Usage:
<new_task>
<context>context to preload new task with</context>
</new_task>

Below is the the user's input when they indicated that they wanted to create a new task.
</explicit_instructions>\n`;
}

/**
 * Response text for the condense tool command
 */
export function condenseToolResponse(): string {
  return `<explicit_instructions type="condense">
The user has explicitly asked you to create a detailed summary of the conversation so far, which will be used to compact the current context window while retaining key information. The user may have provided instructions or additional information for you to consider when summarizing the conversation.
Irrespective of whether additional information or instructions are given, you are only allowed to respond to this message by calling the condense tool.

The condense tool is defined below:

Description:
Your task is to create a detailed summary of the conversation so far, paying close attention to the user's explicit requests and your previous actions. This summary should be thorough in capturing technical details, code patterns, and architectural decisions that would be essential for continuing with the conversation and supporting any continuing tasks.
The user will be presented with a preview of your generated summary and can choose to use it to compact their context window or keep chatting in the current conversation.
Users may refer to this tool as 'smol' or 'compact' as well. You should consider these to be equivalent to 'condense' when used in a similar context.

Parameters:
- Context: (required) The context to continue the conversation with. If applicable based on the current task, this should include:
  1. Previous Conversation: High level details about what was discussed throughout the entire conversation with the user. This should be written to allow someone to be able to follow the general overarching conversation flow.
  2. Current Work: Describe in detail what was being worked on prior to this request to compact the context window. Pay special attention to the more recent messages / conversation.
  3. Key Technical Concepts: List all important technical concepts, technologies, coding conventions, and frameworks discussed, which might be relevant for continuing with this work.
  4. Relevant Files and Code: If applicable, enumerate specific files and code sections examined, modified, or created for the task continuation. Pay special attention to the most recent messages and changes.
  5. Problem Solving: Document problems solved thus far and any ongoing troubleshooting efforts.
  6. Pending Tasks and Next Steps: Outline all pending tasks that you have explicitly been asked to work on, as well as list the next steps you will take for all outstanding work, if applicable. Include code snippets where they add clarity. For any next steps, include direct quotes from the most recent conversation showing exactly what task you were working on and where you left off. This should be verbatim to ensure there's no information loss in context between tasks.

Usage:
<condense>
<context>Your detailed summary</context>
</condense>

Example:
<condense>
<context>
1. Previous Conversation:
  [Detailed description]

2. Current Work:
  [Detailed description]

3. Key Technical Concepts:
  - [Concept 1]
  - [Concept 2]
  - [...]

4. Relevant Files and Code:
  - [File Name 1]
    - [Summary of why this file is important]
    - [Summary of the changes made to this file, if any]
    - [Important Code Snippet]
  - [File Name 2]
    - [Important Code Snippet]
  - [...]

5. Problem Solving:
  [Detailed description]

6. Pending Tasks and Next Steps:
  - [Task 1 details & next steps]
  - [Task 2 details & next steps]
  - [...]
</context>
</condense>

</explicit_instructions>\n`;
}

/**
 * Response text for the plan mode response tool
 */
export function planModeResponse(): string {
  return `<explicit_instructions type="plan_mode">
You are currently in PLAN MODE. In this mode, you should help the user plan a solution to their task. 
Your goal is to create a detailed plan for accomplishing the task, which the user will review and approve 
before switching to ACT MODE to implement the solution.

You should use the plan_mode_respond tool to respond to the user's messages in PLAN MODE.
This tool is ONLY available in PLAN MODE.

The plan_mode_respond tool is defined below:

Description:
Respond to the user's inquiry in an effort to plan a solution to the user's task.
Depending on the user's message, you may ask questions to get clarification about the user's request,
architect a solution to the task, and brainstorm ideas with the user.

Parameters:
- response: (required) The response to provide to the user. This is simply a chat response.

Usage:
<plan_mode_respond>
<response>Your response here</response>
</plan_mode_respond>

Remember to use this tool for all responses while in PLAN MODE.
</explicit_instructions>\n`;
}

/**
 * Response text for the ask_followup_question tool command
 */
export function askFollowupQuestionResponse(): string {
  return `<explicit_instructions type="ask_followup_question">
The user has explicitly asked you to use the ask_followup_question tool to gather more information.

You should use this tool when you need to get clarification or additional details from the user to accomplish the task effectively.

The ask_followup_question tool is defined below:

Description:
Ask the user a question to gather additional information needed to complete the task. This tool should be used when you encounter ambiguities, need clarification, or require more details to proceed effectively.

Parameters:
- question: (required) The question to ask the user. This should be a clear, specific question that addresses the information you need.
- options: (optional) An array of 2-5 options for the user to choose from. Each option should be a string describing a possible answer. Providing options can make it easier for the user to respond quickly.

Usage:
<ask_followup_question>
<question>Your question here</question>
<options>["Option 1", "Option 2", "Option 3"]</options>
</ask_followup_question>

Remember to:
1. Ask one clear, specific question at a time
2. Provide options when appropriate to make responding easier for the user
3. Use the response to inform your next steps
</explicit_instructions>\n`;
}

/**
 * Response text for the attempt_completion tool command
 */
export function attemptCompletionResponse(): string {
  return `<explicit_instructions type="attempt_completion">
The user has explicitly asked you to use the attempt_completion tool to present the final result of your work.

You should use this tool when you have completed the user's task and want to present the final result.

The attempt_completion tool is defined below:

Description:
Present the result of your work to the user. This tool should be used when you have completed the user's task and are ready to show them the final result.

Parameters:
- result: (required) The result of the task. This should be a comprehensive description of what you've accomplished, including any relevant details about the changes you've made, the code you've written, or the problems you've solved.
- command: (optional) A CLI command that can be used to demonstrate the result, such as running a program, opening a file, or starting a server.

Usage:
<attempt_completion>
<result>Your detailed result description here</result>
<command>Command to demonstrate result (optional)</command>
</attempt_completion>

Remember to:
1. Provide a comprehensive description of the work you've completed
2. Include relevant details about changes made, code written, or problems solved
3. When appropriate, include a command that can be used to demonstrate the result
</explicit_instructions>\n`;
}

/**
 * Response text for the browser_action tool command
 */
export function browserActionResponse(): string {
  return `<explicit_instructions type="browser_action">
The user has explicitly asked you to use the browser_action tool to interact with a web browser.

You should use this tool when you need to test web applications, navigate websites, or interact with web content to complete a task.

The browser_action tool is defined below:

Description:
Request to interact with a Puppeteer-controlled browser. Every action, except 'close', will be responded to with a screenshot of the browser's current state, along with any new console logs. You may only perform one browser action per message, and must wait for the user's response including a screenshot and logs before determining the next action.

Parameters:
- action: (required) The action to perform. Available options:
  * launch: Launch a new browser instance at the specified URL (must be the first action)
  * click: Click at a specific x,y coordinate
  * type: Type a string of text on the keyboard
  * scroll_down: Scroll down the page by one page height
  * scroll_up: Scroll up the page by one page height
  * close: Close the browser instance (must be the final action)
- url: (optional) The URL to navigate to (used with the 'launch' action)
- coordinate: (optional) The X and Y coordinates for the 'click' action
- text: (optional) The text to type for the 'type' action

Usage:
<browser_action>
<action>launch|click|type|scroll_down|scroll_up|close</action>
<url>URL to launch the browser at (only for launch)</url>
<coordinate>x,y coordinates (only for click)</coordinate>
<text>Text to type (only for type)</text>
</browser_action>

Remember:
1. Always start with 'launch' and end with 'close'
2. Only use one browser action per message
3. Wait for the screenshot response before your next action
4. Base click coordinates on the current screenshot
5. The browser window has a resolution of 900x600 pixels
</explicit_instructions>\n`;
}

/**
 * Response text for the list_code_definition_names tool command
 */
export function listCodeDefinitionNamesResponse(): string {
  return `<explicit_instructions type="list_code_definition_names">
The user has explicitly asked you to use the list_code_definition_names tool to get an overview of code definitions.

You should use this tool when you need to understand the structure and organization of source code files in a directory without having to read each file individually.

The list_code_definition_names tool is defined below:

Description:
Request to list definition names (classes, functions, methods, etc.) used in source code files at the top level of the specified directory. This tool provides insights into the codebase structure and important constructs, encapsulating high-level concepts and relationships that are crucial for understanding the overall architecture.

Parameters:
- path: (required) The path of the directory to list top-level source code definitions for.

Usage:
<list_code_definition_names>
<path>Directory path here</path>
</list_code_definition_names>

Remember to:
1. Use this tool to get a high-level overview of code structure in a directory
2. Focus on understanding the key components and their relationships
3. Use the results to inform your approach to the task at hand
</explicit_instructions>\n`;
}

/**
 * Response text for the use_mcp_tool command
 */
export function mcpToolResponse(): string {
  return `<explicit_instructions type="use_mcp_tool">
The user has explicitly asked you to use the use_mcp_tool to interact with a connected MCP server tool.

You should use this tool when you need to access functionality provided by a Model Context Protocol (MCP) server that extends your capabilities.

The use_mcp_tool is defined below:

Description:
Request to use a tool provided by a connected MCP server. Each MCP server can provide multiple tools with different capabilities. Tools have defined input schemas that specify required and optional parameters.

Parameters:
- server_name: (required) The name of the MCP server providing the tool
- tool_name: (required) The name of the tool to execute
- arguments: (required) A JSON object containing the tool's input parameters, following the tool's input schema

Usage:
<use_mcp_tool>
<server_name>server name here</server_name>
<tool_name>tool name here</tool_name>
<arguments>
{
  "param1": "value1",
  "param2": "value2"
}
</arguments>
</use_mcp_tool>

Remember to:
1. Use the correct server name as it appears in the MCP servers list
2. Specify the exact tool name provided by the server
3. Format the arguments as a valid JSON object following the tool's schema
4. Wait for the response before proceeding to the next action
</explicit_instructions>\n`;
}

/**
 * Response text for the access_mcp_resource command
 */
export function mcpResourceResponse(): string {
  return `<explicit_instructions type="access_mcp_resource">
The user has explicitly asked you to use the access_mcp_resource tool to retrieve data from a connected MCP server.

You should use this tool when you need to access data resources provided by a Model Context Protocol (MCP) server.

The access_mcp_resource tool is defined below:

Description:
Request to access a resource provided by a connected MCP server. Resources represent data sources that can be used as context, such as files, API responses, or system information.

Parameters:
- server_name: (required) The name of the MCP server providing the resource
- uri: (required) The URI identifying the specific resource to access

Usage:
<access_mcp_resource>
<server_name>server name here</server_name>
<uri>resource URI here</uri>
</access_mcp_resource>

Remember to:
1. Use the correct server name as it appears in the MCP servers list
2. Specify the exact resource URI format required by the server
3. Wait for the response before proceeding to the next action
</explicit_instructions>\n`;
}

/**
 * Response text for MCP documentation loading
 */
export function mcpDocumentationResponse(): string {
  return `<explicit_instructions type="mcp_documentation">
The user has requested information about creating or installing an MCP server. I'll provide you with detailed documentation about the MCP server creation process.

MCP (Model Context Protocol) enables communication between the system and locally running MCP servers that provide additional tools and resources to extend your capabilities.

When creating MCP servers, it's important to understand that they operate in a non-interactive environment. The server cannot initiate OAuth flows, open browser windows, or prompt for user input during runtime. All credentials and authentication tokens must be provided upfront through environment variables in the MCP settings configuration.

Here are the steps to create an MCP server:

1. Create a new directory for the MCP server and initialize a Node.js project:
   - mkdir my-mcp-server
   - cd my-mcp-server
   - npm init -y
   - npm install @modelcontextprotocol/sdk

2. Create a basic server implementation in index.ts:
   - Import the necessary modules from the SDK
   - Define your server class with tool and resource handlers
   - Set up error handling and cleanup
   - Connect to the standard I/O transport

3. Configure the MCP server in your settings file:
   - Specify the command to run the server
   - Provide any necessary environment variables
   - Set the disabled and autoApprove properties

The model may request more detailed instructions for any of these steps as needed.
</explicit_instructions>\n`;
}
