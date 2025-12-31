# Serena LSP Integration Analysis

**Date:** 2025-12-05
**Analyst:** Claude Code
**Purpose:** Analyze Serena's LSP integration for implementing similar capabilities in codex-rs

---

## Executive Summary

Serena is a Python-based coding agent toolkit that provides IDE-like semantic code analysis capabilities through the Language Server Protocol (LSP). This analysis examines Serena's architecture with focus on:

1. **LSP Server Integration** - How language servers are started, managed, and communicated with
2. **LanguageServerManager** - Multi-language orchestration, parallel startup, auto-restart
3. **Build/Packaging System** - How language server binaries are downloaded and installed
4. **File Watching & Synchronization** - How file changes are detected and synced to LSP servers
5. **Search APIs** - What search/retrieval capabilities are exposed to agents
6. **Symbol Editing Tools** - Symbol-based code editing (replace, insert, rename)
7. **CodeEditor Architecture** - Abstract editing layer with symbol-aware operations
8. **Additional LSP Operations** - Diagnostics, completions, rename refactoring

---

## 1. LSP Server Integration Architecture

### 1.1 Core Components

Serena's LSP integration is built on a three-layer architecture:

```
┌─────────────────────────────────────────────────────────────┐
│ SerenaAgent (src/serena/agent.py)                          │
│ - Tool orchestration, MCP interface, session management    │
└────────────────┬────────────────────────────────────────────┘
                 │
┌────────────────▼────────────────────────────────────────────┐
│ SolidLanguageServer (src/solidlsp/ls.py)                   │
│ - Language-agnostic LSP wrapper with caching               │
│ - Symbol discovery, references, definitions                │
│ - File synchronization (open/close/change notifications)   │
└────────────────┬────────────────────────────────────────────┘
                 │
┌────────────────▼────────────────────────────────────────────┐
│ SolidLanguageServerHandler (src/solidlsp/ls_handler.py)    │
│ - JSON-RPC 2.0 communication over stdin/stdout             │
│ - Process lifecycle management                             │
│ - Request/response/notification handling                   │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 LSP Server Lifecycle

#### Initialization Sequence

```python
# 1. Create language server instance (factory pattern)
ls = SolidLanguageServer.create(
    config=LanguageServerConfig(...),
    repository_root_path="/path/to/project",
    timeout=30.0,
    solidlsp_settings=SolidLSPSettings()
)

# 2. Start server process
with ls.start_server():
    # 3. Server sends 'initialize' request
    # 4. Server waits for capabilities response
    # 5. Server sends 'initialized' notification
    # 6. Server is ready for requests

    # Perform LSP operations...

# 7. On context exit: shutdown and cleanup
```

**File:** `src/solidlsp/ls_handler.py:180-200`

```python
def start(self) -> None:
    """Start the language server process"""
    child_proc_env = os.environ.copy()
    child_proc_env.update(self.process_launch_info.env)

    # Start subprocess with stdin/stdout pipes
    self.process = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stdin=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=child_proc_env,
        **kwargs
    )

    # Start background threads for I/O
    self._start_io_threads()
```

#### Graceful Shutdown

**File:** `src/solidlsp/ls.py:393-451`

Multi-stage shutdown process:
1. Send LSP `shutdown` request (2s timeout)
2. Close stdin to signal no more input
3. Send `SIGTERM` to process
4. Wait for termination (configurable timeout)
5. Force kill with `SIGKILL` if needed

### 1.3 Communication Protocol

Serena uses **JSON-RPC 2.0** over stdin/stdout:

**Message Format:**
```
Content-Length: <bytes>\r\n
\r\n
{JSON-RPC payload}
```

**Request/Response Handling** (`src/solidlsp/ls_handler.py:56-94`):

```python
class Request:
    def __init__(self, request_id: int, method: str):
        self._request_id = request_id
        self._method = method
        self._result_queue: Queue[Result] = Queue()

    def on_result(self, params: PayloadLike) -> None:
        self._result_queue.put(Result(payload=params))

    def get_result(self, timeout: float | None = None) -> Result:
        return self._result_queue.get(timeout=timeout)
```

**Threading Model:**
- Main thread: Sends requests via stdin
- Reader thread: Reads from stdout, dispatches to request handlers
- Error thread: Monitors stderr for diagnostics

**File:** `src/solidlsp/lsp_protocol_handler/lsp_requests.py`

### 1.4 Language-Specific Implementations

Each language server has a dedicated class:

**Example: TypeScript** (`src/solidlsp/language_servers/typescript_language_server.py`)

```python
class TypeScriptLanguageServer(SolidLanguageServer):
    def __init__(self, config, repository_root_path, solidlsp_settings):
        ts_lsp_executable_path = self._setup_runtime_dependencies(...)
        super().__init__(
            config,
            repository_root_path,
            ProcessLaunchInfo(cmd=ts_lsp_executable_path, cwd=repository_root_path),
            "typescript",
            solidlsp_settings
        )

    def _start_server(self) -> None:
        # Register notification handlers
        self.server.on_notification("window/logMessage", window_log_message)
        self.server.on_notification("$/progress", do_nothing)

        # Start process and initialize
        self.server.start()
        init_response = self.server.send.initialize(initialize_params)
        self.server.notify.initialized({})
```

**Example: Python** (`src/solidlsp/language_servers/pyright_server.py`)

```python
class PyrightServer(SolidLanguageServer):
    def __init__(self, config, repository_root_path, solidlsp_settings):
        super().__init__(
            config,
            repository_root_path,
            ProcessLaunchInfo(cmd="python -m pyright.langserver --stdio", cwd=repository_root_path),
            "python",
            solidlsp_settings
        )
        self.analysis_complete = threading.Event()

    def _start_server(self) -> None:
        # Wait for Pyright's "Found X source files" log message
        def window_log_message(msg: dict) -> None:
            if re.search(r"Found \d+ source files?", message_text):
                self.analysis_complete.set()

        self.server.on_notification("window/logMessage", window_log_message)
        # ... start server and wait for ready signal
        self.analysis_complete.wait(timeout=5.0)
```

### 1.5 Key LSP Operations

#### Document Symbols (`ls.py:800+`)

```python
def get_document_symbols(self, relative_file_path: str) -> DocumentSymbols:
    """Get all symbols in a file (classes, functions, methods, etc.)"""
    # Check cache first
    if cached := self._document_symbols_cache.get(relative_file_path):
        return cached

    # Request from LSP server
    with self.open_file(relative_file_path) as file_buffer:
        response = self.server.send.document_symbol({
            "textDocument": {"uri": file_buffer.uri}
        })

    # Parse and cache
    symbols = self._parse_document_symbols(response)
    self._document_symbols_cache[relative_file_path] = symbols
    return symbols
```

#### Go to Definition (`ls.py:613-687`)

```python
def request_definition(self, relative_file_path: str, line: int, column: int):
    """Find definition of symbol at given position"""
    with self.open_file(relative_file_path):
        response = self.server.send.definition({
            "textDocument": {"uri": ...},
            "position": {"line": line, "character": column}
        })

    # Convert LSP Location to internal format
    return self._parse_locations(response)
```

#### Find References (`ls.py:689+`)

```python
def request_references(self, relative_file_path: str, line: int, column: int):
    """Find all references to symbol at given position"""
    response = self.server.send.references({
        "textDocument": {"uri": ...},
        "position": {"line": line, "character": column},
        "context": {"includeDeclaration": False}
    })
    return self._parse_locations(response)
```

### 1.6 LanguageServerManager - Multi-Language Orchestration

**File:** `src/serena/ls_manager.py`

The `LanguageServerManager` is a **critical orchestration layer** that manages multiple language servers for a single project.

#### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ SerenaAgent                                                  │
└────────────────┬─────────────────────────────────────────────┘
                 │
┌────────────────▼─────────────────────────────────────────────┐
│ LanguageServerManager                                        │
│ - Routes requests to correct LS based on file type           │
│ - Parallel startup of multiple language servers              │
│ - Auto-restart crashed servers                               │
│ - Cache persistence coordination                             │
└────┬───────────────┬───────────────┬────────────────────────┘
     │               │               │
┌────▼────┐    ┌────▼────┐    ┌────▼────┐
│ Python  │    │ TypeScript│   │  Rust   │
│   LS    │    │    LS    │    │   LS    │
└─────────┘    └──────────┘    └─────────┘
```

#### Factory and Parallel Startup

```python
class LanguageServerFactory:
    """Factory for creating language server instances"""

    def __init__(
        self,
        project_root: str,
        encoding: str,
        ignored_patterns: list[str],
        ls_timeout: float | None = None,
        ls_specific_settings: dict | None = None,
        trace_lsp_communication: bool = False,
    ):
        self.project_root = project_root
        self.encoding = encoding
        self.ignored_patterns = ignored_patterns
        # ...

    def create_language_server(self, language: Language) -> SolidLanguageServer:
        ls_config = LanguageServerConfig(
            code_language=language,
            ignored_paths=self.ignored_patterns,
            trace_lsp_communication=self.trace_lsp_communication,
            encoding=self.encoding,
        )
        return SolidLanguageServer.create(
            ls_config,
            self.project_root,
            timeout=self.ls_timeout,
            solidlsp_settings=SolidLSPSettings(...)
        )


class LanguageServerManager:
    """Manages one or more language servers for a project."""

    @staticmethod
    def from_languages(languages: list[Language], factory: LanguageServerFactory):
        """Create manager with parallel LS startup"""
        language_servers: dict[Language, SolidLanguageServer] = {}
        threads = []
        exceptions = {}
        lock = threading.Lock()

        def start_language_server(language: Language):
            try:
                language_server = factory.create_language_server(language)
                language_server.start()
                if not language_server.is_running():
                    raise RuntimeError(f"Failed to start LS for {language.value}")
                with lock:
                    language_servers[language] = language_server
            except Exception as e:
                with lock:
                    exceptions[language] = e

        # Start all language servers in parallel threads
        for language in languages:
            thread = threading.Thread(
                target=start_language_server,
                args=(language,),
                name=f"StartLS:{language.value}"
            )
            thread.start()
            threads.append(thread)

        # Wait for all to complete
        for thread in threads:
            thread.join()

        # Handle failures
        if exceptions:
            for ls in language_servers.values():
                ls.stop()
            raise Exception(f"Failed to start language servers: {exceptions}")

        return LanguageServerManager(language_servers, factory)
```

#### Language Routing and Auto-Restart

```python
def get_language_server(self, relative_path: str) -> SolidLanguageServer:
    """Get appropriate LS for file, based on extension/path"""
    ls: SolidLanguageServer | None = None

    if len(self._language_servers) > 1:
        # Find LS that supports this file type
        for candidate in self._language_servers.values():
            if not candidate.is_ignored_path(relative_path, ignore_unsupported_files=True):
                ls = candidate
                break

    if ls is None:
        ls = self._default_language_server

    # Auto-restart if not running
    return self._ensure_functional_ls(ls)

def _ensure_functional_ls(self, ls: SolidLanguageServer) -> SolidLanguageServer:
    """Auto-restart crashed language servers"""
    if not ls.is_running():
        log.warning(f"Language server for {ls.language} not running; restarting...")
        ls = self.restart_language_server(ls.language)
    return ls

def restart_language_server(self, language: Language) -> SolidLanguageServer:
    """Force recreation and restart of language server"""
    if language not in self._language_servers:
        raise ValueError(f"No LS for {language.value} present")
    return self._create_and_start_language_server(language)
```

#### Cache Management

```python
def save_all_caches(self) -> None:
    """Persist caches to disk for all servers"""
    for ls in self.iter_language_servers():
        if ls.is_running():
            ls.save_cache()

def stop_all(self, save_cache: bool = False) -> None:
    """Stop all managed language servers"""
    for ls in self.iter_language_servers():
        if ls.is_running():
            if save_cache:
                ls.save_cache()
            ls.stop()
```

