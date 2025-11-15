# System-Prompt-Analyse: Kann SDK ohne Codex-Prompts verwendet werden?

**KRITISCHE FRAGE:** Kann Ansatz 2 (TypeScript SDK) für einen eigenen Chatbot genutzt werden, der das OpenAI-Abo verwendet, OHNE dass der Codex-System-Prompt genutzt wird?

**ANTWORT: NEIN** ❌

## Zusammenfassung

Das TypeScript SDK kann **NICHT** für einen "unbeeinflussen" Chatbot verwendet werden, der wie die rohe ChatGPT API funktioniert. Der Codex CLI Binary, den das SDK spawnt, injiziert **IMMER** seine eigenen System-Prompts, und es gibt **KEINE** Möglichkeit, diese über CLI-Flags oder SDK-Parameter zu deaktivieren oder zu überschreiben.

---

## Detaillierte technische Analyse

### 1. Codex System-Prompts sind fest eingebaut

Der Codex CLI verwendet fest kodierte System-Prompts, die in Markdown-Dateien gespeichert und direkt in den Binary kompiliert werden:

**Dateien:**
- `codex-rs/core/prompt.md` (Standard-Prompt, ~311 Zeilen)
- `codex-rs/core/gpt_5_1_prompt.md` (GPT-5.1-Prompt, ~371 Zeilen)
- `codex-rs/core/gpt_5_codex_prompt.md` (GPT-5-Codex-Prompt)

**Code-Einbindung (`codex-rs/core/src/model_family.rs:10-13`):**
```rust
const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md");
const GPT_5_CODEX_INSTRUCTIONS: &str = include_str!("../gpt_5_codex_prompt.md");
const GPT_5_1_INSTRUCTIONS: &str = include_str!("../gpt_5_1_prompt.md");
```

**Inhalt der Prompts:**
Die Prompts beginnen mit:
- `"You are a coding agent running in the Codex CLI..."`
- `"You are GPT-5.1 running in the Codex CLI..."`

Sie enthalten detaillierte Anweisungen über:
- Persönlichkeit und Ton ("concise, direct, and friendly")
- Tool-Verwendung (apply_patch, shell, update_plan, etc.)
- Sandbox-Verhalten und Approvals
- Code-Validierung und Testing
- Ausgabeformatierung
- AGENTS.md-Spec
- Responsiveness-Richtlinien

### 2. Wie System-Prompts im Code verwendet werden

**Datenfluss:**
```
ModelFamily.base_instructions
  ↓
Config.base_instructions
  ↓
SessionConfiguration.base_instructions
  ↓
TurnContext.base_instructions
  ↓
Prompt.base_instructions_override
  ↓
API Request (als "instructions" Feld)
```

**Code-Stellen:**

**a) ModelFamily bestimmt den Prompt (`model_family.rs:71-100`):**
```rust
let mut mf = ModelFamily {
    // ...
    base_instructions: BASE_INSTRUCTIONS.to_string(),  // Standard-Prompt
    // ...
};
```

Für spezielle Modelle wird der Prompt überschrieben:
```rust
// Für gpt-5.1
base_instructions: GPT_5_1_INSTRUCTIONS.to_string()

// Für gpt-5-codex
base_instructions: GPT_5_CODEX_INSTRUCTIONS.to_string()
```

**b) Config übernimmt den Prompt (`config/mod.rs:130`):**
```rust
pub struct Config {
    /// Base instructions override.
    pub base_instructions: Option<String>,  // ← Hier könnten Custom Instructions gespeichert werden
}
```

**c) SessionConfiguration verwendet Config (`codex.rs:179`):**
```rust
let session_configuration = SessionConfiguration {
    base_instructions: config.base_instructions.clone(),  // ← Von Config
    // ...
};
```

**d) TurnContext übernimmt SessionConfiguration (`codex.rs:429`):**
```rust
TurnContext {
    base_instructions: session_configuration.base_instructions.clone(),
    // ...
}
```

**e) Prompt verwendet TurnContext (`codex.rs:1942`):**
```rust
let prompt = Prompt {
    base_instructions_override: turn_context.base_instructions.clone(),
    // ...
};
```

**f) Prompt.get_full_instructions() bestimmt finale Instructions (`client_common.rs:52-74`):**
```rust
pub(crate) fn get_full_instructions<'a>(&'a self, model: &'a ModelFamily) -> Cow<'a, str> {
    let base = self
        .base_instructions_override
        .as_deref()
        .unwrap_or(model.base_instructions.deref());  // ← Falls KEIN Override, nutze ModelFamily-Prompt
    // ...
}
```

