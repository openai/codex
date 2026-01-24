# cclsp - Claude Code LSP Bridge

> Model Context Protocol (MCP) server that bridges Language Server Protocol (LSP) functionality with Claude Code

**Repository**: https://github.com/ktnyt/cclsp
**Version**: 0.6.2
**License**: MIT

---

## Table of Contents

1. [Overview](#1-overview)
2. [Architecture](#2-architecture)
3. [Core Components](#3-core-components)
4. [Protocol Implementation](#4-protocol-implementation)
5. [MCP Tools](#5-mcp-tools)
6. [LSP Adapter System](#6-lsp-adapter-system)
7. [Configuration](#7-configuration)
8. [Data Flow](#8-data-flow)
9. [Key Features](#9-key-features)
10. [File Reference](#10-file-reference)

---

## 1. Overview

### What is cclsp?

**cclsp** is a Model Context Protocol (MCP) server that enables Claude Code to access Language Server Protocol (LSP) functionality. It acts as a bridge between AI-powered coding assistants and language servers, providing capabilities like:

- Go to definition
- Find references
- Symbol renaming
- Diagnostics (errors, warnings, hints)
- Server management

### Problem Solved

LLM-based coding agents often struggle with providing accurate line/column numbers, making naive LSP integration fragile. cclsp solves this by:

1. **Symbol name matching** - Searches by symbol name instead of requiring exact positions
2. **Multi-position fallback** - Tries multiple position combinations when exact matches fail
3. **Kind-based filtering** - Narrows results by symbol kind (function, class, variable, etc.)

### Technology Stack

| Component | Technology |
|-----------|------------|
| Language | TypeScript |
| Runtime | Node.js 18+ / Bun |
| MCP SDK | @modelcontextprotocol/sdk ^1.12.3 |
| Package Manager | npm / bun |
| Testing | Bun test framework |
| Linting | Biome |

---

## 2. Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Claude Code                                 │
│                          (MCP Client)                                    │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ MCP Protocol (stdio)
                                    │ JSON-RPC 2.0
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                               cclsp                                      │
│                          (MCP Server)                                    │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                        index.ts                                  │   │
│  │                   MCP Server Entry Point                         │   │
│  │                                                                  │   │
│  │  ┌──────────────────────────────────────────────────────────┐  │   │
│  │  │                   Tool Handlers                           │  │   │
│  │  │  • find_definition    • rename_symbol                     │  │   │
│  │  │  • find_references    • rename_symbol_strict              │  │   │
│  │  │  • get_diagnostics    • restart_server                    │  │   │
│  │  └──────────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                    │                                     │
│                                    ▼                                     │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                      lsp-client.ts                               │   │
│  │                    LSP Client Core                               │   │
│  │                                                                  │   │
│  │  • Server process management                                     │   │
│  │  • JSON-RPC message handling                                     │   │
│  │  • Symbol resolution                                             │   │
│  │  • Document synchronization                                      │   │
│  │  • Adapter detection & application                               │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                    │                                     │
└────────────────────────────────────┼─────────────────────────────────────┘
                                     │
              ┌──────────────────────┼──────────────────────┐
              │                      │                      │
              ▼                      ▼                      ▼
┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐
│ TypeScript LSP      │ │ Python LSP          │ │ Go LSP              │
│ (typescript-        │ │ (pylsp)             │ │ (gopls)             │
│  language-server)   │ │                     │ │                     │
└─────────────────────┘ └─────────────────────┘ └─────────────────────┘
        │                        │                        │
        └────────────────────────┼────────────────────────┘
                                 │
                                 ▼
                    LSP Protocol (stdio, JSON-RPC 2.0)
```

### Component Interaction

```
Claude Code
    │
    │ MCP Tool Call: find_definition(file, symbol_name, kind)
    ▼
index.ts: Tool Handler
    │
    │ 1. Resolve file path
    │ 2. Extract parameters
    ▼
LSPClient.findSymbolsByName()
    │
    │ 3. Get/start LSP server for file extension
    │ 4. Open file with LSP server
    │ 5. Get document symbols
    │ 6. Match by name and kind
    ▼
LSPClient.findDefinition()
    │
    │ 7. Send textDocument/definition request
    │ 8. Receive location(s)
    ▼
Transform to MCP Response
    │
    │ 9. Convert URIs to file paths
    │ 10. Format as text content
    ▼
Return to Claude Code
```

---

## 3. Core Components

### 3.1 index.ts - MCP Server Entry Point

**Location**: `/lsp/cclsp/index.ts` (~738 lines)

**Responsibilities**:
- Creates and configures MCP server
- Defines and registers 6 MCP tools
- Handles tool call routing
- Manages server lifecycle

**Key Structures**:

```typescript
// MCP Server Creation
const server = new Server(
  { name: 'cclsp', version: '0.1.0' },
  { capabilities: { tools: {} } }
);

// Tool Registration
server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    { name: 'find_definition', ... },
    { name: 'find_references', ... },
    { name: 'rename_symbol', ... },
    { name: 'rename_symbol_strict', ... },
    { name: 'get_diagnostics', ... },
    { name: 'restart_server', ... },
  ]
}));

// Tool Call Handler
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;
  // Route to appropriate handler based on tool name
});
```

### 3.2 lsp-client.ts - LSP Client Core

**Location**: `/lsp/cclsp/src/lsp-client.ts` (~1690 lines)

**Responsibilities**:
- Manages multiple LSP server processes concurrently
- Handles JSON-RPC 2.0 communication over stdio
- Maps file extensions to language servers
- Maintains process lifecycle and auto-restart
- Applies server-specific adapters
- Provides symbol resolution with intelligent matching

**Key Class: LSPClient**

```typescript
class LSPClient {
  private servers: Map<string, ServerState>;      // Active server processes
  private serversStarting: Map<string, Promise>;  // Startup deduplication
  private config: Config;                          // Server configurations
  private nextId: number;                          // Request ID counter

  constructor(configPath?: string);                // Load config
  async startServer(config): Promise<ServerState>; // Spawn LSP server
  async sendRequest(proc, method, params);         // JSON-RPC request
  async findSymbolsByName(file, name, kind);       // Symbol search
  async findDefinition(file, position);            // Go to definition
  async findReferences(file, position);            // Find references
  async renameSymbol(file, position, newName);     // Rename
  async getDiagnostics(file);                      // Get diagnostics
  async restartServers(extensions?);               // Restart servers
}
```

**ServerState Interface**:

```typescript
interface ServerState {
  process: ChildProcess;               // LSP server process
  initialized: boolean;                // Initialization complete
  initializationPromise: Promise<void>;// Awaitable initialization
  openFiles: Set<string>;              // Tracked open files
  fileVersions: Map<string, number>;   // Document versions
  startTime: number;                   // For restart interval
  config: LSPServerConfig;             // Server configuration
  diagnostics: Map<string, Diagnostic[]>;    // Cached diagnostics
  lastDiagnosticUpdate: Map<string, number>; // Update timestamps
  diagnosticVersions: Map<string, number>;   // Diagnostic versions
  adapter?: ServerAdapter;             // Optional server adapter
}
```

### 3.3 types.ts - Type Definitions

**Location**: `/lsp/cclsp/src/types.ts` (~171 lines)

**Key Types**:

```typescript
// Configuration
interface Config {
  servers: LSPServerConfig[];
}

interface LSPServerConfig {
  extensions: string[];              // File extensions
  command: string[];                 // Command to spawn server
  rootDir?: string;                  // Working directory
  restartInterval?: number;          // Auto-restart interval (minutes)
  initializationOptions?: unknown;   // LSP init options
}

// LSP Position Types
interface Position {
  line: number;      // 0-indexed
  character: number; // 0-indexed
}

interface Range {
  start: Position;
  end: Position;
}

interface Location {
  uri: string;       // file:// URI
  range: Range;
}

// Symbol Types
enum SymbolKind {
  File = 1, Module = 2, Namespace = 3, Package = 4,
  Class = 5, Method = 6, Property = 7, Field = 8,
  Constructor = 9, Enum = 10, Interface = 11, Function = 12,
  Variable = 13, Constant = 14, String = 15, Number = 16,
  Boolean = 17, Array = 18, Object = 19, Key = 20,
  Null = 21, EnumMember = 22, Struct = 23, Event = 24,
  Operator = 25, TypeParameter = 26
}

interface DocumentSymbol {
  name: string;
  detail?: string;
  kind: SymbolKind;
  range: Range;
  selectionRange: Range;
  children?: DocumentSymbol[];
}

// Diagnostic Types
enum DiagnosticSeverity {
  Error = 1, Warning = 2, Information = 3, Hint = 4
}

interface Diagnostic {
  range: Range;
  severity?: DiagnosticSeverity;
  code?: string | number;
  source?: string;
  message: string;
}
```

### 3.4 file-editor.ts - Workspace Edit Application

**Location**: `/lsp/cclsp/src/file-editor.ts` (~200 lines)

**Responsibilities**:
- Applies LSP WorkspaceEdit to files
- Creates backup files (.bak extension)
- Handles symlinks correctly
- Validates changes before applying
- Supports dry-run mode

**Key Function**:

```typescript
interface ApplyEditOptions {
  lspClient?: LSPClient;  // For syncing after edit
}

interface ApplyEditResult {
  success: boolean;
  filesModified: string[];
  backupFiles: string[];
  errors?: string[];
}

async function applyWorkspaceEdit(
  workspaceEdit: WorkspaceEdit,
  options?: ApplyEditOptions
): Promise<ApplyEditResult>;
```

**Edit Application Flow**:

```
WorkspaceEdit received
    │
    ├─► Handle `changes` format (older LSP)
    │   { uri: TextEdit[] }
    │
    └─► Handle `documentChanges` format (modern LSP)
        TextDocumentEdit[]
            │
            ▼
For each file:
    1. Read current content
    2. Create backup (.bak)
    3. Sort edits by position (reverse order)
    4. Apply edits from end to start
    5. Write to temp file
    6. Rename temp to target
    7. Sync with LSP server
```

### 3.5 file-scanner.ts - Project File Scanning

**Location**: `/lsp/cclsp/src/file-scanner.ts` (~170 lines)

**Responsibilities**:
- Scans directories for file extensions
- Respects .gitignore patterns
- Recommends language servers based on detected files
- Supports depth-limited recursion

**Key Functions**:

```typescript
// Load and parse .gitignore
function loadGitignore(dir: string): Ignore;

// Scan directory for file extensions
async function scanDirectoryForExtensions(
  dir: string,
  maxDepth?: number  // Default: 3
): Promise<Set<string>>;

// Match extensions to language servers
function getRecommendedLanguageServers(
  extensions: Set<string>,
  servers: LSPServerConfig[]
): LSPServerConfig[];

// Full project scan
async function scanProjectFiles(
  dir: string
): Promise<{
  extensions: Set<string>;
  recommendations: LSPServerConfig[];
}>;
```

### 3.6 language-servers.ts - Pre-configured LSP Definitions

**Location**: `/lsp/cclsp/src/language-servers.ts` (~180 lines)

**Provides**: `LANGUAGE_SERVERS` array with configurations for 15+ languages

```typescript
export const LANGUAGE_SERVERS: LSPServerConfig[] = [
  {
    extensions: ['ts', 'tsx', 'js', 'jsx'],
    command: ['npx', '--', 'typescript-language-server', '--stdio'],
    installCommand: 'npm install -g typescript-language-server typescript'
  },
  {
    extensions: ['py', 'pyi'],
    command: ['uvx', '--from', 'python-lsp-server', 'pylsp'],
    restartInterval: 5,  // Python LSP needs periodic restart
    initializationOptions: { /* jedi plugins config */ }
  },
  {
    extensions: ['go'],
    command: ['gopls'],
    installCommand: 'go install golang.org/x/tools/gopls@latest'
  },
  {
    extensions: ['rs'],
    command: ['rust-analyzer'],
    installCommand: 'rustup component add rust-analyzer rust-src'
  },
  // ... more languages
];
```

**Supported Languages**:

| Language | Server | Extensions |
|----------|--------|------------|
| TypeScript/JavaScript | typescript-language-server | ts, tsx, js, jsx |
| Python | pylsp (via uvx) | py, pyi |
| Go | gopls | go |
| Rust | rust-analyzer | rs |
| C/C++ | clangd | c, cpp, cc, h, hpp |
| Java | jdtls | java |
| Ruby | solargraph | rb |
| PHP | intelephense | php |
| C# | omnisharp | cs |
| Swift | sourcekit-lsp | swift |
| Vue | vue-language-server | vue |

### 3.7 setup.ts - Interactive Setup Wizard

**Location**: `/lsp/cclsp/src/setup.ts` (~400 lines)

**Responsibilities**:
- Interactive CLI using inquirer
- Auto-detects project languages
- Generates cclsp.json configuration
- Optionally installs language servers
- Optionally registers with Claude MCP

**Setup Flow**:

```
npx cclsp setup
    │
    ▼
Scan project for file types
    │
    ▼
Present language servers (checkboxes)
    │
    ▼
Show installation commands
    │
    ├─► Install automatically? (optional)
    │
    ▼
Generate cclsp.json
    │
    ├─► Add to Claude MCP? (optional)
    │   claude mcp add cclsp npx cclsp@latest --env CCLSP_CONFIG_PATH=...
    │
    ▼
Show verification instructions
```

### 3.8 utils.ts - Utility Functions

**Location**: `/lsp/cclsp/src/utils.ts`

```typescript
// Convert file path to file:// URI
function pathToUri(filePath: string): string {
  const absPath = path.resolve(filePath);
  // Handle Windows paths correctly
  if (process.platform === 'win32') {
    return `file:///${absPath.replace(/\\/g, '/')}`;
  }
  return `file://${absPath}`;
}

// Convert file:// URI to file path
function uriToPath(uri: string): string {
  const url = new URL(uri);
  // Handle Windows paths correctly
  if (process.platform === 'win32') {
    return url.pathname.slice(1); // Remove leading /
  }
  return url.pathname;
}
```

---

## 4. Protocol Implementation

### 4.1 JSON-RPC 2.0 Message Format

**Request Structure**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "textDocument/definition",
  "params": {
    "textDocument": { "uri": "file:///path/to/file.ts" },
    "position": { "line": 10, "character": 5 }
  }
}
```

**Response Structure**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": [
    {
      "uri": "file:///path/to/definition.ts",
      "range": {
        "start": { "line": 45, "character": 0 },
        "end": { "line": 45, "character": 20 }
      }
    }
  ]
}
```

**Notification Structure** (no response expected):
```json
{
  "jsonrpc": "2.0",
  "method": "initialized",
  "params": {}
}
```

### 4.2 LSP Message Framing

LSP uses HTTP-like headers for message framing over stdio:

```
Content-Length: 234\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"textDocument/definition",...}
```

**Implementation** (lsp-client.ts:220-250):

```typescript
// Message parsing from stdout buffer
let buffer = Buffer.alloc(0);

process.stdout.on('data', (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);

  while (true) {
    // Find header end
    const headerEnd = buffer.indexOf('\r\n\r\n');
    if (headerEnd === -1) break;

    // Parse Content-Length
    const header = buffer.subarray(0, headerEnd).toString();
    const match = header.match(/Content-Length: (\d+)/);
    if (!match) break;

    const contentLength = parseInt(match[1], 10);
    const messageStart = headerEnd + 4;
    const messageEnd = messageStart + contentLength;

    if (buffer.length < messageEnd) break;

    // Extract and parse message
    const content = buffer.subarray(messageStart, messageEnd).toString();
    const message = JSON.parse(content);
    handleMessage(message);

    // Remove processed message from buffer
    buffer = buffer.subarray(messageEnd);
  }
});
```

### 4.3 Request/Response Correlation

**Mechanism**: Request ID tracking in `pendingRequests` Map

```typescript
private pendingRequests = new Map<number, {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
}>();

