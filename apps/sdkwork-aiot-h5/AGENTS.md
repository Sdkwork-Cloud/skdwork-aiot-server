# Repository Guidelines

## SDKWORK Soul

Read `../../../sdkwork-specs/SOUL.md` and the parent `../../AGENTS.md` before executing tasks in this application root.

## SDKWORK Standards

Canonical standards are indexed by `../../../sdkwork-specs/README.md`; agent entrypoint rules come from `../../../sdkwork-specs/AGENTS_SPEC.md`. Reference global standards by relative path and do not copy their normative bodies here.

## Application Identity

Read `sdkwork.app.config.json` when work touches H5 application behavior, runtime configuration, SDK wiring, release metadata, packaging, or app-owned capabilities. `etc/` is the concrete source configuration authority. This surface delegates runtime topology to `../../specs/topology.spec.json` through `etc/sdkwork.deployment.config.json#parentTopologySpec`; it must not own a competing topology. Root `../../sdkwork.workflow.json` remains the release authority.

## Local Dictionary Structure

- `AGENTS.md`: nearest application execution entrypoint.
- `sdkwork.app.config.json`: H5 application identity and DRAFT release declaration.
- `etc/`: deployable-root source configuration and parent topology delegation.
- `specs/`: application contracts; `specs/component.spec.json` is the machine-readable component authority.
- `packages/`: authored H5 application modules.
- `package.json`: application lifecycle and package command manifest.
- `vite.config.ts`: Vite development and build configuration.

## Spec Resolution Order

Use dynamic progressive loading: read this file and `../../AGENTS.md`, then load the app manifest, local `specs/`, and `etc/` only when the current task touches their contract. Locate the relevant row in `../../../sdkwork-specs/README.md`, read only the selected global spec sections, and inspect implementation files last.

## Required Specs By Task Type

- Agent or workflow work: `../../../sdkwork-specs/SOUL.md`, `../../../sdkwork-specs/AGENTS_SPEC.md`, `../../../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md`, `../../../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`, and `../../../sdkwork-specs/TEST_SPEC.md`.
- Package commands: `../../../sdkwork-specs/PNPM_SCRIPT_SPEC.md`, `../../../sdkwork-specs/APP_RUNTIME_TOPOLOGY_SPEC.md`, and `../../../sdkwork-specs/TEST_SPEC.md`.
- Source configuration: `../../../sdkwork-specs/SOURCE_CONFIG_SPEC.md`, `../../../sdkwork-specs/CONFIG_SPEC.md`, `../../../sdkwork-specs/ENVIRONMENT_SPEC.md`, and `../../../sdkwork-specs/DEPLOYMENT_SPEC.md`.
- TypeScript or frontend code: `../../../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md`, `../../../sdkwork-specs/FRONTEND_CODE_SPEC.md`, `../../../sdkwork-specs/FRONTEND_SPEC.md`, `../../../sdkwork-specs/UI_ARCHITECTURE_SPEC.md`, and `../../../sdkwork-specs/APP_H5_ARCHITECTURE_SPEC.md`.
- SDK integration: `../../../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md`, `../../../sdkwork-specs/SDK_SPEC.md`, and `../../../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md`.
- List/search work: `../../../sdkwork-specs/PAGINATION_SPEC.md` and the owning API, SDK, service, or frontend spec.

Language-specific specs are on-demand; load only the standards for the touched language and framework.

## Code Style Rules

Any code change also loads `../../../sdkwork-specs/CODE_STYLE_SPEC.md` and `../../../sdkwork-specs/NAMING_SPEC.md`. Feature packages consume SDK clients and types through the approved H5 core composition surface. Build scripts, dev runners, and `pnpm clean` follow `CODE_STYLE_SPEC.md` section 7; clean must preserve tracked build-critical source.

## Build, Test, and Verification

Run commands from this application root. Public lifecycle commands are `pnpm dev`, `pnpm dev:standalone`, `pnpm dev:cloud`, `pnpm stop`, `pnpm build`, `pnpm test`, `pnpm check`, `pnpm verify`, and `pnpm clean`. Run `node ../../../sdkwork-specs/tools/check-source-config-standard.mjs --root .` for source-config changes, `node ../../../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace ../..` for SDK consumer changes, and `node ../../../sdkwork-specs/tools/check-pagination.mjs --workspace ../..` for list/search changes.

## Agent Execution Rules

Keep edits within the owning app or package, do not hand-edit generated SDK transport output, and do not replace composed SDK integration with raw HTTP or manual authorization headers. Record exact verification commands and results. Root packaging, publishing, and deployment authority must not be duplicated here.

## Human Review Rules

Request human review before breaking standards, changing public names, auth/security behavior, database schemas, generated SDK ownership, production release metadata, signing, publishing, upload, deployment apply, or rollback.
