# Repository Guidelines

<!-- SDKWORK-AGENTS-GENERATED: v2 -->

## SDKWORK Soul

Read `../sdkwork-specs/SOUL.md` before executing tasks in this root. Follow specs before memory, dictionary before context, stop on ambiguity, and evidence before completion.

## SDKWORK Standards

Canonical SDKWORK specs path from this root:

- `../sdkwork-specs/README.md`
- `../sdkwork-specs/SOUL.md`
- `../sdkwork-specs/AGENTS_SPEC.md`
- `../sdkwork-specs/PNPM_SCRIPT_SPEC.md`
- `../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`
- `../sdkwork-specs/SOURCE_CONFIG_SPEC.md`
- `../sdkwork-specs/CODE_STYLE_SPEC.md`
- `../sdkwork-specs/NAMING_SPEC.md`

Do not copy root standard text into this repository. If these relative paths do not resolve, stop and report the broken workspace layout.

## Application Identity

Read `sdkwork.app.config.json` only when the task touches AIoT application behavior, runtime config, SDK wiring, release metadata, app-owned capabilities, packaging, or deployment. For unrelated documentation or tooling work, do not expand into the full app manifest unless evidence requires it.

## Local Dictionary Structure

- `AGENTS.md`: repository agent entrypoint and relative SDKWork spec index.
- `CLAUDE.md`, `GEMINI.md`, `CODEX.md`: compatibility shims that point to `AGENTS.md` and must not duplicate rules.
- `sdkwork.app.config.json`: AIoT application identity, runtime, release, and capability metadata.
- `sdkwork.workflow.json`: GitHub packaging/release workflow manifest governed by `GITHUB_WORKFLOW_SPEC.md`.
- `.github/workflows/package.yml`: thin reusable workflow call only.
- `.sdkwork/`: repository/application AI workspace metadata, local skills, local plugins, and manifests.
- `specs/`: local application/component contracts and narrowing rules.
- `apis/`: AIoT-owned API contract sources and materialized OpenAPI inputs.
- `apps/`: PC/H5 application roots and shared app packages.
- `crates/`: reusable Rust crates.
- `services/`: runnable AIoT server binaries.
- `sdks/`: SDK families, SDK generation manifests, route manifests, and generated SDK artifacts.
- `database/`: sdkwork-database assets and lifecycle inputs.
- `etc/`: typed source configuration and topology profile templates.
- `scripts/`, `tools/`, `docs/`, `tests/`: thin command entrypoints, validators, documentation, and verification assets.
- `package.json`, `Cargo.toml`: language/build manifests.

## Documentation Canon

- [docs/README.md](docs/README.md)
- [docs/product/prd/PRD.md](docs/product/prd/PRD.md)
- [docs/architecture/tech/TECH_ARCHITECTURE.md](docs/architecture/tech/TECH_ARCHITECTURE.md)

## Spec Resolution Order

Use dynamic progressive loading:

1. Read this `AGENTS.md` and any nearer component-level `AGENTS.md`.
2. Read `sdkwork.app.config.json` only when app behavior, runtime config, SDK wiring, release, packaging, or app-owned capabilities are touched.
3. Read local `specs/README.md` and `specs/component.spec.json` only when the task touches that local contract.
4. Read local `.sdkwork/README.md`, `.sdkwork/skills/`, and `.sdkwork/plugins/` only when local agent extensions are relevant.
5. Read `../sdkwork-specs/README.md`, then only the task-specific root specs.
6. Inspect implementation files after the dictionary and relevant specs are clear.

Do not load the whole repository or every root spec before identifying the task surface.

## Required Specs By Task Type

