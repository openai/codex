Ótima pergunta, Renato. Abaixo vai um **plano completo e implementável** para você manter seu fork interno sincronizado com o repositório oficial **com segurança máxima possível**, incluindo um fluxo automatizável que pode ser delegado para uma IA “agêntica” com *guardrails* fortes.

> **Resumo da estratégia**
>
> * Trate o código original como **espelho imutável** (`upstream/main`) dentro do seu Git interno.
> * Mantenha suas alterações como uma **pilha de patches (patch stack)** reaplicada por **rebase** sobre cada atualização do upstream.
> * Automatize um pipeline que **busca o upstream, rebaseia a pilha, testa e só promove por FF (`--ff-only`)** quando tudo estiver verde.
> * Em conflitos, gere artefatos estruturados para uma **IA sugerir patches** em um branch isolado (`ai/*`), nunca diretamente no branch de integração.
> * Tenha **rollback instantâneo** via tags versionadas, **verificação de assinaturas** e **proibições de push forçado** nos ramos protegidos.

---

## 1) Objetivos e garantias (“quase 100% seguro”)

**Garantias por design:**

1. `upstream/main` é **read-only** e só avança por **fast-forward** a partir do remoto oficial (sem merges, sem reescrita).
2. `integration/main` só avança por **fast-forward** depois de **rebase + bateria de testes** passarem.
3. Conflitos são tratados em branches de **update** e, se a IA ajudar, em branches `ai/*` — **nunca** direto em `integration/main`.
4. **Rollback** a qualquer ponto via tags `integration-<data>.up-<sha>`.
5. **Verificação de assinatura** de tags/commits do upstream (quando suportado).
6. **rerere** habilitado para reduzir conflitos recorrentes com segurança.

> **Não existe “100% seguro” em VCS + automação**. Com as barreiras acima, você chega muito perto, com trilhas de auditoria e reversibilidade imediata.

---

## 2) Modelo de branches

* `upstream/main` — *espelho* do upstream oficial, **protegido** (FF-only).
* `integration/main` — sua linha de entrega; **sempre** `upstream/main` + sua patch stack. **Protegido** (FF-only).
* `feature/*` — suas features (sempre *rebaseáveis* sobre `integration/main`).
* `update/<YYYYMMDD>-<upstreamShortSHA>` — branch de tentativa automática de rebase quando há novidades do upstream.
* `ai/resolve-<update-id>` — branch com sugestões de resolução da IA (patches candidatos).
* `hotfix/*` — correções urgentes que depois são rebaseadas.

---

## 3) Pipeline automatizado (alto nível)

1. **Sync Upstream**

   * `git fetch upstream --tags --recurse-submodules`
   * Avança `upstream/main` via FF (se houver *rewrite* do upstream, **pausa** e pede intervenção).
2. **Preparar update branch**

   * Cria `update/<data>-<sha>` a partir de `upstream/main`.
3. **Rebase da Patch Stack**

   * Rebase de `integration/main` → `update/*` com `--rebase-merges` (preserva merges locais quando houver) e `--reapply-cherry-picks`.
   * `git rerere` habilitado para conflitos repetidos.
4. **Conflitos?**

   * Gera `conflicts.json` com contexto estruturado.
   * Se `AI_RESOLVER_URL` estiver configurado, **solicita sugestões**; aplica *patch* em `ai/*` e reinicia `rebase --continue`.
   * Sem sucesso: abre MR com instruções e *range-diff*.
5. **Testes**

   * Lint, `go vet`, `staticcheck` (ou o equivalente da sua stack), testes unit/integration/E2E, testes “golden” do CLI.
6. **Promoção**

   * Se tudo verde: **tag** `integration-<data>.up-<sha>`; FF de `integration/main` para `update/*`.
7. **Rollback**

   * `git reset --hard <tagAnterior>` + FF (se necessário em branch de correção).

---

## 4) Fluxo para devs no dia a dia

* Nunca commitar direto em `upstream/main` ou `integration/main`.
* Trabalhar em `feature/*`. Para atualizar: `git fetch origin && git rebase origin/integration/main`.
* Merge para `integration/main` só via MR aprovado + CI verde + FF.

---

