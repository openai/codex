# Model upgrade guidance

Use this file only as a bundled routing fallback when the live migration guide cannot be fetched.

For latest, current, default, or unspecified-model upgrades:

1. Follow the platform-specific resolver instructions in `references/model-migration.md`.
2. Fetch the returned `migrationGuideUrl` exactly; fetch `promptingGuideUrl` only when prompting guidance or prompt changes are needed.
3. Treat the live guides as canonical.
4. If remote retrieval fails, disclose that bundled fallback guidance is being used.

For an explicitly requested GPT-6 model:

1. Preserve the user's explicit target; do not run the latest-model resolver.
2. Fetch the live GPT-6 model guidance:

   https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md

3. Verify the exact target's current official model page and compatibility rules; family examples do not establish support for every sibling. If availability or compatibility cannot be verified, preserve the requested target and report the blocker instead of substituting a model or implementing an unverified migration.
4. Read `references/upgrading-to-gpt-6-astra.md` only for unresolved skill-specific migration judgment.
5. Read `references/prompting-guide.md` only when prompt changes are needed.

For another explicit model target, preserve that target and fetch its current official guidance. Do not reuse another model's defaults, API shapes, or compatibility rules, including Astra-only restrictions for Sol or Luna.
