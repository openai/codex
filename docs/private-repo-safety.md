# Private Repo Safety Checklist

## Never commit secrets

- Do not commit `.env` files, API keys, bearer tokens, wallet secrets, or private keys.
- Never commit Postman environments that include secrets.
- Store credentials in GitHub Actions Secrets (or your deployment secret manager).

## Recommended `.gitignore` coverage

- `.env*`
- `postman/*environment*.json`
- `*.key`
- `*.pem`
- `*.p12`

## If something leaks

1. Rotate the credential immediately.
2. Remove the leaked value from git history if needed.
3. Document the rotation and update any dependent services.

## Postman export flow safety

- Treat generated collections as public documentation.
- Keep authentication examples redacted unless explicitly safe.
- Validate that the output collection contains no sensitive data before sharing.
