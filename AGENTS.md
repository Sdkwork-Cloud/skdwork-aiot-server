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

## Int64 Wire Contract (API_SPEC §13.6)

- OpenAPI `int64` fields and parameters `MUST` be `type: string`, `format: int64`,
  a decimal `pattern` such as `^-?[0-9]+$`, and `x-sdkwork-int64-string: true`.
  `type: integer, format: int64` is a contract violation: generated TypeScript
  SDKs then emit `number`, and browsers silently round ids past
  `Number.MAX_SAFE_INTEGER` (2^53), replaying wrong ids into lookups.
- Rust response DTOs `MUST` serialize `i64` wire fields with
  `#[serde(with = "sdkwork_utils_rust::serde_int64")]` (or `::option`); request
  boundaries parse inbound strings with the same helper.
- Generated TypeScript SDKs keep `int64` as `string`; frontend code `MUST NOT`
  convert ids/snowflake ids/sequence ids to `number` for storage, comparison,
  or submission.
- Verification: `node <sdkwork-specs>/tools/check-api-operation-patterns.mjs --workspace .`

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

<!-- SDKWORK-NAMING-STANDARD: v1 -->
## Rust Naming And Dependency Declaration

Authority: `../sdkwork-specs/NAMING_SPEC.md` section 3.1 and section 3.2.

Two identifier planes exist in every Rust crate and they MUST NOT be mixed: the package plane
(Cargo, filesystem, lock file) uses kebab-case, and the crate plane (lib target, modules, source
imports) uses snake_case.

- `[package].name`, the crate directory, `[features]` keys, and `[[bin]].name` use kebab-case.
- `[lib].name`, module files, module directories, and Rust imports use snake_case.
- A crate whose `[package].name` contains a hyphen SHOULD declare `[lib].name` explicitly
  (default: package name with every `-` replaced by `_`). A shorter lib name is allowed only
  when declared explicitly and used consistently by every consumer.
- Cargo dependency keys, `[workspace.dependencies]` keys, and `Cargo.lock` entries use the
  dependency package name. Use `package = "..."` when an alias is required.
- Every external crate referenced by `src/` MUST be declared in that crate's `[dependencies]`.
  Test-only crates belong in `[dev-dependencies]`; `build.rs` crates belong in
  `[build-dependencies]`.
- Never delete a dependency line, and never demote one from `[dependencies]` to
  `[dev-dependencies]`, while `src/` still imports it. Verify manifest cleanups with the
  command below before committing them.
- Regenerate and commit `Cargo.lock` in the same change as any dependency table edit.

Verification:

```bash
node ../sdkwork-specs/tools/check-rust-crate-naming-standard.mjs --root .
```
<!-- /SDKWORK-NAMING-STANDARD: v1 -->

<!-- SDKWORK-RUST-CODE-STANDARD: v1 -->
## Rust Code Standard

Authority: `../sdkwork-specs/RUST_CODE_SPEC.md` (v2, industry-best baseline); package/crate
naming and dependency declaration are normative in `../sdkwork-specs/NAMING_SPEC.md` section 3.1
and 3.2.

- Crates are responsibility-shaped: service, repository-sqlx, routes, service-host, native-host,
  worker, assembly, gateway. No generic `core`/`common`/`backend`/`runtime` suffixes.
- Errors are typed enums (`thiserror`) implementing `std::error::Error` with a `source` chain.
  `anyhow` only at binary/CLI/test boundaries, never in lib `[dependencies]`.
- No `unsafe` without a `// SAFETY:` comment; crates default to `unsafe_code = "forbid"`.
  No `unwrap`/`expect`/`panic!`/`todo!`/`dbg!` in library code reachable from public API.
- No lock guard held across `.await`; every external await has a timeout; spawned tasks are
  awaited/detached with a documented owner; retries are bounded, jittered, and idempotent.
- Public API is minimal, documented, `#[must_use]` where applicable, and semver-clean. Leaking
  framework types (`sqlx::Row`, axum extractors) through public signatures is forbidden.
- Workspace root declares `[workspace.package]` (edition, rust-version) and `[workspace.lints]`
  (RUST_CODE_SPEC.md section 13 baseline); every member inherits both with
  `edition.workspace = true` and `[lints] workspace = true`.

Verification:

```bash
node ../sdkwork-specs/tools/check-rust-crate-naming-standard.mjs --root .
node ../sdkwork-specs/tools/check-rust-manifest-standard.mjs --root .
# when service/repository/route/gateway dependencies change:
node ../sdkwork-specs/tools/check-rust-backend-composition.mjs --root .
```
<!-- /SDKWORK-RUST-CODE-STANDARD: v1 -->

<!-- SDKWORK-TYPESCRIPT-CODE-STANDARD: v1 -->
## TypeScript Code Standard

