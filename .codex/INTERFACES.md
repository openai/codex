# Interfaces

## Public APIs
- Preserve public items unless clearly crate-internal or confirmed unused by all workspace consumers.

## Cross-Module Contracts
- MCP tool/resource schemas, app-server protocol messages, connector metadata, plugin manifests, and skill metadata are compatibility boundaries.

## Data Schemas
- Do not remove serialized fields solely because Rust reports no direct reads.
