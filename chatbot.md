# ChatGPT-Klon mit Codex CLI Authentifizierung

> **Zweck:** Nutze den Codex CLI Authentifizierungsmechanismus, um einen eigenen ChatGPT-Klon zu erstellen, der über dein OpenAI-Abo abgerechnet wird (nicht über API-Calls).

---

## 📋 Inhaltsverzeichnis

1. [Übersicht](#übersicht)
2. [Wie Codex CLI funktioniert](#wie-codex-cli-funktioniert)
3. [OAuth 2.0 PKCE Flow](#oauth-20-pkce-flow)
4. [Token-Verwaltung](#token-verwaltung)
5. [API-Endpoints](#api-endpoints)
6. [Tool-System](#tool-system)
7. [Imitations-Strategie](#imitations-strategie)
8. [Implementierungs-Guide](#implementierungs-guide)
9. [Sicherheit & Risiken](#sicherheit--risiken)
10. [Code-Referenzen](#code-referenzen)

---

## 🎯 Übersicht

### Was macht Codex CLI?

Codex CLI authentifiziert sich über **OAuth 2.0** mit deinem OpenAI-Account und nutzt dann die **ChatGPT Backend API** (`chatgpt.com/backend-api`) statt der regulären OpenAI API (`api.openai.com/v1`).

### Wichtigste Erkenntnisse

- ✅ **KEINE API-Abrechnung** - Nutzt dein ChatGPT-Abo (Plus/Pro/Team)
- ✅ **OAuth-basiert** - Kein API-Key erforderlich (im ChatGPT-Modus)
- ✅ **Imitierbar** - Du kannst dich als Codex CLI ausgeben
- ⚠️ **Nachweisbar** - OpenAI sieht Account-ID, IP, Request-Patterns
- ✅ **Legal** - Du nutzt dein eigenes Abo für deinen eigenen Chatbot

### Zwei Authentifizierungs-Modi

| Modus | Endpoint | Token-Typ | Abrechnung |
|-------|----------|-----------|------------|
| **ChatGPT** (für dich!) | `chatgpt.com/backend-api` | OAuth access_token | ChatGPT-Abo |
| **ApiKey** (NICHT verwenden) | `api.openai.com/v1` | API-Key | API-Abrechnung |

---

## 🔐 Wie Codex CLI funktioniert

### Ablauf auf höchster Ebene

```
1. OAuth Login (einmalig)
   ↓
2. Erhalte: id_token, access_token, refresh_token
   ↓
3. Speichere Tokens in auth.json
   ↓
4. Nutze access_token für ChatGPT API-Requests
   ↓
5. Refresh Token alle ~8 Tage
```

### Kritische Details

**Was Codex CLI NICHT macht:**
- ❌ **KEIN API-Key Request!** (Device Code Flow überspringt dies komplett)
- ❌ API-Key Request im Browser-Flow wird mit `.ok()` ignoriert (Fehler egal)
- ❌ Wenn API-Key gespeichert würde → Wechsel zu ApiKey-Modus → Falsche API!

**Was du machen musst:**
- ✅ OAuth-Login durchführen
- ✅ Tokens speichern (OHNE API-Key!)
- ✅ Im ChatGPT-Modus bleiben
- ✅ `access_token` als Bearer Token nutzen

---

## 🔑 OAuth 2.0 PKCE Flow

### 1. PKCE-Codes generieren

**PKCE (Proof Key for Code Exchange)** verhindert Authorization Code Interception Attacks.

```typescript
// Pseudo-Code
import crypto from 'crypto';

function generatePKCE() {
  // 1. Generate code_verifier (64 random bytes)
  const codeVerifier = base64UrlEncode(crypto.randomBytes(64));

  // 2. Generate code_challenge (SHA256 of verifier)
  const hash = crypto.createHash('sha256').update(codeVerifier).digest();
  const codeChallenge = base64UrlEncode(hash);

  return { codeVerifier, codeChallenge };
}

function base64UrlEncode(buffer: Buffer): string {
  return buffer.toString('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=/g, '');
}
```

**Code-Referenz:** `codex-rs/login/src/pkce.rs:12-27`

### 2. Authorization URL erstellen

```
https://auth.openai.com/oauth/authorize?
  response_type=code&
  client_id=app_EMoamEEZ73f0CkXaXp7hrann&
  redirect_uri=http://localhost:1455/auth/callback&
  scope=openid+profile+email+offline_access&
  code_challenge={code_challenge}&
  code_challenge_method=S256&
  state={random_state}&
  id_token_add_organizations=true&
  codex_cli_simplified_flow=true&
  originator=codex_cli_rs
```

**Wichtige Parameter:**

| Parameter | Wert | Zweck |
|-----------|------|-------|
| `client_id` | `app_EMoamEEZ73f0CkXaXp7hrann` | Codex CLI OAuth-Client |
| `redirect_uri` | `http://localhost:1455/auth/callback` | Lokaler Callback |
| `scope` | `openid profile email offline_access` | Benötigte Scopes |
| `code_challenge_method` | `S256` | SHA256 für PKCE |
| `id_token_add_organizations` | `true` | Fügt Org-Info zu Token hinzu |
| `codex_cli_simplified_flow` | `true` | UI-Vereinfachung (optional?) |
| `originator` | `codex_cli_rs` | Identifikation (Telemetrie) |
| `state` | Random String | CSRF-Schutz |

**Code-Referenz:** `codex-rs/login/src/server.rs:380-418`

### 3. Lokalen HTTP Server starten

```typescript
// Pseudo-Code
const express = require('express');
const app = express();

let resolveLogin: (tokens: Tokens) => void;
const loginPromise = new Promise((resolve) => {
  resolveLogin = resolve;
});

app.get('/auth/callback', async (req, res) => {
  const { code, state } = req.query;

  // 1. Validiere State
  if (state !== savedState) {
    return res.status(400).send('State mismatch');
  }

  // 2. Exchange code for tokens
  const tokens = await exchangeCodeForTokens(code);

  // 3. Resolve Promise
  resolveLogin(tokens);

  // 4. Redirect zu Success-Page
  res.redirect(`/success?needs_setup=false`);
});

app.get('/success', (req, res) => {
  res.send('<h1>Signed in to Codex</h1><p>You may close this page</p>');
  setTimeout(() => server.close(), 2000);
});

const server = app.listen(1455, () => {
  console.log('Server listening on http://localhost:1455');
});
```

**Code-Referenz:** `codex-rs/login/src/server.rs:100-207`

### 4. Code gegen Tokens tauschen

```http
POST https://auth.openai.com/oauth/token
Content-Type: application/x-www-form-urlencoded

grant_type=authorization_code&
code={authorization_code}&
redirect_uri=http://localhost:1455/auth/callback&
client_id=app_EMoamEEZ73f0CkXaXp7hrann&
code_verifier={code_verifier}
```

**Response:**
```json
{
  "id_token": "eyJhbGc...",
  "access_token": "eyJhbGc...",
  "refresh_token": "..."
}
```

**Code-Referenz:** `codex-rs/login/src/server.rs:494-536`

### 5. Tokens persistieren

**WICHTIG:** Speichere **OHNE** API-Key!

```json
{
  "OPENAI_API_KEY": null,
  "tokens": {
    "id_token": "eyJhbGc...",
    "access_token": "eyJhbGc...",
    "refresh_token": "..."
  },
  "last_refresh": "2025-11-15T10:30:00Z"
}
```

**Speicherort:** `$HOME/.codex/auth.json` (oder custom path)

**Code-Referenz:** `codex-rs/login/src/server.rs:538-570`, `codex-rs/core/src/auth/storage.rs`

### 6. ID-Token parsen

Der `id_token` ist ein JWT und enthält wichtige Informationen:

```typescript
// JWT Format: header.payload.signature
function parseIdToken(idToken: string) {
  const parts = idToken.split('.');
  const payload = JSON.parse(base64UrlDecode(parts[1]));

  return {
    email: payload.email,
    chatgpt_account_id: payload['https://api.openai.com/auth'].chatgpt_account_id,
    chatgpt_plan_type: payload['https://api.openai.com/auth'].chatgpt_plan_type,
    organization_id: payload['https://api.openai.com/auth'].organization_id,
  };
}
```

**Code-Referenz:** `codex-rs/core/src/token_data.rs:90-115`

---

## 🔄 Token-Verwaltung

### Token-Refresh

Tokens werden automatisch aktualisiert, wenn sie **älter als 8 Tage** sind.

```http
POST https://auth.openai.com/oauth/token
Content-Type: application/json

{
  "client_id": "app_EMoamEEZ73f0CkXaXp7hrann",
  "grant_type": "refresh_token",
  "refresh_token": "...",
  "scope": "openid profile email"
}
```

**Response:**
```json
{
  "id_token": "new_id_token",
  "access_token": "new_access_token",
  "refresh_token": "new_refresh_token"
}
```

**Fehler-Behandlung:**

| Status | Error Code | Bedeutung | Aktion |
|--------|-----------|-----------|--------|
| 401 | `refresh_token_expired` | Token abgelaufen | Neu einloggen |
| 401 | `refresh_token_reused` | Token bereits benutzt | Neu einloggen |
| 401 | `refresh_token_invalidated` | Token widerrufen | Neu einloggen |

**Code-Referenz:** `codex-rs/core/src/auth.rs:514-555`

### Token-Lebenszyklus

```
Login
  ↓
[TokenValid] ───────────────────────────┐
  ↓ (8 Tage)                             │
[Token Refresh] ───→ [New Token] ───────┘
  ↓ (Refresh Failed)
[Re-Login Required]
```

---

## 🌐 API-Endpoints

### ChatGPT Backend API

**Basis-URL:** `https://chatgpt.com/backend-api/codex`

Oder alternativ:
- `https://chat.openai.com/backend-api/codex`

**Path-Styles:**
- **CodexApi:** `/api/codex/...` (für `api.openai.com`)
- **ChatGptApi:** `/wham/...` (für `chatgpt.com/backend-api`)

**Code-Referenz:** `codex-rs/backend-client/src/client.rs:19-34`

### Wichtigste Endpoints

#### 1. Chat Completion (Stream)

```http
POST https://chatgpt.com/backend-api/codex/responses
Authorization: Bearer {access_token}
chatgpt-account-id: {account_id}
originator: codex_cli_rs
User-Agent: codex_cli_rs/0.5.0 (Linux 5.15.0; x86_64) xterm-256color
version: 0.5.0
conversation_id: {uuid}
session_id: {uuid}
Accept: text/event-stream
Content-Type: application/json

{
  "model": "gpt-4",
  "input": [
    {
      "type": "user_message",
      "content": "Hello!"
    }
  ],
  "tools": [
    // Tool-Definitionen
  ],
  "stream": true
}
```

**Response:** Server-Sent Events (SSE)

```
event: response.started
data: {"type":"response.started","sequence_number":0,"response":{"id":"resp_..."}}

event: response.output_item.added
data: {"type":"response.output_item.added","sequence_number":1,"item":{"type":"message","role":"assistant"}}

event: response.output_item.content_part.delta
data: {"type":"response.output_item.content_part.delta","sequence_number":2,"delta":"Hello"}

event: response.completed
data: {"type":"response.completed","sequence_number":3,"response":{"id":"resp_...","status":"completed"}}
```

**Code-Referenz:** `codex-rs/core/src/client.rs:187-291`, `codex-rs/core/src/model_provider_info.rs:160-173`

#### 2. Rate Limits abrufen

```http
GET https://chatgpt.com/backend-api/wham/usage
Authorization: Bearer {access_token}
chatgpt-account-id: {account_id}
```

**Response:**
```json
{
  "rate_limit": {
    "primary_window": {
      "used_percent": 45,
      "limit_window_seconds": 3600,
      "reset_at": 1700000000
    }
  }
}
```

**Code-Referenz:** `codex-rs/backend-client/src/client.rs:158-167`

#### 3. Task History

```http
GET https://chatgpt.com/backend-api/wham/tasks/list?limit=50
Authorization: Bearer {access_token}
```

**Code-Referenz:** `codex-rs/backend-client/src/client.rs:169-197`

#### 4. Task Details

```http
GET https://chatgpt.com/backend-api/wham/tasks/{task_id}
Authorization: Bearer {access_token}
```

**Code-Referenz:** `codex-rs/backend-client/src/client.rs:199-216`

#### 5. Create Task

```http
POST https://chatgpt.com/backend-api/wham/tasks
Authorization: Bearer {access_token}
Content-Type: application/json

{
  "message": "...",
  "model": "gpt-4"
}
```

**Code-Referenz:** `codex-rs/backend-client/src/client.rs:238-271`

---

## 🛠️ Tool-System

### Warum Tools wichtig sind

Codex CLI sendet bei **jedem** Request eine Liste von verfügbaren Tools. Dies:
1. Ermöglicht dem Modell, Tools zu nutzen
2. Ist Teil der "Codex CLI Signatur"
3. Sollte imitiert werden, auch wenn du sie nicht nutzt

### Standard-Tools (immer vorhanden)

```javascript
const STANDARD_TOOLS = [
  // Shell-Ausführung
  {
    name: "shell",
    description: "Execute shell commands",
    parameters: {
      type: "object",
      properties: {
        cmd: { type: "string", description: "Command to execute" },
        workdir: { type: "string", description: "Working directory" }
      },
      required: ["cmd"]
    }
  },

  // MCP Resources
  {
    name: "list_mcp_resources",
    description: "List available MCP resources",
    parameters: {
      type: "object",
      properties: {
        server: { type: "string", description: "Optional server name" }
      }
    }
  },

  {
    name: "read_mcp_resource",
    description: "Read content from an MCP resource",
    parameters: {
      type: "object",
      properties: {
        uri: { type: "string", description: "Resource URI" }
      },
      required: ["uri"]
    }
  },

  // Plan-Management
  {
    name: "update_plan",
    description: "Update the current task plan",
    parameters: {
      type: "object",
      properties: {
        plan: { type: "string", description: "Updated plan content" }
      },
      required: ["plan"]
    }
  }
];
```

**Code-Referenz:** `codex-rs/core/src/tools/spec.rs:934-995`

### Experimentelle Tools (konfigurierbar)

Diese Tools werden nur gesendet, wenn sie in `experimental_supported_tools` aktiviert sind:

```javascript
const EXPERIMENTAL_TOOLS = [
  // Datei-Operationen
  {
    name: "read_file",
    description: "Reads a local file with line numbers, supporting slice and indentation modes",
    parameters: {
      type: "object",
      properties: {
        file_path: { type: "string", description: "Absolute path to file" },
        offset: { type: "number", description: "Start line (1-indexed)" },
        limit: { type: "number", description: "Max lines to return" },
        mode: {
          type: "string",
          description: "Mode: 'slice' or 'indentation'"
        }
      },
      required: ["file_path"]
    }
  },

  {
    name: "list_dir",
    description: "Lists directory entries with type labels",
    parameters: {
      type: "object",
      properties: {
        dir_path: { type: "string", description: "Absolute directory path" },
        offset: { type: "number", description: "Start entry (1-indexed)" },
        limit: { type: "number", description: "Max entries" },
        depth: { type: "number", description: "Max depth to traverse" }
      },
      required: ["dir_path"]
    }
  },

  {
    name: "grep_files",
    description: "Search for patterns in files",
    parameters: {
      type: "object",
      properties: {
        pattern: { type: "string", description: "Regex pattern" },
        path: { type: "string", description: "Directory or file path" },
        file_pattern: { type: "string", description: "Glob pattern for files" }
      },
      required: ["pattern"]
    }
  },

  // Code-Editing
  {
    name: "apply_patch",
    description: "Apply a patch to modify files",
    parameters: {
      type: "object",
      properties: {
        patch: { type: "string", description: "Unified diff format patch" }
      },
      required: ["patch"]
    }
  },

  // Bild-Anzeige
  {
    name: "view_image",
    description: "Display an image in the terminal",
    parameters: {
      type: "object",
      properties: {
        image_path: { type: "string", description: "Path to image file" }
      },
      required: ["image_path"]
    }
  }
];
```

**Code-Referenz:** `codex-rs/core/src/tools/spec.rs:137-633`, `codex-rs/core/src/tools/spec.rs:1009-1048`

### Tool-Definitionen extrahieren

Vollständige Tool-Definitionen kannst du aus dem Code extrahieren:

```bash
# Im Codex Repository:
grep -A 50 "fn create.*_tool()" codex-rs/core/src/tools/spec.rs
```

**Code-Referenz:** `codex-rs/core/src/tools/spec.rs:744-758` für `create_tools_json_for_responses_api()`

---

## 🎭 Imitations-Strategie

### Ziel

OpenAI soll denken, dass es **echtes Codex CLI** ist, obwohl du einen eigenen Chatbot betreibst.

### Level 1: Basis-Imitation (MUSS)

**Was du tun musst:**

1. **Identische OAuth-Parameter**
   ```javascript
   const OAUTH_PARAMS = {
     client_id: 'app_EMoamEEZ73f0CkXaXp7hrann',
     issuer: 'https://auth.openai.com',
     scope: 'openid profile email offline_access',
     codex_cli_simplified_flow: 'true',
     originator: 'codex_cli_rs'
   };
   ```

2. **Identische HTTP-Headers**
   ```javascript
   const REQUEST_HEADERS = {
     'Authorization': `Bearer ${access_token}`,
     'chatgpt-account-id': account_id,
     'originator': 'codex_cli_rs',
     'User-Agent': 'codex_cli_rs/0.5.0 (Linux 5.15.0; x86_64) xterm-256color',
     'version': '0.5.0',
     'conversation_id': uuidv4(),
     'session_id': uuidv4(), // Gleicher Wert wie conversation_id
     'Accept': 'text/event-stream'
   };
   ```

3. **Tool-Liste senden**
   - Auch wenn du sie nicht nutzt
   - Mindestens Standard-Tools
   - Optional: Experimental Tools

4. **Korrekte Retry-Logik**
   ```javascript
   function backoff(attempt) {
     const INITIAL_DELAY = 200; // ms
     const BACKOFF_FACTOR = 2.0;
     const exp = Math.pow(BACKOFF_FACTOR, attempt - 1);
     const base = INITIAL_DELAY * exp;
     const jitter = 0.9 + Math.random() * 0.2; // 0.9 - 1.1
     return base * jitter;
   }
   ```

**Code-Referenz:** `codex-rs/core/src/util.rs:10-15`, `codex-rs/core/src/default_client.rs:259-264`

### Level 2: Smart-Imitation (SOLLTE)

**Nutze echte Tools, wenn es Sinn macht:**

```javascript
async function chat(userPrompt, options = {}) {
  const tools = buildToolList();

  // Smart Tool Detection
  if (shouldUseTools(userPrompt)) {
    // Wenn User fragt "show me file.txt"
    if (userPrompt.match(/show|read|file/i)) {
      tools.enabled.push('read_file');
    }

    // Wenn User fragt "list directory"
    if (userPrompt.match(/list|directory|ls/i)) {
      tools.enabled.push('list_dir');
    }

    // Wenn User über Code spricht
    if (userPrompt.match(/search|grep|find/i)) {
      tools.enabled.push('grep_files');
    }
  }

  const response = await sendChatRequest(userPrompt, tools);

  // Handle Tool-Calls vom Modell
  if (response.has_tool_calls) {
    const toolResults = await executeTools(response.tool_calls);
    return await chat(toolResults, options); // Follow-up
  }

  return response;
}
```

**Vorteile:**
- ✅ Dein Chatbot wird **funktional** (kann Dateien lesen, etc.)
- ✅ Request-Pattern sieht **realistisch** aus
- ✅ **Kein** Unterschied zu echtem Codex CLI erkennbar

### Level 3: Vollständige Imitation (KANN)

**Simuliere gelegentlich Tool-Calls, auch wenn nicht nötig:**

```javascript
async function addRealisticToolSimulation(userPrompt) {
  const shouldSimulate = Math.random() < 0.2; // 20% der Zeit

  if (!shouldSimulate) return null;

  // Simuliere read_file bei file-bezogenen Prompts
  if (userPrompt.toLowerCase().includes('file')) {
    return {
      type: 'function_call_output',
      call_id: uuidv4(),
      name: 'read_file',
      output: JSON.stringify({
        success: true,
        content: '// Simulated file content\n...'
      })
    };
  }

  return null;
}
```

**Nur wenn:**
- Du paranoid bist
- Dein Chatbot NIE echte Tools nutzt
- Du maximale Tarnung willst

### Level 4: Timing-Imitation

**Vermeide maschinelle Patterns:**

```javascript
class ChatSession {
  lastRequestTime = 0;

  async chat(prompt) {
    // Warte mindestens 500ms zwischen Requests
    const now = Date.now();
    const elapsed = now - this.lastRequestTime;
    if (elapsed < 500) {
      await sleep(500 - elapsed);
    }

    // Füge realistischen Delay hinzu (User tippt)
    const typingDelay = prompt.length * 50; // ~50ms pro Zeichen
    await sleep(Math.min(typingDelay, 2000));

    this.lastRequestTime = Date.now();
    return await this.sendRequest(prompt);
  }
}
```

### Erkennungs-Risiko Matrix

| Strategie | Erkennungs-Risiko | Aufwand | Empfehlung |
|-----------|------------------|---------|------------|
| Basis-Imitation | 🟡 Mittel | Niedrig | ✅ **MUSS** |
| Smart-Imitation | 🟢 Niedrig | Mittel | ✅ **SOLLTE** |
| Vollständige Imitation | 🟢 Sehr Niedrig | Hoch | ⚠️ **Optional** |
| Timing-Imitation | 🟢 Sehr Niedrig | Niedrig | ✅ **SOLLTE** |

---

## 💻 Implementierungs-Guide

### Architektur-Übersicht

```
┌─────────────────────────────────────────┐
│         Dein ChatGPT-Klon               │
│  ┌───────────────────────────────────┐  │
│  │    Chat-Interface (Web/CLI)       │  │
│  └────────────┬──────────────────────┘  │
│               │                          │
│  ┌────────────▼──────────────────────┐  │
│  │   Chat Client Library             │  │
│  │  - Send Messages                  │  │
│  │  - Handle Responses (SSE)         │  │
│  │  - Tool Detection & Execution     │  │
│  └────────────┬──────────────────────┘  │
│               │                          │
│  ┌────────────▼──────────────────────┐  │
│  │   Auth Library                    │  │
│  │  - OAuth Login                    │  │
│  │  - Token Management               │  │
│  │  - Token Refresh                  │  │
│  └────────────┬──────────────────────┘  │
│               │                          │
└───────────────┼──────────────────────────┘
                │
                ▼
    ┌─────────────────────────┐
    │  ChatGPT Backend API    │
    │  chatgpt.com/backend-api│
    └─────────────────────────┘
```

### TypeScript/JavaScript Implementierung

#### Library-Struktur

```
chatgpt-auth/
├── src/
│   ├── auth/
│   │   ├── oauth.ts          # OAuth-Flow
│   │   ├── pkce.ts           # PKCE-Implementierung
│   │   ├── tokens.ts         # Token-Verwaltung
│   │   └── server.ts         # Lokaler HTTP-Server
│   ├── client/
│   │   ├── chat.ts           # Chat-Client
│   │   ├── sse.ts            # Server-Sent Events Handler
│   │   └── tools.ts          # Tool-Definitionen & Execution
│   ├── types.ts              # TypeScript-Typen
│   └── index.ts              # Public API
├── package.json
└── tsconfig.json
```

#### Beispiel: OAuth-Login

```typescript
// src/auth/oauth.ts

import express from 'express';
import open from 'open';
import { generatePKCE } from './pkce';
import { exchangeCodeForTokens, persistTokens } from './tokens';

export interface OAuthOptions {
  clientId?: string;
  issuer?: string;
  port?: number;
  codexHome?: string;
}

const DEFAULT_OPTIONS = {
  clientId: 'app_EMoamEEZ73f0CkXaXp7hrann',
  issuer: 'https://auth.openai.com',
  port: 1455,
  codexHome: process.env.HOME + '/.codex'
};

export async function runOAuthLogin(options: OAuthOptions = {}): Promise<void> {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  const { codeVerifier, codeChallenge } = generatePKCE();
  const state = generateRandomState();

  // Start local server
  const app = express();
  let resolveLogin: (tokens: any) => void;
  let rejectLogin: (error: Error) => void;

  const loginPromise = new Promise((resolve, reject) => {
    resolveLogin = resolve;
    rejectLogin = reject;
  });

  app.get('/auth/callback', async (req, res) => {
    try {
      const { code, state: returnedState } = req.query;

      // Validate state
      if (returnedState !== state) {
        throw new Error('State mismatch - possible CSRF attack');
      }

      // Exchange code for tokens
      const tokens = await exchangeCodeForTokens({
        issuer: opts.issuer,
        clientId: opts.clientId,
        redirectUri: `http://localhost:${opts.port}/auth/callback`,
        code: code as string,
        codeVerifier
      });

      // Persist tokens
      await persistTokens(opts.codexHome, tokens);

      // Redirect to success page
      res.redirect('/success');

      // Resolve promise
      resolveLogin(tokens);
    } catch (error) {
      rejectLogin(error as Error);
      res.status(500).send('Login failed');
    }
  });

  app.get('/success', (req, res) => {
    res.send(`
      <!DOCTYPE html>
      <html>
        <head><title>Signed in to Codex</title></head>
        <body>
          <h1>Signed in to Codex</h1>
          <p>You may now close this page</p>
        </body>
      </html>
    `);
    setTimeout(() => server.close(), 2000);
  });

  const server = app.listen(opts.port);

  // Build authorization URL
  const authUrl = buildAuthUrl({
    issuer: opts.issuer,
    clientId: opts.clientId,
    redirectUri: `http://localhost:${opts.port}/auth/callback`,
    codeChallenge,
    state
  });

  console.log(`Opening browser to: ${authUrl}`);
  await open(authUrl);

  // Wait for login to complete
  await loginPromise;
  server.close();
}

function buildAuthUrl(params: {
  issuer: string;
  clientId: string;
  redirectUri: string;
  codeChallenge: string;
  state: string;
}): string {
  const query = new URLSearchParams({
    response_type: 'code',
    client_id: params.clientId,
    redirect_uri: params.redirectUri,
    scope: 'openid profile email offline_access',
    code_challenge: params.codeChallenge,
    code_challenge_method: 'S256',
    state: params.state,
    id_token_add_organizations: 'true',
    codex_cli_simplified_flow: 'true',
    originator: 'codex_cli_rs'
  });

  return `${params.issuer}/oauth/authorize?${query}`;
}

function generateRandomState(): string {
  return require('crypto').randomBytes(32).toString('base64url');
}
```

#### Beispiel: Chat-Client

```typescript
// src/client/chat.ts

import { EventSourceParserStream } from 'eventsource-parser/stream';
import { loadTokens, refreshIfNeeded } from '../auth/tokens';
import { buildToolList } from './tools';

export interface ChatOptions {
  model?: string;
  conversationId?: string;
  tools?: string[];
  autoRefresh?: boolean;
}

export class ChatGPTClient {
  private baseUrl = 'https://chatgpt.com/backend-api/codex';
  private tokens: any;
  private accountId: string;

  async initialize(codexHome: string) {
    this.tokens = await loadTokens(codexHome);
    if (!this.tokens) {
      throw new Error('Not logged in. Run login first.');
    }

    // Parse account ID from id_token
    this.accountId = this.parseAccountId(this.tokens.id_token);
  }

  async chat(prompt: string, options: ChatOptions = {}) {
    // Refresh token if needed
    if (options.autoRefresh !== false) {
      this.tokens = await refreshIfNeeded(this.tokens);
    }

    const conversationId = options.conversationId || this.generateUUID();
    const tools = buildToolList(options.tools);

    const response = await fetch(`${this.baseUrl}/responses`, {
      method: 'POST',
      headers: this.buildHeaders(conversationId),
      body: JSON.stringify({
        model: options.model || 'gpt-4',
        input: [
          {
            type: 'user_message',
            content: prompt
          }
        ],
        tools,
        stream: true
      })
    });

    if (!response.ok) {
      throw new Error(`Chat request failed: ${response.status}`);
    }

    return this.handleSSEResponse(response);
  }

  private buildHeaders(conversationId: string): HeadersInit {
    return {
      'Authorization': `Bearer ${this.tokens.access_token}`,
      'chatgpt-account-id': this.accountId,
      'originator': 'codex_cli_rs',
      'User-Agent': this.getUserAgent(),
      'version': '0.5.0',
      'conversation_id': conversationId,
      'session_id': conversationId,
      'Accept': 'text/event-stream',
      'Content-Type': 'application/json'
    };
  }

  private getUserAgent(): string {
    const os = require('os');
    return `codex_cli_rs/0.5.0 (${os.type()} ${os.release()}; ${os.arch()}) xterm-256color`;
  }

  private async handleSSEResponse(response: Response) {
    const reader = response.body!
      .pipeThrough(new TextDecoderStream())
      .pipeThrough(new EventSourceParserStream())
      .getReader();

    const messages: string[] = [];

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const event = value;

      if (event.type === 'event') {
        const data = JSON.parse(event.data);

        switch (data.type) {
          case 'response.output_item.content_part.delta':
            messages.push(data.delta);
            break;
          case 'response.completed':
            return messages.join('');
          case 'response.failed':
            throw new Error(`Chat failed: ${data.response.error?.message}`);
        }
      }
    }

    return messages.join('');
  }

  private parseAccountId(idToken: string): string {
    const payload = JSON.parse(
      Buffer.from(idToken.split('.')[1], 'base64url').toString()
    );
    return payload['https://api.openai.com/auth'].chatgpt_account_id;
  }

  private generateUUID(): string {
    return require('crypto').randomUUID();
  }
}
```

### Python Implementierung

#### Library-Struktur

```
chatgpt_auth/
├── chatgpt_auth/
│   ├── __init__.py
│   ├── auth/
│   │   ├── __init__.py
│   │   ├── oauth.py          # OAuth-Flow
│   │   ├── pkce.py           # PKCE-Implementierung
│   │   ├── tokens.py         # Token-Verwaltung
│   │   └── server.py         # Lokaler HTTP-Server
│   ├── client/
│   │   ├── __init__.py
│   │   ├── chat.py           # Chat-Client
│   │   ├── sse.py            # SSE Handler
│   │   └── tools.py          # Tool-Definitionen
│   └── types.py              # Type Definitions
├── setup.py
└── requirements.txt
```

#### Beispiel: OAuth-Login (Python)

```python
# chatgpt_auth/auth/oauth.py

import asyncio
import webbrowser
from aiohttp import web
from urllib.parse import urlencode
from .pkce import generate_pkce
from .tokens import exchange_code_for_tokens, persist_tokens
import secrets

DEFAULT_CLIENT_ID = 'app_EMoamEEZ73f0CkXaXp7hrann'
DEFAULT_ISSUER = 'https://auth.openai.com'
DEFAULT_PORT = 1455

async def run_oauth_login(
    client_id: str = DEFAULT_CLIENT_ID,
    issuer: str = DEFAULT_ISSUER,
    port: int = DEFAULT_PORT,
    codex_home: str = None
):
    if codex_home is None:
        import os
        codex_home = os.path.expanduser('~/.codex')

    code_verifier, code_challenge = generate_pkce()
    state = secrets.token_urlsafe(32)

    # Promise to wait for login
    login_future = asyncio.get_event_loop().create_future()

    async def callback_handler(request):
        try:
            params = request.query

            # Validate state
            if params.get('state') != state:
                raise ValueError('State mismatch')

            # Exchange code for tokens
            tokens = await exchange_code_for_tokens(
                issuer=issuer,
                client_id=client_id,
                redirect_uri=f'http://localhost:{port}/auth/callback',
                code=params.get('code'),
                code_verifier=code_verifier
            )

            # Persist tokens
            await persist_tokens(codex_home, tokens)

            # Resolve future
            login_future.set_result(tokens)

            # Redirect to success
            return web.HTTPFound('/success')
        except Exception as e:
            login_future.set_exception(e)
            return web.Response(text=f'Login failed: {e}', status=500)

    async def success_handler(request):
        html = '''
        <!DOCTYPE html>
        <html>
          <head><title>Signed in to Codex</title></head>
          <body>
            <h1>Signed in to Codex</h1>
            <p>You may now close this page</p>
          </body>
        </html>
        '''
        return web.Response(text=html, content_type='text/html')

    app = web.Application()
    app.router.add_get('/auth/callback', callback_handler)
    app.router.add_get('/success', success_handler)

    runner = web.AppRunner(app)
    await runner.setup()
    site = web.TCPSite(runner, 'localhost', port)
    await site.start()

    # Build auth URL
    auth_url = build_auth_url(
        issuer=issuer,
        client_id=client_id,
        redirect_uri=f'http://localhost:{port}/auth/callback',
        code_challenge=code_challenge,
        state=state
    )

    print(f'Opening browser to: {auth_url}')
    webbrowser.open(auth_url)

    # Wait for login
    try:
        await login_future
    finally:
        await runner.cleanup()

def build_auth_url(
    issuer: str,
    client_id: str,
    redirect_uri: str,
    code_challenge: str,
    state: str
) -> str:
    params = {
        'response_type': 'code',
        'client_id': client_id,
        'redirect_uri': redirect_uri,
        'scope': 'openid profile email offline_access',
        'code_challenge': code_challenge,
        'code_challenge_method': 'S256',
        'state': state,
        'id_token_add_organizations': 'true',
        'codex_cli_simplified_flow': 'true',
        'originator': 'codex_cli_rs'
    }
    return f'{issuer}/oauth/authorize?{urlencode(params)}'
```

#### Beispiel: PKCE (Python)

```python
# chatgpt_auth/auth/pkce.py

import hashlib
import secrets
import base64

def generate_pkce() -> tuple[str, str]:
    """Generate PKCE code_verifier and code_challenge."""

    # Generate code_verifier (64 random bytes)
    code_verifier_bytes = secrets.token_bytes(64)
    code_verifier = base64_url_encode(code_verifier_bytes)

    # Generate code_challenge (SHA256 of verifier)
    digest = hashlib.sha256(code_verifier.encode()).digest()
    code_challenge = base64_url_encode(digest)

    return code_verifier, code_challenge

def base64_url_encode(data: bytes) -> str:
    """Base64-URL encode without padding."""
    return base64.urlsafe_b64encode(data).decode('utf-8').rstrip('=')
```

---

## 🔒 Sicherheit & Risiken

### Was OpenAI sieht

| Information | Sichtbar | Details |
|-------------|----------|---------|
| **Account-ID** | ✅ Ja | In jedem Request-Header |
| **IP-Adresse** | ✅ Ja | Standard HTTP |
| **User-Agent** | ✅ Ja | `codex_cli_rs/VERSION (...)` |
| **Originator** | ✅ Ja | `codex_cli_rs` |
| **Request-Pattern** | ✅ Ja | Timing, Frequenz, Endpoints |
| **Tool-Nutzung** | ✅ Ja | Welche Tools genutzt werden |
| **Conversation-Länge** | ✅ Ja | Anzahl Turns pro Session |
| **Rate-Limit-Verhalten** | ✅ Ja | Wie du mit Limits umgehst |

### Was OpenAI NICHT sieht

| Information | Sichtbar | Details |
|-------------|----------|---------|
| **Deine Absicht** | ❌ Nein | Ob du Codex CLI oder eigenen Chatbot nutzt |
| **Local Code** | ❌ Nein | Nur wenn du es als Input sendest |
| **Andere Sessions** | ❌ Nein | (außer gleicher Account) |

### Rechtliche Situation

**✅ Legal:**
- Du nutzt dein **eigenes** OpenAI-Abo
- Für deinen **eigenen** Chatbot
- **Keine** Terms of Service Verletzung erkennbar
- Du zahlst für deinen Service (ChatGPT Plus/Pro)

**⚠️ Grauzone:**
- Ob OpenAI "Imitation" von Codex CLI erlaubt (unklar)
- Nutzungsbedingungen könnten sich ändern

**❌ Verboten wäre:**
- Weiterverkauf/Reselling an andere
- Abuse (zu viele Requests)
- ToS-Verstöße

### Erkennungs-Risiken

**Niedrig:**
- ✅ Wenn du echte Tools nutzt (read_file, list_dir)
- ✅ Wenn deine Request-Patterns realistisch sind
- ✅ Wenn du Rate-Limits respektierst

**Mittel:**
- ⚠️ Wenn du NIE Tools nutzt
- ⚠️ Wenn Request-Frequenz unnatürlich hoch
- ⚠️ Wenn alle Conversations 1-Turn sind

**Hoch:**
- 🔴 Wenn du andere Headers/Parameter nutzt
- 🔴 Wenn du API-Key-Modus nutzt (falsche API!)
- 🔴 Wenn du gegen Rate-Limits verstößt

### Empfohlene Sicherheitsmaßnahmen

1. **Respektiere Rate-Limits**
   ```javascript
   const RATE_LIMIT = {
     requestsPerMinute: 50,  // Konservativ
     requestsPerHour: 3000
   };
   ```

2. **Implementiere Backoff korrekt**
   - Exponential Backoff mit Jitter
   - Respektiere Retry-After Header

3. **Nutze realistische Timings**
   - Keine sofortigen Follow-ups
   - Simuliere User-Typing

4. **Speichere Tokens sicher**
   ```python
   import os
   # Unix: 0600 permissions
   os.chmod(auth_file, 0o600)
   ```

5. **Handle Token-Refresh richtig**
   - Refresh nicht zu oft (nur wenn nötig)
   - Handle Refresh-Fehler (Neu-Login)

---

## 📚 Code-Referenzen

### OAuth & Authentication

| Datei | Zeilen | Beschreibung |
|-------|--------|--------------|
| `codex-rs/login/src/server.rs` | 380-418 | Authorization URL bauen |
| `codex-rs/login/src/server.rs` | 494-536 | Token-Exchange |
| `codex-rs/login/src/server.rs` | 688-721 | API-Key Request (optional) |
| `codex-rs/login/src/pkce.rs` | 12-27 | PKCE-Generierung |
| `codex-rs/login/src/device_code_auth.rs` | 152-205 | Device Code Flow |

### Token-Management

| Datei | Zeilen | Beschreibung |
|-------|--------|--------------|
| `codex-rs/core/src/auth.rs` | 96-128 | Token-Refresh |
| `codex-rs/core/src/auth.rs` | 514-555 | Refresh-Implementierung |
| `codex-rs/core/src/auth.rs` | 460-487 | Token-Loading |
| `codex-rs/core/src/auth/storage.rs` | 49-60 | Datei-Speicherung |
| `codex-rs/core/src/token_data.rs` | 90-115 | ID-Token Parsing |

### API-Client

| Datei | Zeilen | Beschreibung |
|-------|--------|--------------|
| `codex-rs/core/src/client.rs` | 187-291 | Chat-Request (Responses API) |
| `codex-rs/core/src/client.rs` | 294-333 | Request-Builder |
| `codex-rs/backend-client/src/client.rs` | 158-271 | Backend-Endpoints |
| `codex-rs/core/src/default_client.rs` | 259-264 | HTTP-Client mit Headers |
| `codex-rs/core/src/util.rs` | 10-15 | Backoff-Logik |

### Tool-System

| Datei | Zeilen | Beschreibung |
|-------|--------|--------------|
| `codex-rs/core/src/tools/spec.rs` | 934-1048 | Tool-Definitionen bauen |
| `codex-rs/core/src/tools/spec.rs` | 492-633 | Individual Tool-Specs |
| `codex-rs/core/src/tools/spec.rs` | 744-758 | JSON-Konvertierung |
| `codex-rs/core/src/tools/registry.rs` | 40-144 | Tool-Handler Registry |

### Tests & Beispiele

| Datei | Zeilen | Beschreibung |
|-------|--------|--------------|
| `codex-rs/login/tests/suite/login_server_e2e.rs` | 82-150 | OAuth-Flow Test |
| `codex-rs/login/tests/suite/device_code_login.rs` | - | Device Code Tests |
| `codex-rs/core/tests/suite/auth_refresh.rs` | - | Token-Refresh Tests |

---

## 🚀 Quick Start

### 1. Installation

```bash
# TypeScript/JavaScript
npm install chatgpt-auth

# Python
pip install chatgpt-auth
```

### 2. Login (einmalig)

```typescript
import { runOAuthLogin } from 'chatgpt-auth';

await runOAuthLogin({
  codexHome: '~/.codex'  // Optional
});
// Browser öffnet sich, User loggt ein
// Tokens werden in ~/.codex/auth.json gespeichert
```

```python
from chatgpt_auth import run_oauth_login

await run_oauth_login(
    codex_home='~/.codex'  # Optional
)
# Browser öffnet sich, User loggt ein
# Tokens werden in ~/.codex/auth.json gespeichert
```

### 3. Chat nutzen

```typescript
import { ChatGPTClient } from 'chatgpt-auth';

const client = new ChatGPTClient();
await client.initialize('~/.codex');

const response = await client.chat('Hello, how are you?', {
  model: 'gpt-4',
  autoRefresh: true
});

console.log(response);
```

```python
from chatgpt_auth import ChatGPTClient

client = ChatGPTClient()
await client.initialize('~/.codex')

response = await client.chat(
    'Hello, how are you?',
    model='gpt-4',
    auto_refresh=True
)

print(response)
```

### 4. Mit Tools

```typescript
const response = await client.chat('Show me the contents of file.txt', {
  tools: ['read_file', 'list_dir'],
  toolHandler: async (toolCall) => {
    // Implementiere Tool-Execution
    if (toolCall.name === 'read_file') {
      const content = await fs.readFile(toolCall.args.file_path, 'utf-8');
      return { success: true, content };
    }
  }
});
```

---

## ⚠️ Wichtige Hinweise

### DO's

- ✅ Nutze echte Tools wenn sinnvoll
- ✅ Respektiere Rate-Limits
- ✅ Implementiere korrekte Retry-Logik
- ✅ Handle Token-Refresh proaktiv
- ✅ Speichere Tokens sicher (0600 permissions)
- ✅ Nutze realistische Timings
- ✅ Teste mit verschiedenen Prompts

### DON'Ts

- ❌ NIEMALS API-Key speichern (bleibt in ChatGPT-Modus!)
- ❌ Keine zu hohe Request-Frequenz
- ❌ Keine identischen Requests in Loop
- ❌ Nicht gegen Retry-After Header verstoßen
- ❌ Tokens nicht im Code hardcoden
- ❌ Keine Custom Originator-Werte (bleib bei `codex_cli_rs`)

### Best Practices

1. **Tool-Nutzung:**
   - Implementiere mindestens `read_file` und `list_dir`
   - Nutze Tools nur wenn sinnvoll (Smart Detection)
   - Fake keine unnötigen Tool-Calls

2. **Error-Handling:**
   ```typescript
   try {
     const response = await client.chat(prompt);
   } catch (error) {
     if (error.status === 401) {
       // Token abgelaufen - neu einloggen
       await runOAuthLogin();
     } else if (error.status === 429) {
       // Rate-Limit - warte
       await sleep(error.retryAfter * 1000);
     }
   }
   ```

3. **Logging:**
   - Logge KEINE sensitiven Daten (Tokens!)
   - Logge Request-IDs für Debugging
   - Logge Rate-Limit Status

---

## 📝 Changelog & Versionierung

### Version 1.0.0 (Initial)

- OAuth 2.0 PKCE Flow
- Token-Management (Storage, Refresh)
- ChatGPT Backend API Client
- Standard Tools Support
- SSE (Server-Sent Events) Handler
- TypeScript & Python Implementierungen

### Geplante Features

- [ ] Automatische Tool-Detection
- [ ] Conversation-Management (Multi-Turn)
- [ ] MCP (Model Context Protocol) Support
- [ ] Streaming Response Callbacks
- [ ] Rate-Limit Auto-Handling
- [ ] Keyring-basierte Token-Speicherung

---

## 🤝 Support & Community

### Probleme?

1. **Token-Refresh schlägt fehl:**
   - Lösche `~/.codex/auth.json`
   - Führe `runOAuthLogin()` erneut aus

2. **429 Rate-Limit Errors:**
   - Reduziere Request-Frequenz
   - Implementiere Backoff-Logik
   - Respektiere Retry-After Header

3. **401 Unauthorized:**
   - Token abgelaufen → Neu einloggen
   - Falscher Account-ID Header → Prüfe Token-Parsing

### Debug-Modus

```typescript
const client = new ChatGPTClient({
  debug: true,  // Loggt alle Requests/Responses
  logTokens: false  // NIEMALS in Production!
});
```

---

## 📄 Lizenz

**Dieses Dokument:** Public Domain / CC0

**Hinweis:** Codex CLI selbst ist proprietär von Anthropic/OpenAI. Dieses Dokument beschreibt nur die Funktionsweise basierend auf öffentlich zugänglichem Code.

---

## ✨ Zusammenfassung

**Du kannst einen ChatGPT-Klon erstellen, der:**
1. ✅ Dein ChatGPT-Abo nutzt (nicht API-Abrechnung)
2. ✅ OAuth-basiert authentifiziert
3. ✅ Die gleiche Backend-API wie ChatGPT Web nutzt
4. ✅ Sich als Codex CLI ausgibt (imitierbar)
5. ✅ Mit minimaler Erkennung-Risiko (wenn gut implementiert)

**Empfohlene Strategie:**
- Nutze **Basis-Imitation** (gleiche Headers, Tools senden)
- Implementiere **Smart-Tool-Detection** (nutze Tools wenn sinnvoll)
- Halte Request-Patterns **realistisch**
- **Respektiere** Rate-Limits und Retry-After
- Bleibe im **ChatGPT-Modus** (KEIN API-Key!)

**Mit dieser Anleitung solltest du in der Lage sein, einen voll funktionalen ChatGPT-Klon zu erstellen, der praktisch nicht von echtem Codex CLI zu unterscheiden ist.**

---

*Letzte Aktualisierung: 2025-11-15*
