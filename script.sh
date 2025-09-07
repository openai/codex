# --- 1) Rewire remotes safely (block any push to upstream) ---

git remote -v
# Your current 'origin' points to the official repo. Rename it to 'upstream':
git remote rename origin upstream

# Block pushes to upstream (belt & suspenders)
git remote set-url --push upstream DISABLED

# Optional: hard block via pre-push hook
mkdir -p .git/hooks
cat > .git/hooks/pre-push <<'EOF'
#!/usr/bin/env bash
# Block any push to 'upstream' remote
if [ "$1" = "upstream" ]; then
  echo "Push to 'upstream' is blocked by policy."; exit 1
fi
EOF
chmod +x .git/hooks/pre-push

# Add your internal remote as the new 'origin'
git remote add origin git@github.com:renatogalera/codex.git

# Make 'origin' the default target for plain `git push`
git config remote.pushDefault origin
git config pull.ff only
git config rerere.enabled true

git fetch --all --tags --prune --recurse-submodules


# --- 2) (Agora sim) Checkpoint local, sem risco de ir pro upstream ---

git status
git add -A
git commit -m "chore: checkpoint before upstream/integration split" || true

# (Opcional) Safety tag só no seu remoto interno
git tag -a safety/before-split-$(date +%Y%m%d) -m "Safety tag before split"
git push origin refs/tags/safety/before-split-$(date +%Y%m%d)


# --- 3) Criar o espelho local upstream/main (limpo, sem suas alterações) ---

git fetch upstream --tags --recurse-submodules
git checkout -B upstream/main remotes/upstream/main
git push -u origin upstream/main


# --- 4) Levar seu trabalho atual para integration/main via rebase ---

# Salve uma referência do seu estado atual (onde estão suas mudanças)
git checkout -                   # volta ao branch anterior, se necessário
git branch work/current          # marca o HEAD atual com suas alterações

# Crie uma branch de bootstrap com seu trabalho e rebase sobre upstream/main
git checkout -b integration/bootstrap work/current
git rebase --rebase-merges --reapply-cherry-picks upstream/main
# (Resolve conflitos se houver: `git status` → `git add -A` → `git rebase --continue`)

# Promova para a branch final de integração
git branch -m integration/main
git push -u origin integration/main

# Configure o tracking explicitamente (garante futuros pulls/pushes corretos)
git branch --set-upstream-to=origin/integration/main integration/main


# --- 5) Ajustes no remoto interno (UI/Config do servidor) ---

# - Defina 'integration/main' como Default Branch do repositório interno.
# - Proteja 'upstream/main' (FF-only, ninguém dá push).
# - Proteja 'integration/main' (CI obrigatório, FF-only; permitir force-push só para rollback, se necessário).

