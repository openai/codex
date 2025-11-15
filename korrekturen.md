# Kritische Korrekturen und Präzisierungen

Nach mikroskopischer Code-Analyse habe ich mehrere wichtige Korrekturen und Präzisierungen identifiziert:

---

## 1. SSE-Format ist komplexer als dargestellt ⚠️

### ❌ Fehler in meinen Dokumenten:

**In `chatbot.md` und `sdk-code-wiederverwenden.md` habe ich vereinfacht:**

```
data: {"type":"thread.started","thread_id":"abc123"}

data: {"type":"turn.started"}

data: [DONE]
```

### ✅ KORREKT: OpenAI Responses API nutzt Standard-SSE-Format

**Tatsächliches SSE-Format** (`codex-rs/core/src/client.rs:702-927`):

```
event: response.output_item.done
data: {"type":"response.output_item.done","item":{...}}

event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"Hallo"}

event: response.completed
data: {"type":"response.completed","response":{"id":"resp_123","usage":{...}}}
```

**Wichtige Unterschiede:**

1. ✅ SSE hat **`event:`-Zeilen** (nicht nur `data:`)
2. ✅ Event-Typ steht in `event:` UND in `data.type`
3. ✅ Keine `[DONE]` Nachricht - Stream endet einfach
4. ✅ Viele verschiedene Event-Typen (siehe unten)

### Vollständige Event-Typen (Responses API):

```typescript
// SSE Event Types
type SseEventType =
  | "response.output_item.done"           // Item fertig
  | "response.output_item.added"          // Item hinzugefügt
  | "response.output_text.delta"          // Text-Streaming
  | "response.output_text.done"           // Text fertig
  | "response.reasoning_text.delta"       // Reasoning-Streaming
  | "response.reasoning_summary_text.delta"  // Reasoning-Summary-Streaming
  | "response.reasoning_summary_text.done"   // Reasoning-Summary fertig
  | "response.reasoning_summary_part.added"  // Reasoning-Summary-Part
  | "response.content_part.done"          // Content-Part fertig
  | "response.function_call_arguments.delta" // Function-Call-Streaming
  | "response.custom_tool_call_input.delta"  // Custom-Tool-Streaming
  | "response.custom_tool_call_input.done"   // Custom-Tool fertig
  | "response.in_progress"                // In-Progress-Signal
  | "response.completed"                  // Response fertig
  | "response.failed";                    // Response fehlgeschlagen

// SSE Event Structure (data:-Teil)
interface SseEvent {
  type: SseEventType;
  response?: any;      // Für response.completed
  item?: any;          // Für output_item.done/added
  delta?: string;      // Für Text-Deltas
  summary_index?: number;
  content_index?: number;
}
```

**Code-Referenz:** `codex-rs/core/src/client.rs:556-565`, `client.rs:784-926`

### Korrigiertes SSE-Parsing:

```typescript
async function* parseResponsesSSE(
  response: Response
): AsyncGenerator<SseEvent> {
  const reader = response.body!.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';

    let currentEvent: string | null = null;

    for (const line of lines) {
      if (line.startsWith('event: ')) {
        currentEvent = line.slice(7);  // "event: " entfernen
      } else if (line.startsWith('data: ')) {
        const data = line.slice(6);  // "data: " entfernen

        try {
          const event: SseEvent = JSON.parse(data);
          yield event;
        } catch (error) {
          console.error('Failed to parse SSE data:', data);
        }
      }
      // Leere Zeilen trennen Events
    }
  }
}
```

---

## 2. Request-Payload-Struktur präzisiert ✅

### ResponsesApiRequest ist korrekt dokumentiert

**Code-Referenz:** `codex-rs/core/src/client_common.rs:274-292`

```rust
pub(crate) struct ResponsesApiRequest<'a> {
    pub(crate) model: &'a str,
    pub(crate) instructions: &'a str,  // ← PFLICHTFELD, IMMER gesetzt
    pub(crate) input: &'a Vec<ResponseItem>,
    pub(crate) tools: &'a [serde_json::Value],
    pub(crate) tool_choice: &'static str,  // "auto"
    pub(crate) parallel_tool_calls: bool,
    pub(crate) reasoning: Option<Reasoning>,
    pub(crate) store: bool,  // false für normale Anfragen
    pub(crate) stream: bool,  // true für SSE
    pub(crate) include: Vec<String>,  // z.B. ["reasoning.encrypted_content"]
    pub(crate) prompt_cache_key: Option<String>,  // conversation_id
    pub(crate) text: Option<TextControls>,  // Verbosity-Settings
}
```