async sendRequest(proc, method, params) {
  const id = this.nextId++;

  return new Promise((resolve, reject) => {
    // Store handlers for later resolution
    this.pendingRequests.set(id, { resolve, reject });

    // Set timeout
    setTimeout(() => {
      if (this.pendingRequests.has(id)) {
        this.pendingRequests.delete(id);
        reject(new Error(`Request ${method} timed out`));
      }
    }, timeout);

    // Send request
    this.sendMessage(proc, { jsonrpc: '2.0', id, method, params });
  });
}

handleMessage(message) {
  if (message.id !== undefined && this.pendingRequests.has(message.id)) {
    const { resolve, reject } = this.pendingRequests.get(message.id);
    this.pendingRequests.delete(message.id);

    if (message.error) {
      reject(new Error(message.error.message));
    } else {
      resolve(message.result);
    }
  }
}
```

### 4.4 Capability Negotiation

**Client Capabilities** (sent during initialize):

```typescript
capabilities: {
  textDocument: {
    synchronization: {
      didOpen: true,
      didChange: true,
      didClose: true
    },
    definition: { linkSupport: false },
    references: {
      includeDeclaration: true,
      dynamicRegistration: false
    },
    rename: { prepareSupport: false },
    documentSymbol: {
      symbolKind: {
        valueSet: [1, 2, 3, ..., 26]  // All SymbolKind values
      },
      hierarchicalDocumentSymbolSupport: true
    },
    completion: {
      completionItem: { snippetSupport: true }
    },
    hover: {},
    signatureHelp: {},
    diagnostic: { dynamicRegistration: false }
  },
  workspace: {
    workspaceEdit: { documentChanges: true },
    workspaceFolders: true
  }
}
```

**Server Capabilities** (returned from initialize):

The server responds with its supported features. cclsp checks these before making requests.

### 4.5 Document Synchronization

**Lifecycle**:

```
File First Accessed
    │
    ▼
