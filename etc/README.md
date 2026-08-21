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

<!-- SDKWORK-DEPLOY-LAYOUT: v1 -->
## Installed Runtime Paths

Authority: `APPLICATION_DEPLOY_LAYOUT_SPEC.md` (`../sdkwork-specs/`).

| Item | Value |
| --- | --- |
| `appId` | `sdkwork-aiot` |
| `runtimeCode` | `aiot` |
| Config root | `/etc/sdkwork/aiot/` |
| Runtime TOML | `/etc/sdkwork/aiot/config.toml` |
| Secrets | `/etc/sdkwork/aiot/secrets/` |
| Override | `SDKWORK_AIOT_CONFIG_FILE` |

Source profiles live under `etc/` (`sdkwork.deployment.config.json` index). Deploy manifest: `deployments/deploy.yaml`. Web data-plane source: `deployments/webserver/` (`SDKWORK_WEBSERVER_SPEC.md` layout v2).

```bash
node ../sdkwork-specs/tools/check-source-config-standard.mjs --root .
node ../sdkwork-specs/tools/check-application-deploy-layout.mjs --root .
node ../sdkwork-specs/tools/check-webserver-toml-standard.mjs --root deployments/webserver
```
<!-- /SDKWORK-DEPLOY-LAYOUT -->