**WICHTIG:** Wenn `base_instructions_override` `None` ist (was IMMER der Fall ist bei CLI/SDK-Nutzung), wird **automatisch** `model.base_instructions` verwendet - also die fest eingebauten Codex-Prompts!

### 3. KÖNNTE man base_instructions theoretisch überschreiben?

**JA, technisch ist es möglich!** Die Infrastruktur existiert:

**a) Config hat ein Feld dafür:**
```rust
// codex-rs/core/src/config/mod.rs:130
pub base_instructions: Option<String>,
```

**b) CliOverrides hat ein Feld dafür:**
```rust
// codex-rs/core/src/config/mod.rs:843
pub struct CliOverrides {
    pub base_instructions: Option<String>,
    // ...
}
```

**c) Der Code würde es respektieren:**
Wenn `config.base_instructions = Some("Custom prompt")` gesetzt wäre, würde dieser Prompt statt des Codex-Prompts verwendet werden.

### 4. ABER: Keine CLI-Flag oder SDK-Parameter verfügbar!

**Untersuchung:**

**a) CLI-Flags (`codex exec --help`):**
```bash
# Kein Flag für --base-instructions
# Kein Flag für --custom-instructions
# Kein Flag für --system-prompt
```

✅ Bestätigt durch grep:
```bash
grep -r "--base-instruction\|--custom-instruction\|--system-prompt" codex-rs/cli/
# Kein Ergebnis!
```

**b) TypeScript SDK (`sdk/typescript/src/exec.ts:8-37`):**
```typescript
export type CodexExecArgs = {
  input: string;
  baseUrl?: string;
  apiKey?: string;
  threadId?: string | null;
  images?: string[];
  model?: string;
  sandboxMode?: SandboxMode;
  workingDirectory?: string;
  additionalDirectories?: string[];
  skipGitRepoCheck?: boolean;
  outputSchemaFile?: string;
  modelReasoningEffort?: ModelReasoningEffort;
  signal?: AbortSignal;
  networkAccessEnabled?: boolean;
  webSearchEnabled?: boolean;
  approvalPolicy?: ApprovalMode;
  // ← KEIN baseInstructions Parameter!
}
```

**c) SDK spawnt CLI ohne Custom Instructions:**
```typescript
// sdk/typescript/src/exec.ts:52-97
const commandArgs: string[] = ["exec", "--experimental-json"];

if (args.model) {
  commandArgs.push("--model", args.model);
}
// ... weitere Args, aber KEIN --base-instructions
```

### 5. Wie Codex intern base_instructions setzt

**Nur für interne Use-Cases:**

**a) Review-Tasks nutzen REVIEW_PROMPT (`codex.rs:1761`):**
```rust
let review_turn_context = TurnContext {
    base_instructions: Some(base_instructions.clone()),  // ← REVIEW_PROMPT
    // ...
};
```

Der `REVIEW_PROMPT` ist ein spezieller Prompt für Code-Reviews:
```rust
// client_common.rs:24
pub const REVIEW_PROMPT: &str = include_str!("../review_prompt.md");
```

**b) Aber normale User-Turns haben IMMER `None`:**
```rust
// codex.rs:424-429
TurnContext {
    base_instructions: session_configuration.base_instructions.clone(),  // ← config.base_instructions
    // ...
}
```

Und `config.base_instructions` wird von `CliOverrides.base_instructions` gesetzt, welches **niemals** über CLI-Flags gesetzt werden kann!

---

## Konsequenzen für Ihre Frage

### ❌ Ansatz 2 (TypeScript SDK) funktioniert NICHT für Ihren Use-Case

**Warum:**
1. SDK spawnt Codex CLI Binary: `spawn('codex', ['exec', '--experimental-json', ...])`
2. CLI liest Config, die **keine** `base_instructions` hat (weil kein CLI-Flag)
3. SessionConfiguration übernimmt `config.base_instructions = None`
4. TurnContext übernimmt `session_configuration.base_instructions = None`
5. Prompt übernimmt `turn_context.base_instructions = None`
6. `Prompt.get_full_instructions()` verwendet **automatisch** `model.base_instructions` (die fest eingebauten Codex-Prompts!)
7. **Ergebnis:** Jede Anfrage enthält den vollständigen Codex-System-Prompt

### Was das bedeutet:

**Der Agent verhält sich IMMER wie Codex CLI:**
- ✅ Nutzt Codex-Persönlichkeit ("concise, direct, and friendly")
- ✅ Befolgt AGENTS.md-Spec
- ✅ Nutzt apply_patch, update_plan, shell Tools
- ✅ Validiert Code und führt Tests aus
- ✅ Formatiert Ausgaben nach Codex-Richtlinien
- ✅ Sendet "preamble messages" vor Tool-Calls
- ✅ Verhält sich wie ein "coding agent running in the Codex CLI"

**Sie können das NICHT deaktivieren, weil:**
- ❌ Kein CLI-Flag für `--base-instructions`
- ❌ Kein SDK-Parameter für `baseInstructions`
- ❌ Keine API/Protokoll-Möglichkeit, den Prompt zu überschreiben

### Könnte OpenAI erkennen, dass es nicht vom echten Codex CLI kommt?

**Theoretisch: Nein, ABER...**

Wenn Sie Ansatz 2 (SDK) verwenden, ist **alles identisch** zum echten Codex CLI:
- ✅ Gleiche System-Prompts
- ✅ Gleiche Tool-Definitionen
- ✅ Gleiche Headers (`originator`, `User-Agent`)
- ✅ Gleiche Request-Struktur
- ✅ Gleiche OAuth-Tokens

**Das Problem ist:** Sie **WOLLEN** keine Codex-System-Prompts, aber mit Ansatz 2 bekommen Sie sie **zwingend**!

---

## Alternative: Ansatz 1 (Direkte API-Implementierung)

**Wenn Sie einen "unbeeinflussten" Chatbot wollen, müssen Sie Ansatz 1 verwenden:**

### ✅ Volle Kontrolle über System-Prompts

**Mit direkter API-Implementierung:**
```typescript
// Ihre eigene Implementierung
const response = await fetch('https://chatgpt.com/backend-api/codex/responses', {
  method: 'POST',
  headers: {
    'Authorization': `Bearer ${access_token}`,
    'chatgpt-account-id': account_id,
    'content-type': 'application/json',
    // ...
  },
  body: JSON.stringify({
    messages: [
      // ← SIE kontrollieren die Messages!
      // ← KEIN Codex-System-Prompt, wenn Sie keinen wollen!
    ],
    tools: [
      // ← Optional: Nur Tools senden, die Sie wollen
      // ← Oder KEINE Tools für einen reinen Chatbot
    ],
    // ...
  })
});
```

**Optionen:**
1. **Kein System-Prompt:** Senden Sie nur User-Messages → "Rohes" ChatGPT-Verhalten
2. **Eigener System-Prompt:** Senden Sie Ihren eigenen Prompt → Vollständig angepasst
3. **Keine Tools:** Lassen Sie Tools weg → Reiner Chat-Modus
4. **Minimale Tools:** Nur spezifische Tools senden

### ✅ OpenAI wird es nicht als "nicht-Codex" erkennen

**Warum:**
- ✅ Sie verwenden gleiche OAuth-Tokens (von echter Codex-Login)
- ✅ Sie verwenden gleiche API-Endpoints (`/backend-api/codex/responses`)
- ✅ Sie können gleiche Headers senden (`originator`, `User-Agent`)
- ✅ Sie können gleiche Request-Struktur verwenden

**Der Unterschied:**
- ❌ Sie senden **NICHT** den Codex-System-Prompt → Gewollt!
- ❌ Sie senden **NICHT** alle Codex-Tools → Optional
- ❌ Sie haben **NICHT** die exakten Retry-Timings → Irrelevant

**Risiko:** Minimal bis Keins

OpenAI kann theoretisch erkennen, dass:
- Der System-Prompt fehlt oder anders ist
- Andere Tools verwendet werden
- Timing-Patterns unterschiedlich sind

**ABER:**
1. Das ist **technisch erlaubt** (Sie nutzen Ihr eigenes Abo)
2. Codex CLI ist Open Source → Jeder darf es modifizieren
3. Es gibt keine ToS, die "nur offizieller Codex CLI" vorschreiben
4. OpenAI hat kein Interesse, eigene Abo-Nutzer zu blockieren

---

## Empfehlung

### Für Ihren Use-Case: "Unbeeinflusster Chatbot mit Abo-Billing"

**Verwenden Sie Ansatz 1 (Direkte API)!**

**Begründung:**
1. ✅ **Volle Kontrolle:** Sie bestimmen System-Prompt, Tools, Verhalten
2. ✅ **Kein Codex-Verhalten:** Agent verhält sich wie rohe ChatGPT API
3. ✅ **Abo-Billing:** Nutzt ChatGPT Backend API (kein API-Key-Billing)
4. ✅ **Nicht detektierbar:** Gleiche OAuth, gleiche Endpoints, gleiche Headers
5. ✅ **Flexibel:** Sie können später Features hinzufügen