Authority: `../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md` (v2, industry-best baseline).

- `tsconfig` runs `strict: true` and the strict family; public APIs are typed and `any`-free.
  `import type` is required for type-only imports (`verbatimModuleSyntax`).
- Errors are typed at package/service boundaries; no empty catches, no swallowed promise
  rejections, no bare `throw new Error('...')` for business failures.
- Async: every promise is settled; external awaits have timeouts; `AbortSignal` accepted for
  cancellable work; bounded concurrency; no unbounded `Promise.all`.
- Public API is minimal, JSDoc-documented, `@deprecated` where applicable, and semver-clean.
- Discriminated unions model closed variant sets; no `as`/`@ts-ignore` bypasses without a guard.
- Node/build runners verify build-critical sources and self-heal from git (CODE_STYLE_SPEC §7);
  `pnpm clean` never deletes git-tracked build-critical files.

Verification:

```bash
pnpm typecheck && pnpm test && pnpm lint
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
```
<!-- /SDKWORK-TYPESCRIPT-CODE-STANDARD: v1 -->

<!-- SDKWORK-FRONTEND-CODE-STANDARD: v1 -->
## Frontend Code Standard

Authority: `../sdkwork-specs/FRONTEND_CODE_SPEC.md` (v2); language rules follow
`../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md` (React/TS) or `../sdkwork-specs/DART_CODE_SPEC.md` (Flutter).

- UI -> service -> injected SDK flow is preserved; components never construct SDK clients or
  assemble raw HTTP/auth headers.
- React: hooks rules clean (`react-hooks`), `useEffect` with full deps and cleanup, stable
  list keys, error boundaries at route/page level, derived state during render (not in effects).
- State: server state behind services/query layer; client state local or minimal typed store;
  no duplication of server state in client stores.
- Accessibility: accessible names, keyboard behavior, visible focus, color is never the only
  signal; error states announced.
- i18n for all user-facing copy in reusable/user-facing packages (I18N_SPEC §6.1).
- PC/H5 `outDir` uses `dist/{standalone,cloud}/{dev,test,staging,prod}`.

Verification:

```bash
pnpm typecheck && pnpm test && pnpm lint
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-browser-dist-layout.mjs --root .   # PC/H5 apps
```
<!-- /SDKWORK-FRONTEND-CODE-STANDARD: v1 -->

<!-- SDKWORK-PNPM-WORKSPACE-STANDARD: v1 -->
## pnpm Workspace Dependency And Package Import

Authority: `../sdkwork-specs/PNPM_WORKSPACE_DEPENDENCY_SPEC.md` (companion to
`../sdkwork-specs/DEPENDENCY_MANAGEMENT_SPEC.md`).

Sibling SDKWork repositories are consumed through a dual-track model that MUST stay consistent:

- **Local development** (`pnpm dev`, `pnpm build`): pnpm workspace protocol. Each sibling
  package is declared ONCE in this repository root `pnpm-workspace.yaml` `packages:` as a
  `../sdkwork-*` relative path, and consumed with `workspace:*` in `package.json`. Never use
  `file:`/`link:`/git-URL specifiers for SDKWork sibling packages in any environment.
- **CI / release packaging**: git-repository dependency checkout. Every sibling referenced by the
  local workspace MUST have a matching `dependencies[]` entry in `sdkwork.workflow.json` so CI
  clones the sibling into the same `../sdkwork-*` relative layout (`GITHUB_WORKFLOW_SPEC.md`).
  `package.json` is never rewritten for CI.

Import rules for sibling SDKWork packages:

- Import by package name only: `import { X } from "@sdkwork/package-name"`. The specifier MUST
  equal the target package's `package.json` `name` exactly - no shortening, renaming, or alias.
- Forbidden: relative imports that cross a package boundary into another SDKWork repository or
  another workspace package's `src/` (for example `import ... from "../../sdkwork-appbase/.../src/..."`).
- Consume only the public `exports` surface of a package; never deep-import sibling `src/` internals.
- Every non-relative import in a workspace member MUST resolve to that member's own
  `dependencies`/`devDependencies`/`peerDependencies` (import closure).
- Vite aliases MUST NOT rename or redirect `@sdkwork/*` packages, MUST NOT be added to make a
  resolution error pass, and are allowed only for documented bootstrap/SDK-generation entrypoints.
- Fix a resolution failure by correcting the workspace declaration or the package `exports`,
  not by adding an alias.

Verification:

```bash
node ../sdkwork-specs/tools/verify-repo.mjs --root .
node ../sdkwork-specs/tools/check-workspace-member-protocol.mjs --root .
node ../sdkwork-specs/tools/check-dependency-list-completeness.mjs --target <repo-name>
```
<!-- /SDKWORK-PNPM-WORKSPACE-STANDARD: v1 -->
