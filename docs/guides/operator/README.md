# Operator Guide

Deployment, monitoring, and incident response entrypoints.

## Release And Deploy

- [Production release runbook](../runbooks/production-release.md)
- [Production readiness checklist](../production-readiness.md)
- [IAM integration](../deployment/iam-integration.md)
- [Topology standard](../topology-standard.md)

## Validation

```powershell
pnpm release:preflight
pnpm check:production-topology
```