ensureFileOpen()
    │
    ├─► Check openFiles Set
    │   └─► Already open? Skip
    │
    ├─► Read file content from disk
    │
    ├─► Send textDocument/didOpen notification
    │   {
    │     "textDocument": {
    │       "uri": "file:///...",
    │       "languageId": "typescript",
    │       "version": 1,
    │       "text": "...file content..."
    │     }
    │   }
    │
    └─► Add to openFiles Set

File Modified (after rename)
    │
    ▼
syncFileContent()
    │
    ├─► Read updated content from disk
    │
    ├─► Increment version number
    │
    └─► Send textDocument/didChange notification
        {
          "textDocument": { "uri": "...", "version": 2 },
          "contentChanges": [{ "text": "...updated content..." }]
        }
```

---

## 5. MCP Tools

### 5.1 find_definition

**Purpose**: Find where a symbol is defined

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| file_path | string | Yes | Path to the file |
| symbol_name | string | Yes | Name of the symbol |
| symbol_kind | string | No | Kind: function, class, variable, method, etc. |

**Example**:
```json
{
  "name": "find_definition",
  "arguments": {
    "file_path": "src/utils.ts",
    "symbol_name": "calculateTotal",
    "symbol_kind": "function"
  }
}
```

**Response**:
```
Results for calculateTotal (function) at src/utils.ts:15:1:
src/math/calculator.ts:45:1
```

**Implementation Flow**:
1. Get document symbols via `textDocument/documentSymbol`
2. Find symbols matching name and kind
3. For each match, send `textDocument/definition`
4. Aggregate and format results

### 5.2 find_references

**Purpose**: Find all usages of a symbol across the workspace

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| file_path | string | Yes | Path to the file |
| symbol_name | string | Yes | Name of the symbol |
| symbol_kind | string | No | Kind of symbol |
| include_declaration | boolean | No | Include the declaration itself (default: true) |

**Example**:
```json
{
  "name": "find_references",
  "arguments": {
    "file_path": "src/config.ts",
    "symbol_name": "CONFIG_PATH",
    "include_declaration": false
  }
}
```

**Response**:
```
Results for CONFIG_PATH (constant) at src/config.ts:10:1:
src/index.ts:45:15
src/utils/loader.ts:23:8
tests/config.test.ts:15:10
tests/config.test.ts:89:12
```

### 5.3 rename_symbol

**Purpose**: Rename a symbol by name across all files

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| file_path | string | Yes | Path to the file |
| symbol_name | string | Yes | Current name of the symbol |
| symbol_kind | string | No | Kind of symbol |
| new_name | string | Yes | New name for the symbol |
| dry_run | boolean | No | Preview only (default: false) |

**Example**:
```json
{
  "name": "rename_symbol",
  "arguments": {
    "file_path": "src/api/user.ts",
    "symbol_name": "getUserData",
    "symbol_kind": "function",
    "new_name": "fetchUserProfile",
    "dry_run": false
  }
}
```

**Response (applied)**:
```
Successfully renamed getUserData (function) to "fetchUserProfile".