#### Key Features

| Feature | Description |
|---------|-------------|
| **Parallel Startup** | Multiple LS start simultaneously in threads |
| **Language Routing** | Routes requests to correct LS based on file extension |
| **Auto-Recovery** | Crashed servers are automatically restarted |
| **Dynamic Add/Remove** | Add/remove language servers at runtime |
| **Cache Coordination** | Coordinates cache saving across all servers |

### 1.7 Process Tree Management

**File:** `src/solidlsp/ls_handler.py:267-297`

Language servers (especially Node.js-based ones like TypeScript) spawn child processes. Serena uses **psutil** to terminate the entire process tree:

```python
def _signal_process_tree(self, process: subprocess.Popen, terminate: bool = True):
    """Send signal to process and all its children"""
    signal_method = "terminate" if terminate else "kill"

    try:
        parent = psutil.Process(process.pid)
    except (psutil.NoSuchProcess, psutil.AccessDenied):
        return

    if parent and parent.is_running():
        # Signal children first (important for Node.js!)
        for child in parent.children(recursive=True):
            try:
                getattr(child, signal_method)()
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass

        # Then signal parent
        try:
            getattr(parent, signal_method)()
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass
    else:
        # Fallback to direct process signaling
        try:
            getattr(process, signal_method)()
        except Exception:
            pass
```

**Why This Matters:**
- Node.js-based LSPs (TypeScript, JavaScript) spawn worker processes
- Without process tree cleanup, child processes become zombies
- **For codex-rs:** Use `sysinfo` crate or platform-specific APIs (`libc::kill` with process groups)

---

## 2. Build/Packaging System for Language Servers

### 2.1 Runtime Dependency Management

Serena uses a **declarative dependency system** to automatically download and install language server binaries.

**File:** `src/solidlsp/language_servers/common.py`

```python
@dataclass(kw_only=True)
class RuntimeDependency:
    """Represents a runtime dependency for a language server"""
    id: str                          # Unique identifier
    platform_id: str | None          # "linux_x64", "osx_arm64", etc., or "any"
    url: str | None                  # Download URL
    archive_type: str | None         # "zip", "gz", "binary"
    binary_name: str | None          # Executable name after extraction
    command: str | list[str] | None  # Installation command (e.g., npm install)
    package_name: str | None         # Package name for package managers
    package_version: str | None      # Package version
    extract_path: str | None         # Path within archive to extract
    description: str | None          # Human-readable description
```

### 2.2 Installation Process

```python
class RuntimeDependencyCollection:
    def install(self, target_dir: str) -> dict[str, str]:
        """Install all dependencies for current platform"""
        os.makedirs(target_dir, exist_ok=True)
        results = {}

        for dep in self.get_dependencies_for_current_platform():
            # Option 1: Download from URL
            if dep.url:
                self._install_from_url(dep, target_dir)

            # Option 2: Run installation command
            if dep.command:
                self._run_command(dep.command, target_dir)

            # Record installed binary path
            if dep.binary_name:
                results[dep.id] = os.path.join(target_dir, dep.binary_name)

        return results
```

### 2.3 Example: TypeScript Language Server

**File:** `src/solidlsp/language_servers/typescript_language_server.py:77-135`

```python
@classmethod
def _setup_runtime_dependencies(cls, config, solidlsp_settings) -> list[str]:
    """Download and install TypeScript LS if not present"""

    # Define dependencies
    deps = RuntimeDependencyCollection([
        RuntimeDependency(
            id="typescript",
            description="typescript package",
            command=["npm", "install", "--prefix", "./", "typescript@5.5.4"],
            platform_id="any"
        ),
        RuntimeDependency(
            id="typescript-language-server",
            description="typescript-language-server package",
            command=["npm", "install", "--prefix", "./", "typescript-language-server@4.3.3"],
            platform_id="any"
        )
    ])

    # Check if already installed
    tsserver_ls_dir = os.path.join(cls.ls_resources_dir(solidlsp_settings), "ts-lsp")
    tsserver_executable_path = os.path.join(
        tsserver_ls_dir, "node_modules", ".bin", "typescript-language-server"
    )

    if not os.path.exists(tsserver_executable_path):
        log.info("TypeScript LS not found, installing...")
        deps.install(tsserver_ls_dir)

    return [tsserver_executable_path, "--stdio"]
```

### 2.4 Storage Locations

Language server resources are stored in:
```
~/.cache/serena/ls-resources/  (Linux)
~/Library/Caches/serena/ls-resources/  (macOS)
%LOCALAPPDATA%\serena\ls-resources\  (Windows)
```

Structure:
```
ls-resources/
├── TypeScriptLanguageServer/
│   └── ts-lsp/
│       └── node_modules/
│           ├── typescript/
│           └── typescript-language-server/
├── PyrightServer/
│   └── (uses system pyright)
├── EclipseJdtls/
│   └── jdt-ls/
│       └── jdt-language-server-*.tar.gz (extracted)
└── ...
```

**File:** `src/solidlsp/ls.py:185-203`

### 2.5 Platform Detection

**File:** `src/solidlsp/ls_utils.py`

```python
class PlatformId(Enum):
    LINUX_x64 = "linux_x64"
    LINUX_arm64 = "linux_arm64"
    OSX = "osx"
    OSX_x64 = "osx_x64"
    OSX_arm64 = "osx_arm64"
    WIN_x64 = "win_x64"
    WIN_arm64 = "win_arm64"

class PlatformUtils:
    @staticmethod
    def get_platform_id() -> PlatformId:
        system = platform.system().lower()
        machine = platform.machine().lower()

        if system == "linux":
            return PlatformId.LINUX_arm64 if "arm" in machine or "aarch64" in machine else PlatformId.LINUX_x64
        elif system == "darwin":
            return PlatformId.OSX_arm64 if machine == "arm64" else PlatformId.OSX_x64
        elif system == "windows":
            return PlatformId.WIN_arm64 if "arm" in machine else PlatformId.WIN_x64
```

### 2.6 Download and Extraction

**File:** `src/solidlsp/ls_utils.py`

```python
class FileUtils:
    @staticmethod
    def download_and_extract_archive(url: str, dest: str, archive_type: str):
        """Download and extract archive from URL"""
        import requests
        import tarfile
        import zipfile

        # Download to temp file
        response = requests.get(url, stream=True)
        temp_file = tempfile.NamedTemporaryFile(delete=False)
        for chunk in response.iter_content(chunk_size=8192):
            temp_file.write(chunk)
        temp_file.close()

        # Extract based on type
        if archive_type == "zip":
            with zipfile.ZipFile(temp_file.name, 'r') as zip_ref:
                zip_ref.extractall(dest)
        elif archive_type in ["tar.gz", "gz"]:
            with tarfile.open(temp_file.name, 'r:gz') as tar_ref:
                tar_ref.extractall(dest)
        elif archive_type == "binary":
            shutil.copy(temp_file.name, dest)
            os.chmod(dest, 0o755)

        os.unlink(temp_file.name)
```

---

## 3. File Watching and LSP Synchronization

### 3.1 File Buffer Management

Serena maintains **in-memory file buffers** synchronized with the LSP server:

**File:** `src/solidlsp/ls.py:63-91`

```python
@dataclasses.dataclass
class LSPFileBuffer:
    """In-memory representation of an LSP file"""
    uri: str              # file:///path/to/file
    contents: str         # Current file contents
    version: int          # Document version (incremented on changes)
    language_id: str      # "python", "typescript", etc.
    ref_count: int        # Number of open references
    content_hash: str     # MD5 hash for cache validation
```

### 3.2 File Open/Close Protocol

**File:** `src/solidlsp/ls.py:467-514`

```python
@contextmanager
def open_file(self, relative_file_path: str) -> Iterator[LSPFileBuffer]:
    """Open file in LSP server (reference counted)"""
    uri = pathlib.Path(absolute_file_path).as_uri()

    if uri in self.open_file_buffers:
        # File already open - increment ref count
        self.open_file_buffers[uri].ref_count += 1
        yield self.open_file_buffers[uri]
        self.open_file_buffers[uri].ref_count -= 1
    else:
        # Read file and send didOpen notification
        contents = FileUtils.read_file(absolute_file_path, encoding)
        self.open_file_buffers[uri] = LSPFileBuffer(
            uri=uri,
            contents=contents,
            version=0,
            language_id=self.language_id,
            ref_count=1
        )

        # Notify LSP server
        self.server.notify.did_open_text_document({
            "textDocument": {
                "uri": uri,
                "languageId": self.language_id,
                "version": 0,
                "text": contents
            }
        })

        yield self.open_file_buffers[uri]
        self.open_file_buffers[uri].ref_count -= 1

    # Close file when ref_count reaches 0
    if self.open_file_buffers[uri].ref_count == 0:
        self.server.notify.did_close_text_document({
            "textDocument": {"uri": uri}
        })
        del self.open_file_buffers[uri]
```

### 3.3 Incremental Text Changes

When edits are made, Serena sends **incremental change notifications**:

**File:** `src/solidlsp/ls.py:530-572`

```python
def insert_text_at_position(self, relative_file_path: str, line: int, column: int, text: str):
    """Insert text and notify LSP server of change"""
    uri = pathlib.Path(absolute_file_path).as_uri()
    file_buffer = self.open_file_buffers[uri]

    # Update version
    file_buffer.version += 1

    # Update in-memory contents
    new_contents, new_line, new_col = TextUtils.insert_text_at_position(
        file_buffer.contents, line, column, text
    )
    file_buffer.contents = new_contents

    # Send incremental change notification
    self.server.notify.did_change_text_document({
        "textDocument": {
            "version": file_buffer.version,
            "uri": file_buffer.uri
        },
        "contentChanges": [{
            "range": {
                "start": {"line": line, "character": column},
                "end": {"line": line, "character": column}
            },
            "text": text
        }]
    })

    return Position(line=new_line, character=new_col)
```

**Delete operation** (`ls.py:574-608`):

```python
def delete_text_between_positions(self, relative_file_path: str, start: Position, end: Position):
    """Delete text range and notify LSP server"""
    file_buffer = self.open_file_buffers[uri]
    file_buffer.version += 1

    new_contents, deleted_text = TextUtils.delete_text_between_positions(
        file_buffer.contents, start["line"], start["character"], end["line"], end["character"]
    )
    file_buffer.contents = new_contents

    self.server.notify.did_change_text_document({
        "textDocument": {"version": file_buffer.version, "uri": file_buffer.uri},
        "contentChanges": [{
            "range": {"start": start, "end": end},
            "text": ""
        }]
    })

    return deleted_text
```

### 3.4 File System Watching

Serena **does not implement active file watching**. Instead:

1. **Agent-driven updates**: Tools that modify files update the LSP buffer immediately
2. **On-demand refresh**: When opening a file, reads from disk and syncs to LSP
3. **Cache invalidation**: Content hash is checked to detect external changes

**LSP Protocol Support** (not actively used):

```python
# From lsp_protocol_handler/lsp_requests.py:548
def did_change_watched_files(self, params: DidChangeWatchedFilesParams) -> None:
    """Notify LS of file system changes"""
    self.send_notification("workspace/didChangeWatchedFiles", params)
```

**Note:** Most language servers have their own internal file watchers and will detect external changes independently.

---

## 4. Search APIs

### 4.1 Symbol-Based Search

Serena provides **four primary search tools** for agents:

#### 4.1.1 Get Symbols Overview

**Tool:** `GetSymbolsOverviewTool`
**File:** `src/serena/tools/symbol_tools.py:48-76`

```python
def apply(self, relative_path: str, max_answer_chars: int = -1) -> str:
    """
    Get high-level overview of top-level symbols in a file.
    Returns JSON with symbol metadata (name, kind, location).
    """
    symbol_retriever = self.create_language_server_symbol_retriever()
    result = symbol_retriever.get_symbol_overview(relative_path)[relative_path]
    return self._to_json([dataclasses.asdict(i) for i in result])
```

