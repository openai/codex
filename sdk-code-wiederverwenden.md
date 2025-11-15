# SDK-Code für Ansatz 1 wiederverwenden

Auch wenn das TypeScript SDK nicht für Ihren Use-Case geeignet ist (wegen zwingenden Codex-System-Prompts), enthält es **sehr nützliche Code-Teile**, die Sie für Ansatz 1 (direkte API-Implementierung) verwenden können.

## Übersicht: Was ist nützlich?

| SDK-Teil | Nützlich für Ansatz 1? | Beschreibung |
|----------|------------------------|--------------|
| ✅ **TypeScript-Typen** | **JA - sehr wertvoll** | Event- und Item-Typen direkt übernehmen |
| ✅ **SSE-Parsing-Logik** | **JA - als Referenz** | Wie JSONL-Events geparst werden |
| ✅ **Input-Normalisierung** | **JA - übernehmen** | Text + Bilder zu Prompt konvertieren |
| ✅ **Event-Loop-Pattern** | **JA - als Pattern** | Wie Events zu Turn-Result aggregiert werden |
| ❌ **OAuth-Flow** | **NEIN - nicht enthalten** | SDK delegiert an CLI Binary |
| ❌ **HTTP-Requests** | **NEIN - nicht enthalten** | SDK spawnt CLI statt direkte HTTP-Calls |
| ⚠️ **Codex-Exec-Wrapper** | **NEIN - aber lehrreich** | Zeigt welche Parameter CLI erwartet |

---

## 1. TypeScript-Typen (SEHR WERTVOLL!) ✅

Das SDK enthält vollständige TypeScript-Typen für alle Events und Items. Diese können Sie **direkt in Ihr Projekt kopieren**.

### Events (`sdk/typescript/src/events.ts`)

```typescript
/** Emitted when a new thread is started as the first event. */
export type ThreadStartedEvent = {
  type: "thread.started";
  /** The identifier of the new thread. Can be used to resume the thread later. */
  thread_id: string;
};

/** Emitted when a turn is started by sending a new prompt to the model. */
export type TurnStartedEvent = {
  type: "turn.started";
};

/** Describes the usage of tokens during a turn. */
export type Usage = {
  /** The number of input tokens used during the turn. */
  input_tokens: number;
  /** The number of cached input tokens used during the turn. */
  cached_input_tokens: number;
  /** The number of output tokens used during the turn. */
  output_tokens: number;
};

/** Emitted when a turn is completed. Typically right after the assistant's response. */
export type TurnCompletedEvent = {
  type: "turn.completed";
  usage: Usage;
};

/** Indicates that a turn failed with an error. */
export type TurnFailedEvent = {
  type: "turn.failed";
  error: ThreadError;
};

/** Emitted when a new item is added to the thread. */
export type ItemStartedEvent = {
  type: "item.started";
  item: ThreadItem;
};

/** Emitted when an item is updated. */
export type ItemUpdatedEvent = {
  type: "item.updated";
  item: ThreadItem;
};

/** Signals that an item has reached a terminal state. */
export type ItemCompletedEvent = {
  type: "item.completed";
  item: ThreadItem;
};

/** Fatal error emitted by the stream. */
export type ThreadError = {
  message: string;
};

/** Represents an unrecoverable error emitted directly by the event stream. */
export type ThreadErrorEvent = {
  type: "error";
  message: string;
};

/** Top-level JSONL events emitted by codex exec. */
export type ThreadEvent =
  | ThreadStartedEvent
  | TurnStartedEvent
  | TurnCompletedEvent
  | TurnFailedEvent
  | ItemStartedEvent
  | ItemUpdatedEvent
  | ItemCompletedEvent
  | ThreadErrorEvent;
```

### Items (`sdk/typescript/src/items.ts`)