Modified files:
- src/api/user.ts
- src/services/auth.ts
- src/components/UserProfile.tsx
```

**Response (dry run)**:
```
[DRY RUN] Would rename getUserData (function) to "fetchUserProfile":
File: src/api/user.ts
  - Line 55, Column 10 to Line 55, Column 21: "fetchUserProfile"
File: src/services/auth.ts
  - Line 123, Column 15 to Line 123, Column 26: "fetchUserProfile"
```

**Multiple Matches Response**:
```
Multiple symbols found matching "data". Please use rename_symbol_strict:
- data (variable) at line 45, character 10
- data (parameter) at line 89, character 25
- data (property) at line 112, character 5
```

### 5.4 rename_symbol_strict

**Purpose**: Rename symbol at specific position (when name is ambiguous)

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| file_path | string | Yes | Path to the file |
| line | number | Yes | Line number (1-indexed) |
| character | number | Yes | Character position (1-indexed) |
| new_name | string | Yes | New name for the symbol |
| dry_run | boolean | No | Preview only (default: false) |

**Example**:
```json
{
  "name": "rename_symbol_strict",
  "arguments": {
    "file_path": "src/utils/parser.ts",
    "line": 45,
    "character": 10,
    "new_name": "userData"
  }
}
```

### 5.5 get_diagnostics

**Purpose**: Get errors, warnings, and hints for a file

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| file_path | string | Yes | Path to the file |

**Example**:
```json
{
  "name": "get_diagnostics",
  "arguments": {
    "file_path": "src/index.ts"
  }
}
```

**Response**:
```
Found 3 diagnostics in src/index.ts:

Error [TS2304]: Cannot find name 'undefinedVar' (Line 10, Column 5)
Warning [no-unused-vars]: 'config' is defined but never used (Line 25, Column 10)
Hint: Consider using const instead of let (Line 30, Column 1)
```

**Implementation Details**:
- Uses hybrid approach: cached `publishDiagnostics` + fallback to `textDocument/diagnostic`
- Waits for diagnostic idle (100ms without changes)
- Triggers file sync if needed to refresh diagnostics

### 5.6 restart_server

**Purpose**: Restart LSP servers (useful when servers become unresponsive)

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| extensions | string[] | No | File extensions to restart (omit for all) |

**Example (specific)**:
```json
{
  "name": "restart_server",
  "arguments": {
    "extensions": ["ts", "tsx"]
  }
}
```

**Example (all)**:
```json
{
  "name": "restart_server",
  "arguments": {}
}
```

**Response**:
```
Successfully restarted 2 LSP server(s)
Restarted servers:
• typescript-language-server --stdio (ts, tsx)
• pylsp (py)
```

---

## 6. LSP Adapter System

### 6.1 Overview

Some LSP servers deviate from the standard protocol or have special requirements. The adapter system handles these cases by:

- Customizing initialization parameters
- Intercepting and handling non-standard messages
- Adjusting timeouts for slow servers
- Providing fallback implementations

### 6.2 Adapter Interface

**Location**: `/lsp/cclsp/src/lsp/adapters/types.ts`

```typescript
interface ServerAdapter {
  // Unique identifier
  readonly name: string;

  // Check if adapter applies to this server config
  matches(config: LSPServerConfig): boolean;

  // Modify initialization parameters
  customizeInitializeParams?(params: InitializeParams): InitializeParams;

  // Handle server-to-client notifications
  handleNotification?(
    method: string,
    params: unknown,
    state: ServerState
  ): boolean;  // Return true if handled

  // Handle server-to-client requests
  handleRequest?(
    method: string,
    params: unknown,
    state: ServerState
  ): Promise<unknown>;

  // Custom timeout for specific methods
  getTimeout?(method: string): number | undefined;

  // Check if method is supported (for fallback)
  isMethodSupported?(method: string): boolean;

  // Provide fallback implementation
  provideFallback?(
    method: string,
    params: unknown,
    state: ServerState
  ): Promise<unknown>;
}
```

### 6.3 Adapter Registry

**Location**: `/lsp/cclsp/src/lsp/adapters/registry.ts`

```typescript
class AdapterRegistry {
  private adapters: ServerAdapter[] = [
    new VueLanguageServerAdapter(),
    new PyrightAdapter(),
  ];

  // Find matching adapter for server config
  getAdapter(config: LSPServerConfig): ServerAdapter | undefined {
    return this.adapters.find(a => a.matches(config));
  }
}

// Global singleton
export const adapterRegistry = new AdapterRegistry();
```

**Usage in LSPClient**:

```typescript
async startServer(config: LSPServerConfig) {
  // ... spawn process ...

  // Detect and attach adapter
  const adapter = adapterRegistry.getAdapter(config);
  if (adapter) {
    serverState.adapter = adapter;
    process.stderr.write(`Detected ${adapter.name} adapter\n`);
  }
}
```

### 6.4 Vue Language Server Adapter

**Location**: `/lsp/cclsp/src/lsp/adapters/vue.ts`

**Problem**: Vue Language Server uses non-standard `tsserver/request` protocol for TypeScript integration.

**Solution**:

```typescript
class VueLanguageServerAdapter implements ServerAdapter {
  readonly name = 'vue-language-server';