**Example Output:**
```json
[
  {
    "name": "MyClass",
    "kind": 5,  // SymbolKind.Class
    "name_path": "MyClass",
    "body_location": {"start_line": 10, "end_line": 50},
    "children": [...]
  }
]
```

#### 4.1.2 Find Symbol

**Tool:** `FindSymbolTool`
**File:** `src/serena/tools/symbol_tools.py:79-148`

```python
def apply(
    self,
    name_path_pattern: str,      # "MyClass/my_method" or just "my_method"
    depth: int = 0,               # Include N levels of children
    relative_path: str = "",      # Restrict to file/directory
    include_body: bool = False,   # Include source code
    include_kinds: list[int] = [],
    exclude_kinds: list[int] = [],
    substring_matching: bool = False,
    max_answer_chars: int = -1
) -> str:
    """
    Find symbols by name path pattern.

    Name path examples:
    - "method" - any symbol named "method"
    - "Class/method" - method inside Class
    - "/Class/method" - exact absolute path within file
    - "Class/method[1]" - second overload of method
    """
    symbols = symbol_retriever.find_by_name(
        name_path_pattern,
        include_kinds=parsed_include_kinds,
        exclude_kinds=parsed_exclude_kinds,
        substring_matching=substring_matching,
        within_relative_path=relative_path
    )
    return self._to_json([s.to_dict(...) for s in symbols])
```

**LSP Symbol Kinds:**
```python
1=file, 2=module, 3=namespace, 4=package, 5=class, 6=method, 7=property,
8=field, 9=constructor, 10=enum, 11=interface, 12=function, 13=variable,
14=constant, 15=string, 16=number, 17=boolean, 18=array, 19=object,
20=key, 21=null, 22=enum member, 23=struct, 24=event, 25=operator,
26=type parameter
```

#### 4.1.3 Find Referencing Symbols

**Tool:** `FindReferencingSymbolsTool`
**File:** `src/serena/tools/symbol_tools.py:151-201`

```python
def apply(
    self,
    name_path: str,           # Name path of target symbol
    relative_path: str,       # File containing target symbol
    include_kinds: list[int] = [],
    exclude_kinds: list[int] = [],
    max_answer_chars: int = -1
) -> str:
    """
    Find all symbols that reference the given symbol.
    Returns symbol metadata + code snippet around reference.
    """
    references_in_symbols = symbol_retriever.find_referencing_symbols(
        name_path,
        relative_file_path=relative_path,
        include_body=False,
        include_kinds=parsed_include_kinds,
        exclude_kinds=parsed_exclude_kinds
    )

    # Add context around each reference
    for ref in references_in_symbols:
        content_around_ref = self.project.retrieve_content_around_line(
            relative_file_path=ref.symbol.location.relative_path,
            line=ref.line,
            context_lines_before=1,
            context_lines_after=1
        )
        ref_dict["content_around_reference"] = content_around_ref.to_display_string()

    return self._to_json(reference_dicts)
```

### 4.2 Text-Based Search

#### 4.2.1 File Search Tool

**Tool:** `FindFileTool`
**File:** `src/serena/tools/file_tools.py:124-157`

```python
def apply(self, file_mask: str, relative_path: str) -> str:
    """
    Find files matching glob pattern.

    :param file_mask: Filename pattern (e.g., "*.py", "test_*.rs")
    :param relative_path: Directory to search in
    """
    dirs, files = scan_directory(
        path=os.path.join(project_root, relative_path),
        recursive=True,
        is_ignored_dir=self.project.is_ignored_path,
        is_ignored_file=lambda p: not fnmatch(os.path.basename(p), file_mask)
    )
    return self._to_json({"files": files})
```

#### 4.2.2 Text Search

**File:** `src/serena/text_utils.py:138-250`

```python
def search_text(
    pattern: str,                    # Regex or glob pattern
    content: str | None = None,      # Text to search in
    source_file_path: str | None = None,  # Or file path
    allow_multiline_match: bool = False,
    context_lines_before: int = 0,
    context_lines_after: int = 0,
    is_glob: bool = False
) -> list[MatchedConsecutiveLines]:
    """
    Search for pattern in text content.
    Returns matched lines with context.
    """
    if is_glob:
        pattern = glob_to_regex(pattern)

    flags = re.MULTILINE | re.DOTALL if allow_multiline_match else re.MULTILINE
    regex = re.compile(pattern, flags)

    # Find all matches
    for match in regex.finditer(content):
        # Extract lines with context
        matched_lines = MatchedConsecutiveLines(...)
        results.append(matched_lines)

    return results
```

**Multi-file search** (`text_utils.py:252+`):

```python
def search_files(
    patterns: list[str],
    paths: list[str],
    is_ignored_path: Callable[[str], bool],
    context_lines_before: int = 0,
    context_lines_after: int = 0,
    is_glob: bool = False,
    max_workers: int = 4
) -> dict[str, list[MatchedConsecutiveLines]]:
    """
    Search multiple files in parallel for patterns.
    Uses joblib for parallel processing.
    """
    all_files = []
    for path in paths:
        if os.path.isfile(path):
            all_files.append(path)
        else:
            _, files = scan_directory(path, recursive=True, is_ignored_file=is_ignored_path)
            all_files.extend(files)

    # Parallel search
    results = Parallel(n_jobs=max_workers)(
        delayed(_search_single_file)(file_path, patterns, ...)
        for file_path in all_files
    )

    return {path: matches for path, matches in results if matches}
```

### 4.3 Search Tool Implementation

**Tool:** `SearchTextTool` (not shown in code, but inferred from architecture)

Typical usage pattern:
```python
class SearchTextTool(Tool):
    def apply(
        self,
        pattern: str,
        relative_path: str = ".",
        context_lines: int = 2,
        is_regex: bool = True
    ) -> str:
        paths = [os.path.join(self.get_project_root(), relative_path)]
        results = search_files(
            patterns=[pattern],
            paths=paths,
            is_ignored_path=self.project.is_ignored_path,
            context_lines_before=context_lines,
            context_lines_after=context_lines,
            is_glob=not is_regex
        )

        # Format results for LLM
        formatted = []
        for file_path, matches in results.items():
            for match in matches:
                formatted.append({
                    "file": file_path,
                    "lines": match.to_display_string(include_line_numbers=True)
                })

        return self._to_json(formatted)
```

### 4.3 Symbol Editing Tools

Serena provides **symbol-based editing tools** that allow precise code modifications using LSP-derived symbol information.

**File:** `src/serena/tools/symbol_tools.py`

#### 4.3.1 ReplaceSymbolBodyTool

Replaces the entire definition of a symbol:

```python
class ReplaceSymbolBodyTool(Tool, ToolMarkerSymbolicEdit):
    """Replaces the full definition of a symbol using the language server backend."""

    def apply(
        self,
        name_path: str,        # e.g., "MyClass/my_method"
        relative_path: str,    # e.g., "src/auth.py"
        body: str,             # New symbol body
    ) -> str:
        """
        Replaces the body of the symbol with the given `name_path`.

        IMPORTANT: The body is the definition of a symbol (including signature line),
        but does NOT include preceding docstrings/comments or imports.

        :param name_path: Symbol path (see find_symbol for syntax)
        :param relative_path: File containing the symbol
        :param body: New symbol body
        """
        code_editor = self.create_code_editor()
        code_editor.replace_body(
            name_path,
            relative_file_path=relative_path,
            body=body,
        )
        return SUCCESS_RESULT
```

#### 4.3.2 InsertAfterSymbolTool

Inserts code after a symbol (e.g., add new method after existing one):

```python
class InsertAfterSymbolTool(Tool, ToolMarkerSymbolicEdit):
    """Inserts content after the end of the definition of a given symbol."""

    def apply(
        self,
        name_path: str,
        relative_path: str,
        body: str,
    ) -> str:
        """
        Inserts the given body/content after the end of the symbol definition.
        Typical use case: insert a new class, function, method, field, or variable.

        :param name_path: Symbol after which to insert
        :param relative_path: File containing the symbol
        :param body: Content to insert (begins on next line after symbol)
        """
        code_editor = self.create_code_editor()
        code_editor.insert_after_symbol(name_path, relative_file_path=relative_path, body=body)
        return SUCCESS_RESULT
```

#### 4.3.3 InsertBeforeSymbolTool

Inserts code before a symbol (e.g., add import before first symbol):

```python
class InsertBeforeSymbolTool(Tool, ToolMarkerSymbolicEdit):
    """Inserts content before the beginning of the definition of a given symbol."""

    def apply(
        self,
        name_path: str,
        relative_path: str,
        body: str,
    ) -> str:
        """
        Inserts content before the beginning of the symbol definition.
        Typical use case: insert import statement before first symbol in file.

        :param name_path: Symbol before which to insert
        :param relative_path: File containing the symbol
        :param body: Content to insert before the symbol's line
        """
        code_editor = self.create_code_editor()
        code_editor.insert_before_symbol(name_path, relative_file_path=relative_path, body=body)
        return SUCCESS_RESULT
```

#### 4.3.4 RenameSymbolTool

Renames a symbol across the entire codebase using LSP refactoring:

```python
class RenameSymbolTool(Tool, ToolMarkerSymbolicEdit):
    """Renames a symbol throughout the codebase using language server refactoring."""

    def apply(
        self,
        name_path: str,
        relative_path: str,
        new_name: str,
    ) -> str:
        """
        Renames the symbol with the given `name_path` to `new_name` throughout
        the entire codebase.

        Note: For languages with method overloading (like Java), name_path may
        have to include a method's signature to uniquely identify a method.

        :param name_path: Symbol to rename
        :param relative_path: File containing the symbol
        :param new_name: New name for the symbol
        :return: Result summary indicating success or failure
        """
        code_editor = self.create_code_editor()
        status_message = code_editor.rename_symbol(
            name_path, relative_file_path=relative_path, new_name=new_name
        )
        return status_message
```

#### 4.3.5 Additional File Tools

**File:** `src/serena/tools/file_tools.py`

| Tool | Purpose | Parameters |
|------|---------|------------|
| `DeleteLinesTool` | Delete line range | `relative_path`, `start_line`, `end_line` |
| `ReplaceLinesTool` | Replace line range | `relative_path`, `start_line`, `end_line`, `content` |
| `InsertAtLineTool` | Insert at line | `relative_path`, `line`, `content` |
| `SearchForPatternTool` | Advanced pattern search | `substring_pattern`, `context_lines_*`, `paths_*_glob` |

**SearchForPatternTool** - Full-featured pattern search:

```python
def apply(
    self,
    substring_pattern: str,           # Regex pattern (DOTALL enabled)
    context_lines_before: int = 0,
    context_lines_after: int = 0,
    paths_include_glob: str = "",     # e.g., "src/**/*.ts"
    paths_exclude_glob: str = "",     # e.g., "*test*"
    relative_path: str = "",          # Restrict to directory
    restrict_search_to_code_files: bool = False,  # Only LS-supported files
    max_answer_chars: int = -1
) -> str:
    """
    Flexible search for arbitrary patterns in the codebase.

    Pattern Matching:
    - DOTALL enabled (dot matches newlines)
    - Never use .* at beginning/end (matches entire file)
    - Use non-greedy .*? for complex patterns

    File Selection:
    - paths_include_glob: Include only matching files
    - paths_exclude_glob: Exclude matching files (takes precedence)
    - restrict_search_to_code_files: Only search LS-analyzed files
    """
```

### 4.4 CodeEditor Architecture

**File:** `src/serena/code_editor.py`

The `CodeEditor` is an **abstract base class** that provides symbol-based editing operations. It bridges the gap between LSP symbol information and actual file modifications.

#### Class Hierarchy

