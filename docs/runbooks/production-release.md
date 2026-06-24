# Production Release Runbook

Operator checklist for promoting SDKWork AIoT `0.1.x` to production.

## Preconditions

- Release candidate branch passes `pnpm verify` and `pnpm release:preflight`.
- Production secrets are prepared out-of-band (never committed).
- IAM proxy (`sdkwork-appbase`) is configured per `docs/deployment/iam-integration.md`.

## Build And Package

```powershell
cd E:\sdkwork-space\sdkwork-aiot
pnpm release:build
pnpm release:package
pnpm release:preflight
```

`release:preflight` runs deploy + release gates and, when local artifacts exist, the CDN publish gate. `release:publish` prints the CDN upload matrix:

- `artifacts/release/linux/x64/server.tar.gz`
- `artifacts/release/windows/x64/server.zip`
- matching CycloneDX SBOM files under `artifacts/release/sbom/`

Upload each archive and SBOM to the URL declared in `sdkwork.app.config.json`.

## Topology Selection

| Profile | Env template | Database |
| --- | --- | --- |
| Self-hosted production | `configs/topology/self-hosted.split-services.production.env` | SQLite file (`SDKWORK_AIOT_DEVICE_DB_PATH`) |
| Cloud production | `configs/topology/cloud-hosted.split-services.production.env` | Postgres (`SDKWORK_AIOT_DEVICE_DATABASE_*`) |

Replace every `DEPLOY_INJECT:` placeholder before starting services.

## Required Production Env

```powershell
$env:SDKWORK_AIOT_ENVIRONMENT='production'
$env:SDKWORK_AIOT_INTERNAL_TOKEN='<random-internal-token>'
$env:SDKWORK_AIOT_CREDENTIAL_PEPPER='<random-pepper>'
$env:SDKWORK_AIOT_CORS_ALLOWED_ORIGINS='https://console.example.com'
# Do NOT set SDKWORK_AIOT_DEV_MODE in production
```

Gateway device auth:

1. Create device via admin API.
2. Issue credential via admin API; store `issuedSecret` on the device.
3. Device connects with `Device-Id` + `Authorization: Bearer <issuedSecret>`.

## Deploy Manifest

`deployments/deploy.yaml` maps topology profiles to install layout and public exposure:

- `cloud-hosted.split-services.production` — CDN/binary package + `aiot.sdkwork.com`
- `self-hosted.split-services.production` — on-prem binary package + local domain

## Post-Deploy Verification

```powershell
pnpm check:production-topology
curl http://<gateway-host>:18080/readyz
curl http://<admin-host>:18081/readyz
curl http://<app-host>:18082/readyz
```

Confirm:

- Gateway `/readyz` reports device DB and outbox lag within threshold.
- Admin/app APIs return `401` without IAM proxy headers.
- Firmware rollout → OTA offer → device completion updates deployment to `completed`.

## Rollback

1. Restore previous CDN binaries using prior `release-packages.manifest.json` checksums.
2. Revert `sdkwork.app.config.json` package checksums to the previous release tag.
3. Roll database forward-only migrations only with an approved down plan from `database/migrations/`.

## References

- `docs/production-readiness.md`
- `docs/product/prd/PRD.md`
- `docs/architecture/tech/TECH_ARCHITECTURE.md`
- `deployments/deploy.yaml`
