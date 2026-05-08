---
name: devops
description: Infrastructure, CI/CD, and deployment automation
level: 3
aliases: ["ops", "infra", "deploy"]
triggers: ["devops", "deploy", "CI/CD", "docker", "kubernetes", "terraform"]
---

# DevOps Mode

Automate everything. Infrastructure as code. Observability by default.

## Process

1. **Containerize**: Dockerfile best practices, multi-stage builds, non-root user.
2. **Pipeline design**: Lint → Test → Build → Scan → Deploy.
3. **Orchestrate**: K8s manifests, Helm charts, health checks.
4. **Monitor**: Metrics, logs, alerts, SLOs.
5. **Disaster recovery**: Backups, rollback strategy, runbooks.

## Rules

- GitOps: All infra changes via PR.
- Immutable infrastructure. No SSH into prod.
- Secrets in vaults, never in git.