```
┌────────────────────────────────────────┐
│ CodeEditor (ABC)                       │
│ - replace_body()                       │
│ - insert_after_symbol()                │
│ - insert_before_symbol()               │
│ - insert_at_line()                     │
│ - delete_lines()                       │
└────────────┬───────────────────────────┘
             │
    ┌────────┴────────┐
    │                 │
┌───▼─────────┐  ┌───▼─────────────┐
│ LSPCodeEditor│  │ JetBrainsEditor │
│ (LS-based)   │  │ (IDE plugin)    │
└─────────────┘  └─────────────────┘
```

#### Core Implementation

```python
class CodeEditor(Generic[TSymbol], ABC):
    def __init__(self, project_root: str, agent: Optional["SerenaAgent"] = None):
        self.project_root = project_root
        self.agent = agent
        self.encoding = self._get_encoding_from_project()

    class EditedFile(ABC):
        """Abstract file handle for editing operations"""
        @abstractmethod
        def get_contents(self) -> str:
            """Get current file contents"""

        @abstractmethod
        def delete_text_between_positions(self, start_pos: PositionInFile, end_pos: PositionInFile):
            """Delete text between two positions"""

        @abstractmethod
        def insert_text_at_position(self, pos: PositionInFile, text: str):
            """Insert text at position"""

    @contextmanager
    def _edited_file_context(self, relative_path: str) -> Iterator["CodeEditor.EditedFile"]:
        """Context manager for editing a file with auto-save"""
        with self._open_file_context(relative_path) as edited_file:
            yield edited_file
            # Auto-save on context exit
            abs_path = os.path.join(self.project_root, relative_path)
            with open(abs_path, "w", encoding=self.encoding) as f:
                f.write(edited_file.get_contents())

    @abstractmethod
    def _find_unique_symbol(self, name_path: str, relative_file_path: str) -> TSymbol:
        """Find the unique symbol with given name in file"""

    def replace_body(self, name_path: str, relative_file_path: str, body: str):
        """Replace entire symbol body"""
        symbol = self._find_unique_symbol(name_path, relative_file_path)
        start_pos = symbol.get_body_start_position_or_raise()
        end_pos = symbol.get_body_end_position_or_raise()

        with self._edited_file_context(relative_file_path) as edited_file:
            # Strip whitespace to avoid extra newlines
            body = body.strip()
            edited_file.delete_text_between_positions(start_pos, end_pos)
            edited_file.insert_text_at_position(start_pos, body)

    def insert_after_symbol(self, name_path: str, relative_file_path: str, body: str):
        """Insert code after symbol end"""
        symbol = self._find_unique_symbol(name_path, relative_file_path)

        # Ensure body ends with newline
        if not body.endswith("\n"):
            body += "\n"

        pos = symbol.get_body_end_position_or_raise()

        # Start at beginning of next line
        line = pos.line + 1
        col = 0

        # Handle proper spacing based on symbol type
        # (functions/classes need blank line separation)
        if symbol.is_neighbouring_definition_separated_by_empty_line():
            original_leading_newlines = self._count_leading_newlines(body)
            body = body.lstrip("\r\n")
            num_leading_empty_lines = max(1, original_leading_newlines)
            body = ("\n" * num_leading_empty_lines) + body

        body = body.rstrip("\r\n") + "\n"

        with self._edited_file_context(relative_file_path) as edited_file:
            edited_file.insert_text_at_position(PositionInFile(line, col), body)

    def insert_before_symbol(self, name_path: str, relative_file_path: str, body: str):
        """Insert code before symbol start"""
        symbol = self._find_unique_symbol(name_path, relative_file_path)
        symbol_start_pos = symbol.get_body_start_position_or_raise()

        # Insert at start of symbol's line
        line = symbol_start_pos.line
        col = 0

        # Handle trailing empty lines
        body = body.rstrip() + "\n"
        if symbol.is_neighbouring_definition_separated_by_empty_line():
            body += "\n"  # Add blank line before symbol

        with self._edited_file_context(relative_file_path) as edited_file:
            edited_file.insert_text_at_position(PositionInFile(line=line, col=col), body)

    def insert_at_line(self, relative_path: str, line: int, content: str):
        """Line-based insertion"""
        with self._edited_file_context(relative_path) as edited_file:
            edited_file.insert_text_at_position(PositionInFile(line, 0), content)

    def delete_lines(self, relative_path: str, start_line: int, end_line: int):
        """Delete line range (inclusive)"""
        with self._edited_file_context(relative_path) as edited_file:
            start_pos = PositionInFile(start_line, 0)
            end_pos = PositionInFile(end_line + 1, 0)  # Include the newline
            edited_file.delete_text_between_positions(start_pos, end_pos)
```

#### Symbol-Aware Spacing

The CodeEditor automatically handles proper spacing based on symbol types:

```python
def is_neighbouring_definition_separated_by_empty_line(self) -> bool:
    """Check if symbol type requires blank line separation"""
    return self.symbol_kind in (
        SymbolKind.Function,
        SymbolKind.Method,
        SymbolKind.Class,
        SymbolKind.Interface,
        SymbolKind.Struct
    )
```

### 4.5 Additional LSP Operations

**File:** `src/solidlsp/ls.py`

Beyond basic symbol search, Serena supports additional LSP operations:

#### 4.5.1 Diagnostics

Get errors, warnings, and hints for a file:

```python
def request_text_document_diagnostics(self, relative_file_path: str) -> list[Diagnostic]:
    """
    Get diagnostics (errors, warnings) for a file.
    Uses textDocument/diagnostic LSP request.

    :param relative_file_path: File to check
    :return: List of diagnostics with severity, message, location
    """
    if not self.server_started:
        raise SolidLSPException("Language Server not started")

    with self.open_file(relative_file_path):
        response = self.server.send.text_document_diagnostic({
            "textDocument": {
                "uri": pathlib.Path(
                    os.path.join(self.repository_root_path, relative_file_path)
                ).as_uri()
            }
        })

    if response is None:
        return []

    return [
        Diagnostic(
            uri=...,
            severity=item["severity"],   # 1=Error, 2=Warning, 3=Info, 4=Hint
            message=item["message"],
            range=item["range"],
            code=item.get("code")
        )
        for item in response["items"]
    ]
```

**Diagnostic Severity Levels:**

| Value | Level | Use Case |
|-------|-------|----------|
| 1 | Error | Compilation errors, type mismatches |
| 2 | Warning | Unused variables, deprecated usage |
| 3 | Information | Style suggestions |
| 4 | Hint | Refactoring opportunities |

#### 4.5.2 Code Completions

Get code completions at a cursor position:

```python
def request_completions(
    self,
    relative_file_path: str,
    line: int,
    column: int,
    allow_incomplete: bool = False
) -> list[CompletionItem]:
    """
    Get code completions at the given position.
    Uses textDocument/completion LSP request.

    :param relative_file_path: File to complete in
    :param line: Line number (0-based)
    :param column: Column number (0-based)
    :param allow_incomplete: Accept incomplete results
    :return: List of completion items
    """
    with self.open_file(relative_file_path) as buffer:
        completion_params = {
            "position": {"line": line, "character": column},
            "textDocument": {"uri": buffer.uri},
            "context": {"triggerKind": CompletionTriggerKind.Invoked}
        }

        # Retry until complete or max retries
        num_retries = 0
        response = None
        while response is None or (response.get("isIncomplete") and num_retries < 30):
            self.completions_available.wait()
            response = self.server.send.completion(completion_params)
            if isinstance(response, list):
                response = {"items": response, "isIncomplete": False}
            num_retries += 1

        if response is None:
            return []

        # Filter out keywords, return meaningful completions
        items = [
            item for item in response.get("items", [])
            if item.get("kind") != CompletionItemKind.Keyword
        ]

        return [
            CompletionItem(
                label=item.get("label"),
                kind=item.get("kind"),
                detail=item.get("detail"),
                insertText=item.get("insertText") or item.get("label")
            )
            for item in items
        ]
```

**Completion Item Kinds:**

| Kind | Value | Example |
|------|-------|---------|
| Text | 1 | Plain text |
| Method | 2 | `object.method()` |
| Function | 3 | `function()` |
| Constructor | 4 | `new Class()` |
| Field | 5 | `object.field` |
| Variable | 6 | Local variables |
| Class | 7 | Class names |
| Interface | 8 | Interface names |
| Module | 9 | Module imports |
| Property | 10 | Properties |
| Snippet | 15 | Code templates |

#### 4.5.3 Rename Refactoring

Rename a symbol across the entire workspace:

```python
def request_rename(
    self,
    relative_file_path: str,
    line: int,
    column: int,
    new_name: str
) -> WorkspaceEdit | None:
    """
    Request rename refactoring for symbol at position.
    Uses textDocument/rename LSP request.

    :param relative_file_path: File containing symbol
    :param line: Line number of symbol
    :param column: Column number of symbol
    :param new_name: New name for symbol
    :return: WorkspaceEdit with all changes, or None if not supported
    """
    with self.open_file(relative_file_path):
        response = self.server.send.rename({
            "textDocument": {"uri": ...},
            "position": {"line": line, "character": column},
            "newName": new_name
        })

    if response is None:
        return None

    # Response contains changes for all affected files
    # {
    #   "changes": {
    #     "file:///path/to/file1.py": [...TextEdit],
    #     "file:///path/to/file2.py": [...TextEdit],
    #   }
    # }
    return response
```

### 4.6 NamePathMatcher - Symbol Path Matching

**File:** `src/serena/symbol.py:117-172`

Serena uses a sophisticated **name path system** for symbol addressing.

#### Path Syntax

| Pattern | Meaning | Example Matches |
|---------|---------|-----------------|
| `method` | Any symbol named "method" | `method`, `Class/method`, `A/B/method` |
| `Class/method` | method inside Class (suffix match) | `Class/method`, `Outer/Class/method` |
| `/Class/method` | Exact absolute path | Only `Class/method` |
| `method[1]` | Second overload (0-indexed) | Second method with same name |
| `get*` (substring) | Symbols containing "get" | `getValue`, `getData`, `get_user` |

#### Implementation

```python
class NamePathMatcher(ToStringMixin):
    """Pattern matcher for symbol name paths"""

    def __init__(self, name_path_expr: str, substring_matching: bool):
        """
        :param name_path_expr: Pattern to match (e.g., "Class/method[1]")
        :param substring_matching: Use substring match for last segment
        """
        assert name_path_expr, "name_path must not be empty"
        self._expr = name_path_expr
        self._substring_matching = substring_matching

        # Check for absolute pattern (starts with /)
        self._is_absolute_pattern = name_path_expr.startswith("/")

        # Split into parts
        self._pattern_parts = name_path_expr.lstrip("/").rstrip("/").split("/")

        # Parse overload index "[idx]" if present
        self._overload_idx: int | None = None
        last_part = self._pattern_parts[-1]
        if last_part.endswith("]") and "[" in last_part:
            bracket_idx = last_part.rfind("[")
            index_part = last_part[bracket_idx + 1:-1]
            if index_part.isdigit():
                self._pattern_parts[-1] = last_part[:bracket_idx]
                self._overload_idx = int(index_part)

    def matches_components(
        self,
        symbol_name_path_parts: list[str],
        overload_idx: int | None
    ) -> bool:
        """Check if pattern matches symbol path"""

        # Can't match if pattern has more parts than symbol
        if len(self._pattern_parts) > len(symbol_name_path_parts):
            return False

        # Absolute patterns require exact length match
        if self._is_absolute_pattern:
            if len(self._pattern_parts) != len(symbol_name_path_parts):
                return False

        # Check ancestor parts match (suffix matching)
        # e.g., pattern "A/B" matches symbol "X/A/B" but not "X/C/B"
        if symbol_name_path_parts[-len(self._pattern_parts):-1] != self._pattern_parts[:-1]:
            return False

        # Match last part (exact or substring)
        name_to_match = self._pattern_parts[-1]
        symbol_name = symbol_name_path_parts[-1]

        if self._substring_matching:
            if name_to_match not in symbol_name:
                return False
        else:
            if name_to_match != symbol_name:
                return False

        # Check overload index
        if self._overload_idx is not None:
            if overload_idx != self._overload_idx:
                return False

        return True

    def matches_ls_symbol(self, symbol: "LanguageServerSymbol") -> bool:
        """Convenience method for matching against LanguageServerSymbol"""
        return self.matches_components(
            symbol.get_name_path_parts(),
            symbol.overload_idx
        )
```