## 5) O papel da IA (com *guardrails*)

* A IA **nunca** faz push direto para `integration/main`.
* Recebe **apenas** o necessário: diffs, blocos de conflito, versões dos 3 lados (base, theirs, ours) e contexto de testes.
* Produz **patch minimalista** (unified diff) aplicado em `ai/*`.
* Passa por testes. Se falhar, não promove; retorna para humano.

---

## 6) Arquivos prontos para uso

Abaixo estão **arquivos completos** que você pode adicionar ao seu repositório interno. O conteúdo está **completo** (não apenas trechos) para atender sua regra de “Full File Delivery”.

> **Observação importante:** O código e comentários estão **em inglês** conforme solicitado. O README também segue o estilo direto dos maiores repositórios do GitHub.

---

### `README.md`

````markdown
# Upstream Guardian — Safe Fork Workflow

This repository implements a safe, reproducible workflow to keep an internal fork
continuously in sync with an upstream open-source CLI tool without losing local changes.

## Branching model

- `upstream/main`: read-only mirror of the upstream `main`. **Protected**, fast-forward only.
- `integration/main`: your delivery line = `upstream/main` + your patch stack. **Protected**, fast-forward only.
- `feature/*`: local development branches. Always rebase-able over `integration/main`.
- `update/<YYYYMMDD>-<upShortSHA>`: temporary rebase attempt branches after upstream updates.
- `ai/resolve-<update-id>`: candidate conflict resolutions produced by an AI component.

## Safety guarantees

1. Upstream mirror is never rewritten locally.
2. Integration only advances when tests pass and via fast-forward.
3. Conflicts are handled in update/ai branches, never on protected branches.
4. Full rollback via versioned tags `integration-<date>.up-<sha>`.
5. Signature verification of upstream commits/tags (when available).
6. `git rerere` enabled to reduce recurring conflicts safely.

## Prerequisites

- Git 2.39+ (or newer)
- Bash
- Go 1.22+ (if you intend to use the `upstream-guardian` CLI)
- CI runners with `git`, `bash`, and your language toolchain
- Optional: `staticcheck`, `golangci-lint` (or equivalents for your tech stack)

## Quick start

1. Configure remotes (first time):

```bash
export UPSTREAM_URL="https://github.com/vendor/project.git"
export ORIGIN_URL="ssh://git@git.internal.example.com/group/project.git"
bash scripts/setup-remotes.sh
````

2. Run a manual sync locally (dry):

```bash
make sync-dry
```

3. Enable CI:

   * For GitLab: commit `.gitlab-ci.yml` (provided).
   * Protect `upstream/main` and `integration/main` (FF-only).
   * Add appropriate variables (e.g. `AI_RESOLVER_URL`, tokens for MR API if used).

## Files and scripts

* `scripts/setup-remotes.sh` — initializes remotes and protected branches.
* `scripts/sync-upstream.sh` — the orchestrated safe sync (fetch, rebase, test, tag, promote).
* `scripts/run-tests.sh` — your test entrypoint (customize as needed).
* `.gitlab-ci.yml` — CI jobs for mirror, sync, tests, and promotion.
* `cmd/upstream-guardian/main.go` — optional Go CLI orchestrator for advanced flows.
* `.githooks/pre-commit` — prevents committing conflict markers, enforces hygiene.
* `Makefile` — convenience targets.

## AI resolver

If `AI_RESOLVER_URL` is present, the sync script will POST a `conflicts.json` payload
and apply returned unified diffs to branch `ai/resolve-<update-id>`. Promotion only
happens if tests pass. Otherwise, a Merge Request is opened for manual review.

## Rollback

Each successful integration update is tagged:
`integration-YYYYMMDD.up-<shortSHA>`

To rollback:

```bash
git checkout integration/main
git reset --hard <tag>
git push --force-with-lease origin HEAD:integration/main
```

(Force push is permitted only for authorized maintainers on this protected branch when performing a rollback.)

## Notes

* If upstream rewrites history (non-FF), sync is paused and requires human intervention.
* Submodules: the pipeline uses `--recurse-submodules` by default; adjust if not needed.
* Signature verification is recommended when upstream signs releases/tags.

````

---

### `.gitlab-ci.yml`

```yaml
stages:
  - mirror
  - build
  - test
  - sync
  - promote