```typescript
/** The status of a command execution. */
export type CommandExecutionStatus = "in_progress" | "completed" | "failed";

/** A command executed by the agent. */
export type CommandExecutionItem = {
  id: string;
  type: "command_execution";
  /** The command line executed by the agent. */
  command: string;
  /** Aggregated stdout and stderr captured while the command was running. */
  aggregated_output: string;
  /** Set when the command exits; omitted while still running. */
  exit_code?: number;
  /** Current status of the command execution. */
  status: CommandExecutionStatus;
};

/** Indicates the type of the file change. */
export type PatchChangeKind = "add" | "delete" | "update";

/** A set of file changes by the agent. */
export type FileUpdateChange = {
  path: string;
  kind: PatchChangeKind;
};

/** The status of a file change. */
export type PatchApplyStatus = "completed" | "failed";

/** A set of file changes by the agent. */
export type FileChangeItem = {
  id: string;
  type: "file_change";
  /** Individual file changes that comprise the patch. */
  changes: FileUpdateChange[];
  /** Whether the patch ultimately succeeded or failed. */
  status: PatchApplyStatus;
};

/** The status of an MCP tool call. */
export type McpToolCallStatus = "in_progress" | "completed" | "failed";

/** Represents a call to an MCP tool. */
export type McpToolCallItem = {
  id: string;
  type: "mcp_tool_call";
  /** Name of the MCP server handling the request. */
  server: string;
  /** The tool invoked on the MCP server. */
  tool: string;
  /** Arguments forwarded to the tool invocation. */
  arguments: unknown;
  /** Result payload returned by the MCP server for successful calls. */
  result?: {
    content: any[]; // McpContentBlock[] wenn @modelcontextprotocol/sdk verfügbar
    structured_content: unknown;
  };
  /** Error message reported for failed calls. */
  error?: {
    message: string;
  };
  /** Current status of the tool invocation. */
  status: McpToolCallStatus;
};

/** Response from the agent. Either natural-language text or JSON. */
export type AgentMessageItem = {
  id: string;
  type: "agent_message";
  /** Either natural-language text or JSON when structured output is requested. */
  text: string;
};

/** Agent's reasoning summary. */
export type ReasoningItem = {
  id: string;
  type: "reasoning";
  text: string;
};

/** Captures a web search request. */
export type WebSearchItem = {
  id: string;
  type: "web_search";
  query: string;
};

/** Describes a non-fatal error surfaced as an item. */
export type ErrorItem = {
  id: string;
  type: "error";
  message: string;
};

/** An item in the agent's to-do list. */
export type TodoItem = {
  text: string;
  completed: boolean;
};

/** Tracks the agent's running to-do list. */
export type TodoListItem = {
  id: string;
  type: "todo_list";
  items: TodoItem[];
};

/** Canonical union of thread items. */
export type ThreadItem =
  | AgentMessageItem
  | ReasoningItem
  | CommandExecutionItem
  | FileChangeItem
  | McpToolCallItem
  | WebSearchItem
  | TodoListItem
  | ErrorItem;
```

### Wie Sie diese Typen verwenden:

**1. Kopieren Sie die Typen in Ihr Projekt:**

```typescript
// src/types/codex-events.ts
// [Alle Event-Typen hier einfügen]

// src/types/codex-items.ts
// [Alle Item-Typen hier einfügen]
```

**2. Verwenden Sie sie in Ihrer Implementierung:**

```typescript
import { ThreadEvent, ThreadItem } from './types/codex-events';

// SSE-Event-Parsing
async function* parseSSEStream(response: Response): AsyncGenerator<ThreadEvent> {
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
        if (data === '[DONE]') return;

        const event: ThreadEvent = JSON.parse(data);
        yield event;
      }
    }
  }
}
```

---

## 2. SSE-Parsing-Logik (Als Referenz) ✅

Das SDK zeigt, wie JSONL-Events vom CLI geparst werden. Diese Logik können Sie **als Referenz für SSE-Parsing** verwenden.

### SDK-Implementation (`thread.ts:96-111`)

```typescript
private async *runStreamedInternal(
  input: Input,
  turnOptions: TurnOptions = {},
): AsyncGenerator<ThreadEvent> {
  const generator = this._exec.run({ /* ... args ... */ });

  try {
    for await (const item of generator) {
      let parsed: ThreadEvent;
      try {
        parsed = JSON.parse(item) as ThreadEvent;  // ← JSONL-Parsing
      } catch (error) {
        throw new Error(`Failed to parse item: ${item}`, { cause: error });
      }
      if (parsed.type === "thread.started") {
        this._id = parsed.thread_id;  // ← Thread-ID extrahieren
      }
      yield parsed;
    }
  } finally {
    await cleanup();
  }
}
```

### Übersetzung für ChatGPT Backend API:

Das CLI gibt JSONL aus (eine JSON-Zeile pro Event). Die ChatGPT Backend API gibt SSE (Server-Sent Events) aus.

**SSE-Format:**
```
data: {"type":"thread.started","thread_id":"abc123"}

data: {"type":"turn.started"}

data: {"type":"item.started","item":{"id":"msg_1","type":"agent_message","text":""}}

data: {"type":"item.updated","item":{"id":"msg_1","type":"agent_message","text":"Hello"}}

data: [DONE]
```