  matches(config: LSPServerConfig): boolean {
    const cmd = config.command.join(' ').toLowerCase();
    return cmd.includes('vue-language-server') ||
           cmd.includes('@vue/language-server');
  }

  // Handle non-standard tsserver requests
  async handleRequest(
    method: string,
    params: unknown,
    state: ServerState
  ): Promise<unknown> {
    if (method === 'tsserver/request') {
      const { command } = params as { command: string };

      if (command === '_vue:projectInfo') {
        // Return minimal response to prevent hangs
        return { projectInfo: {} };
      }
    }
    return undefined;
  }

  // Extended timeouts for Vue analysis
  getTimeout(method: string): number | undefined {
    const timeouts: Record<string, number> = {
      'textDocument/documentSymbol': 60000,  // 60s
      'textDocument/definition': 45000,       // 45s
      'textDocument/references': 45000,       // 45s
      'textDocument/rename': 45000,           // 45s
    };
    return timeouts[method];
  }
}
```

### 6.5 Pyright Adapter

**Location**: `/lsp/cclsp/src/lsp/adapters/pyright.ts`

**Problem**: Pyright/basedpyright can be slow on large Python projects.

**Solution**:

```typescript
class PyrightAdapter implements ServerAdapter {
  readonly name = 'pyright';

  matches(config: LSPServerConfig): boolean {
    const cmd = config.command.join(' ').toLowerCase();
    return cmd.includes('pyright') || cmd.includes('basedpyright');
  }

  // Extended timeouts for large projects
  getTimeout(method: string): number | undefined {
    const timeouts: Record<string, number> = {
      'textDocument/definition': 45000,      // 45s
      'textDocument/references': 60000,       // 60s
      'textDocument/rename': 60000,           // 60s
      'textDocument/documentSymbol': 45000,   // 45s
    };
    return timeouts[method];
  }
}
```

### 6.6 Adding New Adapters

1. Create `src/lsp/adapters/my_adapter.ts`:

```typescript
import { ServerAdapter, ServerState } from './types';
import { LSPServerConfig } from '../types';

export class MyAdapter implements ServerAdapter {
  readonly name = 'my-language-server';

  matches(config: LSPServerConfig): boolean {
    return config.command.some(c => c.includes('my-lsp'));
  }

  getTimeout(method: string): number | undefined {
    // Return custom timeouts as needed
    return undefined;
  }
}
```

2. Register in `registry.ts`:

```typescript
import { MyAdapter } from './my_adapter';

