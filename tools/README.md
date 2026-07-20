# Tools

Repository-local developer and release tools for the SDKWork AIoT server.

- SDK generation and verification: `tools/aiot_sdk_generate.mjs` (`pnpm sdk:generate`, `pnpm sdk:check`)
- Dev orchestration: shared `sdkwork-app` facade backed by `specs/topology.spec.json`
- OpenAPI materialization: `scripts/dev/sync-openapi-web-context.mjs` (`pnpm api:materialize`, `pnpm api:materialize:check`)
- Topology validation: `../sdkwork-app-topology`
- Contract tests: `scripts/dev/*.test.mjs`

Shared SDKWork tooling is consumed from sibling repositories declared in `sdkwork.workflow.json`.
