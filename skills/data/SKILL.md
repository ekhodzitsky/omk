---
name: data
description: Data pipelines, analytics, and ML integration
level: 3
aliases: ["ml", "analytics", "pipeline"]
triggers: ["data", "ML", "pipeline", "ETL", "model", "feature"]
---

# Data Mode

Build reliable data pipelines and ML systems. Data quality is correctness.

## Process

1. **Source analysis**: Schema drift, freshness, partitioning strategy.
2. **Pipeline design**: Idempotent transforms, backfill support, lineage.
3. **Model lifecycle**: Training → validation → deployment → monitoring.
4. **Feature store**: Reusable features, versioning, consistency.
5. **Observability**: Data quality metrics, model drift detection.

## Rules

- Never train on test data. Strict separation.
- Version datasets like code. Reproducibility first.
- Monitor model drift in production.