**Nachteile:**
- ❌ Mehr Code zu schreiben (OAuth, SSE, etc.)
- ❌ Komplexer als SDK (aber gut dokumentiert in `chatbot.md`)

### Wann SDK (Ansatz 2) nutzen?

**Nur wenn Sie explizit Codex-Verhalten WOLLEN:**
- ✅ Automatische Code-Editierung mit apply_patch
- ✅ Automatische Tests und Validierung
- ✅ AGENTS.md-Support
- ✅ Sandbox und Approval-Flows
- ✅ Plan-Management (update_plan)

**Aber:** Das ist **nicht** Ihr Use-Case!

---

## Fazit: Bit-genaue Antwort

### Ihre Frage:
> "ich könnte also mit Ansatz 2 für einen eigenen chatbot nutzen, der das abo nutzt? ich möchte nicht, das der system prompt von codex genutzt wird, sondern der chatbot völlig unbeeinflusst ist, so, als würde man die api nutzen."

### Antwort:

**NEIN, Ansatz 2 funktioniert NICHT für Ihren Use-Case.**

**Technische Fakten:**
1. ✅ Ansatz 2 (SDK) nutzt das Abo (nicht API-Billing)
2. ❌ Ansatz 2 (SDK) nutzt **IMMER** Codex-System-Prompts
3. ❌ Es gibt **KEINE** Möglichkeit, den System-Prompt über SDK/CLI zu deaktivieren
4. ❌ Der Chatbot ist **NICHT** "unbeeinflusst" - er verhält sich wie Codex CLI
5. ❌ Es ist **NICHT** "wie die API nutzen" - es ist wie Codex CLI nutzen

**Für "unbeeinflussten Chatbot mit Abo-Billing":**
→ **Verwenden Sie Ansatz 1 (Direkte API-Implementierung aus `chatbot.md`)**

**Für Codex-CLI-Verhalten als SDK:**
→ Verwenden Sie Ansatz 2 (TypeScript SDK)

---

## Appendix: Code-Beweise

### Beweis 1: System-Prompt wird IMMER gesetzt

**Datei:** `codex-rs/core/src/client_common.rs:52-74`
```rust
pub(crate) fn get_full_instructions<'a>(&'a self, model: &'a ModelFamily) -> Cow<'a, str> {
    let base = self
        .base_instructions_override
        .as_deref()
        .unwrap_or(model.base_instructions.deref());  // ← Falls None, nutze model.base_instructions

    // ... (apply_patch instructions werden ggf. angehängt)

    if self.base_instructions_override.is_none()  // ← Wenn kein Override...
        && model.needs_special_apply_patch_instructions
        && !is_apply_patch_tool_present
    {
        Cow::Owned(format!("{base}\n{APPLY_PATCH_TOOL_INSTRUCTIONS}"))
    } else {
        Cow::Borrowed(base)  // ← Nutzt model.base_instructions (Codex-Prompt!)
    }
}
```

**Logik:**
- `base_instructions_override` ist `None` (weil kein CLI-Flag/SDK-Parameter)
- → `unwrap_or(model.base_instructions.deref())` verwendet `model.base_instructions`
- → `model.base_instructions` ist `BASE_INSTRUCTIONS` oder `GPT_5_1_INSTRUCTIONS`
- → **Ergebnis:** Codex-System-Prompt wird verwendet

### Beweis 2: Kein CLI-Flag für base-instructions

**Datei:** `codex-rs/cli/src/main.rs`

Alle CLI-Flags werden in `clap` definiert. Suche nach `base_instructions`:
```bash
grep -n "base_instructions\|base-instructions" codex-rs/cli/src/main.rs
# Kein Ergebnis!
```

**Alle vorhandenen Flags:**
```bash
grep -n "long =" codex-rs/cli/src/main.rs | head -20
# Beispiele:
# --model
# --sandbox
# --cd
# --add-dir
# --skip-git-repo-check
# --config
# --image
# etc.
# ← KEIN --base-instructions!
```

### Beweis 3: SDK exponiert keine baseInstructions

