# Setting Up and Testing Codex on Windows 11 (Including Web UI)

This guide provides specific instructions for installing, running, and testing the Codex CLI and the experimental Web UI interface on Windows 11.

## Part 1: Standard Codex CLI Setup on Windows 11

### Background
While Codex CLI aims for cross-platform compatibility, native Windows execution has limitations (e.g., sandboxing). The recommended and most tested method for running the standard CLI on Windows is via the **Windows Subsystem for Linux (WSL2)**. This guide assumes you are using WSL2 unless otherwise specified for native Windows experiments.

**Note on `win32-support` Branch:** This specific branch (`win32-support`) includes ongoing experimental work to significantly improve native Windows CLI execution *without* WSL2. This includes enhanced shell command adaptation (translating POSIX commands to PowerShell/cmd equivalents) and direct task processing via custom PowerShell wrappers for better system interaction. While promising for future native support, for general use of the *released* CLI, WSL2 remains the primary supported method on Windows currently documented in the main README.

### Prerequisites
- Windows 11
- [WSL2 installed](https://learn.microsoft.com/en-us/windows/wsl/install) with a Linux distribution (e.g., Ubuntu 20.04+).
- [Node.js v22+](https://nodejs.org/) installed *within your WSL2 environment*.
- [Git](https://git-scm.com/) installed *within your WSL2 environment*.
- OpenAI API Key.

### Installation (via WSL2)
1.  Open your WSL2 terminal (e.g., Ubuntu).
2.  Install Codex CLI globally using npm (or your preferred package manager like pnpm):
    ```bash
    npm install -g @openai/codex
    # or
    # pnpm add -g @openai/codex
    ```
3.  Set your OpenAI API Key:
    ```bash
    export OPENAI_API_KEY="your-api-key-here"
    ```
    (Add this to your `~/.bashrc` or `~/.zshrc` for persistence within WSL).
4.  Verify installation:
    ```bash
    codex --version
    ```

### Testing CLI Modes (via WSL2)
Navigate to a project directory within your WSL2 filesystem (`/mnt/c/Users/...` paths can work but filesystem performance is better outside `/mnt`). Initialize git if needed (`git init`).

1.  **Suggest Mode (Default):**
    ```bash
    codex "Create a simple hello world python script"
    ```
    Codex will propose file changes and commands, requiring your approval ('y') for each step.
2.  **Auto Edit Mode:**
    ```bash
    codex --approval-mode auto-edit "Refactor hello.py to use a function"
    ```
    Codex will automatically *apply file edits* but still ask for approval before *running shell commands*.
3.  **Full Auto Mode:**
    ```bash
    codex --approval-mode full-auto "Add a comment to hello.py explaining the function"
    ```
    Codex will apply edits and run necessary commands without asking for approval. Use with caution.

---

## Part 2: Experimental Web UI & Multi-CLI Setup (Leveraging Native Windows Enhancements)

This section details how to set up and run the experimental Web UI features developed in the `win32-support` branch. This setup specifically **leverages the enhanced native Windows capabilities** described above.

### Architecture Overview
The new system introduces:
- A **Web UI** front-end (built with React/Tailwind) providing a chat interface, located in `orchestrator-web/`.
- A **Backend** server (part of the `codex-cli` package) that manages agent sessions and orchestrates tasks, typically running on `localhost:3080`. This backend utilizes the native Windows execution enhancements from this branch.
- **Multi-CLI Worker Support:** The backend architecture is designed to manage multiple CLI instances, enabling parallel tasks and specialized agents (work in progress).
- **Enhanced Win32 Raw Exec:** Leverages modifications allowing direct, adapted command execution via PowerShell on native Windows, enabling tasks initiated from the Web UI to interact more directly with the Windows system.

### Prerequisites (Native Windows)
- Windows 11
- [Node.js v22+](https://nodejs.org/) installed *natively* on Windows.
- [Git](https://git-scm.com/download/win) installed *natively* on Windows.
- [pnpm](https://pnpm.io/installation) installed *natively* on Windows (`npm install -g pnpm`).
- A terminal capable of running PowerShell (like Windows Terminal).
- OpenAI API Key.

### Setup & Deployment
These steps assume you have cloned the `codex` repository and checked out the `win32-support` branch (`git clone https://github.com/dorialn68/codex.git; cd codex; git checkout win32-support`).

1.  **Install Dependencies:**
    Open PowerShell *in the root `codex` directory* and run:
    ```powershell
    # Ensure pnpm is enabled (if using corepack with Node >=22)
    corepack enable
    # Install all dependencies for the monorepo
    pnpm install
    ```

2.  **Build Packages:**
    ```powershell
    pnpm run build
    ```
    This builds both the `codex-cli` package and the web UI components.

3.  **Configure API Key:**
    Create a `.env` file in the `codex/codex-cli` directory and add your API key:
    ```env
    # codex/codex-cli/.env
    OPENAI_API_KEY=your-api-key-here
    ```
    *(Note: The backend server loads this `.env` file)*

4.  **Run the Backend Server:**
    Open a PowerShell terminal *in the root `codex` directory* and start the backend server component:
    ```powershell
    # Assuming the server is started via a script in the root package.json
    # Verify the exact command if this doesn't work. Check codex-cli/package.json or root package.json for a relevant script.
    pnpm --filter @openai/codex run start:server
    ```
    Look for output indicating the server is listening, typically on `localhost:3080`.

5.  **Run the Front-end (Web UI):**
    Open a *second* PowerShell terminal, navigate to the Web UI directory (`orchestrator-web/`), and start the Vite development server:
    ```powershell
    cd orchestrator-web
    pnpm dev
    ```
    This will typically start the UI on `http://localhost:5173` (Vite's default). The terminal output will show the correct address.

### Testing & Validation
1.  **Access Web UI:** Open your web browser and navigate to the address provided by the `pnpm dev` command (e.g., `http://localhost:5173`).
2.  **Interface:** Verify the ChatGPT-like interface loads correctly with the chat history and input area.
3.  **Backend Connection:** Send a message. The UI should communicate with the backend server (running on port 3080, accessed via Vite proxy). Check both the browser's developer console (F12) and the backend server terminal for logs confirming the connection and message processing. Errors here might indicate the backend isn't running or the proxy isn't configured correctly.
4.  **Multi-CLI Interaction (Conceptual):** While full multi-CLI management might be pending roadmap items, test the basic interaction flow. Does the backend respond appropriately? If any basic CLI interaction is hooked up, test that (e.g., running a simple command requested via chat that leverages the native Windows execution).
5.  **Native Windows Execution:** Verify that features involving shell commands utilize the branch's enhanced native execution capabilities. Observe the backend logs to confirm it attempts execution using the adapted commands and PowerShell wrappers as expected on native Windows. This validates the direct Windows system interaction achieved in this branch for tasks run via the Web UI.

### Roadmap (As of 2025-05-02)
The following are the next planned steps for this feature branch:
- [ ] Replace `node-pty` with `child_process` bridge for improved Windows I/O.
- [ ] Implement `/cli?instance` multi-worker endpoint for managing multiple CLI instances.
- [ ] Define Task & Event DB schema (using SQLite).
- [ ] Build the backend Dispatcher & Task status API.
- [ ] Create the front-end TaskBoard UI with CLI tabbing.
- [ ] Develop the Memory summariser service.
- [ ] Refresh overall README/docs for the new developer workflow.

---
*This documentation is specific to the experimental `win32-support` branch and may change significantly.*

### Experimental Native Windows CLI Usage (`win32-support` branch)
This branch contains significant improvements specifically aimed at running the Codex CLI directly on native Windows without WSL2. This capability is **experimental** and primarily intended for development and testing of these new features.

**Capabilities Enabled in this Branch:**
*   **Shell Adaptation:** Attempts to translate many common POSIX commands (like `ls`, `cp`, `rm`) to their Windows equivalents (`dir`, `copy`, `del`) automatically for broader command compatibility in PowerShell/cmd.
*   **Direct PowerShell Execution:** Utilizes a custom PowerShell wrapper for executing certain complex or sensitive operations directly on the Windows system, aiming to provide capabilities closer to POSIX environments.
*   **Foundation for Mixed Mode:** This work lays the groundwork for potential future scenarios involving mixed-mode operation (e.g., a native UI coordinating tasks across both native Windows and WSL environments).

**How to Try (Requires Building from Source):**
1.  Ensure you meet the "Prerequisites (Native Windows)" listed in Part 2 below.
2.  Follow "Setup & Deployment" steps 1-3 from Part 2 (clone, checkout `win32-support`, install dependencies, build, configure API key).
3.  From a PowerShell terminal located inside the `codex/codex-cli` directory, you can attempt to run the locally built CLI directly:
    ```powershell
    # Example: Run interactively
    node ./dist/cli.js

    # Example: Run with a prompt
    node ./dist/cli.js "Create a PowerShell script to list running processes."
    ```
**Caveats:**
*   This is experimental; not all features, prompts, or commands may work as reliably as in the WSL2 or standard POSIX environments. Test thoroughly.
*   Sandboxing capabilities differ significantly from macOS/Linux; native execution relies more heavily on standard user process permissions and the security features of PowerShell itself. 