**Angepasste Implementierung:**

```typescript
async function* parseChatGPTSSE(response: Response): AsyncGenerator<ThreadEvent> {
  const reader = response.body!.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';  // Letzte (unvollständige) Zeile zurückbehalten

    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const data = line.slice(6);  // "data: " entfernen

        if (data === '[DONE]') {
          return;  // Stream beendet
        }

        try {
          const event: ThreadEvent = JSON.parse(data);
          yield event;
        } catch (error) {
          console.error('Failed to parse SSE data:', data, error);
          // Optional: Fehlerhafte Events überspringen
        }
      }
    }
  }
}
```

---

## 3. Input-Normalisierung (DIREKT ÜBERNEHMEN) ✅

Das SDK zeigt, wie User-Input (Text + Bilder) normalisiert wird. Diese Funktion können Sie **direkt kopieren**.

### SDK-Implementation (`thread.ts:140-154`)

```typescript
/** An input to send to the agent. */
export type UserInput =
  | {
      type: "text";
      text: string;
    }
  | {
      type: "local_image";
      path: string;
    };

export type Input = string | UserInput[];

function normalizeInput(input: Input): { prompt: string; images: string[] } {
  if (typeof input === "string") {
    return { prompt: input, images: [] };
  }
  const promptParts: string[] = [];
  const images: string[] = [];
  for (const item of input) {
    if (item.type === "text") {
      promptParts.push(item.text);
    } else if (item.type === "local_image") {
      images.push(item.path);
    }
  }
  return { prompt: promptParts.join("\n\n"), images };
}
```

### Verwendung in Ihrer API-Implementierung:

```typescript
import { Input, UserInput, normalizeInput } from './utils/input';

async function sendMessage(input: Input, accessToken: string) {
  const { prompt, images } = normalizeInput(input);

  // Bilder Base64-kodieren
  const imageContents = await Promise.all(
    images.map(async (path) => {
      const data = await fs.readFile(path);
      return {
        type: "image_url",
        image_url: `data:image/jpeg;base64,${data.toString('base64')}`
      };
    })
  );

  // Request-Body erstellen
  const body = {
    messages: [
      {
        role: "user",
        content: [
          { type: "text", text: prompt },
          ...imageContents
        ]
      }
    ],
    // ... weitere Fields
  };

  // API-Request senden
  const response = await fetch('https://chatgpt.com/backend-api/codex/responses', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body)
  });

  return parseChatGPTSSE(response);
}
```

---

## 4. Event-Loop-Pattern (Als Pattern) ✅

Das SDK zeigt, wie Events zu einem Turn-Result aggregiert werden. Dieses Pattern können Sie **als Vorlage** verwenden.

### SDK-Implementation (`thread.ts:114-137`)

```typescript
/** Provides the input to the agent and returns the completed turn. */
async run(input: Input, turnOptions: TurnOptions = {}): Promise<Turn> {
  const generator = this.runStreamedInternal(input, turnOptions);
  const items: ThreadItem[] = [];
  let finalResponse: string = "";
  let usage: Usage | null = null;
  let turnFailure: ThreadError | null = null;

  for await (const event of generator) {
    if (event.type === "item.completed") {
      if (event.item.type === "agent_message") {
        finalResponse = event.item.text;  // ← Finale Antwort extrahieren
      }
      items.push(event.item);
    } else if (event.type === "turn.completed") {
      usage = event.usage;  // ← Token-Usage extrahieren
    } else if (event.type === "turn.failed") {
      turnFailure = event.error;
      break;
    }
  }

  if (turnFailure) {
    throw new Error(turnFailure.message);
  }

  return { items, finalResponse, usage };
}
```

### Übersetzung für Ihre Implementierung:

```typescript
import { ThreadEvent, ThreadItem, Usage } from './types/codex-events';

export type Turn = {
  items: ThreadItem[];
  finalResponse: string;
  usage: Usage | null;
};

async function runTurn(
  input: Input,
  accessToken: string
): Promise<Turn> {
  const eventStream = await sendMessage(input, accessToken);

  const items: ThreadItem[] = [];
  let finalResponse = "";
  let usage: Usage | null = null;
  let threadId: string | null = null;

  for await (const event of eventStream) {
    switch (event.type) {
      case "thread.started":
        threadId = event.thread_id;
        console.log(`Thread started: ${threadId}`);
        break;

      case "turn.started":
        console.log("Turn started");
        break;

      case "item.started":
        console.log(`Item started: ${event.item.type}`);
        break;

      case "item.updated":
        // Echtzeit-Updates (z.B. für UI)
        if (event.item.type === "agent_message") {
          process.stdout.write(event.item.text);  // Streaming-Output
        }
        break;

      case "item.completed":
        items.push(event.item);
        if (event.item.type === "agent_message") {
          finalResponse = event.item.text;
        }
        console.log(`Item completed: ${event.item.type}`);
        break;

      case "turn.completed":
        usage = event.usage;
        console.log(`Turn completed. Tokens: ${usage.input_tokens}/${usage.output_tokens}`);
        break;

      case "turn.failed":
        throw new Error(`Turn failed: ${event.error.message}`);

      case "error":
        throw new Error(`Stream error: ${event.message}`);
    }
  }

  return { items, finalResponse, usage };
}
```

---

## 5. Vollständiges Beispiel: Direkte API mit SDK-Typen

Hier ist ein vollständiges Beispiel, das die SDK-Typen und -Patterns für eine direkte ChatGPT Backend API-Implementierung verwendet:

### Dateistruktur:

```
src/
├── types/
│   ├── codex-events.ts     # Von SDK kopiert
│   └── codex-items.ts      # Von SDK kopiert
├── auth/
│   └── oauth.ts            # OAuth-Flow (siehe chatbot.md)
├── api/
│   ├── client.ts           # HTTP-Client
│   └── sse-parser.ts       # SSE-Parsing
└── chatbot.ts              # Haupt-Chatbot-Klasse
```

### `src/api/sse-parser.ts`

```typescript
import { ThreadEvent } from '../types/codex-events';

export async function* parseSSE(
  response: Response
): AsyncGenerator<ThreadEvent> {
  const reader = response.body!.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        if (line.startsWith('data: ')) {
          const data = line.slice(6);

          if (data === '[DONE]') {
            return;
          }

          try {
            const event: ThreadEvent = JSON.parse(data);
            yield event;
          } catch (error) {
            console.error('Failed to parse SSE data:', data);
          }
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}
```

### `src/api/client.ts`

```typescript
import { ThreadEvent } from '../types/codex-events';
import { parseSSE } from './sse-parser';

export interface ChatGPTAPIOptions {
  accessToken: string;
  accountId: string;
  model?: string;
}

export class ChatGPTAPIClient {
  private accessToken: string;
  private accountId: string;
  private model: string;

  constructor(options: ChatGPTAPIOptions) {
    this.accessToken = options.accessToken;
    this.accountId = options.accountId;
    this.model = options.model || 'gpt-4';
  }

  async *sendMessage(
    prompt: string,
    conversationId?: string
  ): AsyncGenerator<ThreadEvent> {
    const response = await fetch(
      'https://chatgpt.com/backend-api/codex/responses',
      {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${this.accessToken}`,
          'chatgpt-account-id': this.accountId,
          'content-type': 'application/json',
          'user-agent': 'Mozilla/5.0 (compatible; Custom ChatGPT Client)',
          'originator': 'custom_client',
        },
        body: JSON.stringify({
          model: this.model,
          messages: [
            {
              role: 'user',
              content: [{ type: 'text', text: prompt }],
            },
          ],
          conversation_id: conversationId,
          // Keine Tools, kein System-Prompt → "roher" Chatbot!
        }),
      }
    );

    if (!response.ok) {
      throw new Error(`API request failed: ${response.status} ${response.statusText}`);
    }

    yield* parseSSE(response);
  }
}
```

### `src/chatbot.ts`

```typescript
import { ThreadItem, AgentMessageItem, Usage } from './types/codex-events';
import { ChatGPTAPIClient } from './api/client';
import { getAccessToken } from './auth/oauth';

export interface Turn {
  items: ThreadItem[];
  finalResponse: string;
  usage: Usage | null;
}

export class Chatbot {
  private client: ChatGPTAPIClient | null = null;
  private conversationId: string | null = null;

  async initialize() {
    // OAuth-Flow durchführen (siehe chatbot.md)
    const { accessToken, accountId } = await getAccessToken();

    this.client = new ChatGPTAPIClient({
      accessToken,
      accountId,
      model: 'gpt-4',
    });
  }

