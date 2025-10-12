Treat **THIS AGENTS.md** as the source of truth. Across branch switches, pulls, rebases, and resets (including hard resets), do not modify, replace, or delete this file. If a merge conflict involves AGENTS.md, resolve it by choosing **OURS** (keep the local version) for this file only. If preservation cannot be guaranteed, stop and notify me.

IMPORTANT: **THINK** carefully and analyze all points of view.

When in doubt, **RESEARCH** or **ASK**.

The cardinal rule you must follow is: **NEVER** write code haphazardly with only the final result in mind. The final result is the CONSEQUENCE of code written with excellence, robustness, and elegance.
**NEVER** do anything in a hurry; haste is the enemy of perfection. Take the time you need to write perfect code.

Whenever you propose or implement a solution, **DO NOT REINVENT THE WHEEL**. Fix root causes; do not rely on quick fixes, hacks, or shit workarounds. Do not remove, disable, or narrow existing functionality to make an error disappear; fixes must preserve functional parity and user-facing behavior.

Prioritize error handling instead of fallbacks.
Avoid generic helpers and redundant, unnecessary validations.
Be thorough with verbose output and debugging.
In Python scripts, include progress bars when appropriate.
Only change variable and function names when STRICTLY necessary.
Robust code BUT without frills.
Use descriptive, intelligible variable and function names.

⚠️ IMPORTANT: **DO NOT** use git clean or git revert under any circumstances.
Do **NOT** use commands that are destructive, overwrite, or completely reset configurations and parameters.

# MCP HTTP Gateway

Gateway HTTP para o **Model Context Protocol (MCP)** que atua como ponte entre clientes HTTP (ex.: ChatGPT Connector) e o processo **Codex** servindo ferramentas via STDIO.

## Como funciona

1. O cliente envia `POST /mcp` com o payload MCP JSON-RPC e `Authorization: Bearer <JWT>`.
2. O gateway valida o JWT HS256 com o segredo local (`JWT_SECRET`) e checa o header `Origin` quando presente.
3. A requisição MCP é roteada para o Codex usando um `StdioClientTransport` compartilhado.
4. A resposta streamable do Codex é devolvida na mesma conexão HTTP.
5. O mesmo servidor publica `/.well-known/oauth-authorization-server` e `/.well-known/openid-configuration`, além dos endpoints `/authorize` e `/token`, permitindo que o ChatGPT conclua o fluxo OAuth.

O endpoint `GET /mcp` responde `405` reforçando o uso exclusivo de POST. Demais rotas retornam `404`.

## Estrutura do projeto

- `src/gateway.ts` – servidor Express, transporte HTTP streamable e roteamento das chamadas MCP.
- `src/auth.ts` – carregamento da configuração JWT, verificação do Bearer e checagem de origem.
- `src/codexClient.ts` – cliente MCP STDIO persistente para o processo `codex`.
- `src/oauth.ts` – implementação do fluxo OAuth 2.0 (Authorization Code + PKCE) e publicação de `.well-known/oauth-authorization-server`.
- `.env.example` – variáveis de ambiente necessárias.
- `package.json`, `tsconfig.json` – configuração de build e scripts.

## Variáveis de ambiente

| Nome | Descrição / Exemplo | Default |
| --- | --- | --- |
| `PORT` | Porta HTTP local | `8787` |
| `ALLOWED_ORIGINS` | Origens permitidas separadas por vírgula (`https://chat.openai.com,https://chatgpt.com`) | vazio (sem restrição extra) |
| `ALLOWED_HOSTS` | Lista de `Host` aceitos (`127.0.0.1,localhost,mcp.sangoi.dev`) para proteção DNS rebinding | vazio |
| `TRANSPORT_ALLOWED_ORIGINS` | Origens aceitas pelo transporte streamable (`https://mcp.sangoi.dev`) | vazio |
| `JWT_SECRET` | Segredo simétrico HS256 para validar o Bearer | — |
| `PUBLIC_BASE_URL` | Origem pública do gateway (ex.: `https://mcp.sangoi.dev`) | derivado de `PUBLIC_URL` |
| `OAUTH_USERS` | Lista de credenciais `usuario:senha` separadas por vírgula | — |
| `OAUTH_ALLOWED_REDIRECTS` | Prefixos de redirect URI permitidos (`https://chat.openai.com/,https://chatgpt.com/`) | valores padrão do ChatGPT |
| `OAUTH_AUTH_CODE_TTL_MS` | TTL do authorization code em ms | `300000` |
| `OAUTH_ACCESS_TOKEN_TTL_SECONDS` | TTL do access token em segundos | `3600` |
| `CODEX_CMD` | Binário do Codex | `codex` |
| `CODEX_ARGS` | Args passados ao Codex | `mcp serve --expose-all-tools --max-aux-agents=2` |
| `CODEX_CWD` | Diretório de trabalho ao spawnar o Codex | — |
| `LOG_LEVEL` | Nível de log do Pino (`info`, `debug`, `warn`, `error`) | `info` |
| `LOG_PRETTY` | Ativa saída colorida legível no console (`true`/`false`) | depende do TTY |
| `PUBLIC_URL` | URL pública exposta (ex.: `https://mcp.sangoi.dev/mcp`) | `https://mcp.sangoi.dev/mcp` |
| `MCP_MIN_REQUEST_INTERVAL_MS` | Dampening mínimo entre chamadas MCP para evitar burst | `0` |