#### Usage Examples

```python
# Find any method named "login"
matcher = NamePathMatcher("login", substring_matching=False)

# Find "login" method specifically in "AuthService" class
matcher = NamePathMatcher("AuthService/login", substring_matching=False)

# Find "login" method at exact root level of file (not nested)
matcher = NamePathMatcher("/login", substring_matching=False)

# Find second overload of "process" method
matcher = NamePathMatcher("process[1]", substring_matching=False)

# Find any method containing "get" in its name
matcher = NamePathMatcher("get", substring_matching=True)

# Combine: find "get*" methods in DataService
matcher = NamePathMatcher("DataService/get", substring_matching=True)
```

### 4.7 LanguageServerSymbolRetriever

**File:** `src/serena/symbol.py:470+`

This is the **integration layer** between LanguageServerManager and symbol operations:

```python
class LanguageServerSymbolRetriever:
    """Bridge between LS manager and symbol search/edit operations"""

    def __init__(
        self,
        ls: SolidLanguageServer | LanguageServerManager,
        agent: "SerenaAgent" | None = None
    ):
        """
        :param ls: Language server or manager for symbol retrieval
        :param agent: Agent to notify of file modifications
        """
        if isinstance(ls, SolidLanguageServer):
            self._ls_manager = LanguageServerManager({ls.language: ls})
        else:
            self._ls_manager = ls
        self.agent = agent

    def get_language_server(self, relative_path: str) -> SolidLanguageServer:
        """Get appropriate LS for file type"""
        return self._ls_manager.get_language_server(relative_path)

    def find_by_name(
        self,
        name_path: str,
        include_kinds: Sequence[SymbolKind] | None = None,
        exclude_kinds: Sequence[SymbolKind] | None = None,
        substring_matching: bool = False,
        within_relative_path: str | None = None,
    ) -> list[LanguageServerSymbol]:
        """
        Find symbols by name pattern.

        :param name_path: Pattern to match (see NamePathMatcher)
        :param include_kinds: Only include these symbol kinds
        :param exclude_kinds: Exclude these symbol kinds
        :param substring_matching: Use substring match for name
        :param within_relative_path: Restrict search to path
        """
        matcher = NamePathMatcher(name_path, substring_matching)
        results = []

        # Get files to search
        if within_relative_path:
            files = self._get_files_in_path(within_relative_path)
        else:
            files = self._get_all_source_files()

        # Search each file
        for file_path in files:
            ls = self.get_language_server(file_path)
            symbols = ls.get_document_symbols(file_path)

            for symbol in symbols.iter_all():
                if matcher.matches_ls_symbol(symbol):
                    if include_kinds and symbol.symbol_kind not in include_kinds:
                        continue
                    if exclude_kinds and symbol.symbol_kind in exclude_kinds:
                        continue
                    results.append(symbol)

        return results

    def find_referencing_symbols(
        self,
        name_path: str,
        relative_file_path: str,
        include_body: bool = False,
        include_kinds: Sequence[SymbolKind] | None = None,
        exclude_kinds: Sequence[SymbolKind] | None = None,
    ) -> list[ReferenceInLanguageServerSymbol]:
        """
        Find symbols that reference the given symbol.

        :param name_path: Target symbol to find references for
        :param relative_file_path: File containing target symbol
        :param include_body: Include source code context
        :param include_kinds: Only include these symbol kinds
        :param exclude_kinds: Exclude these symbol kinds
        """
        # First find the target symbol
        target = self._find_unique_symbol(name_path, relative_file_path)

        # Get its position
        pos = target.location

        # Request references from LSP
        ls = self.get_language_server(relative_file_path)
        refs = ls.request_references(relative_file_path, pos.line, pos.column)

        # Convert to ReferenceInSymbol
        results = []
        for ref in refs:
            ref_file = ref.relative_path
            ref_ls = self.get_language_server(ref_file)

            # Find containing symbol for this reference
            containing_symbol = self._find_containing_symbol(
                ref_ls, ref_file, ref.range.start.line, ref.range.start.character
            )

            if include_kinds and containing_symbol.symbol_kind not in include_kinds:
                continue
            if exclude_kinds and containing_symbol.symbol_kind in exclude_kinds:
                continue

            results.append(ReferenceInLanguageServerSymbol(
                symbol=containing_symbol,
                line=ref.range.start.line,
                character=ref.range.start.character
            ))

        return results

    def get_symbol_overview(
        self,
        relative_paths: list[str] | str
    ) -> dict[str, list[LanguageServerSymbol]]:
        """
        Get top-level symbols for files/directories.

        :param relative_paths: Paths to analyze
        :return: Map of file path to top-level symbols
        """
        if isinstance(relative_paths, str):
            relative_paths = [relative_paths]

        results = {}
        for path in relative_paths:
            try:
                ls = self.get_language_server(path)
                symbols = ls.get_document_symbols(path)
                results[path] = symbols.root_symbols
            except Exception as e:
                log.warning(f"Failed to get symbols for {path}: {e}")
                results[path] = []

        return results
```

---

## 5. Implementation Insights for codex-rs

### 5.1 Key Architectural Decisions

#### 1. **Separate LSP Layer from Tool Layer**

Serena cleanly separates:
- `solidlsp/` - Pure LSP client library (reusable)
- `serena/tools/` - Agent-facing tools that consume LSP

**Recommendation for codex-rs:**
```
codex-rs/
├── lsp/           # Pure LSP client (like solidlsp)
│   ├── handler.rs # JSON-RPC communication
│   ├── server.rs  # Language server lifecycle
│   └── types.rs   # LSP protocol types
└── tools/         # Agent tools
    └── lsp_tools.rs  # find_symbol, find_references, etc.
```

#### 2. **Language Server Registry Pattern**

**File:** `src/solidlsp/ls_config.py:29-85`

```python
class Language(str, Enum):
    PYTHON = "python"
    RUST = "rust"
    TYPESCRIPT = "typescript"
    # ...

    def get_ls_class(self) -> Type[SolidLanguageServer]:
        """Factory method to get language-specific LS class"""
        match self:
            case Language.PYTHON:
                from .language_servers.pyright_server import PyrightServer
                return PyrightServer
            case Language.RUST:
                from .language_servers.rust_analyzer import RustAnalyzer
                return RustAnalyzer
            # ...
```

**Recommendation:** Use similar enum-based registry in Rust:

```rust
// codex-rs/lsp/src/language.rs
pub enum Language {
    Python,
    Rust,
    TypeScript,
    // ...
}

impl Language {
    pub fn create_server(&self, config: &LspConfig) -> Result<Box<dyn LanguageServer>> {
        match self {
            Language::Python => Ok(Box::new(PyrightServer::new(config)?)),
            Language::Rust => Ok(Box::new(RustAnalyzer::new(config)?)),
            Language::TypeScript => Ok(Box::new(TypeScriptLs::new(config)?)),
            // ...
        }
    }
}
```

#### 3. **Caching Strategy**

Serena uses **two-level caching**:

1. **Raw symbol cache** - LSP server's raw response
2. **Processed symbol cache** - Parsed/unified format

**File:** `src/solidlsp/ls.py:276-291`

```python
# Cache directory structure
.serena/cache/
├── python/
│   ├── raw_document_symbols.pkl     # Raw LSP responses
│   └── document_symbols.pkl         # Parsed symbols
└── typescript/
    ├── raw_document_symbols.pkl
    └── document_symbols.pkl

# Cache validation
self._document_symbols_cache: dict[str, tuple[str, DocumentSymbols]] = {}
#                                          ^^^^ content_hash for invalidation
```

**Recommendation for codex-rs:**
```rust
// codex-rs/lsp/src/cache.rs
pub struct SymbolCache {
    cache_dir: PathBuf,
    raw_cache: HashMap<String, (String, Vec<DocumentSymbol>)>, // (hash, symbols)
    processed_cache: HashMap<String, (String, ProcessedSymbols)>,
}

impl SymbolCache {
    pub fn get_or_fetch<F>(
        &mut self,
        file_path: &str,
        fetcher: F
    ) -> Result<ProcessedSymbols>
    where
        F: FnOnce() -> Result<Vec<DocumentSymbol>>
    {
        let file_hash = self.compute_hash(file_path)?;

        if let Some((cached_hash, symbols)) = self.processed_cache.get(file_path) {
            if cached_hash == &file_hash {
                return Ok(symbols.clone());
            }
        }

        // Cache miss or invalidated - fetch fresh
        let raw_symbols = fetcher()?;
        let processed = self.process_symbols(raw_symbols);

        self.processed_cache.insert(file_path.to_string(), (file_hash, processed.clone()));
        Ok(processed)
    }
}
```

#### 4. **Async vs Sync**

Serena uses **synchronous LSP calls with threading**:
- Main thread sends requests
- Background thread reads responses
- Blocking `.get_result(timeout)` to wait for response

**File:** `src/solidlsp/ls_handler.py:87-92`

```python
def get_result(self, timeout: float | None = None) -> Result:
    try:
        return self._result_queue.get(timeout=timeout)
    except Empty as e:
        raise TimeoutError(f"Request timed out ({timeout=})") from e
```

**Recommendation for codex-rs:** Use async/await (already in place):

```rust
// codex-rs/lsp/src/handler.rs
pub struct LspHandler {
    pending_requests: Arc<Mutex<HashMap<i64, oneshot::Sender<JsonValue>>>>,
}

impl LspHandler {
    pub async fn send_request(&self, method: &str, params: JsonValue) -> Result<JsonValue> {
        let id = self.next_id();
        let (tx, rx) = oneshot::channel();

        self.pending_requests.lock().unwrap().insert(id, tx);

        // Send request
        self.write_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        })).await?;

        // Wait for response with timeout
        timeout(Duration::from_secs(30), rx).await??
    }
}
```

### 5.2 Tool Design Patterns

#### Pattern 1: Progressive Disclosure

Serena's tools follow a **"narrow → broad"** information flow:

1. `get_symbols_overview` - High-level file structure
2. `find_symbol` - Specific symbol details
3. `find_referencing_symbols` - Cross-file relationships

**Example Agent Workflow:**
```
Agent: I need to understand auth.py
→ Call get_symbols_overview(relative_path="auth.py")
← Returns: [AuthService (class), login (function), logout (function)]

Agent: I need to modify AuthService
→ Call find_symbol(name_path="AuthService", depth=1, include_body=False)
← Returns: AuthService class with method signatures

Agent: Where is AuthService.login called?
→ Call find_referencing_symbols(name_path="AuthService/login", relative_path="auth.py")
← Returns: List of call sites with code context
```

**Recommendation:** Implement similar progressive tools in codex-rs.

#### Pattern 2: Context-Aware Limits

All search tools have `max_answer_chars` parameter:

```python
def apply(self, ..., max_answer_chars: int = -1) -> str:
    result = self._to_json(...)
    return self._limit_length(result, max_answer_chars)

def _limit_length(self, result: str, max_chars: int) -> str:
    if max_chars == -1:
        max_chars = self.config.default_max_answer_chars

    if len(result) > max_chars:
        return json.dumps({
            "error": "Result too large",
            "size": len(result),
            "limit": max_chars,
            "hint": "Narrow your search or increase max_answer_chars"
        })
    return result
```

**Recommendation:** Add similar limits to codex-rs tools to prevent token overflow.

### 5.3 Error Handling Strategies

#### 1. Language Server Crashes

**File:** `src/solidlsp/ls_handler.py:41-53`