- Agent/workflow changes: `../sdkwork-specs/SOUL.md`, `../sdkwork-specs/AGENTS_SPEC.md`, `../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md`, `../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`, and `../sdkwork-specs/TEST_SPEC.md`.
- Package script changes: `../sdkwork-specs/PNPM_SCRIPT_SPEC.md`, `../sdkwork-specs/APP_RUNTIME_TOPOLOGY_SPEC.md`, `../sdkwork-specs/CONFIG_SPEC.md`, and `../sdkwork-specs/TEST_SPEC.md`.
- Any code change: `../sdkwork-specs/CODE_STYLE_SPEC.md`, `../sdkwork-specs/NAMING_SPEC.md`, plus only the touched language/framework spec.
- Rust code: `../sdkwork-specs/RUST_CODE_SPEC.md`; add `../sdkwork-specs/RUST_RPC_SPEC.md` when RPC is touched.
- Java/Spring code: `../sdkwork-specs/JAVA_CODE_SPEC.md` and `../sdkwork-specs/WEB_BACKEND_SPEC.md` when HTTP backend behavior is touched.
- TypeScript/Node code: `../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md`.
- Frontend/UI code: `../sdkwork-specs/FRONTEND_CODE_SPEC.md`, `../sdkwork-specs/FRONTEND_SPEC.md`, `../sdkwork-specs/UI_ARCHITECTURE_SPEC.md`, and exactly one detailed UI architecture spec.
- API/SDK changes: `../sdkwork-specs/API_SPEC.md`, `../sdkwork-specs/WEB_FRAMEWORK_SPEC.md`, `../sdkwork-specs/WEB_BACKEND_SPEC.md`, `../sdkwork-specs/SDK_SPEC.md`, `../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md`, and `../sdkwork-specs/TEST_SPEC.md`.
- Database changes: `../sdkwork-specs/DATABASE_SPEC.md` and the local `database/` assets.
- Runtime/deployment/release changes: `../sdkwork-specs/SOURCE_CONFIG_SPEC.md`, `../sdkwork-specs/CONFIG_SPEC.md`, `../sdkwork-specs/ENVIRONMENT_SPEC.md`, `../sdkwork-specs/DEPLOYMENT_SPEC.md`, `../sdkwork-specs/RELEASE_SPEC.md`, `../sdkwork-specs/SUPPLY_CHAIN_SECURITY_SPEC.md`, and `../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`.
- Security/auth changes: `../sdkwork-specs/SECURITY_SPEC.md` and `../sdkwork-specs/PRIVACY_SPEC.md`.

Language-specific specs are on-demand; do not load Rust, Java, TypeScript, and frontend specs for unrelated tasks.

## Code Style Rules

Read `../sdkwork-specs/CODE_STYLE_SPEC.md` and `../sdkwork-specs/NAMING_SPEC.md` before code changes. Use `sdkwork-utils-rust` and `@sdkwork/utils` for shared utility helpers instead of duplicating string, bytes, datetime, validation, id, or crypto logic locally. For Rust, keep `src/lib.rs` limited to module declarations, re-exports, light docs, and wiring; move handlers, services, repositories, DTOs, SQL, provider clients, and tests into focused modules. Generated SDK transport output is changed only through source contracts, generator inputs, or approved composed facades.

Build scripts, dev runners, and `pnpm clean` must follow `CODE_STYLE_SPEC.md` §7 (Build Source Integrity And Self-Healing). Git-tracked build-critical source files must be verified before builds and self-healed from git when missing; `clean` must not delete them.

## Build, Test, and Verification

Use canonical root package scripts from `PNPM_SCRIPT_SPEC.md`:

- `pnpm dev`: delegates exactly to `pnpm dev:standalone`.
- `pnpm dev:standalone`: one application standalone gateway, the device edge runtime, and the selected client.
- `pnpm dev:cloud`: selected local client only, consuming deployed cloud URLs.
- `pnpm build`, `pnpm test`, `pnpm check`, `pnpm verify`, `pnpm clean`: standard root lifecycle commands.
- `pnpm check:pnpm-script-standard`: validate package script standardization.
- `pnpm check:agent-workflow-standard`: validate AGENTS and GitHub packaging workflow standardization.
- `pnpm check:docs-standard`: validate Canon docs layout per `DOCUMENTATION_SPEC.md`.
- `pnpm api:check`, `pnpm sdk:check`, `pnpm db:validate`, `pnpm test:topology-validate`, `pnpm test:topology-baggage`: contract and topology gates.
- `cargo fmt -- --check`, `cargo clippy --workspace --tests -- -D warnings`, `cargo test --workspace`: Rust verification.

Run the narrowest relevant check first, then broader verification when API contracts, SDK generation, persistence, security, packaging, or cross-package boundaries change. Repository-specific structure, security, and verification notes live in `specs/README.md`.

## Agent Execution Rules

Use the convention dictionary before broad source loading. Follow dynamic progressive loading: nearest AGENTS, relevant manifests/specs, task-specific root standards, then implementation. Do not preserve legacy aliases or local guidance blocks when root SDKWork standards already govern the behavior. Do not replace generated SDK integration with raw HTTP. Record exact verification commands and important outputs before reporting completion.

## Standards Routing

- App SDK consumer work follows `../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md`, `../sdkwork-specs/SDK_SPEC.md`, and `../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md`; verify with `node ../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace .`.
- HTTP contract and response work follows `../sdkwork-specs/API_SPEC.md`; verify with `node ../sdkwork-specs/tools/check-api-operation-patterns.mjs --workspace .` and `node ../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace .`.
- List/search work follows `../sdkwork-specs/PAGINATION_SPEC.md`, plus the owning API, database, SDK, backend, or frontend spec; verify with `node ../sdkwork-specs/tools/check-pagination.mjs --workspace .`.

## Human Review Rules

Request human review before breaking SDKWork standards, changing public naming, altering security/auth behavior, changing database migrations or production deployment config, deleting data/files, changing generated SDK ownership, or modifying release/deployment governance. Surface unresolved spec paths, app identity conflicts, component ownership conflicts, and API authority ambiguity instead of guessing.
