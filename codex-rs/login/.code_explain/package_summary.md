# login Package Summary

## Purpose
Authentication management including OAuth flows and API key handling. Provides secure authentication mechanisms for accessing AI services and managing user credentials.

## Key Components

### OAuth Implementation
- **PKCE Flow**: Proof Key for Code Exchange OAuth
- **Authorization**: Browser-based authorization
- **Token Exchange**: Code for token exchange
- **Token Refresh**: Automatic token renewal

### Local Server
- **Callback Handler**: OAuth redirect handling
- **HTTP Server**: Local server for callbacks
- **Port Management**: Dynamic port allocation
- **Security**: CSRF protection

### Token Management
- **Storage**: Secure token storage
- **Refresh Logic**: Automatic refresh
- **Expiration Handling**: Token lifecycle
- **Revocation**: Token invalidation

### API Key Support
- **Key Validation**: Verify API keys
- **Key Storage**: Secure key management
- **Multiple Providers**: Support various services
- **Fallback Auth**: Alternative to OAuth

## Main Functionality
1. **OAuth Authentication**: Complete OAuth 2.0 flow
2. **Token Management**: Store and refresh tokens
3. **Browser Integration**: Open auth URLs
4. **Credential Storage**: Secure credential management
5. **Multi-provider Support**: Various auth providers

## Dependencies
- `reqwest`: HTTP client
- `sha2`: Cryptographic hashing
- `base64`: Encoding/decoding
- `tiny_http`: Local HTTP server
- Browser opening utilities

## Integration Points
- Used by `core` for authentication
- Integrated in `tui` for login flow
- Works with `cli` for auth commands
- Provides auth for all API calls

## Authentication Flows

### OAuth 2.0 + PKCE
1. Generate code verifier/challenge
2. Open browser with auth URL
3. Start local callback server
4. Receive authorization code
5. Exchange code for tokens
6. Store tokens securely

### API Key Auth
1. Prompt for API key
2. Validate key format
3. Test key validity
4. Store securely
5. Use for requests

### Token Refresh
1. Check token expiration
2. Use refresh token
3. Get new access token
4. Update stored tokens
5. Retry original request

## Security Features

### PKCE Security
- Code verifier generation
- SHA256 challenge
- State parameter
- CSRF protection
- Secure random generation

### Token Security
- Encrypted storage
- Secure file permissions
- Memory protection
- Token rotation
- Expiration enforcement

### Network Security
- HTTPS enforcement
- Certificate validation
- Redirect validation
- Port randomization
- Timeout protection

## Storage Locations
- User home directory
- XDG config directory
- Platform-specific paths
- Encrypted containers
- Keychain integration (future)

## Provider Support
- Anthropic (Claude)
- OpenAI (GPT)
- Custom OAuth providers
- API key providers
- Future providers

## Error Handling
- Network failures
- Invalid credentials
- Expired tokens
- Revoked access
- Browser issues

## User Experience
- Browser auto-open
- Clear instructions
- Progress feedback
- Error messages
- Retry mechanisms