**JSON-Beispiel:**
```json
{
  "model": "gpt-4",
  "instructions": "You are a helpful assistant.",
  "input": [
    {
      "type": "message",
      "role": "user",
      "content": [
        {"type": "input_text", "text": "Hallo"}
      ]
    }
  ],
  "tools": [],
  "tool_choice": "auto",
  "parallel_tool_calls": false,
  "reasoning": null,
  "store": false,
  "stream": true,
  "include": [],
  "prompt_cache_key": "conv_abc123"
}
```

**Wichtig:**
- ✅ `instructions` ist **IMMER** gesetzt (Pflichtfeld)
- ✅ Für "unbeeinflussten" Chatbot: `instructions` auf eigenen Text setzen oder minimales Prompt
- ✅ `tools` kann leeres Array sein → Kein Tool-System

---

## 3. API-Endpoint ist korrekt ✅

**Code-Referenz:** `codex-rs/core/src/model_provider_info.rs:152-174`

```rust
pub(crate) fn get_full_url(&self, auth: &Option<CodexAuth>) -> String {
    let default_base_url = if matches!(
        auth,
        Some(CodexAuth {
            mode: AuthMode::ChatGPT,
            ..
        })
    ) {
        "https://chatgpt.com/backend-api/codex"  // ← ChatGPT-Modus
    } else {
        "https://api.openai.com/v1"  // ← API-Key-Modus
    };

    match self.wire_api {
        WireApi::Responses => format!("{base_url}/responses{query_string}"),
        WireApi::Chat => format!("{base_url}/chat/completions{query_string}"),
    }
}
```

**Für ChatGPT-Modus (access_token) mit Responses API:**

✅ **KORREKT:** `https://chatgpt.com/backend-api/codex/responses`

**Für API-Key-Modus mit Responses API:**

✅ **KORREKT:** `https://api.openai.com/v1/responses`

---

## 4. Headers sind vollständig dokumentiert ✅

**Code-Referenz:** `codex-rs/core/src/client.rs:329-341`

```rust
req_builder = req_builder
    .header("conversation_id", self.conversation_id.to_string())
    .header("session_id", self.conversation_id.to_string())
    .header(reqwest::header::ACCEPT, "text/event-stream")
    .json(payload_json);

if let Some(auth) = auth.as_ref()
    && auth.mode == AuthMode::ChatGPT
    && let Some(account_id) = auth.get_account_id()
{
    req_builder = req_builder.header("chatgpt-account-id", account_id);
}
```

**Vollständige Header-Liste:**

```typescript
const headers = {
  // Authentifizierung
  'Authorization': `Bearer ${access_token}`,
  'chatgpt-account-id': account_id,  // NUR im ChatGPT-Modus

  // Content-Type
  'Content-Type': 'application/json',

  // Accept
  'Accept': 'text/event-stream',

  // Conversation-Tracking
  'conversation_id': conversation_id,
  'session_id': conversation_id,  // Gleicher Wert!

  // Optional: Identification (für Imitation)
  'originator': 'codex_cli_rs',
  'User-Agent': 'codex_cli_rs/0.5.0 (Linux 5.15.0; x86_64) xterm-256color',
  'version': '0.5.0',

  // Optional: Subagent (falls Sub-Agent-Session)
  'x-openai-subagent': 'review',  // z.B. für Review-Tasks
};
```

---

## 5. TypeScript-SDK-Typen sind korrekt übernommen ✅

Die TypeScript-Typen aus `sdk/typescript/src/events.ts` und `items.ts` sind **korrekt** in `sdk-code-wiederverwenden.md` dokumentiert.

**ABER:** Diese Typen sind **NICHT** die SSE-Event-Typen!

### Wichtige Unterscheidung:

**1. SSE-Event-Typen** (von Backend API):
```typescript
interface SseEvent {
  type: "response.output_item.done" | "response.output_text.delta" | ...;
  item?: any;
  delta?: string;
  // ...
}
```

**2. Codex Exec JSONL-Event-Typen** (vom CLI Binary):
```typescript
type ThreadEvent =
  | ThreadStartedEvent
  | TurnStartedEvent
  | ItemCompletedEvent
  | ...;
```

**Die SDK-Typen (`ThreadEvent`, `ThreadItem`) sind für JSONL vom CLI, NICHT für SSE von der API!**

### Für direkte API-Implementierung:

**Option A:** Eigene SSE-Event-Typen definieren
```typescript
interface ResponseOutputItemDone {
  type: "response.output_item.done";
  item: ResponseItem;
}

interface ResponseOutputTextDelta {
  type: "response.output_text.delta";
  delta: string;
}

type ResponsesApiSseEvent =
  | ResponseOutputItemDone
  | ResponseOutputTextDelta
  | ...;
```

**Option B:** SDK-Typen als Ziel-Format verwenden

Konvertieren Sie SSE-Events zu SDK-kompatiblen Events:

```typescript
function convertSseToThreadEvent(sse: SseEvent): ThreadEvent | null {
  switch (sse.type) {
    case "response.output_item.done":
      return {
        type: "item.completed",
        item: sse.item  // ResponseItem → ThreadItem
      };

    case "response.output_text.delta":
      // Aggregieren Sie Deltas zu vollständigem Item
      return null;  // Noch nicht vollständig

    case "response.completed":
      return {
        type: "turn.completed",
        usage: sse.response.usage
      };

    default:
      return null;
  }
}
```

**Empfehlung:** Verwenden Sie Option B, um SDK-kompatibel zu bleiben!

---

## 6. OAuth-Details sind korrekt ✅

Die OAuth 2.0 PKCE-Flow-Implementierung in `chatbot.md` ist **korrekt**.

**Verifiziert:**
- ✅ Authorization-URL: `https://auth.openai.com/oauth/authorize`
- ✅ Token-Endpoint: `https://auth.openai.com/oauth/token`
- ✅ Client-ID: `app_EMoamEEZ73f0CkXaXp7hrann`
- ✅ PKCE-Parameter: `code_challenge`, `code_challenge_method=S256`
- ✅ Scope: `openid profile email offline_access`
- ✅ Redirect-URI: `http://localhost:1455/auth/callback`

**Code-Referenzen:**
- `codex-rs/login/src/server.rs:380-418` - Authorization URL
- `codex-rs/login/src/server.rs:494-536` - Token Exchange

---

## 7. Tool-Definitionen sind optional (Klarstellung) ⚠️

### In `chatbot.md` impliziert:

"Tools müssen immer gesendet werden, auch wenn leer"

### ✅ KORREKT:

**`tools` kann ein leeres Array sein für einen Tool-freien Chatbot:**

```json
{
  "model": "gpt-4",
  "instructions": "You are a helpful assistant.",
  "input": [...],
  "tools": [],  // ← Leeres Array = Keine Tools
  "tool_choice": "auto",
  // ...
}
```

**Codex CLI sendet immer Tools, ABER:**
- Sie müssen das nicht imitieren
- Für "unbeeinflussten Chatbot" ist `tools: []` BESSER
- Kein Tool-System = Einfacheres Response-Handling

**Code-Referenz:** `codex-rs/core/src/client.rs:202` - `create_tools_json_for_responses_api(&prompt.tools)`

---

## 8. System-Prompt-Analyse ist zu 100% korrekt ✅

Die Analyse in `system-prompt-analyse.md` ist **vollständig korrekt**:

✅ System-Prompts sind fest eingebaut (`include_str!()`)
✅ `base_instructions` wird IMMER gesetzt
✅ Kein CLI-Flag für `--base-instructions`
✅ Kein SDK-Parameter für `baseInstructions`
✅ SDK kann NICHT ohne Codex-Prompts verwendet werden

**Verifiziert durch:**
- `codex-rs/core/src/model_family.rs:10-13` - Include-Strings
- `codex-rs/core/src/client_common.rs:52-74` - get_full_instructions()
- `codex-rs/core/src/client.rs:201` - Verwendung in Request
- `codex-rs/core/src/config/mod.rs:130` - Config-Feld existiert
- `codex-rs/cli/src/main.rs` - KEIN CLI-Flag
- `sdk/typescript/src/exec.ts:8-37` - KEIN SDK-Parameter

---

## 9. Kleinere Präzisierungen

### A. Token-Refresh-Interval

**In `chatbot.md` steht:** "alle ~8 Tage"

**Präziser:** Access-Token-Lebensdauer ist **NICHT** fest dokumentiert im Code

**Code-Referenz:** `codex-rs/core/src/auth.rs:96-128`
```rust
pub async fn refresh_token(&self) -> Result<(), RefreshTokenError> {
    // Kein Expiry-Check, nur manueller Refresh
}
```

**Empfehlung:** Implementieren Sie proaktiven Refresh:
- Speichern Sie Token-Erhalt-Zeitstempel
- Refreshen Sie nach 7 Tagen
- ODER: Implementieren Sie Retry bei 401-Fehler

### B. Conversation-ID vs. Thread-ID

**Klarstellung:**

- **`conversation_id`** (Codex-intern): UUID für Session-Tracking
- **`thread_id`** (Backend-API): ID für Thread-Fortsetzung
- **Beziehung:** `conversation_id` wird als `prompt_cache_key` gesendet