```python
class LanguageServerTerminatedException(Exception):
    def __init__(self, message: str, language: Language, cause: Exception | None = None):
        self.message = message
        self.language = language
        self.cause = cause
```

Serena **automatically restarts** crashed language servers (agent tool):

```python
class RestartLanguageServerTool(Tool):
    def apply(self) -> str:
        """Restart the language server (use on crash or hang)"""
        self.agent.reset_language_server_manager()
        return SUCCESS_RESULT
```

**Recommendation:** Implement similar auto-recovery in codex-rs.

#### 2. Timeout Handling

Every LSP request has configurable timeout:

```python
self.server.set_request_timeout(timeout=30.0)  # Global default

# Per-request override
result = self.server.send.definition(params)  # Uses global timeout
```

On timeout:
```python
raise TimeoutError(f"Request timed out ({timeout=})") from e
```

**Recommendation:** Use `tokio::time::timeout` in codex-rs:

```rust
pub async fn request_definition(&self, params: DefinitionParams) -> Result<Vec<Location>> {
    timeout(
        Duration::from_secs(30),
        self.handler.send_request("textDocument/definition", serde_json::to_value(params)?)
    )
    .await
    .map_err(|_| CodexErr::Fatal("LSP request timed out".into()))??
}
```

#### 3. Partial Failures

When some files fail to parse, Serena continues with partial results:

```python
def get_symbol_overview(self, relative_paths: list[str]) -> dict[str, list[Symbol]]:
    results = {}
    for path in relative_paths:
        try:
            results[path] = self._get_symbols_for_file(path)
        except Exception as e:
            log.warning(f"Failed to get symbols for {path}: {e}")
            results[path] = []  # Empty list, not error
    return results
```

**Recommendation:** Return partial results rather than failing completely.

### 5.4 Multi-Language Support

#### Language Detection

**File:** `src/solidlsp/ls_config.py:101-150`

```python
def get_source_fn_matcher(self) -> FilenameMatcher:
    """Get file extension matcher for language"""
    match self:
        case Language.PYTHON:
            return FilenameMatcher("*.py", "*.pyi")
        case Language.RUST:
            return FilenameMatcher("*.rs")
        case Language.TYPESCRIPT:
            return FilenameMatcher("*.ts", "*.tsx", "*.js", "*.jsx", "*.mts", "*.cts")
        # ...
```

#### Project Language Detection

Serena auto-detects project languages:

```python
def detect_languages(project_root: str) -> list[Language]:
    """Scan project root for language markers"""
    detected = []

    # Check for known config files
    if os.path.exists(os.path.join(project_root, "Cargo.toml")):
        detected.append(Language.RUST)
    if os.path.exists(os.path.join(project_root, "package.json")):
        detected.append(Language.TYPESCRIPT)
    if os.path.exists(os.path.join(project_root, "pyproject.toml")):
        detected.append(Language.PYTHON)
    # ...

    # Fallback: Scan file extensions
    if not detected:
        detected = scan_for_source_files(project_root)

    return detected
```

**Recommendation for codex-rs:**

```rust
// codex-rs/lsp/src/detection.rs
pub fn detect_languages(project_root: &Path) -> Vec<Language> {
    let mut languages = Vec::new();

    // Marker files
    if project_root.join("Cargo.toml").exists() {
        languages.push(Language::Rust);
    }
    if project_root.join("package.json").exists() {
        languages.push(Language::TypeScript);
    }

    // Fallback: walk directory
    if languages.is_empty() {
        languages = scan_source_files(project_root);
    }

    languages
}
```

---

## 6. Critical Implementation Details

### 6.1 LSP Initialization Parameters

Each language server requires specific initialization parameters:

**Python (Pyright):**
```python
{
    "rootUri": "file:///path/to/project",
    "initializationOptions": {
        "exclude": ["**/__pycache__", "**/.venv"],
        "reportMissingImports": "error"
    },
    "capabilities": {
        "workspace": {
            "didChangeConfiguration": {"dynamicRegistration": True},
            "didChangeWatchedFiles": {"dynamicRegistration": True}
        },
        "textDocument": {
            "documentSymbol": {
                "hierarchicalDocumentSymbolSupport": True,
                "symbolKind": {"valueSet": list(range(1, 27))}
            }
        }
    }
}
```

**TypeScript:**
```python
{
    "rootUri": "file:///path/to/project",
    "capabilities": {
        "textDocument": {
            "documentSymbol": {
                "hierarchicalDocumentSymbolSupport": True
            }
        }
    }
}
```

**Rust (rust-analyzer):**
```python
{
    "rootUri": "file:///path/to/project",
    "initializationOptions": {
        "checkOnSave": {"command": "clippy"},
        "cargo": {"features": "all"}
    }
}
```

### 6.2 Symbol Name Path System

Serena uses a **hierarchical name path system** for symbol addressing:

**Format:** `Parent/Child/Grandchild[overload_index]`

Examples:
- `MyClass` - Top-level class
- `MyClass/my_method` - Method in class
- `MyClass/my_method[1]` - Second overload of method (0-indexed)
- `/MyClass/my_method` - Absolute path (requires exact match)

**Implementation:** `src/serena/symbol.py`

```python
class SymbolNamePath:
    def __init__(self, path: str):
        self.is_absolute = path.startswith("/")
        self.parts = path.lstrip("/").split("/")
        self.overload_index = None

        # Parse overload index: "method[1]"
        if "[" in self.parts[-1]:
            name, index = self.parts[-1].split("[")
            self.parts[-1] = name
            self.overload_index = int(index.rstrip("]"))

    def matches(self, symbol_path: str, substring: bool = False) -> bool:
        """Check if this pattern matches a symbol path"""
        symbol_parts = symbol_path.split("/")

        if self.is_absolute:
            # Exact match required
            return symbol_parts == self.parts
        else:
            # Suffix match
            if substring:
                # Last part can be substring
                if not symbol_parts[-1].endswith(self.parts[-1]):
                    return False
                return symbol_parts[-len(self.parts):-1] == self.parts[:-1]
            else:
                return symbol_parts[-len(self.parts):] == self.parts
```

### 6.3 Symbol Kind Filtering

Serena exposes LSP SymbolKind as integers to agents:

**File:** `src/solidlsp/ls_types.py`

```python
class SymbolKind(IntEnum):
    File = 1
    Module = 2
    Namespace = 3
    Package = 4
    Class = 5
    Method = 6
    Property = 7
    Field = 8
    Constructor = 9
    Enum = 10
    Interface = 11
    Function = 12
    Variable = 13
    Constant = 14
    String = 15
    Number = 16
    Boolean = 17
    Array = 18
    Object = 19
    Key = 20
    Null = 21
    EnumMember = 22
    Struct = 23
    Event = 24
    Operator = 25
    TypeParameter = 26
```

**Usage in tools:**
```python
# Find all classes in a file
find_symbol(..., include_kinds=[5])

# Find functions and methods
find_symbol(..., include_kinds=[6, 12])

# Find everything except variables
find_symbol(..., exclude_kinds=[13])
```

**Recommendation:** Expose same SymbolKind enum in codex-rs tools.

### 6.4 Reference Context Formatting

When finding references, Serena includes surrounding code:

```python
content_around_ref = self.project.retrieve_content_around_line(
    relative_file_path=ref.symbol.location.relative_path,
    line=ref.line,
    context_lines_before=1,
    context_lines_after=1
)

# Output format:
"""
  > 125: result = auth_service.login(username, password)
... 126: if result.success:
... 127:     redirect_to_dashboard()
"""
```

This gives agents enough context to understand how symbols are used.

---

## 7. Performance Optimizations

### 7.1 Parallel Processing

Serena uses **joblib** for parallel file searching:

```python
from joblib import Parallel, delayed

results = Parallel(n_jobs=max_workers)(
    delayed(_search_single_file)(file_path, patterns, ...)
    for file_path in all_files
)
```

**Recommendation:** Use Rayon in codex-rs:

```rust
use rayon::prelude::*;

let results: Vec<_> = files
    .par_iter()
    .filter_map(|file| search_file(file, pattern))
    .collect();
```

### 7.2 Lazy Loading

Language servers are only started when needed:

```python
class LanguageServerManager:
    def __init__(self):
        self._servers: dict[Language, SolidLanguageServer | None] = {}

    def get_server(self, language: Language) -> SolidLanguageServer:
        """Lazy-load language server on first use"""
        if language not in self._servers or self._servers[language] is None:
            self._servers[language] = self._create_server(language)
        return self._servers[language]
```

### 7.3 Symbol Cache Persistence

Caches are saved to disk to speed up subsequent sessions:

```python
def _save_document_symbols_cache(self):
    """Persist symbol cache to disk"""
    if not self._document_symbols_cache_is_modified:
        return

    cache_file = self.cache_dir / "document_symbols.pkl"
    save_cache(cache_file, {
        "version": self.DOCUMENT_SYMBOL_CACHE_VERSION,
        "cache": self._document_symbols_cache
    })
```

**Recommendation:** Use serde + bincode for Rust cache serialization.

---

## 8. Recommendations for codex-rs

### 8.1 High-Priority Features (CRITICAL)

1. **LSP Client Library** (`codex-rs/lsp/`)
   - JSON-RPC 2.0 handler with async/await
   - Process lifecycle management (start, shutdown, restart)
   - Language-specific server implementations (Rust, Python, TypeScript)
   - Symbol caching with content hash validation

2. **LanguageServerManager** (`codex-rs/lsp/src/manager.rs`) ← NEW
   - Multi-language server orchestration
   - Parallel server startup
   - Auto-restart crashed servers
   - Language routing by file extension
   - Cache persistence coordination

3. **Core LSP Tools** (`codex-rs/core/src/tools/lsp_tools_ext.rs`)
   - `get_symbols_overview` - File symbol outline
   - `find_symbol` - Symbol search by name/path (with NamePathMatcher)
   - `find_references` - Cross-file reference finding
   - `get_definition` - Jump to definition

4. **CodeEditor** (`codex-rs/lsp/src/editor.rs`) ← NEW
   - Symbol-based editing operations
   - `replace_body()` - Replace symbol definition
   - `insert_after_symbol()` - Insert code after symbol
   - `insert_before_symbol()` - Insert code before symbol
   - Symbol-aware spacing (blank lines between functions/classes)

5. **Symbol Editing Tools** (`codex-rs/core/src/tools/lsp_tools_ext.rs`) ← NEW
   - `replace_symbol_body` - Replace entire symbol
   - `insert_after_symbol` - Add new code after symbol
   - `insert_before_symbol` - Add code before symbol (imports)
   - `rename_symbol` - LSP-based rename refactoring

6. **File Synchronization**
   - In-memory buffer management
   - `textDocument/didOpen`, `didChange`, `didClose` notifications
   - Incremental text change protocol

### 8.2 Medium-Priority Features

7. **Runtime Dependency System**
   - Auto-download language server binaries
   - Platform detection (Windows, macOS, Linux x64/ARM)
   - Version pinning and upgrade logic

8. **Advanced Search**
   - Multi-file text search with context (glob filtering)
   - Symbol kind filtering
   - Overload disambiguation (`method[1]`)
   - NamePathMatcher for pattern matching

9. **Caching Infrastructure**
   - Two-level symbol cache (raw + processed)
   - Content hash-based invalidation
   - Disk persistence

10. **Error Recovery**
    - Auto-restart on language server crash
    - Graceful degradation when LSP unavailable
    - Timeout handling with retry logic
    - Process tree cleanup (psutil equivalent)

### 8.3 Low-Priority Features

11. **Additional LSP Operations**
    - `request_diagnostics` - Get errors/warnings
    - `request_completions` - Code completions
    - `request_rename` - Workspace rename refactoring
    - Code actions (quick fixes)
    - Hover information
    - Signature help

12. **Performance Optimizations**
    - Parallel file scanning (rayon)
    - Lazy language server initialization
    - Request batching

