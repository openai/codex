# Optimize Database Query Performance

**Blueprint ID**: `2025-11-02T14:00:00Z_optimize-db-query`  
**Status**: approved  
**Mode**: competition  
**Created**: 2025-11-02 14:00:00 UTC  
**Updated**: 2025-11-02 15:30:00 UTC  

## Goal

Optimize slow user listing query (currently 800ms avg) to <100ms by adding appropriate indexes, query optimization, and caching strategies.

## Assumptions

- PostgreSQL 14+
- SQLAlchemy ORM
- Redis available for caching
- User table has ~1M rows
- Current query scans full table

## Clarifying Questions

- Cache TTL? (default: 5 minutes)
- Index maintenance window? (default: off-peak hours)
- Acceptable cache staleness? (default: 5 minutes)

## Approach

**Competition Mode**: Test 3 different optimization approaches in parallel, measure performance, and auto-merge the winner.

### Variant A: Composite Index + Pagination

- Add composite index on (created_at, status, id)
- Implement cursor-based pagination
- No caching

### Variant B: Materialized View + Caching

- Create materialized view for common queries
- Refresh view every 5 minutes
- Redis cache for hot queries

### Variant C: Partial Index + Query Rewrite

- Partial index for active users only
- Rewrite query to use index hints
- Minimal caching (1 minute TTL)

## Work Items

### Database Optimization

**Files**: db/migrations/003_optimize_users.sql, db/models.py
**Diff Contract**: patch
**Tests**: tests/performance/test_user_query.py::test_query_speed

### Caching Layer (Variants B & C)

**Files**: cache/user_cache.py
**Diff Contract**: patch (variant-specific)
**Tests**: tests/cache/test_user_cache.py

## Risks & Mitigations

**Risk**: Index creation blocking table
**Mitigation**: Use CREATE INDEX CONCURRENTLY

**Risk**: Cache inconsistency
**Mitigation**: Cache invalidation on user updates, short TTL

**Risk**: Materialized view refresh lag
**Mitigation**: 5-minute refresh interval, stale data acceptable per requirements

## Evaluation Criteria

**Tests**:
- pytest tests/performance/ -v --benchmark
- Load test with 1000 concurrent requests

**Metrics**:
- query_latency_p50: <50ms
- query_latency_p95: <100ms
- throughput: >500 req/sec
- index_size: <200MB

## Competition Scoring

### Weights

- Tests: 50% (must pass all performance tests)
- Performance: 30% (p95 latency)
- Simplicity: 20% (LOC, maintenance complexity)

### Results

| Variant | Tests | Performance | Simplicity | Total | Winner |
|---------|-------|-------------|------------|-------|--------|
| A | 100.0 | 95.2 | 92.0 | 95.6 | ✅ |
| B | 100.0 | 98.5 | 75.0 | 92.2 | |
| C | 100.0 | 88.0 | 95.0 | 92.6 | |

**Winner**: Variant A (Composite Index + Pagination)

**Analysis**:
- Variant A: Best balance - excellent performance with high maintainability
- Variant B: Fastest but added complexity of materialized view refresh
- Variant C: Simplest but slightly slower performance

## Budget

- Max tokens per step: 15000 (per variant)
- Session token cap: 80000 (all variants combined)
- Time estimate: 30 minutes
- Time cap: 60 minutes

## Rollback Plan

```bash
# Revert migration
psql -U user -d db < migrations/rollback/003_rollback.sql

# Revert code changes
git revert HEAD

# Remove Redis cache keys
redis-cli --scan --pattern "user_cache:*" | xargs redis-cli DEL
```

## Artifacts

- docs/blueprints/2025-11-02_optimize-db-query.md
- db/migrations/003_optimize_users.sql
- Performance benchmark report (variant comparison)
- Winning variant: A (merged to main)