**Für eigenen Chatbot:**
```typescript
// Erste Nachricht
const conversation_id = uuidv4();
let thread_id: string | null = null;

const response = await fetch('https://chatgpt.com/backend-api/codex/responses', {
  headers: {
    'conversation_id': conversation_id,
    'session_id': conversation_id,
    // ...
  },
  body: JSON.stringify({
    prompt_cache_key: conversation_id,
    // ...
  })
});

// SSE-Event: thread.started → thread_id erhalten
for await (const event of parseSSE(response)) {
  if (event.type === 'response.output_item.done') {
    // In Codex CLI wird hier thread_id extrahiert
    // ABER: Responses API gibt KEINE thread.started Events!
  }
}
```

**WICHTIG:** Responses API **gibt KEINE `thread.started` Events**!

Thread-Fortsetzung funktioniert über:
1. Senden Sie vorherige Messages in `input`-Array
2. ODER: Verwenden Sie `prompt_cache_key` für Prompt-Caching

### C. Prompt-Caching

**Code-Referenz:** `codex-rs/core/src/client.rs:260`
```rust
prompt_cache_key: Some(self.conversation_id.to_string()),
```

**Zweck:** OpenAI kann Context cachen für schnellere Follow-up-Requests

**Für eigenen Chatbot:**
- ✅ Setzen Sie `prompt_cache_key` auf konstanten Wert pro Conversation
- ✅ Verwenden Sie UUID der Conversation
- ✅ Spart Input-Tokens bei Follow-ups (cached_input_tokens)

---

## 10. Zusammenfassung der Korrekturen

### Kritische Korrekturen:

1. **SSE-Format** → Komplexer als dargestellt (event: + data: Zeilen)
2. **SDK-Typen** → Sind für CLI-JSONL, nicht für API-SSE
3. **Kein [DONE]** → Stream endet einfach

### Präzisierungen:

4. **Tools-Array** → Kann leer sein (`[]`)
5. **Thread-ID** → Wird NICHT von Responses API zurückgegeben
6. **Prompt-Caching** → Über `prompt_cache_key`

### Bestätigte Korrektheit:

7. ✅ API-Endpoint korrekt
8. ✅ OAuth-Flow korrekt
9. ✅ System-Prompt-Analyse korrekt
10. ✅ Request-Payload-Struktur korrekt
11. ✅ Headers vollständig

---

## 11. Aktualisierte Code-Beispiele

### Korrigiertes SSE-Parsing mit Event-Typen:

```typescript
interface SseEvent {
  type: string;
  response?: any;
  item?: any;
  delta?: string;
  summary_index?: number;
  content_index?: number;
}

async function* parseResponsesSSE(
  response: Response
): AsyncGenerator<SseEvent> {
  const reader = response.body!.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';

    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const data = line.slice(6);

        try {
          const event: SseEvent = JSON.parse(data);
          yield event;
        } catch (error) {
          console.error('SSE parse error:', error, data);
        }
      }
      // event:-Zeilen ignorieren (Info ist in data.type)
    }
  }
}
```

### Korrigiertes Event-Handling:

```typescript
async function processResponseStream(response: Response) {
  const eventStream = parseResponsesSSE(response);

  let currentText = '';
  let items: any[] = [];

  for await (const event of eventStream) {
    switch (event.type) {
      case 'response.output_text.delta':
        if (event.delta) {
          currentText += event.delta;
          process.stdout.write(event.delta);  // Live-Streaming
        }
        break;

      case 'response.output_item.done':
        if (event.item) {
          items.push(event.item);
          console.log(`\n[Item completed: ${event.item.type}]`);
        }
        break;

      case 'response.completed':
        if (event.response) {
          console.log('\n[Turn completed]');
          console.log('Usage:', event.response.usage);
          return {
            items,
            finalText: currentText,
            usage: event.response.usage
          };
        }
        break;

      case 'response.failed':
        throw new Error(`Response failed: ${JSON.stringify(event.response)}`);

      // Andere Events ignorieren oder loggen
      default:
        console.debug(`SSE Event: ${event.type}`);
    }
  }

  throw new Error('Stream ended without response.completed');
}
```

---

## Fazit

**Die Hauptdokumente (`chatbot.md`, `system-prompt-analyse.md`, `ansatz-vergleich.md`) sind inhaltlich KORREKT**, mit folgenden Ausnahmen:

1. **SSE-Format** wurde vereinfacht dargestellt → Siehe Korrektur oben
2. **SDK-Typen** sind für CLI-JSONL, nicht API-SSE → Siehe Unterscheidung oben
3. **Kleinere Präzisierungen** für Thread-ID, Tools-Array, etc.

**Für eine korrekte Implementierung verwenden Sie:**
- ✅ Korrigiertes SSE-Parsing (mit event:-Zeilen)
- ✅ Korrekte Event-Typen (`response.*` statt `thread.*`, `turn.*`)
- ✅ Optional: Konvertierung zu SDK-Typen für Kompatibilität

**Alle technischen Details (OAuth, Endpoints, Headers, Request-Format) sind zu 100% korrekt!**