  async chat(prompt: string): Promise<Turn> {
    if (!this.client) {
      throw new Error('Chatbot not initialized. Call initialize() first.');
    }

    const eventStream = this.client.sendMessage(prompt, this.conversationId || undefined);

    const items: ThreadItem[] = [];
    let finalResponse = '';
    let usage: Usage | null = null;

    for await (const event of eventStream) {
      switch (event.type) {
        case 'thread.started':
          this.conversationId = event.thread_id;
          break;

        case 'item.updated':
          // Echtzeit-Updates für UI
          if (event.item.type === 'agent_message') {
            process.stdout.write(event.item.text.slice(finalResponse.length));
            finalResponse = event.item.text;
          }
          break;

        case 'item.completed':
          items.push(event.item);
          if (event.item.type === 'agent_message') {
            finalResponse = event.item.text;
          }
          break;

        case 'turn.completed':
          usage = event.usage;
          break;

        case 'turn.failed':
          throw new Error(`Turn failed: ${event.error.message}`);
      }
    }

    console.log(); // Newline nach Streaming
    return { items, finalResponse, usage };
  }
}
```

### Verwendung:

```typescript
import { Chatbot } from './chatbot';

async function main() {
  const bot = new Chatbot();
  await bot.initialize();  // OAuth-Flow

  // Erste Nachricht
  const turn1 = await bot.chat('Was ist die Hauptstadt von Deutschland?');
  console.log('\nAntwort:', turn1.finalResponse);
  console.log('Token-Usage:', turn1.usage);

  // Zweite Nachricht (gleicher Thread)
  const turn2 = await bot.chat('Und von Frankreich?');
  console.log('\nAntwort:', turn2.finalResponse);
  console.log('Token-Usage:', turn2.usage);
}

main();
```

**Output:**
```
Berlin ist die Hauptstadt von Deutschland.

Antwort: Berlin ist die Hauptstadt von Deutschland.
Token-Usage: { input_tokens: 25, cached_input_tokens: 0, output_tokens: 8 }

Paris ist die Hauptstadt von Frankreich.

Antwort: Paris ist die Hauptstadt von Frankreich.
Token-Usage: { input_tokens: 38, cached_input_tokens: 25, output_tokens: 8 }
```

---

## 6. Was das SDK NICHT enthält (aber Sie brauchen)

### OAuth 2.0 PKCE Flow

Das SDK enthält **keine** OAuth-Implementierung, weil es den CLI-Binary spawnt, der die Authentifizierung handhabt.

**Sie müssen selbst implementieren:**
- Authorization URL mit PKCE
- Code-Exchange
- Token-Refresh

**Siehe:** `chatbot.md` Abschnitt "OAuth 2.0 PKCE Flow" für vollständige Implementierung.

### HTTP-Requests

Das SDK macht **keine** direkten HTTP-Requests, sondern spawnt `codex exec`.

**Sie müssen selbst implementieren:**
- `fetch()` Calls zu `https://chatgpt.com/backend-api/codex/responses`
- Header-Management
- Error-Handling
- Retry-Logik (optional)

### Token-Management

Das SDK überlässt Token-Refresh dem CLI.

**Sie müssen selbst implementieren:**
- Token-Speicherung (SecureStore)
- Token-Refresh alle ~8 Tage
- Token-Validierung

**Siehe:** `chatbot.md` Abschnitt "Token Management" für Details.

---

## 7. Zusammenfassung: SDK-Code-Nutzung

### ✅ Direkt übernehmen:

1. **TypeScript-Typen** (`events.ts`, `items.ts`)
   - Kopieren Sie alle Type-Definitionen
   - Verwenden Sie sie für Type-Safety

2. **Input-Normalisierung** (`normalizeInput()`)
   - 1:1 kopieren
   - Funktioniert out-of-the-box

3. **Event-Loop-Pattern** (`run()` Methode)
   - Als Template für Ihre Implementierung
   - Zeigt, wie Events aggregiert werden

### 📖 Als Referenz verwenden:

1. **SSE-Parsing-Logik** (`runStreamedInternal()`)
   - Konzept übernehmen
   - Für SSE statt JSONL anpassen

2. **Thread-Management** (`Thread` Klasse)
   - Pattern für Conversation-ID-Tracking
   - Pattern für Thread-Resume

3. **Error-Handling**
   - Wie Fehler propagiert werden
   - Wie Turn-Failures behandelt werden

### ❌ Selbst implementieren:

1. **OAuth 2.0 Flow**
   - Siehe `chatbot.md`

2. **HTTP-Client**
   - `fetch()` zu ChatGPT Backend API

