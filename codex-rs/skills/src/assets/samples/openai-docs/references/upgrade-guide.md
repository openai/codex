# Model upgrade guidance

Use this file only as a bundled routing fallback when the live migration guide cannot be fetched.

For latest, current, default, or unspecified-model upgrades:

1. Run `scripts/resolve-latest-model-info`.
2. Fetch the returned `migrationGuideUrl` and `promptingGuideUrl` exactly.
3. Treat the live guides as canonical.
4. If remote retrieval fails, disclose that bundled fallback guidance is being used.

For an explicit GPT-5.6 Sol or GPT-5.6-family migration:

1. Preserve the user's explicit target; do not run the latest-model resolver.
2. Fetch the live GPT-5.6 model guidance:

   https://developers.openai.com/api/docs/guides/model-guidance?model=gpt-5.6

3. Read `references/upgrading-to-gpt-5p6-sol.md` for skill-specific migration judgment.
4. Read `references/prompting-guide.md` only when prompt changes are needed.

For another explicit model target, preserve that target and fetch its current official guidance. Do not reuse GPT-5.6-specific defaults, API shapes, or compatibility rules for a different model.
