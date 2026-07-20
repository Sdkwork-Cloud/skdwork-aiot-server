# SDKWork AIoT Source Configuration

`sdkwork.deployment.config.json` is the deployment profile index for the AIoT application.
`topology/` contains tracked, non-secret profile inputs consumed by the topology runtime and
application lifecycle facade.

Local overlays, access tokens, passwords, private keys, signing material, and runtime state are not
committed here. Production placeholders are resolved by an authorized deployment environment.

The retired repository-root `configs/` tree has been removed. New lifecycle commands resolve only
the typed profiles declared by this directory.

Related standards:

- `../../sdkwork-specs/SOURCE_CONFIG_SPEC.md`
- `../../sdkwork-specs/CONFIG_SPEC.md`
- `../../sdkwork-specs/ENVIRONMENT_SPEC.md`
- `../../sdkwork-specs/APP_RUNTIME_TOPOLOGY_SPEC.md`