13. **Multi-Language Support**
    - Auto-detect project languages
    - Support 5+ languages (Rust, Python, TypeScript, Go, C++)

### 8.4 Integration with codex-rs (Updated)

#### Phase 1: Core LSP Infrastructure

```rust
// codex-rs/lsp/src/lib.rs
pub struct LspServer {
    handler: LspHandler,
    language: Language,
    root_path: PathBuf,
    open_buffers: HashMap<Url, FileBuffer>,
}

impl LspServer {
    pub async fn new(language: Language, root_path: PathBuf) -> Result<Self>;
    pub async fn shutdown(&mut self) -> Result<()>;

    // Core operations
    pub async fn get_document_symbols(&self, path: &str) -> Result<Vec<Symbol>>;
    pub async fn find_definition(&self, path: &str, pos: Position) -> Result<Vec<Location>>;
    pub async fn find_references(&self, path: &str, pos: Position) -> Result<Vec<Location>>;

    // File sync
    pub async fn open_file(&mut self, path: &str) -> Result<()>;
    pub async fn close_file(&mut self, path: &str) -> Result<()>;
    pub async fn update_file(&mut self, path: &str, changes: Vec<TextChange>) -> Result<()>;
}
```

#### Phase 2: LanguageServerManager (NEW)

```rust
// codex-rs/lsp/src/manager.rs
pub struct LspManager {
    servers: HashMap<Language, Arc<RwLock<LspServer>>>,
    factory: LspServerFactory,
    default_language: Language,
}

impl LspManager {
    /// Start multiple language servers in parallel
    pub async fn new(languages: Vec<Language>, project_root: PathBuf) -> Result<Self> {
        let handles: Vec<_> = languages.iter()
            .map(|lang| tokio::spawn(LspServer::new(*lang, project_root.clone())))
            .collect();

        let servers = futures::future::try_join_all(handles).await?;
        // ...
    }

    /// Get server for file, auto-restart if crashed
    pub async fn get_server(&self, file_path: &str) -> Result<Arc<RwLock<LspServer>>> {
        let lang = self.detect_language(file_path)?;
        let server = self.servers.get(&lang).unwrap();

        // Auto-restart if not running
        {
            let server_lock = server.read().await;
            if !server_lock.is_running() {
                drop(server_lock);
                let mut server_write = server.write().await;
                *server_write = self.factory.create(lang, &self.project_root).await?;
            }
        }

        Ok(Arc::clone(server))
    }

    /// Save caches for all servers
    pub async fn save_all_caches(&self) -> Result<()>;

    /// Stop all servers
    pub async fn stop_all(&self) -> Result<()>;
}
```

#### Phase 3: Tools Integration + CodeEditor (NEW)

```rust
// codex-rs/lsp/src/editor.rs - Symbol-based editing
pub struct CodeEditor {
    lsp_manager: Arc<LspManager>,
    project_root: PathBuf,
}

impl CodeEditor {
    pub async fn replace_body(
        &self,
        name_path: &str,
        relative_path: &str,
        body: &str
    ) -> Result<()> {
        let symbol = self.find_unique_symbol(name_path, relative_path).await?;
        let start = symbol.body_start_position();
        let end = symbol.body_end_position();

        // Read file, apply edit, write back
        let content = self.read_file(relative_path)?;
        let new_content = self.apply_edit(&content, start, end, body.trim());
        self.write_file(relative_path, &new_content)?;

        // Sync with LSP
        let server = self.lsp_manager.get_server(relative_path).await?;
        server.write().await.update_file(relative_path, vec![...]).await?;

        Ok(())
    }

    pub async fn insert_after_symbol(...) -> Result<()>;
    pub async fn insert_before_symbol(...) -> Result<()>;
    pub async fn rename_symbol(...) -> Result<WorkspaceEdit>;
}

// codex-rs/core/src/tools/lsp_tools_ext.rs
pub struct ReplaceSymbolBodyTool;
impl Tool for ReplaceSymbolBodyTool {
    async fn apply(&self, ctx: &mut ToolContext, args: Value) -> Result<String> {
        let args: ReplaceSymbolBodyArgs = serde_json::from_value(args)?;
        let editor = ctx.code_editor()?;
        editor.replace_body(&args.name_path, &args.relative_path, &args.body).await?;
        Ok("OK".to_string())
    }
}

// Register in core/src/tools/spec.rs (minimal integration)
pub fn build_specs() -> Vec<ToolSpec> {
    vec![
        // ... existing tools
        // Search tools
        get_symbols_overview_tool_spec(),
        find_symbol_tool_spec(),
        find_references_tool_spec(),
        // Editing tools (NEW)
        replace_symbol_body_tool_spec(),
        insert_after_symbol_tool_spec(),
        insert_before_symbol_tool_spec(),
        rename_symbol_tool_spec(),
    ]
}
```

#### Phase 4: Name Path Matching (NEW)

```rust
// codex-rs/lsp/src/name_path.rs
pub struct NamePathMatcher {
    parts: Vec<String>,
    is_absolute: bool,
    overload_idx: Option<usize>,
    substring_matching: bool,
}

impl NamePathMatcher {
    /// Parse pattern like "Class/method[1]"
    pub fn new(pattern: &str, substring_matching: bool) -> Self {
        let is_absolute = pattern.starts_with('/');
        let trimmed = pattern.trim_matches('/');
        let mut parts: Vec<String> = trimmed.split('/').map(|s| s.to_string()).collect();

        // Parse overload index from last part
        let mut overload_idx = None;
        if let Some(last) = parts.last_mut() {
            if let Some(bracket_pos) = last.find('[') {
                if last.ends_with(']') {
                    let idx_str = &last[bracket_pos + 1..last.len() - 1];
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        overload_idx = Some(idx);
                        *last = last[..bracket_pos].to_string();
                    }
                }
            }
        }

        Self { parts, is_absolute, overload_idx, substring_matching }
    }

    pub fn matches(&self, symbol_path: &[String], symbol_overload: Option<usize>) -> bool {
        // ... matching logic (see section 4.6)
    }
}
```

#### Phase 5: Caching & Runtime Deps

```rust
// codex-rs/lsp/src/cache.rs
pub struct SymbolCache {
    cache_dir: PathBuf,
    symbols: HashMap<String, (String, Vec<ProcessedSymbol>)>, // (hash, symbols)
}

impl SymbolCache {
    pub fn get_or_fetch<F>(&mut self, path: &str, fetcher: F) -> Result<Vec<ProcessedSymbol>>
    where F: FnOnce() -> Result<Vec<DocumentSymbol>>;

    pub fn save_to_disk(&self) -> Result<()>;
    pub fn load_from_disk(&mut self) -> Result<()>;
}

// codex-rs/lsp/src/runtime.rs
pub struct RuntimeDependency {
    id: String,
    url: Option<String>,
    command: Option<Vec<String>>,
    binary_name: Option<String>,
}

impl RuntimeDependency {
    pub async fn install(&self, target_dir: &Path) -> Result<PathBuf>;
}
```

### 8.5 Configuration Example

```toml
# codex-rs/core/src/config/mod.rs - add LSP config
[lsp]
enabled = true
timeout_secs = 30
cache_dir = "~/.cache/codex/lsp"

[[lsp.servers]]
language = "rust"
command = "rust-analyzer"
args = []

[[lsp.servers]]
language = "python"
command = "python"
args = ["-m", "pyright.langserver", "--stdio"]
auto_install = true
install_command = ["pip", "install", "pyright"]

[[lsp.servers]]
language = "typescript"
binary_path = "~/.cache/codex/lsp/TypeScriptLanguageServer/ts-lsp/node_modules/.bin/typescript-language-server"
args = ["--stdio"]
auto_install = true
install_url = "https://registry.npmjs.org/typescript-language-server/-/typescript-language-server-4.3.3.tgz"
```

---

## 9. Code Examples for codex-rs

### 9.1 Basic LSP Client

```rust
// codex-rs/lsp/src/handler.rs
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub struct LspHandler {
    process: Child,
    pending_requests: Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>,
    request_id: Arc<Mutex<i64>>,
    shutdown_tx: mpsc::Sender<()>,
}

impl LspHandler {
    pub async fn start(command: &str, args: &[&str], cwd: &Path) -> Result<Self> {
        let mut process = Command::new(command)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();
        let stderr = process.stderr.take().unwrap();

        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Spawn reader task
        let pending_clone = pending_requests.clone();
        tokio::spawn(Self::read_responses(stdout, pending_clone, shutdown_rx));

        // Spawn stderr logger
        tokio::spawn(Self::log_stderr(stderr));

        Ok(Self {
            process,
            pending_requests,
            request_id: Arc::new(Mutex::new(1)),
            shutdown_tx,
        })
    }

    async fn read_responses(
        stdout: ChildStdout,
        pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>,
        mut shutdown_rx: mpsc::Receiver<()>
    ) {
        let mut reader = BufReader::new(stdout);
        let mut content_length = 0;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                line = reader.read_line(&mut String::new()) => {
                    let line = match line {
                        Ok(0) => break, // EOF
                        Ok(_) => line.unwrap(),
                        Err(e) => {
                            log::error!("Error reading from LSP: {}", e);
                            break;
                        }
                    };

                    // Parse Content-Length header
                    if line.starts_with("Content-Length:") {
                        content_length = line.split(":").nth(1).unwrap().trim().parse().unwrap();
                    } else if line == "\r\n" && content_length > 0 {
                        // Read JSON body
                        let mut body = vec![0u8; content_length];
                        reader.read_exact(&mut body).await.unwrap();

                        let msg: Value = serde_json::from_slice(&body).unwrap();

                        // Dispatch to pending request
                        if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
                            if let Some(tx) = pending.lock().await.remove(&id) {
                                let _ = tx.send(msg["result"].clone());
                            }
                        }

                        content_length = 0;
                    }
                }
            }
        }
    }

    pub async fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let id = {
            let mut id_lock = self.request_id.lock().await;
            let current_id = *id_lock;
            *id_lock += 1;
            current_id
        };

        let (tx, rx) = oneshot::channel();
        self.pending_requests.lock().await.insert(id, tx);

        // Build and send JSON-RPC request
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        self.write_message(request).await?;

        // Wait for response with timeout
        timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| CodexErr::Fatal("LSP request timed out".into()))?
            .map_err(|_| CodexErr::Fatal("LSP response channel closed".into()))
    }

    pub async fn send_notification(&self, method: &str, params: Value) -> Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.write_message(notification).await
    }

    async fn write_message(&self, msg: Value) -> Result<()> {
        let body = serde_json::to_string(&msg)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        let mut stdin = self.process.stdin.as_ref().unwrap();
        stdin.write_all(message.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        // Send LSP shutdown request
        self.send_request("shutdown", json!(null)).await?;
        self.send_notification("exit", json!(null)).await?;

        // Signal reader thread to stop
        let _ = self.shutdown_tx.send(()).await;

        // Wait for process to exit
        timeout(Duration::from_secs(5), self.process.wait())
            .await
            .map_err(|_| {
                // Force kill if timeout
                self.process.kill().ok();
                CodexErr::Fatal("LSP shutdown timed out".into())
            })??;

        Ok(())
    }
}
```

### 9.2 Language Server Implementation

