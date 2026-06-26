# apps/

Application: sdkwork-aiot
Status: active
Owner: SDKWork maintainers
Specs: APPLICATION_SPEC.md, SDKWORK_WORKSPACE_SPEC.md

## Primary App Surface

The repository root is the primary runnable app surface.
The repository root `sdkwork.app.config.json` governs the primary application manifest.

## Directory Index

| Directory | Surface role | Runnable | Purpose | Entry |
| --- | --- | --- | --- | --- |
| sdkwork-aiot-h5 | h5 | yes | sdkwork-aiot-h5 h5 application root. | `sdkwork-aiot-h5/` |
| sdkwork-aiot-mini-program | mini-program | yes | sdkwork-aiot-mini-program mini-program application root. | `sdkwork-aiot-mini-program/` |
| sdkwork-aiot-pc | pc | yes | SDKWork AIoT PC | [README](sdkwork-aiot-pc/README.md) |
| sdkwork-aiot-shared | app | yes | sdkwork-aiot-shared app application root. | `sdkwork-aiot-shared/` |

## Allowed Content

- Selected language/architecture application roots with `README.md`, `AGENTS.md`, `.sdkwork/`, and `specs/` when authored packages exist.
- Architecture-local `packages/`, `config/`, `src/`, `lib/`, `App/`, or `entry/` directories required by the owning architecture standard.

## Forbidden Content

- Repository-root API contracts, generated SDK workspaces, Rust crates, or deployment descriptors moved under `apps/`.
- Runtime secrets, user-private state, generated SDK transport output, or cross-application copied business logic.

## Related Specs

- `../sdkwork-specs/APPLICATION_SPEC.md`
- `../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md`
- `../sdkwork-specs/APP_CLIENT_ARCHITECTURE_ALIGNMENT_SPEC.md`

## Verification

```bash
node ../sdkwork-specs/tools/check-apps-directory-index.mjs --root .
```