class AdapterRegistry {
  private adapters: ServerAdapter[] = [
    new VueLanguageServerAdapter(),
    new PyrightAdapter(),
    new MyAdapter(),  // Add here
  ];
}
```

---

## 7. Configuration

### 7.1 Configuration File Format

**File**: `cclsp.json`

```json
{
  "servers": [
    {
      "extensions": ["ts", "tsx", "js", "jsx"],
      "command": ["npx", "--", "typescript-language-server", "--stdio"],
      "rootDir": ".",
      "initializationOptions": {}
    },
    {
      "extensions": ["py", "pyi"],
      "command": ["uvx", "--from", "python-lsp-server", "pylsp"],
      "rootDir": ".",
      "restartInterval": 5,
      "initializationOptions": {
        "settings": {
          "pylsp": {
            "plugins": {
              "jedi_completion": { "enabled": true },
              "jedi_definition": { "enabled": true },
              "pylint": { "enabled": false }
            }
          }
        }
      }
    },
    {
      "extensions": ["go"],
      "command": ["gopls"],
      "rootDir": "."
    }
  ]
}
```

### 7.2 Configuration Options

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `extensions` | string[] | Yes | File extensions this server handles |
| `command` | string[] | Yes | Command array to spawn the server |
| `rootDir` | string | No | Working directory (default: ".") |
| `restartInterval` | number | No | Auto-restart interval in minutes |
| `initializationOptions` | object | No | LSP initialization options |

### 7.3 Configuration Loading

**Priority**:
1. `CCLSP_CONFIG_PATH` environment variable
2. `cclsp.json` in current directory
3. Error if neither found

**Implementation** (lsp-client.ts:62-107):

```typescript
constructor(configPath?: string) {
  // Priority 1: Environment variable
  if (process.env.CCLSP_CONFIG_PATH) {
    const envPath = process.env.CCLSP_CONFIG_PATH;
    if (!existsSync(envPath)) {
      process.stderr.write(`Config not found: ${envPath}\n`);
      process.exit(1);
    }
    this.config = JSON.parse(readFileSync(envPath, 'utf-8'));
    return;
  }

  // Priority 2: Provided path or default
  const path = configPath || 'cclsp.json';
  if (!existsSync(path)) {
    process.stderr.write(`Config not found: ${path}\n`);
    process.exit(1);
  }
  this.config = JSON.parse(readFileSync(path, 'utf-8'));
}
```

### 7.4 Claude Code MCP Registration

**Automated** (via setup wizard):
```bash
npx cclsp setup
# Select "Add to Claude MCP" option
```

**Manual** (via CLI):
```bash
claude mcp add cclsp npx cclsp@latest --env CCLSP_CONFIG_PATH=/path/to/cclsp.json
```

**Manual** (edit Claude config directly):
```json
{
  "mcpServers": {
    "cclsp": {
      "command": "cclsp",
      "env": {
        "CCLSP_CONFIG_PATH": "/path/to/cclsp.json"
      }
    }
  }
}
```

### 7.5 Configuration Locations

| Scope | Location | Created By |
|-------|----------|-----------|
| Project | `.claude/cclsp.json` | `npx cclsp setup` |
| User | `~/.config/claude/cclsp.json` | `npx cclsp setup --user` |

---

## 8. Data Flow

### 8.1 Complete Request Lifecycle

```
┌────────────────────────────────────────────────────────────────────────┐
│                         Claude Code                                     │
│                                                                         │
│  User: "Find the definition of calculateTotal function"                │
│                                                                         │
│  Claude: Let me find the definition of calculateTotal                   │
│          > Using cclsp.find_definition                                 │
└────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ MCP Tool Call (stdio)
                                    │ {
                                    │   "name": "find_definition",
                                    │   "arguments": {
                                    │     "file_path": "src/utils.ts",
                                    │     "symbol_name": "calculateTotal",
                                    │     "symbol_kind": "function"
                                    │   }
                                    │ }
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                         cclsp (index.ts)                                │
│                                                                         │
│  1. Parse tool call                                                     │
│  2. Resolve absolute path: /project/src/utils.ts                       │
│  3. Call lspClient.findSymbolsByName(path, "calculateTotal", "function")│
└────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                      LSPClient.findSymbolsByName()                      │
│                                                                         │
│  4. getServer(filePath) - Get/start TypeScript LSP                     │
│     └─► Server not running? startServer(config)                        │
│         └─► spawn(['npx', '--', 'typescript-language-server', '--stdio'])│
│         └─► Send initialize request                                     │
│         └─► Send initialized notification                               │
│         └─► Wait for server ready                                       │
│                                                                         │
│  5. ensureFileOpen(server, filePath)                                   │
│     └─► Read file content                                              │
│     └─► Send textDocument/didOpen                                      │
│                                                                         │
│  6. getDocumentSymbols(filePath)                                       │
│     └─► Send textDocument/documentSymbol                               │
│     └─► Receive DocumentSymbol[]                                        │
│                                                                         │
│  7. flattenDocumentSymbols(symbols)                                    │
│     └─► Flatten nested symbols to array                                │
│                                                                         │
│  8. Filter by name and kind                                            │
│     └─► name === "calculateTotal" && kind === Function                 │
│     └─► Return matches with positions                                  │
└────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ matches: [{ name: "calculateTotal",
                                    │            kind: 12,
                                    │            position: {line: 10, char: 0} }]
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                       LSPClient.findDefinition()                        │
│                                                                         │
│  9. For each match:                                                     │
│     └─► Send textDocument/definition                                   │
│         {                                                               │
│           "textDocument": { "uri": "file:///project/src/utils.ts" },   │
│           "position": { "line": 10, "character": 0 }                   │
│         }                                                               │
│                                                                         │
│ 10. Receive Location[]                                                  │
│     └─► [{ uri: "file:///project/src/math.ts",                         │
│            range: { start: {line: 45, char: 0}, end: {line: 45, char: 20} } }]│
└────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                    Transform to MCP Response                            │
│                                                                         │
│ 11. Convert URIs to paths: uriToPath()                                 │
│ 12. Format as text:                                                     │
│     "Results for calculateTotal (function) at src/utils.ts:11:1:       │
│      src/math.ts:46:1"                                                 │
└────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ MCP Response (stdio)
                                    │ {
                                    │   "content": [{
                                    │     "type": "text",
                                    │     "text": "Results for calculateTotal..."
                                    │   }]
                                    │ }
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                         Claude Code                                     │
│                                                                         │
│  Result: Found definition at src/math.ts:46:1                          │
└────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Symbol Resolution Strategy

```
findSymbolsByName(file, "data", "variable")
    │
    ▼
Get all document symbols
    │
    ▼
Flatten hierarchical symbols
    │
    │  DocumentSymbol (class Foo)
    │  ├─► DocumentSymbol (method bar)
    │  │   └─► DocumentSymbol (variable data)  ← Nested
    │  └─► DocumentSymbol (property data)
    │
    │  Flattened: [class Foo, method bar, variable data, property data]
    │
    ▼
Filter by name
    │
    │  "data" matches: [variable data, property data]
    │
    ▼
Filter by kind (if provided)
    │
    │  kind === "variable": [variable data]
    │
    ▼
Return matches with positions
    │
    └─► [{ name: "data", kind: 13, position: {line: 15, char: 8} }]
```

### 8.3 Multi-Position Fallback

When exact symbol matching fails:

```
Search for "config" as "constant"
    │
    ▼
No exact matches found
    │
    ▼
Fallback: Search all kinds
    │
    │  Found:
    │  - config (variable) at line 10
    │  - config (property) at line 25
    │  - configPath (constant) at line 5
    │
    ▼
Return with warning:
    "Invalid symbol kind 'constant'. Valid kinds are: ...
     Searching all symbol types instead."

    Matches: [config (variable), config (property)]
```

---

## 9. Key Features

### 9.1 Multi-Language Support

- **15+ languages** with pre-configured servers
- **Extensible** via cclsp.json
- **Concurrent** server management
- **Extension-based** routing

### 9.2 Intelligent Symbol Resolution

