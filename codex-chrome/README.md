# Codex Chrome Extension

A Chrome extension that ports the codex-rs agent architecture to the browser, preserving the SQ/EQ (Submission Queue/Event Queue) pattern.

## Prerequisites

- Node.js 20+ and npm
- Chrome browser (for testing)

## Installation

1. Clone the repository and navigate to the project directory:
```bash
cd codex-chrome
```

2. Install dependencies:
```bash
npm install
```

## Building the Extension

### Production Build

Build the extension for production:
```bash
npm run build
```

This will:
- Compile TypeScript files
- Build the Svelte UI components
- Bundle all assets with Vite
- Copy manifest.json to dist/
- Copy and fix HTML files (sidepanel.html, welcome.html) to dist/
- Create placeholder SVG icons if needed
- Output everything to the `dist/` directory

The build script automatically handles path corrections for Chrome extension compatibility.

### Development Build

For development with file watching:
```bash
npm run build:watch
```

This will rebuild automatically when files change.

## Loading the Extension in Chrome

1. Open Chrome and navigate to `chrome://extensions/`
2. Enable **"Developer mode"** toggle in the top right corner
3. Click **"Load unpacked"** button
4. Select the `dist/` directory from this project
5. The Codex extension icon should appear in your toolbar

## Using the Extension

### Opening the Side Panel
- Click the Codex icon in the toolbar
- Or press `Alt+Shift+C` (keyboard shortcut)

### Context Menu Actions
- Select text on any webpage
- Right-click and choose:
  - "Explain with Codex" - Get explanations
  - "Improve with Codex" - Get suggestions for improvement
  - "Extract data with Codex" - Extract structured data from the page

### Quick Action
- Press `Alt+Shift+Q` to analyze the current page

## Development

### Project Structure
```
codex-chrome/
├── src/
│   ├── protocol/        # Protocol types (Submission, Op, Event, EventMsg)
│   ├── core/           # Core agent logic (CodexAgent, Session, QueueProcessor)
│   ├── background/     # Service worker
│   ├── content/        # Content script for DOM access
│   ├── sidepanel/      # Svelte UI components
│   └── welcome/        # Welcome page
├── dist/               # Built extension (git-ignored)
├── manifest.json       # Chrome extension manifest
├── vite.config.mjs     # Vite build configuration
└── tsconfig.json       # TypeScript configuration
```

### Available Scripts

```bash
# Build for production
npm run build

# Build and watch for changes
npm run build:watch

# Run type checking
npm run type-check

# Run linter
npm run lint

# Format code
npm run format

# Run tests
npm run test
```

### Type Checking

Run TypeScript type checking without building:
```bash
npm run type-check
```

## Architecture

The extension preserves the core SQ/EQ architecture from codex-rs:

- **Submission Queue**: User requests (Op operations)
- **Event Queue**: Agent responses (EventMsg)
- **CodexAgent**: Main coordinator class
- **Session**: Conversation state management
- **MessageRouter**: Chrome extension message passing

## Troubleshooting

### Build Errors

If you encounter build errors:

1. Clear the dist directory:
```bash
rm -rf dist/
```

2. Reinstall dependencies:
```bash
rm -rf node_modules package-lock.json
npm install
```

3. Run the build again:
```bash
npm run build
```

### Extension Not Loading

- Ensure you're loading the `dist/` directory, not the project root
- Check Chrome console for errors: View → Developer → JavaScript Console
- Make sure "Developer mode" is enabled in chrome://extensions/

### Content Script Issues

If the content script isn't working on certain pages:
- Chrome blocks content scripts on chrome:// URLs and the Chrome Web Store
- Some websites with strict CSP may block the content script
- Try refreshing the page after loading the extension

## License

ISC