## Executando localmente

```bash
npm install
npm run dev   # inicia com Vite Node em modo watch (exposto em 0.0.0.0)
# ou
npm run build && npm start
```

Com o servidor rodando, faça uma chamada de fumaça (substitua `XXX` por um JWT válido):

```bash
curl -sS \
  -X POST http://127.0.0.1:8787/mcp \
  -H "authorization: Bearer XXX" \
  -H "content-type: application/json" \
  -d '{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}'
```

## Segurança

- JWT obrigatório em todas as requisições; falhas retornam `401` com códigos específicos (`missing_authorization_header`, `invalid_authorization_header`, `invalid_token`, `origin_not_allowed`).
- `Origin` verificado quando presente contra `ALLOWED_ORIGINS`.
- Logs (`pino`) evitam imprimir payloads inteiros, mantendo apenas metadados.
- O processo não executa comandos shell adicionais, apenas inicia o Codex via STDIO e mantém a conexão persistente.

## Checklist

- [x] `POST /mcp` responde com JSON válido (handlers `tools/*`, `resources/*`, `prompts/*`).
- [x] `GET /mcp` retorna 405.
- [x] 401 para JWT inválido/faltando e `origin_not_allowed`.
- [x] Sem acesso fora de `/home/lucas/work/mpc` (apenas STDIO do Codex).
- [x] Logs úteis em `info` e `debug` com Pino.

## Próximos passos sugeridos

1. Configurar `.env` com `JWT_SECRET`, credenciais em `OAUTH_USERS` e confirmar `ALLOWED_HOSTS=127.0.0.1,localhost,mcp.sangoi.dev`.
2. Pousar o processo por trás do Cloudflare Tunnel apontando `mcp.sangoi.dev` → `http://127.0.0.1:8787`.
3. Registrar o conector no ChatGPT Developer Mode (`MCP Server URL = https://mcp.sangoi.dev/mcp`, auth OAuth) e seguir o fluxo de login exposto em `/authorize`.

## Perfis de execução (`--profile`)

- O gateway suporta `--profile <nome>` (ou `-p <nome>`) para carregar overrides de ambiente a partir de um arquivo TOML.
- Caminho padrão do arquivo: `~/.mcp-gateway/config.toml` (pode ser sobrescrito com `MCP_CONFIG_PATH`).
- Estrutura exemplo:

```toml
[profiles.codex]
CODEX_CWD = "/home/lucas/work/codex"
PUBLIC_URL = "https://mcp.sangoi.dev/mcp"

[profiles.staging.env]
CODEX_CWD = "/srv/codex"
LOG_LEVEL = "debug"
```

- Ao iniciar com `node dist/gateway.js -p codex`, as variáveis listadas são injetadas antes do bootstrap (ex.: `CODEX_CWD`).
- Valores suportados: strings, números e booleanos. Entradas não reconhecidas são ignoradas com aviso no console.

---

GitHub + Docs
- Use the GitHub CLI `gh` for all GitHub interactions (PRs, issues, merges, remote branch management). Do not use raw `git` for remote state changes.
- All documentation must always be written in English.

Repository Hygiene
- Before any commit, ensure no irrelevant artifacts or vendor directories are tracked (e.g., `target/`, `node_modules/`, `dist/`, `build/`, `.cache/`, `coverage/`, `.DS_Store`, `*.log`, `tmp/`).
- Keep these paths in `.gitignore` and never commit them; remove from index with `git rm --cached` if needed (avoid destructive cleanup commands).

Git Ignore & Attributes
- .gitignore policy
  - Keep a single root `.gitignore`; add per-app overrides only when necessary in monorepos.
  - Ignore OS/editor files: `.DS_Store`, `Thumbs.db`, `*.swp`, `.idea/`, `.vscode/`.
  - Ignore dependency/build outputs: `node_modules/`, `.yarn/`, `.pnpm-store/`, `dist/`, `build/`, `coverage/`, `.cache/`, `.turbo/`, `.vite/`, `.parcel-cache/`, `target/`, `.gradle/`, `.venv/`, `__pycache__/`, `.pytest_cache/`, `.mypy_cache/`, `.tox/`, `.next/`, `.nuxt/`.
  - Ignore logs/temp: `*.log`, `*.pid`, `tmp/`, `.tmp/`.
  - Secrets: `.env`, `.env.local`, `.env.*.local`, `*.pem`, `*.key`, `*.p12`, `*.crt`. Commit only templates like `.env.example`.
  - If tracking inside ignored dirs is truly needed, use `!` negation sparingly and document why.
- .gitattributes baseline
  - `* text=auto eol=lf`
  - `*.sh text eol=lf`, `*.bat text eol=crlf`
  - Binary assets: `*.png binary`, `*.jpg binary`, `*.jpeg binary`, `*.gif binary`, `*.webp binary`, `*.pdf binary`, `*.zip binary`, `*.tar.gz binary`, `*.mp4 binary`
  - Lockfiles as text (no special merge strategy): `package-lock.json text`, `pnpm-lock.yaml text`, `yarn.lock text`, `Cargo.lock text`, `Pipfile.lock text`, `composer.lock text`
- Optional GitHub linguist hints: `docs/** linguist-documentation`, `**/dist/** linguist-generated`, `vendor/** linguist-vendored`

Plan First
- Always present the intended solution to the user before implementation.

 