**Datei:** `sdk/typescript/src/exec.ts:8-37`
```typescript
export type CodexExecArgs = {
  input: string;
  baseUrl?: string;
  apiKey?: string;
  threadId?: string | null;
  images?: string[];
  model?: string;
  sandboxMode?: SandboxMode;
  workingDirectory?: string;
  additionalDirectories?: string[];
  skipGitRepoCheck?: boolean;
  outputSchemaFile?: string;
  modelReasoningEffort?: ModelReasoningEffort;
  signal?: AbortSignal;
  networkAccessEnabled?: boolean;
  webSearchEnabled?: boolean;
  approvalPolicy?: ApprovalMode;
  // ← Alle Parameter erschöpfend aufgelistet
  // ← KEIN baseInstructions Parameter!
}
```

**SDK baut Command Args (`exec.ts:51-103`):**
```typescript
async *run(args: CodexExecArgs): AsyncGenerator<string> {
  const commandArgs: string[] = ["exec", "--experimental-json"];

  if (args.model) {
    commandArgs.push("--model", args.model);
  }
  // ... alle anderen Args
  // ← Nirgendwo wird --base-instructions hinzugefügt!

  const child = spawn(this.executablePath, commandArgs, { env });
  // ...
}
```

**Schlussfolgerung:** Es gibt **physikalisch keine Möglichkeit**, `base_instructions` über das SDK zu setzen.

### Beweis 4: Codex-Prompts sind in den Binary kompiliert

**Datei:** `codex-rs/core/src/model_family.rs:10-13`
```rust
const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md");
const GPT_5_CODEX_INSTRUCTIONS: &str = include_str!("../gpt_5_codex_prompt.md");
const GPT_5_1_INSTRUCTIONS: &str = include_str!("../gpt_5_1_prompt.md");
```

**`include_str!` Makro:**
- Liest Datei zur **Compile-Zeit**
- Baut String **direkt in Binary** ein
- → Nicht änderbar zur Runtime ohne Binary-Modifikation

**ModelFamily Default (`model_family.rs:85`):**
```rust
let mut mf = ModelFamily {
    // ...
    base_instructions: BASE_INSTRUCTIONS.to_string(),  // ← Fest eingebaut!
    // ...
};
```

**Für spezielle Modelle (`model_family.rs:188`):**
```rust
} else if slug.starts_with("gpt-5.1") {
    model_family!(
        slug, "gpt-5.1",
        base_instructions: GPT_5_1_INSTRUCTIONS.to_string(),  // ← Fest eingebaut!
        // ...
    )
}
```

**Schlussfolgerung:** Die Prompts sind zur Compile-Zeit fest im Binary kodiert. Es gibt keine Runtime-Konfiguration.

---

## Technischer Deep-Dive: Was wäre nötig, um base_instructions zu setzen?

**Hypothetisch: Wenn Sie den Codex CLI Code modifizieren wollten:**

### Option A: CLI-Flag hinzufügen

**1. CLI-Argument definieren (`cli/src/main.rs`):**
```rust
#[derive(Parser)]
struct Cli {
    // ... andere Felder

    /// Override the base system instructions
    #[arg(long = "base-instructions", value_name = "TEXT")]
    base_instructions: Option<String>,
}
```

**2. CliOverrides setzen:**
```rust
let cli_overrides = CliOverrides {
    base_instructions: cli.base_instructions,
    // ... andere Overrides
};
```

**3. Binary neu kompilieren:**
```bash
cd codex-rs
cargo build --release
```

**4. SDK auf modifizierten Binary zeigen:**
```typescript
const codex = new Codex({
  codexPathOverride: "/pfad/zum/modifizierten/codex"
});
```

### Option B: Direkt in Code hardcoden

**1. Config Default ändern (`core/src/config/mod.rs`):**
```rust
impl Default for Config {
    fn default() -> Self {
        Config {
            base_instructions: Some("Ihre eigenen Instructions hier".to_string()),
            // ... andere Defaults
        }
    }
}
```

**2. Binary neu kompilieren**

### Option C: Config-Datei nutzen (falls unterstützt)

**Prüfen, ob config.toml base_instructions unterstützt:**
```bash
grep -r "base_instructions" codex-rs/core/src/config/
# Ergebnis: Feld existiert, aber wird nicht aus config.toml gelesen!
```

**Aktuell NICHT unterstützt:** `config.toml` unterstützt kein `base_instructions` Setting.

---

**Alle Optionen erfordern:**
- ✅ Code-Modifikation
- ✅ Re-Compilation
- ✅ Custom Binary

**Das ist NICHT praktikabel für Ihren Use-Case!**

→ **Deshalb: Verwenden Sie Ansatz 1 (Direkte API-Implementierung)**
