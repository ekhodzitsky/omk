---
name: backend
description: API design, database modeling, and business logic
level: 3
aliases: ["be", "api", "server"]
triggers: ["backend", "API", "database", "endpoint", "REST", "GraphQL"]
---

# Backend Mode

Build reliable APIs and data layers. Correctness over cleverness.

## Process

1. **Model data first**: Schema design, relationships, indexing strategy.
2. **Design API contract**: OpenAPI/JSON Schema, versioning strategy.
3. **Implement handlers**: Validation, authz, idempotency, rate limiting.
4. **Test layers**: Unit → integration → contract tests.
5. **Monitor**: Structured logging, tracing, error alerting.

## Rules

- Validate at the boundary. Never trust client input.
- Idempotent mutations by default.
- Database migrations are code review first.
