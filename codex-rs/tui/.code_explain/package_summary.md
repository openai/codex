# tui Package Summary

## Purpose
Terminal User Interface providing a rich, interactive experience for Codex conversations. Offers a full-featured chat interface with file browsing, session management, and real-time interaction.

## Key Components

### UI Framework
- **Ratatui Integration**: Terminal UI rendering
- **Crossterm Backend**: Cross-platform terminal control
- **Layout Management**: Responsive layout system
- **Widget System**: Reusable UI components

### Core Widgets
- **Chat Widget**: Conversation display and interaction
- **File Browser**: Navigate and select files
- **Model Selector**: Choose AI models
- **Session List**: Manage conversation sessions
- **Help Dialog**: Context-sensitive help

### Interaction System
- **Keyboard Handling**: Vi-like and standard keybindings
- **Mouse Support**: Optional mouse interaction
- **Command Palette**: Quick command access
- **Search Interface**: Find in conversation

### State Management
- **Application State**: Global app state
- **View States**: Per-view state management
- **Event Loop**: Main event processing
- **Update Cycle**: UI refresh management

## Main Functionality
1. **Interactive Conversations**: Real-time chat with AI
2. **File Management**: Browse and attach files
3. **Session Management**: Create/resume/manage sessions
4. **Authentication Flow**: Interactive login UI
5. **Configuration UI**: In-app settings management

## Dependencies
- `ratatui`: Terminal UI framework
- `crossterm`: Terminal manipulation
- `core`: Core Codex functionality
- `login`: Authentication integration
- `ollama`: Local model support
- `file-search`: File navigation
- `ansi-escape`: ANSI rendering

## Integration Points
- Launched by `cli` as subcommand
- Uses `core` for all operations
- Integrates `login` for auth
- Uses `file-search` for browsing
- Connects to `ollama` for OSS models

## UI Components

### Main Views
- **Chat View**: Primary conversation interface
- **File View**: File browser and selector
- **Session View**: Session management
- **Settings View**: Configuration interface
- **Help View**: Documentation and help

### Dialogs
- **Model Selection**: Choose AI model
- **File Picker**: Select files to attach
- **Confirmation**: Confirm actions
- **Error Display**: Show errors
- **Progress**: Long operation feedback

### Status Elements
- **Status Bar**: Current state info
- **Title Bar**: Context information
- **Message Bar**: User feedback
- **Progress Indicators**: Operation status

## Interaction Modes
- **Normal Mode**: Default interaction
- **Insert Mode**: Text input
- **Command Mode**: Command input
- **Search Mode**: Search in content
- **Selection Mode**: Multi-select

## Keyboard Shortcuts
- Vi-like navigation
- Emacs-style editing
- Custom Codex shortcuts
- Mode-specific bindings
- Configurable keymaps

## Features
- **Syntax Highlighting**: Code highlighting
- **Markdown Rendering**: Rich text display
- **Image Preview**: ASCII art previews
- **Auto-completion**: Command completion
- **History**: Command and search history

## Theming
- Color scheme support
- Terminal color detection
- High contrast mode
- Custom theme configuration