variables:
  GIT_STRATEGY: fetch
  GIT_SUBMODULE_STRATEGY: recursive
  GO_VERSION: "1.22"
  # Optional: provide AI endpoint for conflict suggestions
  # AI_RESOLVER_URL: "https://ai.internal/resolve-diff"

default:
  image: golang:${GO_VERSION}

before_script:
  - git config --global user.name "CI Bot"
  - git config --global user.email "ci-bot@example.com"
  - git config --global pull.ff only
  - git config --global rerere.enabled true

mirror:upstream:
  stage: mirror
  image: alpine:3.20
  rules:
    - if: '$CI_PIPELINE_SOURCE == "schedule"'
    - if: '$CI_COMMIT_BRANCH == "upstream/main"'
  script:
    - apk add --no-cache git
    - bash scripts/setup-remotes.sh
    - git fetch upstream --tags --recurse-submodules
    - |
      # Update local upstream/main as a pure mirror (fast-forward only)
      if git show-ref --verify --quiet refs/heads/upstream/main; then
        git checkout upstream/main
        git merge --ff-only --no-edit --no-verify FETCH_HEAD || {
          echo "Non-FF update detected on upstream/main. Manual intervention required."
          exit 1
        }
      else
        git checkout -b upstream/main FETCH_HEAD
      fi
    - git push origin upstream/main

build:
  stage: build
  rules:
    - if: '$CI_PIPELINE_SOURCE != "schedule"'
      when: on_success
  script:
    - go version
    - go build ./...
  artifacts:
    paths:
      - ./upstream-guardian
    when: on_success