- **Name-based search** instead of exact positions
- **Kind filtering** for disambiguation
- **Hierarchical symbol** flattening
- **Fallback strategies** for invalid inputs

### 9.3 Auto-Restart Mechanism

```typescript
// Configuration
{
  "extensions": ["py"],
  "command": ["pylsp"],
  "restartInterval": 5  // Restart every 5 minutes
}

// Implementation (lsp-client.ts:370-371)
if (config.restartInterval && config.restartInterval >= 1) {
  setInterval(() => {
    this.restartServers(config.extensions);
  }, config.restartInterval * 60 * 1000);
}
```

**Use Case**: Python LSP (pylsp) degrades after extended use; periodic restart maintains performance.

### 9.4 Dry-Run Mode for Rename

```
rename_symbol with dry_run=true
    │
    ▼
Get WorkspaceEdit from LSP
    │
    ▼
Format as preview (don't apply)
    │
    └─► "[DRY RUN] Would rename..."
        "File: src/utils.ts"
        "  - Line 10, Column 5 to Line 10, Column 15: \"newName\""
```

### 9.5 Backup File Creation

```
applyWorkspaceEdit()
    │
    ▼
For each file to modify:
    │
    ├─► Read original content
    │
    ├─► Create backup: src/utils.ts.bak
    │
    ├─► Apply edits
    │
    └─► Write to temp file, then rename
```

### 9.6 Diagnostic Caching

```
LSP Server publishes diagnostics
    │
    ▼
textDocument/publishDiagnostics notification
    │
    ▼
Cache in serverState.diagnostics Map
    │
    └─► Key: file URI
    └─► Value: Diagnostic[]
    └─► Track: lastDiagnosticUpdate, diagnosticVersions

getDiagnostics() called
    │
    ▼
Check cache freshness (idle detection)
    │
    ├─► Fresh? Return cached diagnostics
    │
    └─► Stale? Trigger file sync, wait for update
```

### 9.7 Server Preloading

```
cclsp starts
    │
    ▼
Scan configured directories for file types
    │
    ▼
For each detected extension:
    │
    └─► Start corresponding LSP server
        (Parallel startup with Promise.all)
```

---

## 10. File Reference

### Core Files

| File | Lines | Purpose |
|------|-------|---------|
| `index.ts` | ~738 | MCP server entry, tool handlers |
| `src/lsp-client.ts` | ~1690 | LSP client core, process management |
| `src/types.ts` | ~171 | TypeScript type definitions |
| `src/file-editor.ts` | ~200 | Workspace edit application |
| `src/file-scanner.ts` | ~170 | Project file scanning |
| `src/language-servers.ts` | ~180 | Pre-configured LSP definitions |
| `src/setup.ts` | ~400 | Interactive setup wizard |
| `src/utils.ts` | ~50 | URI/path conversion utilities |

### Adapter Files

| File | Purpose |
|------|---------|
| `src/lsp/adapters/types.ts` | ServerAdapter interface definition |
| `src/lsp/adapters/registry.ts` | Adapter registry (singleton) |
| `src/lsp/adapters/vue.ts` | Vue Language Server adapter |
| `src/lsp/adapters/pyright.ts` | Pyright adapter |

### Test Files

| File | Purpose |
|------|---------|
| `src/lsp-client.test.ts` | LSP client unit tests |
| `src/file-editor.test.ts` | File editing tests |
| `src/file-editor-rollback.test.ts` | Rollback functionality |
| `src/file-editor-symlink.test.ts` | Symlink handling |
| `src/file-scanner.test.ts` | File scanning tests |
| `src/setup.test.ts` | Setup wizard tests |
| `src/mcp-tools.test.ts` | MCP tool tests |
| `src/get-diagnostics.test.ts` | Diagnostic tests |
| `src/multi-position.test.ts` | Multi-position matching |
| `src/lsp/adapters/*.test.ts` | Adapter tests |

### Configuration Files

| File | Purpose |
|------|---------|
| `package.json` | NPM metadata, dependencies, scripts |
| `tsconfig.json` | TypeScript compiler options |
| `biome.json` | Linting and formatting rules |
| `bunfig.toml` | Bun runtime configuration |
| `cclsp.json` | Example LSP server configuration |

---

## Quick Reference

### Installation

```bash
# One-time use (recommended)
npx cclsp@latest setup

# Global install
npm install -g cclsp
```

### Common Commands

```bash
# Interactive setup
npx cclsp setup

# User-wide configuration
npx cclsp setup --user

# Run server directly
cclsp

# Development
bun run dev      # Watch mode
bun test         # Run tests
bun run lint     # Check code style
```

### MCP Registration

```bash
# Automated
npx cclsp setup  # Select "Add to Claude MCP"

# Manual
claude mcp add cclsp npx cclsp@latest --env CCLSP_CONFIG_PATH=/path/to/cclsp.json
```

### Troubleshooting

| Issue | Solution |
|-------|----------|
| LSP not starting | Check language server installation |
| Config not loading | Verify `CCLSP_CONFIG_PATH` or `cclsp.json` exists |
| Symbol not found | Ensure file is saved and server supports file type |
| Python LSP slow | Add `"restartInterval": 5` to config |
| Server unresponsive | Use `restart_server` tool |

---

*Document generated from cclsp v0.6.2 source analysis*
