# Backend TypeScript SDK Generation

Generated output lives under `generated/server-openapi/`. The hand-written `src/index.ts` re-exports the generated client and types.

## Regenerate

From repository root:

```bash
pnpm sdk:generate
```

Generation uses workspace `@sdkwork/sdk-generator` through `tools/run-sdkgen.mjs` with `--standard-profile sdkwork-v3`. OpenAPI authorities under `apis/backend-api/iot/` are materialized first via `pnpm api:materialize`.

## Verification

```bash
pnpm sdk:check
pnpm api:check
cargo test -p sdkwork-aiot-architecture -p sdkwork-iot-platform-service
node ../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace .
```
