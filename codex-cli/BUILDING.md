# Building and Running Codex CLI with Ollama Support

This document provides instructions for building and running Codex CLI with the new Ollama integration.

## About TypeScript Errors

You may notice TypeScript errors (red lines) in the editor. These are expected during development and won't affect the functionality of the code when built. The errors are primarily related to missing type definitions, which will be resolved during the build process.

## Prerequisites

1. Make sure you have Node.js 22 or newer installed
2. Install [Ollama](https://ollama.ai/) on your system
3. Pull the models you want to use (e.g., `ollama pull gemma3:1b`)
4. Make sure Ollama is running in the background

## Building the Project

1. Navigate to the codex-cli directory:
   ```bash
   cd codex/codex-cli
   ```

2. Install dependencies:
   ```bash
   npm install
   ```
   
   If you encounter permission issues, you can try:
   ```bash
   npm install --no-optional
   ```
   
   Or if you're using WSL, you might need to fix permissions:
   ```bash
   sudo chown -R $(whoami) ~/.npm
   sudo chown -R $(whoami) .
   npm install
   ```

3. Build the project:
   ```bash
   npm run build
   ```

## Running Codex CLI with Ollama

### Option 1: Run the locally built version

```bash
node dist/cli.js --model ollama:gemma3:1b "Your prompt here"
```

### Option 2: Link the command globally

```bash
npm link
codex --model ollama:gemma3:1b "Your prompt here"
```

### Option 3: Run the test script

```bash
node test-ollama.js
```

This script will check if Ollama is running, list available models, and verify that the integration is working correctly.

## Verifying the Integration

To verify that the Ollama integration is working correctly:

1. Start Codex CLI in interactive mode:
   ```bash
   node dist/cli.js
   ```

2. Press `Ctrl+M` to open the model selection overlay

3. You should see Ollama models in the list, marked with a 🚀 icon

4. Select an Ollama model and start using it

## Troubleshooting

### Ollama Models Not Showing Up

If Ollama models are not showing up in the model selection overlay:

1. Make sure Ollama is running:
   ```bash
   ollama list
   ```

2. Check that the Ollama API is accessible:
   ```bash
   curl http://localhost:11434/api/tags
   ```

3. Verify that Ollama integration is enabled:
   ```bash
   export OLLAMA_ENABLED=true
   ```

### Build Errors

If you encounter build errors:

1. Make sure you have the latest version of Node.js installed
2. Try cleaning the node_modules directory and reinstalling:
   ```bash
   rm -rf node_modules
   npm install
   ```

3. Check for any error messages in the build output and address them specifically

## Running Tests

To run the tests for the Ollama integration:

```bash
npm test -- -t "Ollama"
```

This will run only the tests related to the Ollama integration.