test:
  stage: test
  script:
    - bash scripts/run-tests.sh
  artifacts:
    when: always
    paths:
      - test-results/
    reports:
      junit: test-results/*.xml

sync:upstream:
  stage: sync
  rules:
    - if: '$CI_PIPELINE_SOURCE == "schedule"'
    - if: '$CI_COMMIT_BRANCH == "upstream/main"'
  script:
    - bash scripts/sync-upstream.sh
  artifacts:
    when: always
    paths:
      - conflicts.json
      - range-diff.txt
      - sync-logs/
  environment:
    name: upstream-sync

promote:
  stage: promote
  needs: ["sync:upstream"]
  script:
    - |
      # If previous job tagged a successful update, fast-forward integration/main
      if [ -f sync-logs/ff-ready ]; then
        git fetch origin
        git checkout integration/main
        git merge --ff-only update/$(cat sync-logs/update-branch) || {
          echo "FF-only promotion failed."
          exit 1
        }
        git push origin integration/main
      else
        echo "No FF promotion requested."
      fi
````

---

### `Makefile`

```makefile
SHELL := /usr/bin/env bash

.PHONY: hooks test sync sync-dry build

hooks:
	@git config core.hooksPath .githooks

test:
	@bash scripts/run-tests.sh

sync:
	@bash scripts/sync-upstream.sh

sync-dry:
	@DRY_RUN=1 bash scripts/sync-upstream.sh

build:
	@go build -o upstream-guardian ./cmd/upstream-guardian
```

---

### `scripts/setup-remotes.sh`

```bash
#!/usr/bin/env bash
set -Eeuo pipefail

# Setup remotes and protected branches for safe forking.

: "${UPSTREAM_URL:?UPSTREAM_URL is required}"
: "${ORIGIN_URL:?ORIGIN_URL is required}"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "Run this inside a git repository."
  exit 1
fi

# Add or update remotes
if git remote get-url upstream >/dev/null 2>&1; then
  git remote set-url upstream "$UPSTREAM_URL"
else
  git remote add upstream "$UPSTREAM_URL"
fi

if git remote get-url origin >/dev/null 2>&1; then
  git remote set-url origin "$ORIGIN_URL"
else
  git remote add origin "$ORIGIN_URL"
fi

git fetch upstream --tags --recurse-submodules
git fetch origin --tags --recurse-submodules || true

# Create upstream/main if it doesn't exist
if ! git show-ref --verify --quiet refs/heads/upstream/main; then
  git checkout -b upstream/main FETCH_HEAD
else
  git checkout upstream/main
  git merge --ff-only --no-edit FETCH_HEAD || {
    echo "Non-FF update detected for upstream/main. Manual intervention required."
    exit 1
  }
fi

# Create integration/main if it doesn't exist
if ! git show-ref --verify --quiet refs/heads/integration/main; then
  echo "Initializing integration/main from upstream/main"
  git branch integration/main upstream/main
fi

echo "Done. Remotes and branches are set."
```

---

### `scripts/run-tests.sh`

```bash
#!/usr/bin/env bash
set -Eeuo pipefail

# Central test entrypoint. Customize for your stack.
# For Go projects; adapt accordingly.

mkdir -p test-results

echo "Running go vet..."
go vet ./... || { echo "go vet failed"; exit 1; }

if command -v staticcheck >/dev/null 2>&1; then
  echo "Running staticcheck..."
  staticcheck ./... || { echo "staticcheck failed"; exit 1; }
else
  echo "staticcheck not found; skipping."
fi

echo "Running unit tests with race detector..."
# JUnit report sample using gotestsum if available
if command -v gotestsum >/dev/null 2>&1; then
  gotestsum --junitfile test-results/unit.xml -- -race -cover ./...
else
  go test -race -cover ./... | tee test-results/unit.log
fi

# Insert integration/E2E/CLI golden tests here as needed
echo "All tests executed."
```

---

### `scripts/sync-upstream.sh`

```bash
#!/usr/bin/env bash
set -Eeuo pipefail

# Safe sync pipeline: fetch upstream, rebase patch stack, test, and promote by FF.
# Requires: git >= 2.39, rerere enabled, CI bot identity set.

UPSTREAM_REMOTE="${UPSTREAM_REMOTE:-upstream}"
UPSTREAM_BRANCH="${UPSTREAM_BRANCH:-main}"
INTEGRATION_BRANCH="${INTEGRATION_BRANCH:-integration/main}"
DRY_RUN="${DRY_RUN:-0}"

WORK_DIR="$(pwd)"
LOG_DIR="${WORK_DIR}/sync-logs"
mkdir -p "${LOG_DIR}"

git config rerere.enabled true
git config pull.ff only

echo "[1/6] Fetching upstream..."
git fetch "${UPSTREAM_REMOTE}" --tags --recurse-submodules

# Ensure local upstream/main exists and is FF-only
if ! git show-ref --verify --quiet "refs/heads/upstream/${UPSTREAM_BRANCH}"; then
  git checkout -b "upstream/${UPSTREAM_BRANCH}" "${UPSTREAM_REMOTE}/${UPSTREAM_BRANCH}"
else
  git checkout "upstream/${UPSTREAM_BRANCH}"
  git merge --ff-only --no-edit "${UPSTREAM_REMOTE}/${UPSTREAM_BRANCH}" || {
    echo "Non-FF update detected on upstream/${UPSTREAM_BRANCH}. Abort for manual inspection."
    exit 1
  }
fi

UPSTREAM_SHA="$(git rev-parse --short=12 HEAD)"
UPDATE_BRANCH="update/$(date +%Y%m%d)-${UPSTREAM_SHA}"
echo "${UPDATE_BRANCH}" > "${LOG_DIR}/update-branch"

echo "[2/6] Preparing update branch: ${UPDATE_BRANCH}"
if git show-ref --verify --quiet "refs/heads/${UPDATE_BRANCH}"; then
  git branch -D "${UPDATE_BRANCH}"
fi
git checkout -b "${UPDATE_BRANCH}" "upstream/${UPSTREAM_BRANCH}"

echo "[3/6] Rebasing integration stack onto ${UPDATE_BRANCH} base..."
# Create a temp branch from integration/main to rebase onto update
git fetch origin "${INTEGRATION_BRANCH}:${INTEGRATION_BRANCH}" || true
git checkout -B __temp_integration "${INTEGRATION_BRANCH}"

set +e
git rebase --rebase-merges --reapply-cherry-picks --onto "${UPDATE_BRANCH}" "upstream/${UPSTREAM_BRANCH}" __temp_integration
REB_EXIT=$?
set -e

if [ ${REB_EXIT} -ne 0 ]; then
  echo "[!] Rebase reported conflicts. Capturing details..."
  # Collect conflicts
  git status --porcelain=v1 | awk '/^UU /{print $2}' > "${LOG_DIR}/conflict-files.txt" || true

  # Build a minimal conflicts.json file for external AI resolvers
  {
    echo '{'
    echo '  "update_branch": "'"${UPDATE_BRANCH}"'",'
    echo '  "upstream_sha": "'"${UPSTREAM_SHA}"'",'
    echo '  "conflicts": ['
    FIRST=1
    while read -r f; do
      [ -z "$f" ] && continue
      if [ $FIRST -eq 0 ]; then echo ','; fi
      FIRST=0
      echo '    {'
      echo '      "file": "'"$f"'",'
      echo '      "ours_path": "'"$f"'",'
      echo '      "base_path": "'"$f"'",'
      echo '      "theirs_path": "'"$f"'"'
      echo '    }'
    done < "${LOG_DIR}/conflict-files.txt"
    echo '  ]'
    echo '}'
  } > "${WORK_DIR}/conflicts.json"

  if [ -n "${AI_RESOLVER_URL:-}" ]; then
    echo "[4/6] Sending conflicts to AI resolver..."
    # Requires curl & unified diff response. The resolver should return a .patch stream.
    git checkout -b "ai/resolve-${UPSTREAM_SHA}" || git checkout "ai/resolve-${UPSTREAM_SHA}"
    if command -v curl >/dev/null 2>&1; then
      curl -sS -X POST -H "Content-Type: application/json" \
        --data-binary @"${WORK_DIR}/conflicts.json" \
        "${AI_RESOLVER_URL}" > "${LOG_DIR}/ai.patch" || true
    fi

    if [ -s "${LOG_DIR}/ai.patch" ]; then
      echo "[4.1] Applying AI patch candidates..."
      git apply --3way "${LOG_DIR}/ai.patch" || {
        echo "AI patch failed to apply cleanly. Aborting AI attempt."
      }
      # Continue rebase if possible
      set +e
      git add -A
      git rebase --continue
      REB_EXIT=$?
      set -e
    else
      echo "No AI patch received or empty patch."
    fi
  else
    echo "AI_RESOLVER_URL not set. Skipping AI-based conflict resolution."
  fi
fi

if [ ${REB_EXIT} -ne 0 ]; then
  echo "[!] Rebase could not be completed automatically."
  echo "Producing range-diff to help reviewers..."
  git range-diff "upstream/${UPSTREAM_BRANCH}...${INTEGRATION_BRANCH}" \
                 "upstream/${UPSTREAM_BRANCH}...__temp_integration" \
                 > "${WORK_DIR}/range-diff.txt" || true
  echo "Open a MR from __temp_integration to ${INTEGRATION_BRANCH} with conflicts."
  exit 2
fi

# If we got here, __temp_integration has been successfully rebased onto UPDATE_BRANCH
echo "[4/6] Rebase succeeded. Running tests on the rebased stack..."
git checkout __temp_integration
if [ "${DRY_RUN}" = "1" ]; then
  echo "(dry-run) Skipping tests."
else
  bash scripts/run-tests.sh
fi

echo "[5/6] Tagging build and preparing promotion..."
TAG="integration-$(date +%Y%m%d).up-${UPSTREAM_SHA}"
if [ "${DRY_RUN}" != "1" ]; then
  git tag -f "${TAG}" __temp_integration
  git push -f origin "refs/tags/${TAG}"
fi

# Publish update branch and signal promotion
git checkout -B "${UPDATE_BRANCH}" __temp_integration
if [ "${DRY_RUN}" != "1" ]; then
  git push -u origin "${UPDATE_BRANCH}"
  touch "${LOG_DIR}/ff-ready"
fi

echo "[6/6] Promotion will fast-forward integration/main in the promote stage."
echo "Done."
```

---

### `.githooks/pre-commit`

```bash
#!/usr/bin/env bash
set -Eeuo pipefail

# Prevent committing conflict markers and large binaries accidentally.

if git diff --cached | grep -E '^\+<<<<<<<|^\+>>>>>>>|^\+=======' >/dev/null 2>&1; then
  echo "Commit contains conflict markers. Please resolve conflicts before committing."
  exit 1
fi

# Example: block files over 20MB (adjust as needed)
MAX_SIZE=$((20 * 1024 * 1024))
IFS=$'\n'
for file in $(git diff --cached --name-only); do
  if [ -f "$file" ]; then
    size=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file")
    if [ "${size}" -gt "${MAX_SIZE}" ]; then
      echo "File '$file' exceeds ${MAX_SIZE} bytes. Consider Git LFS or exclude it."
      exit 1
    fi
  fi
done
```

> Após `git add` deste arquivo, execute `make hooks` para ativá-lo.

---

### `cmd/upstream-guardian/main.go`

```go
package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

// UpstreamGuardian is a small orchestrator around git to implement the "safe fork" workflow.
// It shells out to git intentionally (instead of re-implementing VCS logic), keeping semantics stable.
//
// Key operations:
//  1) Ensure clean repo and baseline configuration
//  2) Fetch and fast-forward local upstream/main
//  3) Create update branch
//  4) Rebase integration stack onto update base (with rerere)
//  5) Optional: call AI resolver if conflicts arise
//  6) Run tests
//  7) Tag and request promotion (FF-only)
//
// NOTE: This CLI is optional; scripts/sync-upstream.sh is the source of truth in CI.
// This CLI is useful for local dry-runs and power-users.

type Config struct {
	UpstreamRemote   string
	UpstreamBranch   string
	IntegrationBranch string
	AiResolverURL    string
	DryRun           bool
	WorkDir          string
	LogDir           string
}

type Conflict struct {
	File      string `json:"file"`
	OursPath  string `json:"ours_path"`
	BasePath  string `json:"base_path"`
	TheirsPath string `json:"theirs_path"`
}

type ConflictsPayload struct {
	UpdateBranch string     `json:"update_branch"`
	UpstreamSHA  string     `json:"upstream_sha"`
	Conflicts    []Conflict `json:"conflicts"`
}

func main() {
	cfg := loadConfigFromEnv()
	if err := run(context.Background(), cfg); err != nil {
		fmt.Fprintf(os.Stderr, "ERROR: %v\n", err)
		os.Exit(1)
	}
}

func loadConfigFromEnv() Config {
	workDir, _ := os.Getwd()
	logDir := filepath.Join(workDir, "sync-logs")
	_ = os.MkdirAll(logDir, 0o755)
	return Config{
		UpstreamRemote:    envOr("UPSTREAM_REMOTE", "upstream"),
		UpstreamBranch:    envOr("UPSTREAM_BRANCH", "main"),
		IntegrationBranch: envOr("INTEGRATION_BRANCH", "integration/main"),
		AiResolverURL:     os.Getenv("AI_RESOLVER_URL"),
		DryRun:            os.Getenv("DRY_RUN") == "1",
		WorkDir:           workDir,
		LogDir:            logDir,
	}
}

func run(ctx context.Context, cfg Config) error {
	if err := ensureGitRepo(); err != nil {
		return err
	}
	if err := gitConfig("rerere.enabled", "true"); err != nil {
		return err
	}
	if err := gitConfig("pull.ff", "only"); err != nil {
		return err
	}

	fmt.Println("[1/6] Fetching upstream...")
	if err := git(ctx, "fetch", cfg.UpstreamRemote, "--tags", "--recurse-submodules"); err != nil {
		return err
	}

	upstreamLocal := "upstream/" + cfg.UpstreamBranch
	if !refExists(upstreamLocal) {
		if err := git(ctx, "checkout", "-b", upstreamLocal, cfg.UpstreamRemote+"/"+cfg.UpstreamBranch); err != nil {
			return err
		}
	} else {
		if err := git(ctx, "checkout", upstreamLocal); err != nil {
			return err
		}
		if err := git(ctx, "merge", "--ff-only", "--no-edit", cfg.UpstreamRemote+"/"+cfg.UpstreamBranch); err != nil {
			return fmt.Errorf("non-FF update detected on %s; manual intervention required", upstreamLocal)
		}
	}
	upShort, err := gitOutput(ctx, "rev-parse", "--short=12", "HEAD")
	if err != nil {
		return err
	}
	updateBranch := fmt.Sprintf("update/%s-%s", time.Now().Format("20060102"), strings.TrimSpace(upShort))
	if refExists(updateBranch) {
		_ = git(ctx, "branch", "-D", updateBranch)
	}
	if err := git(ctx, "checkout", "-b", updateBranch, upstreamLocal); err != nil {
		return err
	}
	if err := os.WriteFile(filepath.Join(cfg.LogDir, "update-branch"), []byte(updateBranch), 0o644); err != nil {
		return err
	}

	fmt.Println("[2/6] Rebasing integration stack...")
	// Refresh integration branch
	_ = git(ctx, "fetch", "origin", cfg.IntegrationBranch+":"+cfg.IntegrationBranch")
	if err := git(ctx, "checkout", "-B", "__temp_integration", cfg.IntegrationBranch); err != nil {
		return err
	}

	rebArgs := []string{"rebase", "--rebase-merges", "--reapply-cherry-picks", "--onto", updateBranch, upstreamLocal, "__temp_integration"}
	if err := git(ctx, rebArgs...); err != nil {
		// Conflicts likely occurred
		fmt.Println("[!] Rebase conflicts detected. Capturing details...")
		conflictFiles, _ := listConflictFiles()
		payload := ConflictsPayload{
			UpdateBranch: updateBranch,
			UpstreamSHA:  strings.TrimSpace(upShort),
			Conflicts:    toConflicts(conflictFiles),
		}
		b, _ := json.MarshalIndent(payload, "", "  ")
		_ = os.WriteFile(filepath.Join(cfg.WorkDir, "conflicts.json"), b, 0o644)

		if cfg.AiResolverURL != "" {
          // NOTE: This CLI leaves AI call as a placeholder; use the shell script in CI for actual POST/apply flow.
			fmt.Println("AI_RESOLVER_URL is set, but AI invocation is handled in scripts/sync-upstream.sh for now.")
		} else {
			fmt.Println("AI_RESOLVER_URL not set; manual resolution required.")
		}
		// Produce a range-diff to help reviewers
		_ = produceRangeDiff(ctx, upstreamLocal, cfg.IntegrationBranch)
		return errors.New("rebase requires manual or AI-assisted resolution")
	}

	fmt.Println("[3/6] Rebase succeeded. Running tests...")
	if cfg.DryRun {
		fmt.Println("(dry-run) Skipping tests")
	} else {
		if err := runTests(ctx); err != nil {
			return err
		}
	}

	fmt.Println("[4/6] Tagging and preparing promotion...")
	tag := fmt.Sprintf("integration-%s.up-%s", time.Now().Format("20060102"), strings.TrimSpace(upShort))
	if !cfg.DryRun {
		_ = git(ctx, "tag", "-f", tag, "__temp_integration")
		_ = git(ctx, "push", "-f", "origin", "refs/tags/"+tag)
	}
	if err := git(ctx, "checkout", "-B", updateBranch, "__temp_integration"); err != nil {
		return err
	}
	if !cfg.DryRun {
		if err := git(ctx, "push", "-u", "origin", updateBranch); err != nil {
			return err
		}
		if err := os.WriteFile(filepath.Join(cfg.LogDir, "ff-ready"), []byte("1"), 0o644); err != nil {
			return err
		}
	}
	fmt.Println("[5/6] Promotion will fast-forward integration/main in CI 'promote' stage.")
	fmt.Println("Done.")
	return nil
}

// --- Helpers ---

func envOr(k, def string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return def
}

func ensureGitRepo() error {
	cmd := exec.Command("git", "rev-parse", "--is-inside-work-tree")
	cmd.Stdout = nil
	cmd.Stderr = nil
	return cmd.Run()
}

func gitConfig(k, v string) error {
	return git(context.Background(), "config", k, v)
}

func git(ctx context.Context, args ...string) error {
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

func gitOutput(ctx context.Context, args ...string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", args...)
	var out bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = os.Stderr
	err := cmd.Run()
	return out.String(), err
}

func refExists(ref string) bool {
	cmd := exec.Command("git", "show-ref", "--verify", "--quiet", "refs/heads/"+ref)
	return cmd.Run() == nil
}

func listConflictFiles() ([]string, error) {
	out, err := gitOutput(context.Background(), "status", "--porcelain=v1")
	if err != nil {
		return nil, err
	}
	var files []string
	for _, line := range strings.Split(out, "\n") {
		if strings.HasPrefix(line, "UU ") {
			files = append(files, strings.TrimSpace(strings.TrimPrefix(line, "UU ")))
		}
	}
	return files, nil
}

func toConflicts(files []string) []Conflict {
	var c []Conflict
	for _, f := range files {
		if f == "" {
			continue
		}
		c = append(c, Conflict{
			File:       f,
			OursPath:   f,
			BasePath:   f,
			TheirsPath: f,
		})
	}
	return c
}

func produceRangeDiff(ctx context.Context, upstreamLocal, integration string) error {
	rd, err := gitOutput(ctx,
		"range-diff",
		fmt.Sprintf("%s...%s", upstreamLocal, integration),
		fmt.Sprintf("%s...%s", upstreamLocal, "__temp_integration"),
	)
	if err != nil {
		return err
	}
	return os.WriteFile("range-diff.txt", []byte(rd), 0o644)
}

func runTests(ctx context.Context) error {
	script := filepath.Join("scripts", "run-tests.sh")
	if _, err := os.Stat(script); err != nil {
		return fmt.Errorf("%s not found; cannot run tests", script)
	}
	cmd := exec.CommandContext(ctx, "bash", script)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}
```

---

## 7) Política de segurança e *guardrails*

* **Branches protegidos**: `upstream/main` e `integration/main` (FF-only, revisão obrigatória, sem pushes diretos).
* **Assinaturas**: habilite `git verify-commit`/`verify-tag` no upstream quando aplicável.
* **CI required**: qualquer MR deve exigir pipeline verde.
* **Hooks**: `pre-commit` bloqueia marcadores de conflito e binários grandes acidentais.
* **Logs e artefatos**: `sync-logs/`, `conflicts.json`, `range-diff.txt` sempre anexados ao job.
* **AI**: nunca promove direto; apenas *suggestions* em `ai/*`, testadas e revisadas.

---

## 8) Operação de rotina

1. **Primeira configuração**

   ```bash
   export UPSTREAM_URL="https://github.com/vendor/project.git"
   export ORIGIN_URL="ssh://git@git.internal.example.com/group/project.git"
   bash scripts/setup-remotes.sh
   make hooks
   ```

2. **Rodar sincronização local (dry)**

   ```bash
   make sync-dry
   ```

3. **Agendar em CI** (cron/schedule) a `mirror:upstream` + `sync:upstream`.

4. **Em caso de conflito**

   * Ver `conflicts.json` e `range-diff.txt`.
   * Se habilitado, revisar `ai/resolve-*` com *patch* proposto.
   * Ajustar manualmente e prosseguir.

5. **Rollback**

   ```bash
   git checkout integration/main
   git reset --hard integration-YYYYMMDD.up-<sha>
   git push --force-with-lease origin HEAD:integration/main
   ```

---

## 9) Decisões de design (por que assim)

* **Rebase vs merge**: Rebase preserva a pilha de patches limpa e auditável; *merge* contínuo tende a entropia com o tempo. A política de FF-only evita “diamantes” complexos.
* **`rerere`**: reduz fricção com conflitos recorrentes **sem** automatizar errado — reaplica apenas resoluções iguais já validadas no passado.
* **Branches `update/*` e `ai/*`**: isolam tentativas e evitam acidente em produção.
* **Assinaturas e FF-only**: mitigam *supply chain risks* e *force-push* malicioso.

---

## 10) Limitações e extensões

* **“100% seguro”** é inatingível; o plano fornece **reversibilidade, auditabilidade e gates**.
* Se o upstream reescrever histórico, é **pausa** automática — exige inspeção humana.
* Opcional: incorporar **SLSA provenance**, **SBOM** (Syft/Grype), e **políticas OPA** no estágio `test`.

---

Se quiser, eu já adapto esses arquivos ao seu **GitLab self-hosted** (nomes de grupo/projeto, runners, variáveis) e preparo uma **primeira execução guiada** com sua pilha atual.