```rust
// codex-rs/lsp/src/server.rs
use crate::handler::LspHandler;
use crate::types::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

pub struct LspServer {
    handler: LspHandler,
    language: Language,
    root_path: PathBuf,
    open_buffers: HashMap<Url, FileBuffer>,
    cache: SymbolCache,
}

#[derive(Clone)]
pub struct FileBuffer {
    uri: Url,
    contents: String,
    version: i32,
    language_id: String,
}

impl LspServer {
    pub async fn new(language: Language, root_path: PathBuf) -> Result<Self> {
        let (command, args) = language.get_command();
        let handler = LspHandler::start(&command, &args, &root_path).await?;

        let mut server = Self {
            handler,
            language,
            root_path: root_path.clone(),
            open_buffers: HashMap::new(),
            cache: SymbolCache::new(&root_path),
        };

        server.initialize().await?;
        Ok(server)
    }

    async fn initialize(&mut self) -> Result<()> {
        let root_uri = Url::from_file_path(&self.root_path).unwrap();

        let params = json!({
            "processId": std::process::id(),
            "rootPath": self.root_path.to_str(),
            "rootUri": root_uri.to_string(),
            "capabilities": {
                "textDocument": {
                    "documentSymbol": {
                        "hierarchicalDocumentSymbolSupport": true,
                        "symbolKind": {
                            "valueSet": (1..=26).collect::<Vec<i32>>()
                        }
                    },
                    "definition": {"dynamicRegistration": true},
                    "references": {"dynamicRegistration": true}
                }
            },
            "workspaceFolders": [{
                "uri": root_uri.to_string(),
                "name": self.root_path.file_name().unwrap().to_str()
            }]
        });

        let response = self.handler.send_request("initialize", params).await?;
        log::info!("LSP initialized: {:?}", response["capabilities"]);

        self.handler.send_notification("initialized", json!({})).await?;
        Ok(())
    }

    pub async fn open_file(&mut self, relative_path: &str) -> Result<()> {
        let abs_path = self.root_path.join(relative_path);
        let uri = Url::from_file_path(&abs_path).unwrap();

        if self.open_buffers.contains_key(&uri) {
            return Ok(()); // Already open
        }

        let contents = tokio::fs::read_to_string(&abs_path).await?;

        self.handler.send_notification("textDocument/didOpen", json!({
            "textDocument": {
                "uri": uri.to_string(),
                "languageId": self.language.language_id(),
                "version": 0,
                "text": contents
            }
        })).await?;

        self.open_buffers.insert(uri.clone(), FileBuffer {
            uri,
            contents,
            version: 0,
            language_id: self.language.language_id(),
        });

        Ok(())
    }

    pub async fn get_document_symbols(&mut self, relative_path: &str) -> Result<Vec<Symbol>> {
        // Check cache first
        if let Some(cached) = self.cache.get(relative_path).await? {
            return Ok(cached);
        }

        // Open file if not already open
        self.open_file(relative_path).await?;

        let abs_path = self.root_path.join(relative_path);
        let uri = Url::from_file_path(&abs_path).unwrap();

        let result = self.handler.send_request("textDocument/documentSymbol", json!({
            "textDocument": {"uri": uri.to_string()}
        })).await?;

        let symbols = Self::parse_symbols(result)?;

        // Cache result
        self.cache.insert(relative_path, &symbols).await?;

        Ok(symbols)
    }

    pub async fn find_definition(
        &mut self,
        relative_path: &str,
        line: u32,
        character: u32
    ) -> Result<Vec<Location>> {
        self.open_file(relative_path).await?;

        let abs_path = self.root_path.join(relative_path);
        let uri = Url::from_file_path(&abs_path).unwrap();

        let result = self.handler.send_request("textDocument/definition", json!({
            "textDocument": {"uri": uri.to_string()},
            "position": {"line": line, "character": character}
        })).await?;

        Self::parse_locations(result, &self.root_path)
    }

    pub async fn find_references(
        &mut self,
        relative_path: &str,
        line: u32,
        character: u32
    ) -> Result<Vec<Location>> {
        self.open_file(relative_path).await?;

        let abs_path = self.root_path.join(relative_path);
        let uri = Url::from_file_path(&abs_path).unwrap();

        let result = self.handler.send_request("textDocument/references", json!({
            "textDocument": {"uri": uri.to_string()},
            "position": {"line": line, "character": character},
            "context": {"includeDeclaration": false}
        })).await?;

        Self::parse_locations(result, &self.root_path)
    }

    fn parse_symbols(value: Value) -> Result<Vec<Symbol>> {
        // Parse LSP DocumentSymbol[] or SymbolInformation[]
        if let Some(arr) = value.as_array() {
            arr.iter().map(|v| {
                Ok(Symbol {
                    name: v["name"].as_str().unwrap().to_string(),
                    kind: v["kind"].as_i64().unwrap() as u32,
                    range: Range {
                        start: Position {
                            line: v["range"]["start"]["line"].as_u64().unwrap() as u32,
                            character: v["range"]["start"]["character"].as_u64().unwrap() as u32,
                        },
                        end: Position {
                            line: v["range"]["end"]["line"].as_u64().unwrap() as u32,
                            character: v["range"]["end"]["character"].as_u64().unwrap() as u32,
                        },
                    },
                    children: if let Some(children) = v.get("children") {
                        Self::parse_symbols(children.clone())?
                    } else {
                        vec![]
                    },
                })
            }).collect()
        } else {
            Ok(vec![])
        }
    }

    fn parse_locations(value: Value, root_path: &Path) -> Result<Vec<Location>> {
        if let Some(arr) = value.as_array() {
            arr.iter().map(|v| {
                let uri = Url::parse(v["uri"].as_str().unwrap()).unwrap();
                let abs_path = uri.to_file_path().unwrap();
                let rel_path = abs_path.strip_prefix(root_path).unwrap().to_path_buf();

                Ok(Location {
                    uri,
                    relative_path: rel_path,
                    range: Range {
                        start: Position {
                            line: v["range"]["start"]["line"].as_u64().unwrap() as u32,
                            character: v["range"]["start"]["character"].as_u64().unwrap() as u32,
                        },
                        end: Position {
                            line: v["range"]["end"]["line"].as_u64().unwrap() as u32,
                            character: v["range"]["end"]["character"].as_u64().unwrap() as u32,
                        },
                    },
                })
            }).collect()
        } else {
            Ok(vec![])
        }
    }

    pub async fn shutdown(mut self) -> Result<()> {
        // Close all open buffers
        for (uri, _) in self.open_buffers.drain() {
            self.handler.send_notification("textDocument/didClose", json!({
                "textDocument": {"uri": uri.to_string()}
            })).await.ok();
        }

        self.handler.shutdown().await
    }
}
```

### 9.3 Tool Implementation

```rust
// codex-rs/core/src/tools/lsp_tools_ext.rs (extension pattern)
use crate::tools::{Tool, ToolSpec};
use crate::lsp::{LspServer, Symbol, SymbolKind};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct GetSymbolsOverviewArgs {
    relative_path: String,
    #[serde(default = "default_max_chars")]
    max_answer_chars: i32,
}

fn default_max_chars() -> i32 { -1 }

pub struct GetSymbolsOverviewTool;

impl Tool for GetSymbolsOverviewTool {
    fn name(&self) -> &str { "get_symbols_overview" }

    fn description(&self) -> &str {
        "Get overview of top-level symbols in a file (classes, functions, etc.)"
    }

    async fn apply(&self, ctx: &mut ToolContext, args: Value) -> Result<String> {
        let args: GetSymbolsOverviewArgs = serde_json::from_value(args)?;

        // Get LSP server for current project
        let lsp = ctx.lsp_manager.get_server_for_project(&ctx.project)?;

        // Fetch symbols
        let symbols = lsp.get_document_symbols(&args.relative_path).await?;

        // Convert to JSON and limit length
        let json = serde_json::to_string_pretty(&symbols)?;

        if args.max_answer_chars > 0 && json.len() > args.max_answer_chars as usize {
            Ok(serde_json::to_string(&json!({
                "error": "Result too large",
                "size": json.len(),
                "limit": args.max_answer_chars,
                "hint": "Narrow your search or increase max_answer_chars"
            }))?)
        } else {
            Ok(json)
        }
    }
}

pub fn get_symbols_overview_tool_spec() -> ToolSpec {
    ToolSpec {
        name: "get_symbols_overview".to_string(),
        description: "Get overview of top-level symbols in a file".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "relative_path": {
                    "type": "string",
                    "description": "Relative path to the file"
                },
                "max_answer_chars": {
                    "type": "integer",
                    "description": "Max characters in response (-1 for default)",
                    "default": -1
                }
            },
            "required": ["relative_path"]
        }),
        tool: Box::new(GetSymbolsOverviewTool),
    }
}

// In core/src/tools/spec.rs (minimal integration):
pub fn build_specs() -> Vec<ToolSpec> {
    vec![
        // ... existing tools ...
        crate::tools::lsp_tools_ext::get_symbols_overview_tool_spec(),
        crate::tools::lsp_tools_ext::find_symbol_tool_spec(),
        crate::tools::lsp_tools_ext::find_references_tool_spec(),
    ]
}
```

---

## 10. Testing Strategy

### 10.1 Unit Tests

Serena has extensive test coverage:

```
test/
├── solidlsp/
│   ├── python/
│   │   └── test_pyright.py
│   ├── typescript/
│   │   └── test_typescript_ls.py
│   └── rust/
│       └── test_rust_analyzer.py
└── serena/
    ├── test_symbol_tools.py
    └── test_file_tools.py
```

**Example test** (`test/solidlsp/python/test_pyright.py`):

```python
def test_get_document_symbols():
    with PyrightServer.create(config, test_repo_path) as ls:
        symbols = ls.get_document_symbols("calculator.py")

        # Verify structure
        assert len(symbols.root_symbols) == 2  # Calculator class + add function
        assert symbols.root_symbols[0].name == "Calculator"
        assert symbols.root_symbols[0].kind == SymbolKind.Class

        # Verify children
        calc_methods = symbols.root_symbols[0].children
        assert any(m.name == "add" for m in calc_methods)
```

**Recommendation for codex-rs:**

```rust
// codex-rs/lsp/tests/test_python.rs
#[tokio::test]
async fn test_get_document_symbols() {
    let test_repo = setup_test_repo("python_calculator");
    let mut ls = LspServer::new(Language::Python, test_repo.path()).await.unwrap();

    let symbols = ls.get_document_symbols("calculator.py").await.unwrap();

    assert_eq!(symbols.len(), 2); // Calculator class + add function
    assert_eq!(symbols[0].name, "Calculator");
    assert_eq!(symbols[0].kind, SymbolKind::Class as u32);

    // Verify methods
    let methods = &symbols[0].children;
    assert!(methods.iter().any(|m| m.name == "add"));
}
```

### 10.2 Integration Tests

Test full tool workflows:

```python
def test_find_and_edit_symbol():
    """Test finding a symbol and editing it"""
    agent = SerenaAgent(project=test_project_path)

    # Find symbol
    result = agent.apply_tool("find_symbol", {
        "name_path_pattern": "Calculator/add",
        "include_body": True
    })
    symbols = json.loads(result)
    assert len(symbols) == 1

    # Edit symbol body
    result = agent.apply_tool("replace_symbol_body", {
        "name_path": "Calculator/add",
        "relative_path": "calculator.py",
        "new_body": "return a + b + 1"  # Bug!
    })
    assert "success" in result.lower()
```

---

## 11. Conclusion

Serena provides a comprehensive reference implementation for LSP integration in coding agents. Key takeaways:

1. **Three-layer architecture**: Handler → Server → Tools provides clean separation
2. **Automatic dependency management**: Download/install language servers on demand
3. **Progressive disclosure**: Tools expose information at different granularities
4. **Robust error handling**: Timeouts, retries, graceful degradation
5. **Multi-language support**: 30+ languages through LSP abstraction

For codex-rs, the recommended approach is:

1. Build standalone LSP crate (`codex-rs/lsp/`)
2. Implement 3-5 core tools first (symbols, definition, references)
3. Use extension pattern (`*_ext.rs`) to minimize upstream conflicts
4. Start with 2-3 languages (Rust, Python, TypeScript)
5. Add caching and auto-install in later iterations

**Estimated effort:**
- Phase 1 (Core LSP): 1-2 weeks
- Phase 2 (Tools): 1 week
- Phase 3 (Polish): 1 week
- **Total:** 3-4 weeks for production-ready LSP integration

---

**End of Analysis**
