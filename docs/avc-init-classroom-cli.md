# `avc init classroom` — Meta-Orchestrator Specification

## Purpose

`avc init classroom` is the bootstrap command for spinning up an **AVC Systems Studio** classroom deployment in minutes. It composes the Economy, Governance, Repair, and Glossary Engines into three coordinated repositories, optionally generates SDKs, and emits step-by-step deployment guidance tailored to a teacher or facilitator.

## Command synopsis

```bash
avc init classroom [CLASSROOM_NAME] \
  --repo-owner <github-user-or-org> \
  [--deployment-host netlify|vercel|local-only] \
  [--template finance|history|science] \
  [--generate-sdk]
```

- **CLASSROOM_NAME**: Required positional; becomes the parent folder and repository prefix (e.g., `MsLee_Math_2026`).
- The command is idempotent for an existing target directory when `--force` is later added (future extension) but currently fails fast on collisions.

## Flags

| Flag                | Type    | Description                                                                                                      | Required | Default   |
| ------------------- | ------- | ---------------------------------------------------------------------------------------------------------------- | -------- | --------- |
| `--repo-owner`      | string  | GitHub username/organization for all generated repos. Used in READMEs, workflow deploy targets, and git remotes. | Yes      | —         |
| `--deployment-host` | enum    | Primary API hosting target. Drives function scaffolding and CI templates.                                        | No       | `netlify` |
| `--template`        | enum    | Curriculum theme that seeds glossary terms and SEL prompt stubs.                                                 | No       | `finance` |
| `--generate-sdk`    | boolean | Emit TypeScript and Python client stubs for both APIs.                                                           | No       | `false`   |

## Output layout

```
[CLASSROOM_NAME]/
├── README.md                  # Teacher-facing launch guide
├── [CLASSROOM_NAME]-api/      # SmallWallets API (Economy + Governance hooks)
├── [CLASSROOM_NAME]-glossary/ # Glossary API (Repair + Glossary engines)
├── [CLASSROOM_NAME]-dashboard/# Static dashboard shell (Pages-friendly)
└── sdk/                       # Optional: TS + Python clients
```

## Execution flow

1. **Validate inputs**: Confirm `CLASSROOM_NAME` slug safety, `--repo-owner` presence, and supported enums. Refuse to overwrite existing targets.
2. **Create parent directory**: `mkdir [CLASSROOM_NAME]`.
3. **Generate repositories (concurrently when runtime permits)**:
   - **API repo** (`[CLASSROOM_NAME]-api`)
     - Scaffold `smallwallets.yaml` with placeholder operations: `transfer`, `glueApply`, `vote`, and shared schemas.
     - Create serverless handlers for the selected `--deployment-host`:
       - `netlify`: `netlify/functions/{transfer,glueApply,vote}.js` (or `.ts`) plus `netlify.toml`.
       - `vercel`: `api/{transfer,glueApply,vote}.ts` with `vercel.json`.
       - `local-only`: Express/Fastify dev server stub with `.env.example` only.
     - Add `public/index.html` wiring Swagger UI to `smallwallets.yaml`.
     - Write `README_DEPLOY.md` with host-specific steps and env vars (`API_KEY`, `NOTION_TOKEN`).
   - **Glossary repo** (`[CLASSROOM_NAME]-glossary`)
     - Seed `master_glossary.json` with MWRA Canon entries plus template-specific terms (finance/history/science switch).
     - Provide `GET /terms` endpoint (mirrored per hosting option) and placeholder Notion-to-JSON exporter script.
     - Include `README_DEPLOY.md` with deployment and sync instructions.
   - **Dashboard repo** (`[CLASSROOM_NAME]-dashboard`)
     - Static HTML/JS shell referencing the API + glossary endpoints and a “Duplicate Notion Template” CTA.
     - GitHub Pages–ready defaults (`/docs` output, no server dependencies).
4. **Pipeline automation**: Drop `.github/workflows/main.yml` into API and Glossary repos with CI + deploy-on-push for the chosen host; include lint/test placeholders and `NETLIFY_AUTH_TOKEN`/`VERCEL_TOKEN` hints when relevant.
5. **Optional SDK generation** (`--generate-sdk`):
   - Create `sdk/avc-classroom-client.ts` and `sdk/avc_classroom_client.py` with methods for `transfer`, `glueApply`, `vote`, and `get_term`.
   - Emit inline usage examples pointing to the generated API base URL env vars.
6. **Top-level README**: Summarize repos, env vars, and a minimal **Teacher Flow**: Deploy APIs → Duplicate Notion template → Run rituals (Economy/Governance/Repair cadence).
7. **Git initialization (deferred)**: Provide commented `git init` + remote setup hints without auto-pushing; users remain in control of credentials.

## File scaffolds (per repo)

- **API**
  - `smallwallets.yaml`: versioned OpenAPI stub with info block and security scheme placeholder.
  - `functions/` or `api/`: handlers returning mocked responses plus TODOs for ledger integrations.
  - `README_DEPLOY.md`: host-specific commands, required secrets, and sample curl calls.
  - `.github/workflows/main.yml`: install, lint/test, build, deploy.
- **Glossary**
  - `master_glossary.json`: MWRA Canon + template-inflected starter terms.
  - `routes/terms.{js,ts}`: returns glossary payload and etag stub.
  - `scripts/notion_export.{js,py}`: placeholder pipeline with TODO markers.
  - `.github/workflows/main.yml`: mirror of API workflow with glossary-specific deploy step.
- **Dashboard**
  - `index.html` + `assets/`: minimal dashboard pulling from API + glossary, with classroom branding from `CLASSROOM_NAME`.
  - `pages.config` or `vite.config` tuned for static hosting.

## Defaults and extensibility

- **Hosting priority**: Netlify is optimized (function layout + deploy instructions). Vercel mirrors structure; `local-only` skips deploy steps but keeps CI for lint/test.
- **Templates**: Each template swaps glossary seeds and SEL prompt snippets while keeping API surface identical.
- **Future flags** (reserved): `--force`, `--language ts|py`, `--with-tests` for richer handler scaffolds.

## Error handling & guardrails

- Fail fast on missing `--repo-owner`, unsupported enum values, or pre-existing directories.
- Emit actionable messages with the exact path to clean up before retrying.
- Never write secrets; only sample placeholders are generated.

## Example invocations

```bash
# Standard Netlify deployment with finance theme
avc init classroom MsLee_Math_2026 --repo-owner CodexArchitects

# Vercel + science glossary, with SDK stubs
avc init classroom Orion_Science_2027 \
  --repo-owner CodexArchitects \
  --deployment-host vercel \
  --template science \
  --generate-sdk
```
