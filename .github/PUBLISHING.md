# Publishing to Maven Central

This document describes how to set up and use the automated publishing workflow for the KotlinMania TUI libraries.

## Prerequisites

1. **Maven Central Account**: Register at [central.sonatype.com](https://central.sonatype.com)
2. **Verified Namespace**: `io.github.kotlinmania` (already verified)
3. **GPG Key**: For signing artifacts

## Required GitHub Secrets

Add these secrets to your repository at **Settings > Secrets and variables > Actions**:

| Secret Name | Description | How to Get |
|-------------|-------------|------------|
| `MAVEN_CENTRAL_USERNAME` | Maven Central token username | Generate at [Central Portal > Account > Generate User Token](https://central.sonatype.com) |
| `MAVEN_CENTRAL_PASSWORD` | Maven Central token password | Same as above |
| `SIGNING_KEY_ID` | Last 8 characters of your GPG key ID | Run `gpg --list-keys --keyid-format SHORT` |
| `SIGNING_KEY_PASSWORD` | Passphrase for your GPG key | The passphrase you set when creating the key |
| `SIGNING_KEY` | ASCII-armored private key | See below |

### Generating the Signing Key Secret

Export your GPG private key in ASCII armor format:

```bash
gpg --export-secret-keys --armor YOUR_KEY_ID > private-key.asc
```

Copy the entire contents of `private-key.asc` (including the `-----BEGIN PGP PRIVATE KEY BLOCK-----` and `-----END PGP PRIVATE KEY BLOCK-----` lines) into the `SIGNING_KEY` secret.

**Important**: Delete the `private-key.asc` file after copying!

```bash
rm private-key.asc
```

### Creating a GPG Key (if you don't have one)

```bash
# Generate a new key
gpg --full-generate-key

# Choose:
# - RSA and RSA (default)
# - 4096 bits
# - Key does not expire (or set expiration)
# - Your name and email

# List your keys to get the key ID
gpg --list-keys --keyid-format SHORT

# Upload public key to keyserver
gpg --keyserver keyserver.ubuntu.com --send-keys YOUR_KEY_ID
```

## Publishing

### Automatic (on Release)

1. Go to **Releases** in your GitHub repository
2. Click **Draft a new release**
3. Create a tag (e.g., `v0.1.0`)
4. Add release notes
5. Click **Publish release**

The workflow will automatically publish all libraries to Maven Central.

### Manual (via Workflow Dispatch)

1. Go to **Actions** > **Publish to Maven Central**
2. Click **Run workflow**
3. Select which library to publish (or "all")
4. Click **Run workflow**

## Library Dependencies

The libraries have the following dependency order:

```
roff-kotlin (no dependencies)
cansi-kotlin (no dependencies)
    └── anstyle-kotlin (depends on both)
```

The publish workflow handles this by:
1. Publishing `roff-kotlin` and `cansi-kotlin` in parallel
2. Publishing `anstyle-kotlin` after both complete

## Version Management

Before publishing a release:

1. Update the version in each library's `build.gradle.kts`:
   ```kotlin
   version = "0.1.0"  // Remove -SNAPSHOT for releases
   ```

2. Commit and push the version changes

3. Create and publish the release

4. After release, bump to the next snapshot version:
   ```kotlin
   version = "0.2.0-SNAPSHOT"
   ```

## Verifying Publication

After publishing:

1. Check [Maven Central](https://central.sonatype.com/search?q=io.github.kotlinmania) for your artifacts
2. Artifacts typically appear within 15-30 minutes
3. Full indexing may take a few hours

## Maven Coordinates

Once published, users can add the libraries:

```kotlin
// build.gradle.kts
dependencies {
    implementation("io.github.kotlinmania:roff-kotlin:0.1.0")
    implementation("io.github.kotlinmania:cansi-kotlin:0.1.0")
    implementation("io.github.kotlinmania:anstyle-kotlin:0.1.0")
}
```

## Troubleshooting

### Signing Failures

- Verify the `SIGNING_KEY_ID` is the last 8 characters of your key
- Ensure the `SIGNING_KEY` includes the full armor block
- Check that the passphrase is correct

### Authentication Failures

- Regenerate your Maven Central token
- Ensure you're using the token credentials, not your login credentials

### Publication Timeouts

The plugin waits up to 15 minutes for Maven Central validation. If it times out:
1. Check [Central Portal Deployments](https://central.sonatype.com/publishing/deployments)
2. You may need to manually release from there
