# Building a Personalized Copilot with Codex CLI

This document outlines how to assemble a tailored, local-first copilot using the building blocks already available in Codex CLI. It focuses on configuration-first integration so you can keep safety controls, model flexibility, and tool access aligned with your workflow.

## Where to find Primator and current status

- The blueprint lives at `docs/copilot-blueprint.md` in this repo. It describes how to assemble the copilot—codenamed **Primator**—using Codex CLI components you already have locally.
- Codex CLI itself installs from npm/Homebrew/Linux binaries (see [Install & build](./install.md)). There is no separate Primator binary yet; Primator is a configuration, naming, and profile layer on top of Codex CLI.
- Start a session with `codex --profile primator` once you add the profile below to your `~/.codex/config.toml`. Primator runs in **command-only** mode by design.

## Architecture overview

- **Codex CLI orchestration**: Use the existing CLI loop to chat, plan, and execute actions. Model routing and session state stay within the standard Codex runtime.
- **Model providers**: Point the copilot at any compatible backend by defining providers (OpenAI, Ollama, Mistral, Azure, etc.) and selecting the active one via `model_provider`. Multiple providers can coexist so you can swap “brains” per task.
- **Sandbox + approvals**: Keep command execution guarded with `approval_policy` while the sandbox isolates shell commands by default; loosen or tighten prompts per profile as needed.
- **Tooling via MCP**: Attach task-specific tools or data sources with `mcp_servers` to give the copilot structured capabilities without modifying the core binary.
- **Environment shaping**: Control what the copilot passes to subprocesses with `shell_environment_policy`, ensuring only the variables you want are exposed.
- **Voice gateway (optional)**: Add a thin speech-to-text/text-to-speech layer that feeds Codex CLI input and relays responses, without changing the core agent.

## Flow for the new copilot

1. **Start with a profile**: Launch Codex with a profile that picks the model, provider, and approval posture appropriate for the task.
2. **Plan and gather context**: The model inspects AGENTS.md notes and MCP tool metadata to frame the task before executing commands.
3. **Command-only execution**: The copilot never acts until you issue a command. Pair this with strict `approval_policy` settings (`untrusted` or `on-request`) so every shell call is gated unless you explicitly allow it.
4. **Iterate with tool calls**: The copilot loops through tool use (shell, MCP tools) and model reasoning until it declares success or requests input.
5. **Notify and log**: Optional notifications keep you informed of long-running work; logs live under `~/.codex` alongside the active profile state.

## Langkah demi langkah: hidupkan Primator di komputer anda

Ikuti urutan ini jika anda mahu menghidupkan copilot (Primator) di laptop atau desktop tanpa perlu merujuk banyak bahagian lain:

1. **Pasang Codex CLI**
   - macOS/Homebrew: `brew install codex`
   - Node/npm (macOS/Linux/WSL): `npm i -g @openai/codex`
   - Atau muat turun binari platform daripada [rilis terkini](https://github.com/openai/codex/releases/latest) dan letak dalam `PATH` sebagai `codex`.
2. **Sediakan fail konfigurasi**
   - Buka/hasilkan `~/.codex/config.toml`.
   - Salin blok profil `primator` dalam seksyen "Configuration blueprint" di bawah ke dalam fail tersebut.
3. **Tetapkan pembekal model pilihan**
   - Untuk kerja tanpa internet, pastikan anda telah menjalankan pelayan model setempat seperti Ollama, kemudian tukar `model_provider = "ollama"` pada profil Primator.
   - Untuk sambungan awan, pastikan kunci API tersedia (contoh `OPENAI_API_KEY`, `GEMINI_API_KEY`, atau `DEEPSEEK_API_KEY`) dan pembekal yang sepadan diaktifkan dalam `config.toml`.
4. **Kawal autonomi**
   - Pastikan `approval_policy` ditetapkan kepada `untrusted` atau `on-request` supaya Primator hanya menjalankan perintah selepas anda beri arahan.
   - Biarkan `preamble` menyatakan bahawa Primator ialah copilot `command-only`.
5. **Jalankan sesi Primator**
   - Mulakan dengan `codex --profile primator` dari terminal.
   - Berikan arahan tugas secara jelas (contoh: "Sediakan projek Node"), kemudian luluskan sebarang permintaan kelulusan perintah yang dipaparkan.
6. **(Pilihan) Tambah suara**
   - Pasang enjin ucapan-ke-teks luar talian (Whisper.cpp, Vosk) untuk menyalin suara kepada input teks.
   - Gunakan enjin teks-ke-ucapan kegemaran anda untuk membaca balasan. Gerbang suara ini hanya perlu menghantar/terima teks ke proses `codex`.
7. **Uji dan kemas kini**
   - Semak log di `~/.codex` jika ada ralat.
   - Laras profil (model, pembekal, MCP tools) mengikut tugas dan sumber yang ada.

## Quickstart (desktop/laptop): get Primator running now

Follow this concise sequence if you just want Primator working on a Mac, Linux, or WSL machine:

1. **Install Codex CLI**
   ```bash
   # macOS (Homebrew)
   brew install codex

   # macOS/Linux/WSL (npm)
   npm i -g @openai/codex
   ```

2. **Create a minimal Primator profile**
   ```bash
   mkdir -p ~/.codex
   cat > ~/.codex/config.toml <<'EOF'
   model = "gpt-4o"
   model_provider = "openai"
   approval_policy = "untrusted"
   profile = "primator"

   [profiles.primator]
   model = "gpt-4o"
   model_provider = "openai"
   approval_policy = "untrusted"
   preamble = "You are Primator, a command-only copilot. Never run a command unless the user explicitly requests a task."

   # Optional offline provider
   [model_providers.ollama]
   name = "Ollama"
   base_url = "http://localhost:11434/v1"
   EOF
   ```

   - Swap `model_provider` to `ollama` if you have a local model server and want offline inference.
   - Keep `approval_policy = "untrusted"` so every command requires your confirmation.

3. **Run Primator in command-only mode**
   ```bash
   codex --profile primator
   ```
   - Issue a task (e.g., "initialize a Python project") and approve commands as prompted.
   - End the session with `Ctrl+C` or `codex end-session` before switching to a new task.

4. **Optional: add voice I/O**
   - Feed speech-to-text transcripts (Whisper.cpp, Vosk) into the running `codex` process.
   - Pipe responses through a text-to-speech tool; the voice layer is just an I/O relay, not a new agent.

5. **Troubleshoot and iterate**
   - Check `~/.codex` logs if something fails.
   - Add more providers (Gemini/DeepSeek style) or MCP servers as you need them, then restart Primator.

### Command-only operating contract

- **No autonomy unless commanded**: The agent waits for explicit, user-issued tasks. Use a prompt preamble in `~/.codex/config.toml` (for example, via `system_prompts` or a profile `preamble`) that states: “Do not run commands unless the user requested a task.”
- **Approval discipline**: Set `approval_policy = "untrusted"` to force confirmation before any command runs, or `on-request` if you want to approve only when the model deems a step risky.
- **Session reset**: When switching tasks, clear the conversation (`codex end-session` or restart the CLI) to avoid accidental carryover of intent.

## Configuration blueprint

Here is a starter `~/.codex/config.toml` sketch for a "copilot" profile that favors autonomy while keeping approvals on model-initiated escalation:

```toml
model = "gpt-4o"
model_provider = "openai"
approval_policy = "on-request"
profile = "primator"

[profiles.primator]
# Primator: command-only copilot profile
model = "gpt-4o"
model_provider = "openai"
approval_policy = "on-request"
# Optional: prepend a name and behavior reminder
preamble = "You are Primator, a command-only copilot. Never run a command unless the user explicitly requests a task."

# Optional: add a local model for offline work
[model_providers.ollama]
name = "Ollama"
base_url = "http://localhost:11434/v1"

# Example: add a Gemini-compatible endpoint (self-hosted or vendor-provided)
[model_providers.gemini]
name = "Gemini-compatible"
base_url = "http://localhost:8080/v1"
api_key = "${GEMINI_API_KEY}"

# Example: add a DeepSeek-style OpenAI-compatible endpoint
[model_providers.deepseek]
name = "DeepSeek-compatible"
base_url = "http://localhost:8081/v1"
api_key = "${DEEPSEEK_API_KEY}"

# Optional: attach MCP tools for project data or automation
[mcp_servers.project-tools]
command = "npx"
args = ["-y", "mcp-server"]

[shell_environment_policy]
inherit = "core"
include_only = ["PATH", "HOME", "USER"]
```

### Routing multiple “brains”

- **Swap providers per profile**: Create profiles like `profiles.offline` (pointing at Ollama) and `profiles.cloud` (pointing at OpenAI/Azure) to change “brains” without code changes.
- **Task-scoped overrides**: Launch Codex with `--config model_provider=deepseek` to temporarily route a session through a different provider.
- **Capabilities matrix**: Document which provider supports which tool calls (e.g., function calling vs. plain text), then constrain prompts accordingly.

## Installation targets: laptop and Android

- **Laptop (macOS/Linux/Windows via WSL)**
  - Install Codex CLI from npm (`npm i -g @openai/codex`) or Homebrew (`brew install codex`), or download the platform binary from the [latest release](https://github.com/openai/codex/releases/latest) (see `docs/install.md`).
  - Create `~/.codex/config.toml` and paste the Primator profile above. Run `codex --profile primator` to start the command-only copilot.
  - For voice, pair an offline speech-to-text engine (e.g., Whisper.cpp) and a text-to-speech tool, piping transcripts into `codex` and reading responses back to audio.

- **Android (Termux / ARM64)**
  - Install [Termux](https://termux.dev/) and update packages (`pkg update && pkg upgrade`).
  - Download the Linux ARM64 release asset `codex-aarch64-unknown-linux-musl` from the latest GitHub release, mark it executable (`chmod +x codex-aarch64-unknown-linux-musl`), and move it into your `~/bin` or add its directory to `PATH` as `codex`.
  - Create `~/.codex/config.toml` with the Primator profile. Run `codex --profile primator` inside Termux. Keep in mind Termux runs a headless shell, so MCP tools and voice gateways should be CLI-friendly.

## Next implementation steps

- Create a dedicated profile in `~/.codex/config.toml` following the blueprint above and toggle between providers depending on connectivity needs.
- Add MCP servers for the tasks you care about (e.g., docs search, ticketing, deployment triggers) so the copilot can act with structured tools.
- Iterate on approval policies per profile—`never` for unattended automation, `on-request` for guided autonomy, or `untrusted` when exploring new codebases.
- Extend shell environment rules to keep secrets out of tool calls while preserving essentials like `PATH` and `HOME` for reproducible runs.
- Layer in a voice gateway: connect a speech-to-text engine (e.g., Whisper.cpp or Vosk offline) to feed Codex CLI input, and a text-to-speech engine to read responses. Keep this gateway stateless so it only relays user-issued commands.
