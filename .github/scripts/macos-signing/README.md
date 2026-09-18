# Provisioned CLI signing identity

The provisioned CLI has two deliberately separate identifiers:

| Purpose                               | Identifier                      |
| ------------------------------------- | ------------------------------- |
| Code-signing identifier               | `codex`                         |
| Bundle identifier                     | `com.openai.codex.cli`          |
| Provisioned App ID and keychain group | `<TeamID>.com.openai.codex.cli` |

The code-signing identifier preserves access to existing login-keychain items
whose access rules require the original CLI identity. Giving the CLI an app
bundle must not change that identifier. The bundle identifier, App ID,
entitlements, profile, and approved certificate remain independently validated.
The native-verification keys use the provisioned keychain group; they do not
replace existing login-keychain credential access rules.

Changing the new binary's designated requirement to accept both identifiers
would not make it satisfy an existing item's requirement for `codex`. Items
created by a binary signed as `com.openai.codex.cli` are a separate compatibility
case: restoring `codex` does not silently authorize access to those items.

The signing backend must honor an explicit `--binary-identifier` when signing
an app bundle. The wrapper signs the bundle once; rcodesign seals its metadata
and resources while retaining `codex` as the main executable's identity.
Release tool artifact URIs and SHA-256 digests are pinned in the signing
environment configuration.

## Release verification

When `CODEX_PROVISIONED_MACOS_CANDIDATE` is enabled, tag releases automatically
publish the provisioned archives and versioned `codex-provisioned` manifest after
both architecture packages pass the existing signature, entitlement, profile,
architecture, stapling, Gatekeeper, and package-smoke checks. An enabled
provisioned build that fails verification blocks release publication.

The signing-driver tests use generated credentials and stubbed native tools.
They cover the signing command contract and failure handling. Actual keychain
access and credential recovery across identity changes require runtime testing;
these unit tests do not establish that behavior.
