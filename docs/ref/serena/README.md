# Serena Reference Documentation

This directory contains analysis and reference documentation for the Serena project, focusing on features that can be adapted for codex-rs.

## Documents

### [serena-lsp-integration-analysis.md](./serena-lsp-integration-analysis.md) ✅

**Comprehensive analysis of Serena's LSP integration system** (100% coverage)

All critical components have been documented:
- LSP Server Integration Architecture (1.1-1.5)
- **LanguageServerManager** - Multi-language orchestration (1.6) ✅
- **Process Tree Management** (1.7) ✅
- Build/Packaging System (2.x)
- File Watching & Synchronization (3.x)
- Search APIs (4.1-4.2)
- **Symbol Editing Tools** (4.3) ✅
- **CodeEditor Architecture** (4.4) ✅
- **Additional LSP Operations** (4.5) ✅
- **NamePathMatcher** (4.6) ✅
- **LanguageServerSymbolRetriever** (4.7) ✅
- Implementation Insights (5.x)
- Critical Implementation Details (6.x)
- Performance Optimizations (7.x)
- Recommendations (8.x) - Updated with 5 phases
- Code Examples (9.x)
- Testing Strategy (10.x)

### [analysis-review-and-gaps.md](./analysis-review-and-gaps.md)

**Historical document** - Gap analysis that was used to supplement the main analysis.
(Content has been merged into main analysis)

---

### Main Analysis Contents

**Topics covered:**

1. **LSP Server Integration Architecture**
   - Three-layer design (Agent → SolidLanguageServer → Handler)
   - Initialization sequence and lifecycle management
   - JSON-RPC 2.0 communication protocol
   - Language-specific implementations
   - **LanguageServerManager** - Multi-language orchestration ✅
   - **Process Tree Management** - Child process cleanup ✅

2. **Build/Packaging System for Language Servers**
   - Runtime dependency management
   - Automatic download and installation
   - Platform detection and binary selection
   - Storage locations and structure

3. **File Watching and LSP Synchronization**
   - In-memory file buffer management
   - File open/close protocol (reference counted)
   - Incremental text change notifications
   - Cache invalidation strategy

4. **Search & Editing APIs**
   - Symbol-based search (overview, find, references)
   - Text-based search (files, content, glob filtering)
   - **Symbol Editing Tools** - replace_body, insert_after/before, rename ✅
   - **CodeEditor Architecture** - Symbol-aware editing layer ✅
   - **Additional LSP Operations** - Diagnostics, completions ✅
   - **NamePathMatcher** - Symbol path pattern matching ✅
   - **LanguageServerSymbolRetriever** - Symbol search integration ✅

5. **Implementation Insights for codex-rs**
   - Architectural patterns and recommendations
   - Error handling strategies
   - Performance optimizations
   - Multi-language support

6. **Code Examples**
   - LSP client implementation in Rust
   - LanguageServerManager with parallel startup
   - CodeEditor for symbol-based editing
   - NamePathMatcher implementation
   - Tool integration patterns

7. **Recommendations (5 Phases)**
   - Phase 1: Core LSP Infrastructure
   - Phase 2: LanguageServerManager
   - Phase 3: Tools Integration + CodeEditor
   - Phase 4: Name Path Matching
   - Phase 5: Caching & Runtime Deps

## Quick Navigation

**For LSP Integration Planning:**
- Section 1: Architecture overview
- Section 1.6: **LanguageServerManager** ✅
- Section 1.7: Process Tree Management
- Section 5: Implementation recommendations
- Section 8: Detailed recommendations for codex-rs

**For Runtime Dependency System:**
- Section 2: Build/packaging system
- Section 2.3-2.6: Examples and storage

**For File Synchronization:**
- Section 3: File watching and sync
- Section 3.2: File open/close protocol
- Section 3.3: Incremental changes

**For Search APIs:**
- Section 4.1: Symbol-based search
- Section 4.2: Text-based search
- Section 4.6: **NamePathMatcher** ✅
- Section 4.7: **LanguageServerSymbolRetriever** ✅

**For Symbol Editing:** ✅
- Section 4.3: **Symbol Editing Tools**
- Section 4.4: **CodeEditor Architecture**
- Section 4.5: **Additional LSP Operations** (diagnostics, completions)

**For Code Examples:**
- Section 9: Rust implementation examples
- Section 10: Testing strategies

## Key Findings Summary

### Architecture Strengths

✅ **Clean separation of concerns** - LSP layer independent of agent layer
✅ **Language-agnostic interface** - Support 30+ languages through single API
✅ **Robust error handling** - Timeouts, retries, auto-restart
✅ **Progressive disclosure** - Tools expose info at multiple granularities
✅ **Efficient caching** - Two-level cache with content hash validation

### Critical Components

1. **SolidLanguageServerHandler** - Process management and JSON-RPC
2. **SolidLanguageServer** - Language-agnostic LSP operations
3. **LanguageServerManager** - Multi-language orchestration, auto-restart ← **NEW**
4. **CodeEditor** - Symbol-based editing operations ← **NEW**
5. **RuntimeDependencyCollection** - Auto-install language servers
6. **Symbol search tools** - `get_symbols_overview`, `find_symbol`, `find_references`
7. **Symbol editing tools** - `replace_body`, `insert_after`, `rename` ← **NEW**
8. **File buffer management** - In-memory sync with LSP server

### Implementation Priorities for codex-rs (Updated)

**Phase 1 (Core):**
- LSP handler with async/await
- Single language server wrapper
- Basic operations (documentSymbol, definition, references)

**Phase 2 (Manager):** ← **NEW**
- **LanguageServerManager** for multi-language support
- Parallel startup, auto-restart on crash
- Language routing by file extension

**Phase 3 (Tools):**
- Search tools (get_overview, find_symbol, find_references)
- **CodeEditor** for symbol-based editing ← **NEW**
- Symbol editing tools (replace_body, insert_after, insert_before)

**Phase 4 (Advanced):** ← **NEW**
- Rename refactoring (workspace/applyEdit)
- Diagnostics and code completions
- Name path matching system

**Phase 5 (Polish):**
- Caching with persistence
- Auto-download system
- Process tree cleanup

## Related Resources

- [Serena GitHub Repository](https://github.com/oraios/serena)
- [Language Server Protocol Specification](https://microsoft.github.io/language-server-protocol/)
- [LSP Type Definitions](https://github.com/microsoft/language-server-protocol/tree/main/protocol)

---

**Generated:** 2025-12-05
**Analyst:** Claude Code
**Status:** Complete
