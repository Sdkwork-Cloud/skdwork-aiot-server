# Scripts

Thin command entrypoints for SDKWork AIoT development, verification, and packaging.

| Script | Purpose |
| --- | --- |
| `dev/*.test.mjs` | Contract and baggage tests |
| `release-package.mjs` | Canonical standalone gateway and device edge runtime packaging |
| `sbom-generate.mjs` / `sbom-check.mjs` | SBOM evidence helpers |

Public root commands are declared in `package.json` per `PNPM_SCRIPT_SPEC.md`.
