# Refactor Authentication System

**Blueprint ID**: `2025-11-02T13:00:00Z_refactor-auth-system`  
**Status**: approved  
**Mode**: orchestrated  
**Created**: 2025-11-02 13:00:00 UTC  
**Updated**: 2025-11-02 14:00:00 UTC  

## Goal

Refactor the authentication system to use JWT tokens instead of session-based auth, improving scalability and enabling stateless API design.

## Assumptions

- Python 3.11+, FastAPI
- PostgreSQL database
- Redis for token blacklist
- PyJWT library available
- Current session auth working but needs replacement

## Clarifying Questions

- Token expiration time? (default: 1 hour access, 7 days refresh)
- Token signing algorithm? (default: RS256)
- Blacklist storage? (default: Redis with TTL)
- Migration strategy? (default: Parallel run for 1 week)

## Approach

Multi-phase refactoring with orchestrated sub-agent coordination:

1. **Backend Agent**: Implement JWT token generation/validation
2. **Database Agent**: Add refresh_tokens table
3. **Security Agent**: Review token security & rotation
4. **Frontend Agent**: Update API client with token headers
5. **QA Agent**: Generate integration tests

## Work Items

### JWT Token Service

**Files**: auth/jwt_service.py, auth/models.py
**Diff Contract**: patch
**Tests**: tests/auth/test_jwt.py::test_token_generation, test_token_validation

### Database Migration

**Files**: migrations/002_add_refresh_tokens.sql
**Diff Contract**: new file
**Tests**: tests/migrations/test_002.py

### API Endpoints Update

**Files**: api/auth.py, api/middleware.py
**Diff Contract**: patch
**Tests**: tests/api/test_auth_endpoints.py

### Frontend Client Update

**Files**: frontend/src/api/client.ts
**Diff Contract**: patch
**Tests**: frontend/tests/api/client.test.ts

## Risks & Mitigations

**Risk**: Token secret leakage
**Mitigation**: Store in environment variables, rotate monthly, use asymmetric keys (RS256)

**Risk**: Token replay attacks
**Mitigation**: Short-lived access tokens (1hr), refresh token rotation, Redis blacklist

**Risk**: Breaking existing sessions
**Mitigation**: Parallel run mode - support both session & JWT for 1 week transition

**Risk**: Database migration downtime
**Mitigation**: Online migration with backward-compatible schema

## Evaluation Criteria

**Tests**:
- pytest tests/auth/ -v --cov=auth
- npm test --prefix frontend
- Integration tests with real Redis/PostgreSQL

**Metrics**:
- test_coverage: >=90%
- perf_ms: <50ms for token validation
- migration_time: <5 seconds

## Budget

- Max tokens per step: 20000
- Session token cap: 100000
- Time estimate: 45 minutes
- Time cap: 90 minutes

## Rollback Plan

```bash
# Revert all changes
git revert HEAD~5..HEAD

# Revert database migration
psql -U user -d db < migrations/rollback/002_rollback.sql

# Feature flag: ENABLE_JWT_AUTH=false
```

## Research Results

**Query**: JWT best practices Python FastAPI
**Depth**: 2
**Strategy**: focused
**Confidence**: 0.89

### Sources

- [FastAPI Security](https://fastapi.tiangolo.com/tutorial/security/)
  - Date: 2024-10-15
  - Finding: Use OAuth2PasswordBearer for JWT validation in dependencies
  - Confidence: 0.95

- [PyJWT Documentation](https://pyjwt.readthedocs.io/)
  - Date: 2024-09-20
  - Finding: RS256 recommended for production, algorithm parameter must be explicit
  - Confidence: 0.92

- [OWASP JWT Security](https://cheatsheetseries.owasp.org/cheatsheets/JSON_Web_Token_for_Java_Cheat_Sheet.html)
  - Date: 2024-08-10
  - Finding: Always validate 'aud', 'iss', 'exp' claims; use short expiration times
  - Confidence: 0.98

### Synthesis

FastAPI's OAuth2PasswordBearer provides native JWT support. Use RS256 (asymmetric) for production with short-lived access tokens (1hr) and refresh tokens (7 days). Always validate audience, issuer, and expiration claims. Implement token rotation and Redis-based blacklist for revocation.

## Artifacts

- docs/blueprints/2025-11-02_refactor-auth-system.md
- auth/jwt_service.py (new file)
- migrations/002_add_refresh_tokens.sql (new file)
- tests/auth/test_jwt.py (new file)
- Integration test report (generated)

