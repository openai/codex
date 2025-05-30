# LLM Bridge: Codex ↔ Claude Communication

This directory contains the "Mixture-of-Idiots" (MoI) bridge system, designed to facilitate communication and collaboration between OpenAI's Codex and Anthropic's Claude language models.

## Overview

The LLM Bridge orchestrates a dialogue between Codex and Claude, allowing them to work together on tasks, with a human operator able to monitor, intervene, and direct the conversation. It operates by launching several Node.js scripts that communicate through a file-based messaging system.

## Components

The bridge consists of the following core JavaScript files:

*   **`smart_bridge.js`**: The central message routing engine. It monitors designated text files for messages from each component and routes them accordingly. It manages the conversation flow, context, and enforces protocol constraints.
*   **`master_control.js`**: The human operator's command interface. This script runs in its own terminal and allows the user to send messages to either LLM directly, issue system commands (like `/status`, `/pause`, `/continue`, `/quit`), or let the AIs converse autonomously.
*   **`codex_enhanced.js`**: An adapter for the OpenAI Codex CLI. It handles communication to and from the Codex model, passing messages through the `smart_bridge.js`.
*   **`claude_enhanced.js`**: An adapter for the Anthropic Claude model. It handles communication to and from Claude, passing messages through the `smart_bridge.js`.
*   **`mixture_config.js`**: Manages the configuration for the bridge system, including API keys (loaded from `.env`), model names, and file paths for communication.

## Communication Mechanism

The components communicate by reading and writing to a set of text files, typically:
*   `master_to_system.txt`: Messages from Master Control to the Smart Bridge.
*   `system_to_master.txt`: Messages/feedback from Smart Bridge to Master Control.
*   `claude_to_codex.txt`: Messages from Claude intended for Codex.
*   `codex_to_claude.txt`: Messages from Codex intended for Claude.
*   `current_context.txt`: Stores shared context for the conversation.
*   `system_status.json`: Contains the current status of the bridge system.

The `smart_bridge.js` orchestrates this by monitoring these files for changes.

## Prerequisites

1.  **Node.js**: Version 18.0+ is required.
2.  **OpenAI API Key**: You need an API key from OpenAI with access to the Codex models.
3.  **Codex CLI**: The OpenAI Codex CLI should be cloned and built. The main project `README.md` provides instructions for this. (Typically cloned into a `codex/codex-cli` subdirectory relative to the project root).
4.  **Environment Setup**:
    *   Navigate to the `llm_bridge` directory.
    *   Create a `.env` file with your OpenAI API key:
        ```bash
        echo "OPENAI_API_KEY=your_sk_xxxx_key_here" > .env
        ```

## How to Run

1.  Navigate to the `llm_bridge` directory from your terminal.
2.  Ensure the `start_mixture.sh` script is executable:
    ```bash
    chmod +x start_mixture.sh
    ```
3.  Run the startup script:
    ```bash
    ./start_mixture.sh
    ```
This script will attempt to open four new terminal windows (or use tmux if graphical terminals aren't available) for each of the components: Smart Bridge, Codex Enhanced, Claude Enhanced, and Master Control.

Follow the instructions in the Master Control terminal to interact with the system.

## Configuration

Key configuration options can be found in `mixture_config.js` and can be overridden via the `.env` file or environment variables. Important configurations include:

*   `OPENAI_API_KEY`: Your OpenAI API key (from `.env`).
*   `CLAUDE_MODEL`: The Claude model to use.
*   `CODEX_MODEL`: The Codex model to use.
*   `AUTO_CONTINUE_CONVERSATION`: Boolean to control if AIs converse autonomously.
*   `LOG_LEVEL`: Logging verbosity.
*   `MAX_CONVERSATION_TURNS`: Maximum turns for autonomous AI conversation.

Refer to `mixture_config.js` and the main project `README.md` for more details on the system architecture and protocol specifications.