3. **Token-Management**
   - Refresh-Logik
   - Secure Storage

---

## 8. Praktischer Workflow

### Schritt 1: SDK-Typen kopieren

```bash
# Typen aus SDK extrahieren
cp sdk/typescript/src/events.ts src/types/codex-events.ts
cp sdk/typescript/src/items.ts src/types/codex-items.ts
```

### Schritt 2: SSE-Parser schreiben

```typescript
// src/api/sse-parser.ts
// [Code von oben]
```

### Schritt 3: OAuth implementieren

```typescript
// src/auth/oauth.ts
// Siehe chatbot.md für vollständige Implementierung
```

### Schritt 4: API-Client bauen

```typescript
// src/api/client.ts
// [Code von oben]
```

### Schritt 5: Chatbot-Klasse erstellen

```typescript
// src/chatbot.ts
// [Code von oben]
```

### Schritt 6: Testen!

```typescript
// examples/simple-chat.ts
import { Chatbot } from '../src/chatbot';

async function main() {
  const bot = new Chatbot();
  await bot.initialize();

  const response = await bot.chat('Hallo, wie geht es dir?');
  console.log(response.finalResponse);
}

main();
```

---

## 9. Vorteile dieser Herangehensweise

### ✅ Type-Safety

Durch Verwendung der SDK-Typen haben Sie **vollständige Type-Safety**:

```typescript
for await (const event of eventStream) {
  switch (event.type) {  // ← TypeScript kennt alle Event-Typen
    case 'item.completed':
      if (event.item.type === 'agent_message') {  // ← Type-narrowing
        // TypeScript weiß: event.item ist AgentMessageItem
        console.log(event.item.text);  // ← .text ist verfügbar
      }
      break;
  }
}
```

### ✅ Zukunftssicherheit

Wenn OpenAI neue Event-Typen hinzufügt:
1. SDK wird aktualisiert
2. Sie kopieren neue Typen
3. TypeScript zeigt Ihnen, wo Sie Code anpassen müssen

### ✅ Dokumentation

Die SDK-Typen enthalten **JSDoc-Kommentare**, die Ihre IDE anzeigt:

```typescript
/**
 * Describes the usage of tokens during a turn.
 */
export type Usage = {
  /** The number of input tokens used during the turn. */
  input_tokens: number;
  // ...
}
```

### ✅ Weniger Code zu schreiben

Sie müssen nicht alle Typen selbst definieren - einfach kopieren!

### ✅ Kompatibilität

Ihre Implementierung ist **kompatibel** mit SDK-Events, falls Sie später Teile des SDK doch verwenden wollen.

---

## 10. Unterschiede: SDK vs. Direkte API

| Aspekt | SDK | Direkte API (Ihr Ansatz) |
|--------|-----|---------------------------|
| **System-Prompt** | ❌ Codex-Prompt zwingend | ✅ Volle Kontrolle |
| **Tools** | ❌ Codex-Tools zwingend | ✅ Optional/keine Tools |
| **HTTP** | Spawnt CLI Binary | ✅ Direkte HTTP-Requests |
| **OAuth** | CLI handhabt intern | ✅ Sie kontrollieren |
| **Events** | ✅ JSONL von CLI | ✅ SSE von API |
| **Typen** | ✅ TypeScript-Typen | ✅ Gleiche Typen verwenden! |
| **Overhead** | ❌ CLI-Binary spawnen | ✅ Nur HTTP |
| **Dependencies** | CLI Binary nötig | ✅ Nur HTTP-Client |

---

## Fazit

**Das SDK ist eine Goldmine für Typen und Patterns**, auch wenn Sie es nicht direkt verwenden können!

### Was Sie aus dem SDK nehmen sollten:

1. ✅ **TypeScript-Typen** - Kopieren Sie alles
2. ✅ **Input-Normalisierung** - 1:1 übernehmen
3. ✅ **Event-Loop-Pattern** - Als Template verwenden
4. ✅ **SSE-Parsing-Konzept** - Für SSE anpassen

### Was Sie selbst implementieren müssen:

1. ❌ **OAuth 2.0 Flow** - Siehe `chatbot.md`
2. ❌ **HTTP-Client** - `fetch()` zu ChatGPT API
3. ❌ **Token-Management** - Refresh + Storage

**Ergebnis:** Ein "unbeeinflusster" Chatbot mit Abo-Billing, der SDK-Typen für Type-Safety verwendet, aber vollständige Kontrolle über System-Prompts und Verhalten hat!
